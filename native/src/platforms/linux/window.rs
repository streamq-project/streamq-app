use crate::config::Config;
use napi::Result;

pub struct WindowManager;

impl WindowManager {
    pub fn new(_config: Config) -> Self {
        Self
    }

    pub fn get_decorations(&self) -> Result<Option<Vec<String>>> {
        use gtk::prelude::*;
        use gtk::{Settings, init};

        if init().is_err() {
            return Ok(None);
        }

        if let Some(settings) = Settings::default() {
            let layout: String = settings.property("gtk-decoration-layout");
            let decorations: Vec<String> = layout
                .split(':')
                .flat_map(|s| s.split(','))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();

            if decorations.is_empty() { Ok(None) } else { Ok(Some(decorations)) }
        } else {
            Ok(None)
        }
    }
}
