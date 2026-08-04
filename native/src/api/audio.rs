use crate::config::Config;
use crate::error::ResultExt;
use crate::models::media::AudioSource;
use napi::{Env, Result};

#[cfg(target_os = "windows")]
use crate::platforms::windows::media_session::MediaSessionManager;

#[cfg(target_os = "linux")]
use crate::platforms::linux::media_session::MediaSessionManager;

#[napi]
pub struct Audio {
    manager: std::sync::Arc<MediaSessionManager>,
}

#[napi]
impl Audio {
    #[napi(constructor)]
    pub fn new(config: Config) -> Self {
        Self {
            manager: std::sync::Arc::new(MediaSessionManager::new(config, std::sync::Arc::new(crate::event_emitter::EventEmitter::new()))),
        }
    }

    pub fn from_manager(manager: std::sync::Arc<MediaSessionManager>) -> Self {
        Self { manager }
    }

    #[napi]
    pub fn get_audio_sources(&self, env: Env) -> Result<Vec<AudioSource>> {
        self.manager.get_audio_sources().into_napi(&env)
    }

    #[napi]
    pub fn set_app_source(&self, env: Env, source: String) -> Result<()> {
        self.manager.set_app_source(source).into_napi(&env)
    }

    #[napi]
    pub fn get_app_source(&self, env: Env) -> Result<Option<AudioSource>> {
        self.manager.get_app_source().into_napi(&env)
    }

    #[napi]
    pub fn set_system_volume(&self, env: Env, volume: f64) -> Result<()> {
        self.manager.set_system_volume(volume).into_napi(&env)
    }
}
