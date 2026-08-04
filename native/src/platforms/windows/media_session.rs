use std::sync::Arc;
use std::thread;

use tracing::{info, instrument, warn};

use crate::config::Config;
use crate::error::NativeError;
use crate::models::media::{AppMediaSession, AudioSource, MediaResponse, MediaSessionState, SharedState};

use super::audio;

use super::media_session_control;
use super::media_session_events::AudioSessionEventsManager;
use super::media_session_events::start_media_transport_listener;

pub struct MediaSessionManager {
    is_dev: bool,
    extract_thumbnails: bool,
    state: SharedState,
    audio_manager: Arc<super::audio_sessions::WindowsAudioSessionManager>,
}

impl MediaSessionManager {
    pub fn new(config: Config, event_emitter: Arc<crate::event_emitter::EventEmitter>) -> Self {
        let manager = Self {
            is_dev: config.debug,
            extract_thumbnails: config.extract_thumbnails.unwrap_or(true),
            state: Arc::new(MediaSessionState::new(event_emitter)),
            audio_manager: Arc::new(super::audio_sessions::WindowsAudioSessionManager::new()),
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

        let initial_volume = audio::get_system_volume();
        *self.state.system_volume.lock().unwrap() = initial_volume;
        tracing::info!("initialize: Initial system volume = {}", initial_volume);

        audio::start_volume_listener(self.state.clone());
        audio::start_app_source_watcher(self.state.clone());

        if let Err(e) = media_session_control::handle_playback_update(&self.state, self.is_dev, self.extract_thumbnails, &self.audio_manager) {
            warn!(error = %e, "Failed to collect initial media sessions");
        }

        let state_for_audio_events = self.state.clone();
        let is_dev = self.is_dev;
        let extract_thumbnails = self.extract_thumbnails;
        let audio_manager = self.audio_manager.clone();
        thread::spawn(move || {
            AudioSessionEventsManager::new(state_for_audio_events, is_dev, extract_thumbnails, audio_manager).run();
        });

        start_media_transport_listener(self.state.clone(), self.is_dev, self.extract_thumbnails, self.audio_manager.clone());
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
        media_session_control::pause(apps, &self.state, self.is_dev, self.extract_thumbnails, &self.audio_manager)
    }

    #[instrument(skip(self))]
    pub fn resume(&self, apps: Vec<String>) -> Result<(), NativeError> {
        media_session_control::resume(apps, &self.state, self.is_dev, self.extract_thumbnails, &self.audio_manager)
    }

    #[instrument(skip(self))]
    pub fn set_volume(&self, app: String, volume: f64) -> Result<(), NativeError> {
        media_session_control::set_volume(app, volume, &self.state, &self.audio_manager)
    }
}
