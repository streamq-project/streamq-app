use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, bounded, unbounded};
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
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::{ComInterface, implement};

use super::media_session_control;
use crate::error::NativeError;
use crate::models::media::{AppMediaSession, SharedState};

const MEDIA_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

enum MediaTransportCommand {
    Refresh,
    SessionsChanged,
    Pause(Vec<String>, Sender<Result<Vec<AppMediaSession>, NativeError>>),
    Resume(Vec<String>, Sender<Result<(), NativeError>>),
    SetVolume(String, f64, Sender<Result<(), NativeError>>),
}

#[derive(Clone)]
pub struct MediaTransportHandle {
    sender: Sender<MediaTransportCommand>,
}

impl MediaTransportHandle {
    fn refresh(&self) {
        let _ = self.sender.send(MediaTransportCommand::Refresh);
    }

    fn sessions_changed(&self) {
        let _ = self.sender.send(MediaTransportCommand::SessionsChanged);
    }

    pub fn pause(&self, apps: Vec<String>) -> Result<Vec<AppMediaSession>, NativeError> {
        let (response_tx, response_rx) = bounded(1);
        self.sender
            .send(MediaTransportCommand::Pause(apps, response_tx))
            .map_err(|_| NativeError::MediaSession("Media transport worker is unavailable".into()))?;
        receive_response(response_rx)
    }

    pub fn resume(&self, apps: Vec<String>) -> Result<(), NativeError> {
        let (response_tx, response_rx) = bounded(1);
        self.sender
            .send(MediaTransportCommand::Resume(apps, response_tx))
            .map_err(|_| NativeError::MediaSession("Media transport worker is unavailable".into()))?;
        receive_response(response_rx)
    }

    pub fn set_volume(&self, app: String, volume: f64) -> Result<(), NativeError> {
        let (response_tx, response_rx) = bounded(1);
        self.sender
            .send(MediaTransportCommand::SetVolume(app, volume, response_tx))
            .map_err(|_| NativeError::MediaSession("Media transport worker is unavailable".into()))?;
        receive_response(response_rx)
    }
}

fn receive_response<T>(receiver: Receiver<Result<T, NativeError>>) -> Result<T, NativeError> {
    match receiver.recv_timeout(MEDIA_COMMAND_TIMEOUT) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(NativeError::MediaSession("Media transport operation timed out".into())),
        Err(RecvTimeoutError::Disconnected) => Err(NativeError::MediaSession("Media transport worker stopped".into())),
    }
}

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
    debouncer: Arc<EventDebouncer<()>>,
    media_transport: MediaTransportHandle,
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
        self.media_transport.refresh();
        Ok(())
    }

    fn OnSessionDisconnected(&self, _: windows::Win32::Media::Audio::AudioSessionDisconnectReason) -> windows::core::Result<()> {
        self.media_transport.refresh();
        Ok(())
    }
}

#[implement(IAudioSessionNotification)]
struct AudioSessionNotificationHandler {
    endpoint_id: String,
    debouncer: Arc<EventDebouncer<()>>,
    registry: SessionRegistry,
    media_transport: MediaTransportHandle,
}

impl IAudioSessionNotification_Impl for AudioSessionNotificationHandler {
    fn OnSessionCreated(&self, session: Option<&IAudioSessionControl>) -> windows::core::Result<()> {
        if let Some(control) = session {
            register_session_events(control, &self.endpoint_id, &self.debouncer, &self.registry, &self.media_transport);
            self.media_transport.refresh();
        }
        Ok(())
    }
}

