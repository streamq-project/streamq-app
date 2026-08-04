use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::unbounded;
use debounce::EventDebouncer;
use futures::executor::block_on;
use std::sync::Mutex;
use tracing::{debug, error, warn};
use windows::Foundation::{EventRegistrationToken, TypedEventHandler};
use windows::Media::Control::{GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager};
use windows::Win32::Foundation::BOOL;
use windows::Win32::Media::Audio::{
    IAudioSessionControl, IAudioSessionControl2, IAudioSessionEvents, IAudioSessionEvents_Impl, IAudioSessionManager2, IAudioSessionNotification,
    IAudioSessionNotification_Impl, IMMDeviceEnumerator, MMDeviceEnumerator, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::core::{ComInterface, implement};

use super::media_session_control::handle_playback_update;
use crate::models::media::SharedState;

struct ComSession {
    control: IAudioSessionControl,
    #[allow(dead_code)]
    handler: IAudioSessionEvents,
}
unsafe impl Send for ComSession {}
unsafe impl Sync for ComSession {}

type SessionRegistry = Arc<Mutex<Vec<ComSession>>>;

#[implement(IAudioSessionEvents)]
#[derive(Clone)]
struct AudioSessionEventsHandler {
    pid: u32,
    endpoint_id: String,
    state: SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    debouncer: Arc<EventDebouncer<()>>,
    audio_manager: Arc<super::audio_sessions::WindowsAudioSessionManager>,
}

impl IAudioSessionEvents_Impl for AudioSessionEventsHandler {
    fn OnDisplayNameChanged(&self, _: &windows::core::PCWSTR, _: *const windows::core::GUID) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnIconPathChanged(&self, _: &windows::core::PCWSTR, _: *const windows::core::GUID) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnChannelVolumeChanged(&self, _: u32, _: *const f32, _: u32, _: *const windows::core::GUID) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnGroupingParamChanged(&self, _: *const windows::core::GUID, _: *const windows::core::GUID) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnSimpleVolumeChanged(&self, _newvolume: f32, _newmute: BOOL, _eventcontext: *const windows::core::GUID) -> windows::core::Result<()> {
        debug!("OnSimpleVolumeChanged: pid={} endpoint={} volume={}", self.pid, self.endpoint_id, _newvolume);
        self.debouncer.put(());
        Ok(())
    }

    fn OnStateChanged(&self, _: windows::Win32::Media::Audio::AudioSessionState) -> windows::core::Result<()> {
        let _ = handle_playback_update(&self.state, self.is_dev, self.extract_thumbnails, &self.audio_manager);
        Ok(())
    }

    fn OnSessionDisconnected(&self, _: windows::Win32::Media::Audio::AudioSessionDisconnectReason) -> windows::core::Result<()> {
        let _ = handle_playback_update(&self.state, self.is_dev, self.extract_thumbnails, &self.audio_manager);
        Ok(())
    }
}

#[implement(IAudioSessionNotification)]
struct AudioSessionNotificationHandler {
    endpoint_id: String,
    state: SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    debouncer: Arc<EventDebouncer<()>>,
    registry: SessionRegistry,
    audio_manager: Arc<super::audio_sessions::WindowsAudioSessionManager>,
}

impl IAudioSessionNotification_Impl for AudioSessionNotificationHandler {
    fn OnSessionCreated(&self, session: Option<&IAudioSessionControl>) -> windows::core::Result<()> {
        if let Some(control) = session {
            register_session_events(
                control,
                &self.endpoint_id,
                &self.state,
                self.is_dev,
                self.extract_thumbnails,
                &self.debouncer,
                &self.registry,
                &self.audio_manager,
            );
        }
        Ok(())
    }
}

fn register_session_events(
    control: &IAudioSessionControl,
    endpoint_id: &str,
    state: &SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    debouncer: &Arc<EventDebouncer<()>>,
    registry: &SessionRegistry,
    audio_manager: &Arc<super::audio_sessions::WindowsAudioSessionManager>,
) {
    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
        unsafe {
            let pid = control2.GetProcessId().unwrap_or(0);
            if pid != 0 {
                let handler: IAudioSessionEvents = AudioSessionEventsHandler {
                    pid,
                    endpoint_id: endpoint_id.to_string(),
                    state: state.clone(),
                    is_dev,
                    extract_thumbnails,
                    debouncer: debouncer.clone(),
                    audio_manager: audio_manager.clone(),
                }
                .into();

                if control.RegisterAudioSessionNotification(&handler).is_ok() {
                    debug!("Registered audio session events for PID {} on endpoint {}", pid, endpoint_id);

                    let mut reg = registry.lock().unwrap();

                    reg.retain(|session| {
                        if let Ok(c2) = session.control.cast::<IAudioSessionControl2>() {
                            if let Ok(state) = c2.GetState() {
                                return state != windows::Win32::Media::Audio::AudioSessionStateExpired;
                            }
                        }
                        false
                    });

                    reg.push(ComSession {
                        control: control.clone(),
                        handler,
                    });
                }
            }
        }
    }
}

pub struct AudioSessionEventsManager {
    state: SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: Arc<super::audio_sessions::WindowsAudioSessionManager>,
}

impl AudioSessionEventsManager {
    pub fn new(state: SharedState, is_dev: bool, extract_thumbnails: bool, audio_manager: Arc<super::audio_sessions::WindowsAudioSessionManager>) -> Self {
        Self {
            state,
            is_dev,
            extract_thumbnails,
            audio_manager,
        }
    }

