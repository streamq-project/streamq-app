use crate::config::Config;
use std::sync::Arc;

#[cfg(target_os = "windows")]
use crate::platforms::windows::keybinds::KeybindsManager;

#[cfg(target_os = "linux")]
use crate::platforms::linux::keybinds::KeybindsManager;

#[napi]
pub struct Keybinds {
    manager: Arc<KeybindsManager>,
}

#[napi]
impl Keybinds {
    #[napi(constructor)]
    pub fn new(config: Config) -> Self {
        Self {
            manager: Arc::new(KeybindsManager::new(config, Arc::new(crate::event_emitter::EventEmitter::new()))),
        }
    }

    pub fn from_manager(manager: Arc<KeybindsManager>) -> Self {
        Self { manager }
    }

    #[napi]
    pub fn set_keybinds(&self, keybinds_list: Vec<crate::config::Keybind>) {
        self.manager.set_keybinds(keybinds_list);
    }

    #[napi]
    pub fn cleanup(&self) {
        self.manager.cleanup();
    }
}
