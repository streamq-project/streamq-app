use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::Arc;
use std::time::Duration;

use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl};
use windows::Win32::Media::Audio::{DEVICE_STATE_ACTIVE, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::core::{PWSTR, implement};

use super::audio::get_audio_sources;
use super::audio_policy::get_app_source;
use crate::models::media::AudioSource;

unsafe fn pwstr_to_string_and_free(ptr: PWSTR) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    let out = unsafe { ptr.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(ptr.0 as *const c_void)) };
    out
}

unsafe fn get_device_id(device: &IMMDevice) -> Option<String> {
    let id_pwstr = unsafe { device.GetId() }.ok()?;
    unsafe { pwstr_to_string_and_free(id_pwstr) }
}

#[implement(windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolumeCallback)]
struct VolumeChangeCallback {
    state: Arc<crate::models::media::MediaSessionState>,
    endpoint_id: String,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeChangeCallback {
    fn OnNotify(&self, pnotify: *mut windows::Win32::Media::Audio::AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        if pnotify.is_null() {
            tracing::warn!("VolumeChangeCallback: null notification pointer for endpoint {}", self.endpoint_id);
            return Ok(());
        }

        let new_volume = unsafe { (*pnotify).fMasterVolume as f64 };
        tracing::debug!("VolumeChangeCallback: endpoint={} volume={}", self.endpoint_id, new_volume);

        *self.state.system_volume.lock().unwrap() = new_volume;

        match get_audio_sources() {
            Ok(sources) => {
                self.state.event_emitter.emit(crate::event_emitter::Event::SourcesChanged(sources));
            }
            Err(e) => {
                tracing::warn!(
                    "VolumeChangeCallback: failed to collect audio sources for endpoint {}: {:?}",
                    self.endpoint_id,
                    e
                );
            }
        }

        Ok(())
    }
}

struct EndpointSubscription {
    endpoint_volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

pub struct VolumeListener {
    state: Arc<crate::models::media::MediaSessionState>,
    rescan_interval: Duration,
}

impl VolumeListener {
    pub fn new(state: Arc<crate::models::media::MediaSessionState>) -> Self {
        Self {
            state,
            rescan_interval: Duration::from_secs(2),
        }
    }

    unsafe fn subscribe_endpoint(&self, device: &IMMDevice, endpoint_id: &str) -> Option<EndpointSubscription> {
        let endpoint_volume: IAudioEndpointVolume = match unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) } {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("VolumeListener: failed to activate IAudioEndpointVolume for {}: {:?}", endpoint_id, e);
                return None;
            }
        };

        let callback_impl = VolumeChangeCallback {
            state: self.state.clone(),
            endpoint_id: endpoint_id.to_string(),
        };
        let callback: IAudioEndpointVolumeCallback = callback_impl.into();

        if let Err(e) = unsafe { endpoint_volume.RegisterControlChangeNotify(&callback) } {
            tracing::warn!("VolumeListener: failed to register callback for {}: {:?}", endpoint_id, e);
            return None;
        }

        Some(EndpointSubscription { endpoint_volume, callback })
    }

    unsafe fn sync_subscriptions(&self, enumerator: &IMMDeviceEnumerator, subscriptions: &mut HashMap<String, EndpointSubscription>) {
        let collection = match unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) } {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("VolumeListener: EnumAudioEndpoints failed: {:?}", e);
                return;
            }
        };

        let count = match unsafe { collection.GetCount() } {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("VolumeListener: GetCount failed: {:?}", e);
                return;
            }
        };

        let mut active_ids = HashSet::with_capacity(count as usize);

        for i in 0..count {
            let device = match unsafe { collection.Item(i) } {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!("VolumeListener: failed to get endpoint #{}: {:?}", i, e);
                    continue;
                }
            };

            let endpoint_id = match unsafe { get_device_id(&device) } {
                Some(id) if !id.is_empty() => id,
                _ => {
                    tracing::warn!("VolumeListener: endpoint #{} has no valid id", i);
                    continue;
                }
            };

            active_ids.insert(endpoint_id.clone());

            if !subscriptions.contains_key(&endpoint_id) {
                if let Some(sub) = unsafe { self.subscribe_endpoint(&device, &endpoint_id) } {
                    subscriptions.insert(endpoint_id.clone(), sub);
                    tracing::info!("VolumeListener: subscribed to endpoint {}", endpoint_id);
                }
            }
        }

        let removed: Vec<String> = subscriptions.keys().filter(|id| !active_ids.contains(*id)).cloned().collect();

        for endpoint_id in removed {
            if let Some(sub) = subscriptions.remove(&endpoint_id) {
                let _ = unsafe { sub.endpoint_volume.UnregisterControlChangeNotify(&sub.callback) };
                tracing::info!("VolumeListener: unsubscribed from endpoint {}", endpoint_id);
            }
        }
    }

    pub fn run(&self) {
        tracing::info!("VolumeListener: starting all-endpoints callback listener");

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!("VolumeListener: failed to create IMMDeviceEnumerator: {:?}", e);
                    return;
                }
            };

            let mut subscriptions: HashMap<String, EndpointSubscription> = HashMap::new();

            self.sync_subscriptions(&enumerator, &mut subscriptions);
            tracing::info!("VolumeListener: initialized subscriptions, count={}", subscriptions.len());

            loop {
                self.sync_subscriptions(&enumerator, &mut subscriptions);
                std::thread::sleep(self.rescan_interval);
            }
        }
    }
}

pub struct AppSourceWatcher {
    poll_interval: Duration,
    state: Arc<crate::models::media::MediaSessionState>,
}

impl AppSourceWatcher {
    pub fn new(poll_interval: Duration, state: Arc<crate::models::media::MediaSessionState>) -> Self {
        Self { poll_interval, state }
    }

    pub fn run(&self) {
        let mut last_value: Option<Option<AudioSource>> = None;

        loop {
            if let Ok(current) = get_app_source() {
                if last_value.as_ref() != Some(&current) {
                    self.state
                        .event_emitter
                        .emit(crate::event_emitter::Event::AppAudioSourceChanged(current.clone()));
                    last_value = Some(current);
                }
            }

            std::thread::sleep(self.poll_interval);
        }
    }
}

pub fn start_app_source_watcher(state: Arc<crate::models::media::MediaSessionState>) {
    let watcher = AppSourceWatcher::new(Duration::from_millis(1000), state);
    std::thread::spawn(move || watcher.run());
}

pub fn start_volume_listener(state: Arc<crate::models::media::MediaSessionState>) {
    let listener = VolumeListener::new(state);
    std::thread::spawn(move || listener.run());
}
