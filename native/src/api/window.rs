use crate::config::Config;
use napi::{Env, Result};

#[cfg(target_os = "windows")]
use crate::error::ResultExt;

#[cfg(target_os = "windows")]
use crate::platforms::windows::window::WindowManager;

#[cfg(target_os = "linux")]
use crate::platforms::linux::window::WindowManager;

#[napi]
pub struct Window {
    manager: WindowManager,
}

#[napi]
impl Window {
    #[napi(constructor)]
    pub fn new(config: Config) -> Self {
        Self {
            manager: WindowManager::new(config),
        }
    }

    #[napi]
    #[allow(unused_variables)]
    pub fn set_acrylic(&self, env: Env, hwnd: i64, enable: bool, color: Option<(u8, u8, u8, u8)>) -> Result<()> {
        #[cfg(target_os = "linux")]
        return Ok(());
        #[cfg(target_os = "windows")]
        self.manager.set_acrylic(hwnd, enable, color).into_napi(&env)
    }

    #[napi]
    #[allow(unused_variables)]
    pub fn restore_native_frame(&self, env: Env, hwnd: i64) -> Result<()> {
        #[cfg(target_os = "linux")]
        return Ok(());
        #[cfg(target_os = "windows")]
        self.manager.restore_native_frame(hwnd).into_napi(&env)
    }

    #[napi]
    #[allow(unused_variables)]
    pub fn disable_rounds(&self, env: Env, hwnd: i64) -> Result<()> {
        #[cfg(target_os = "linux")]
        return Ok(());
        #[cfg(target_os = "windows")]
        return self.manager.disable_rounds(hwnd).into_napi(&env);
    }

    #[napi]
    #[allow(unused_variables)]
    pub fn set_pip_always_on_top_mode(&self, env: Env, on_top: bool) -> Result<bool> {
        #[cfg(target_os = "linux")]
        return Ok(false);
        #[cfg(target_os = "windows")]
        self.manager.set_pip_always_on_top_mode(on_top).into_napi(&env)
    }

    #[napi]
    pub fn get_decorations(&self) -> Result<Option<Vec<String>>> {
        #[cfg(target_os = "linux")]
        return self.manager.get_decorations();
        #[cfg(target_os = "windows")]
        return Ok(None);
    }
}