fn register_session_events(
    control: &IAudioSessionControl,
    endpoint_id: &str,
    debouncer: &Arc<EventDebouncer<()>>,
    registry: &SessionRegistry,
    media_transport: &MediaTransportHandle,
) {
    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
        unsafe {
            let pid = control2.GetProcessId().unwrap_or(0);
            if pid != 0 {
                let handler: IAudioSessionEvents = AudioSessionEventsHandler {
                    pid,
                    endpoint_id: endpoint_id.to_string(),
                    debouncer: debouncer.clone(),
                    media_transport: media_transport.clone(),
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
    media_transport: MediaTransportHandle,
}

impl AudioSessionEventsManager {
    pub fn new(media_transport: MediaTransportHandle) -> Self {
        Self { media_transport }
    }

    pub fn run(&self) {
        let media_transport = self.media_transport.clone();
        let debouncer = Arc::new(EventDebouncer::new(Duration::from_millis(10), move |_| {
            media_transport.refresh();
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
                                    debouncer: debouncer.clone(),
                                    registry: registry.clone(),
                                    media_transport: self.media_transport.clone(),
                                }
                                .into();

                                if session_manager.RegisterSessionNotification(&notification_handler).is_ok() {
                                    if let Ok(enum_) = session_manager.GetSessionEnumerator() {
                                        if let Ok(s_count) = enum_.GetCount() {
                                            for j in 0..s_count {
                                                if let Ok(control) = enum_.GetSession(j) {
                                                    register_session_events(&control, &endpoint_id, &debouncer, &registry, &self.media_transport);
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

pub fn start_media_transport_worker(
    state: SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: Arc<super::audio_sessions::WindowsAudioSessionManager>,
) -> MediaTransportHandle {
    let (sender, receiver) = unbounded();
    let media_transport = MediaTransportHandle { sender };
    let worker_handle = media_transport.clone();

    thread::spawn(move || {
        run_media_transport_worker(receiver, worker_handle, state, is_dev, extract_thumbnails, audio_manager);
    });

    media_transport
}

fn run_media_transport_worker(
    receiver: Receiver<MediaTransportCommand>,
    media_transport: MediaTransportHandle,
    state: SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: Arc<super::audio_sessions::WindowsAudioSessionManager>,
) {
    let init_result = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    if init_result.is_err() {
        error!(error = ?init_result, "Failed to initialize Windows Runtime for media transport worker");
        return;
    }

    let operation = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
        Ok(operation) => operation,
        Err(error) => {
            error!(error = ?error, "Failed to create media transport session manager request");
            unsafe { RoUninitialize() };
            return;
        }
    };

    let manager = match block_on(operation) {
        Ok(manager) => manager,
        Err(error) => {
            error!(error = ?error, "Failed to request media transport session manager");
            unsafe { RoUninitialize() };
            return;
        }
    };

    let sessions_changed_handle = media_transport.clone();
    let _ = manager.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
        sessions_changed_handle.sessions_changed();
        Ok(())
    }));

    let mut subscriptions = Vec::new();
    update_gsmtc_subscriptions(
        &manager,
        &mut subscriptions,
        &media_transport,
        &state,
        is_dev,
        extract_thumbnails,
        &audio_manager,
    );

    while let Ok(command) = receiver.recv() {
        match command {
            MediaTransportCommand::Refresh => {
                if let Err(error) = media_session_control::handle_playback_update(&manager, &state, is_dev, extract_thumbnails, &audio_manager) {
                    warn!(error = %error, "Failed to refresh media sessions");
                }
            }
            MediaTransportCommand::SessionsChanged => update_gsmtc_subscriptions(
                &manager,
                &mut subscriptions,
                &media_transport,
                &state,
                is_dev,
                extract_thumbnails,
                &audio_manager,
            ),
            MediaTransportCommand::Pause(apps, response) => {
                let result = media_session_control::pause(&manager, apps, &state, is_dev, extract_thumbnails, &audio_manager);
                let _ = response.send(result);
            }
            MediaTransportCommand::Resume(apps, response) => {
                let result = media_session_control::resume(&manager, apps, &state, is_dev, extract_thumbnails, &audio_manager);
                let _ = response.send(result);
            }
            MediaTransportCommand::SetVolume(app, volume, response) => {
                let result = media_session_control::set_volume(&manager, app, volume, &state, &audio_manager);
                let _ = response.send(result);
            }
        }
    }

    unsafe { RoUninitialize() };
}

fn update_gsmtc_subscriptions(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
    subscriptions: &mut Vec<SessionSubscription>,
    media_transport: &MediaTransportHandle,
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
        Ok(sessions) => sessions,
        Err(error) => {
            warn!(error = ?error, "Failed to get media transport sessions");
            return;
        }
    };

    for i in 0..sessions.Size().unwrap_or(0) {
        if let Ok(session) = sessions.GetAt(i) {
            let playback_handle = media_transport.clone();
            let media_handle = media_transport.clone();

            let playback_token = session
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    playback_handle.refresh();
                    Ok(())
                }))
                .unwrap_or_default();

            let media_token = session
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    media_handle.refresh();
                    Ok(())
                }))
                .unwrap_or_default();

            subscriptions.push(SessionSubscription {
                session,
                playback_token,
                media_token,
            });
        }
    }

    if let Err(error) = media_session_control::handle_playback_update(manager, state, is_dev, extract_thumbnails, audio_manager) {
        warn!(error = %error, "Failed to collect media sessions");
    }
}
