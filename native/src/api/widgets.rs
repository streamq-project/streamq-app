use crate::error::ResultExt;
use crate::models::widgets::WidgetBounds;
use napi::{Env, Result};

#[cfg(target_os = "linux")]
use crate::platforms::linux::widgets::WidgetsManager;

#[cfg(target_os = "windows")]
use crate::platforms::windows::widgets::WidgetsManager;

#[napi]
pub struct Widgets {
    manager: WidgetsManager,
}

#[napi]
impl Widgets {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            manager: WidgetsManager::new(),
        }
    }

    #[napi]
    pub fn create_overlay(&self, env: Env, url: String, bounds: WidgetBounds) -> Result<u32> {
        self.manager.create_overlay(url, bounds).into_napi(&env)
    }

    #[napi]
    pub fn destroy(&self, env: Env, id: u32) -> Result<()> {
        self.manager.destroy(id).into_napi(&env)
    }

    #[napi]
    pub fn destroy_all(&self, env: Env) -> Result<()> {
        self.manager.destroy_all().into_napi(&env)
    }
}
