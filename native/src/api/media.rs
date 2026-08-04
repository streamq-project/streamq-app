use crate::config::Config;
use crate::models::media::{AppMediaSession, MediaResponse};
use napi::{Env, Result};

use crate::error::ResultExt;
#[cfg(target_os = "linux")]
use crate::error::execute_napi_future;
#[cfg(target_os = "linux")]
use napi::bindgen_prelude::PromiseRaw;

#[cfg(target_os = "windows")]
use crate::platforms::windows::media_session::MediaSessionManager;

#[cfg(target_os = "linux")]
use crate::platforms::linux::media_session::MediaSessionManager;

#[napi]
pub struct Media {
    manager: std::sync::Arc<MediaSessionManager>,
}

#[napi]
impl Media {
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
    pub fn get_state(&self) -> MediaResponse {
        self.manager.get_state()
    }

    #[napi]
    pub fn set_app_source(&self, env: &Env, source: String) -> Result<()> {
        self.manager.set_app_source(source).into_napi(env)
    }

    #[napi]
    #[cfg(target_os = "windows")]
    pub fn pause(&self, env: &Env, apps: Vec<String>) -> Result<Vec<AppMediaSession>> {
        self.manager.pause(apps).into_napi(env)
    }

    #[napi]
    #[cfg(target_os = "linux")]
    pub fn pause<'env>(&self, env: &'env Env, apps: Vec<String>) -> Result<PromiseRaw<'env, Vec<AppMediaSession>>> {
        let manager = self.manager.clone();
        execute_napi_future(env, async move { manager.pause(apps).await })
    }

    #[napi]
    #[cfg(target_os = "windows")]
    pub fn resume(&self, env: &Env, apps: Vec<String>) -> Result<()> {
        self.manager.resume(apps).into_napi(env)
    }

    #[napi]
    #[cfg(target_os = "linux")]
    pub fn resume<'env>(&self, env: &'env Env, apps: Vec<String>) -> Result<PromiseRaw<'env, ()>> {
        let manager = self.manager.clone();
        execute_napi_future(env, async move { manager.resume(apps).await })
    }

    #[napi]
    #[cfg(target_os = "windows")]
    pub fn set_volume(&self, env: &Env, app: String, volume: f64) -> Result<()> {
        self.manager.set_volume(app, volume).into_napi(env)
    }

    #[napi]
    #[cfg(target_os = "linux")]
    pub fn set_volume<'env>(&self, env: &'env Env, app: String, volume: f64) -> Result<PromiseRaw<'env, ()>> {
        let manager = self.manager.clone();
        execute_napi_future(env, async move { manager.set_volume(app, volume).await })
    }
}
