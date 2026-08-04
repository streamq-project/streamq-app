use crate::models::mpris::{MprisMetadata, MprisPlaybackState};
use napi::Result;

#[napi]
pub struct Mpris {
    #[cfg(target_os = "linux")]
    manager: std::sync::Arc<crate::platforms::linux::mpris::MprisManager>,
}

#[napi]
impl Mpris {
    #[cfg(target_os = "linux")]
    pub fn from_manager(manager: std::sync::Arc<crate::platforms::linux::mpris::MprisManager>) -> Self {
        Self { manager }
    }

    #[cfg(target_os = "windows")]
    pub fn new_dummy() -> Self {
        Self {}
    }

    #[napi]
    pub fn init(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        self.manager.init();
        Ok(())
    }

    #[napi]
    #[allow(unused_variables)]
    pub fn update_metadata(&self, metadata: Option<MprisMetadata>) -> Result<()> {
        #[cfg(target_os = "linux")]
        self.manager.update_metadata(metadata);
        Ok(())
    }

    #[napi]
    #[allow(unused_variables)]
    pub fn update_playback_state(&self, state: MprisPlaybackState) -> Result<()> {
        #[cfg(target_os = "linux")]
        self.manager.update_playback_state(state);
        Ok(())
    }
}
