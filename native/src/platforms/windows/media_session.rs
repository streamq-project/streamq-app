use std::sync::Arc;
use std::thread;

use tracing::{info, instrument};

use crate::config::Config;
use crate::error::NativeError;
use crate::models::media::{AppMediaSession, AudioSource, MediaResponse, MediaSessionState, SharedState};

use super::audio;

use super::media_session_events::{AudioSessionEventsManager, MediaTransportHandle, start_media_transport_worker};

pub struct MediaSessionManager {
    is_dev: bool,
    extract_thumbnails: bool,
    state: SharedState,
    media_transport: MediaTransportHandle,
}

impl MediaSessionManager {
    pub fn new(config: Config, event_emitter: Arc<crate::event_emitter::EventEmitter>) -> Self {
        let is_dev = config.debug;
        let extract_thumbnails = config.extract_thumbnails.unwrap_or(true);
        let state = Arc::new(MediaSessionState::new(event_emitter));
        let audio_manager = Arc::new(super::audio_sessions::WindowsAudioSessionManager::new());
        let media_transport = start_media_transport_worker(state.clone(), is_dev, extract_thumbnails, audio_manager.clone());

        let manager = Self {
            is_dev,
            extract_thumbnails,
            state,
            media_transport,
        };

        manager.initialize();
        manager
    }

    #[instrument(skip(self))]
    fn initialize(&self) {
        info!(
            debug = self.is_dev,
            extract_thumbnails = self.extract_thumbnails,
            "Windows media session manager initializing"
        );

        let state_for_initial_volume = self.state.clone();
        thread::spawn(move || {
            let initial_volume = audio::get_system_volume();
            *state_for_initial_volume.system_volume.lock().unwrap() = initial_volume;
            tracing::info!("initialize: Initial system volume = {}", initial_volume);
        });

        audio::start_volume_listener(self.state.clone());
        audio::start_app_source_watcher(self.state.clone());

        let media_transport = self.media_transport.clone();
        thread::spawn(move || {
            AudioSessionEventsManager::new(media_transport).run();
        });
    }

    #[instrument(skip(self))]
    pub fn get_state(&self) -> MediaResponse {
        let sessions = self.state.sessions.lock().unwrap().clone();
        let volume = self.get_system_volume();
        let sources = self.get_audio_sources().unwrap_or_else(|e| {
            tracing::warn!("get_state: failed to load audio sources: {:?}", e);
            Vec::new()
        });
        let app_source = self.get_app_source().ok().flatten();

        MediaResponse {
            volume,
            sessions,
            sources,
            app_source,
        }
    }

    fn get_system_volume(&self) -> f64 {
        audio::get_system_volume()
    }

    pub fn set_system_volume(&self, volume: f64) -> Result<(), NativeError> {
        audio::set_system_volume(volume).map_err(|e| {
            tracing::error!("set_system_volume: Failed to set volume: {:?}", e);
            NativeError::Generic(e.to_string())
        })
    }

    pub fn get_audio_sources(&self) -> Result<Vec<AudioSource>, NativeError> {
        audio::get_audio_sources().map_err(|e| {
            tracing::error!("get_audio_sources: Failed to list sources: {:?}", e);
            NativeError::Generic(e.to_string())
        })
    }

    pub fn set_app_source(&self, source: String) -> Result<(), NativeError> {
        audio::set_app_source(&source).inspect_err(|e| {
            tracing::error!("set_app_source: Failed to set source '{}': {:?}", source, e);
        })
    }

    pub fn get_app_source(&self) -> Result<Option<AudioSource>, NativeError> {
        audio::get_app_source().map_err(|e| {
            tracing::error!("get_app_source: Failed to get app source: {:?}", e);
            NativeError::Generic(e.to_string())
        })
    }

    pub fn pause(&self, apps: Vec<String>) -> Result<Vec<AppMediaSession>, NativeError> {
        self.media_transport.pause(apps)
    }

    #[instrument(skip(self))]
    pub fn resume(&self, apps: Vec<String>) -> Result<(), NativeError> {
        self.media_transport.resume(apps)
    }

    #[instrument(skip(self))]
    pub fn set_volume(&self, app: String, volume: f64) -> Result<(), NativeError> {
        self.media_transport.set_volume(app, volume)
    }
}
