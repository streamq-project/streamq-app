use crate::error::NativeError;
use crate::models::widgets::WidgetBounds;

pub struct WidgetsManager;

impl WidgetsManager {
    pub fn new() -> Self {
        Self
    }

    #[allow(unused_variables)]
    pub fn create_overlay(&self, url: String, bounds: WidgetBounds) -> Result<u32, NativeError> {
        Err(NativeError::OperationFailed("Native overlay widgets are only supported on Linux".into()))
    }

    pub fn destroy(&self, _id: u32) -> Result<(), NativeError> {
        Ok(())
    }

    pub fn destroy_all(&self) -> Result<(), NativeError> {
        Ok(())
    }
}
