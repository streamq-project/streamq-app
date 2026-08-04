use std::sync::Arc;

use crate::config::Config;
use crate::error::NativeError;
use crate::models::media::{AppMediaSession, AudioSource, MediaResponse, MediaSessionState, SharedState};

use super::audio;
use super::media_session_events;
use super::media_session_mpris;

pub struct MediaSessionManager {
    is_dev: bool,
    extract_thumbnails: bool,
    state: SharedState,
    audio_manager: Arc<super::audio_sessions::AudioSessionManager>,
}

impl MediaSessionManager {
    pub fn new(config: Config, event_emitter: Arc<crate::event_emitter::EventEmitter>) -> Self {
        let manager = Self {
            is_dev: config.debug,
            extract_thumbnails: config.extract_thumbnails.unwrap_or(true),
            state: Arc::new(MediaSessionState::new(event_emitter)),
            audio_manager: Arc::new(super::audio_sessions::AudioSessionManager::new()),
        };

        manager.initialize();
        manager
    }

    fn initialize(&self) {
        let initial_volume = audio::get_system_volume();
        *self.state.system_volume.lock().unwrap() = initial_volume;
        tracing::info!("initialize: Initial system volume = {}", initial_volume);

        let (refresh_tx, refresh_rx) = tokio::sync::mpsc::unbounded_channel();

        audio::start_volume_listener(self.state.clone(), refresh_tx, self.is_dev, self.audio_manager.clone());

        media_session_events::start_monitor(self.state.clone(), refresh_rx, self.audio_manager.clone(), self.extract_thumbnails);
    }

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
        audio::set_app_source(&source, self.is_dev).inspect_err(|e| {
            tracing::error!("set_app_source: Failed to set source '{}': {:?}", source, e);
        })
    }

    pub fn get_app_source(&self) -> Result<Option<AudioSource>, NativeError> {
        audio::get_app_source(self.is_dev).map_err(|e| {
            tracing::error!("get_app_source: Failed to get app source: {:?}", e);
            NativeError::Generic(e.to_string())
        })
    }

    pub async fn pause(&self, apps: Vec<String>) -> Result<Vec<AppMediaSession>, NativeError> {
        media_session_mpris::pause_async(apps, &self.state, self.is_dev, self.extract_thumbnails, &self.audio_manager)
            .await
            .map_err(NativeError::from)
    }

    pub async fn resume(&self, apps: Vec<String>) -> Result<(), NativeError> {
        media_session_mpris::resume_async(apps, &self.state).await.map_err(NativeError::from)
    }

    pub async fn set_volume(&self, app: String, volume: f64) -> Result<(), NativeError> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(NativeError::InvalidVolume(volume));
        }

        media_session_mpris::set_volume_async(app, volume).await
    }
}
