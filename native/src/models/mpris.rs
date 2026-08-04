use napi_derive::napi;
use serde::{Deserialize, Serialize};

#[napi(object)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MprisMetadata {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length: Option<f64>,
    pub art_url: Option<String>,
    pub url: Option<String>,
    pub volume: Option<f64>,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct MprisPlaybackState {
    pub status: Option<String>,
    pub position: Option<f64>,
    pub volume: Option<f64>,
    pub shuffle: Option<bool>,
    pub repeat: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum MediaAction {
    Play,
    Pause,
    Playpause,
    Next,
    Previous,
    Stop,
    #[serde(rename = "setVolume")]
    SetVolume {
        value: f64,
    },
    Seek {
        offset: f64,
    },
    SeekTo {
        position: f64,
    },
    #[serde(rename = "setShuffle")]
    SetShuffle {
        state: bool,
    },
    #[serde(rename = "setRepeat")]
    SetRepeat {
        state: String,
    },
}