    pub fn run(&self) {
        let state_for_debouncer = self.state.clone();
        let is_dev = self.is_dev;
        let extract_thumbnails = self.extract_thumbnails;
        let audio_manager_for_debouncer = self.audio_manager.clone();

        let debouncer = Arc::new(EventDebouncer::new(Duration::from_millis(10), move |_| {
            let _ = handle_playback_update(&state_for_debouncer, is_dev, extract_thumbnails, &audio_manager_for_debouncer);
        }));

        let registry: SessionRegistry = Arc::new(Mutex::new(Vec::new()));

        let mut session_managers: Vec<IAudioSessionManager2> = Vec::new();

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!("Failed to create device enumerator: {:?}", e);
                    return;
                }
            };

            if let Ok(collection) = enumerator.EnumAudioEndpoints(eRender, windows::Win32::Media::Audio::DEVICE_STATE_ACTIVE) {
                if let Ok(count) = collection.GetCount() {
                    for i in 0..count {
                        if let Ok(device) = collection.Item(i) {
                            let endpoint_id = match device.GetId() {
                                Ok(id_pwstr) => {
                                    let id = id_pwstr.to_string().unwrap_or_default();
                                    CoTaskMemFree(Some(id_pwstr.0 as _));
                                    id
                                }
                                Err(_) => continue,
                            };

                            if endpoint_id.is_empty() {
                                continue;
                            }

                            if let Ok(session_manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) {
                                let notification_handler: IAudioSessionNotification = AudioSessionNotificationHandler {
                                    endpoint_id: endpoint_id.clone(),
                                    state: self.state.clone(),
                                    is_dev: self.is_dev,
                                    extract_thumbnails: self.extract_thumbnails,
                                    debouncer: debouncer.clone(),
                                    registry: registry.clone(),
                                    audio_manager: self.audio_manager.clone(),
                                }
                                .into();

                                if session_manager.RegisterSessionNotification(&notification_handler).is_ok() {
                                    if let Ok(enum_) = session_manager.GetSessionEnumerator() {
                                        if let Ok(s_count) = enum_.GetCount() {
                                            for j in 0..s_count {
                                                if let Ok(control) = enum_.GetSession(j) {
                                                    register_session_events(
                                                        &control,
                                                        &endpoint_id,
                                                        &self.state,
                                                        self.is_dev,
                                                        self.extract_thumbnails,
                                                        &debouncer,
                                                        &registry,
                                                        &self.audio_manager,
                                                    );
                                                }
                                            }
                                        }
                                    }

                                    session_managers.push(session_manager);
                                }
                            }
                        }
                    }
                }
            }

            tracing::info!("Audio session events manager started on ALL devices");

            let (_tx, rx) = std::sync::mpsc::channel::<()>();
            let _ = rx.recv();
        }
    }
}

struct SessionSubscription {
    session: GlobalSystemMediaTransportControlsSession,
    playback_token: EventRegistrationToken,
    media_token: EventRegistrationToken,
}

pub fn start_media_transport_listener(
    state: SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: Arc<super::audio_sessions::WindowsAudioSessionManager>,
) {
    thread::spawn(move || {
        let manager: GlobalSystemMediaTransportControlsSessionManager =
            match block_on(GlobalSystemMediaTransportControlsSessionManager::RequestAsync().unwrap()) {
                Ok(m) => m,
                Err(e) => {
                    error!(error = ?e, "Failed to request session manager");
                    return;
                }
            };

        let (tx, rx) = unbounded();

        let tx_clone = tx.clone();
        let _ = manager.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
            let _ = tx_clone.send(());
            Ok(())
        }));

        let mut subscriptions: Vec<SessionSubscription> = Vec::new();

        update_gsmtc_subscriptions(&manager, &mut subscriptions, &state, is_dev, extract_thumbnails, &audio_manager);

        while rx.recv().is_ok() {
            update_gsmtc_subscriptions(&manager, &mut subscriptions, &state, is_dev, extract_thumbnails, &audio_manager);
        }
    });
}

fn update_gsmtc_subscriptions(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    subscriptions: &mut Vec<SessionSubscription>,
    state: &SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: &Arc<super::audio_sessions::WindowsAudioSessionManager>,
) {
    for sub in subscriptions.drain(..) {
        let _ = sub.session.RemovePlaybackInfoChanged(sub.playback_token);
        let _ = sub.session.RemoveMediaPropertiesChanged(sub.media_token);
    }

    let sessions = match manager.GetSessions() {
        Ok(s) => s,
        Err(e) => {
            warn!(error = ?e, "Failed to get sessions");
            return;
        }
    };

    for i in 0..sessions.Size().unwrap_or(0) {
        if let Ok(session) = sessions.GetAt(i) {
            let state_pb = state.clone();
            let state_media = state.clone();
            let audio_manager_pb = audio_manager.clone();
            let audio_manager_media = audio_manager.clone();

            let pb_token = session
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = handle_playback_update(&state_pb, is_dev, extract_thumbnails, &audio_manager_pb);
                    Ok(())
                }))
                .unwrap_or_default();

            let media_token = session
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    let _ = handle_playback_update(&state_media, is_dev, extract_thumbnails, &audio_manager_media);
                    Ok(())
                }))
                .unwrap_or_default();

            subscriptions.push(SessionSubscription {
                session,
                playback_token: pb_token,
                media_token,
            });
        }
    }

    let _ = handle_playback_update(state, is_dev, extract_thumbnails, audio_manager);
}
