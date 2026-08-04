use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[napi(object, use_nullable = true)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AppMediaSession {
    pub app: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub is_resumed: bool,
    pub is_suppressed: bool,
    pub volume: Option<f64>,
    pub device: Option<String>,
    pub device_volume: Option<f64>,
    pub can_control: Option<bool>,
    pub updated_at: f64,
    pub activated_at: f64,
    pub url: Option<String>,
    pub art: Option<String>,
    pub position: Option<f64>,
    pub duration: Option<f64>,
}

#[napi(object, use_nullable = true)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudioSource {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub volume: f64,
}

pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as f64
}

impl AppMediaSession {
    fn content_changed(&self, other: &Self) -> bool {
        self.activation_changed(other) || self.position != other.position
    }
    fn activation_changed(&self, other: &Self) -> bool {
        self.app != other.app
            || self.title != other.title
            || self.artist != other.artist
            || self.is_resumed != other.is_resumed
            || self.is_suppressed != other.is_suppressed
            || self.url != other.url
            || self.art != other.art
            || self.duration != other.duration
    }
}

impl PartialEq for AppMediaSession {
    fn eq(&self, other: &Self) -> bool {
        self.app == other.app
            && self.title == other.title
            && self.artist == other.artist
            && self.is_resumed == other.is_resumed
            && self.is_suppressed == other.is_suppressed
            && self.volume == other.volume
            && self.device == other.device
            && self.device_volume == other.device_volume
            && self.can_control == other.can_control
            && self.url == other.url
            && self.art == other.art
            && self.position == other.position
            && self.duration == other.duration
    }
}

pub struct MediaSessionState {
    pub sessions: Mutex<Vec<AppMediaSession>>,
    pub suppressed_apps: Mutex<HashSet<String>>,
    pub system_volume: Mutex<f64>,
    pub event_emitter: Arc<crate::event_emitter::EventEmitter>,
}

#[napi(object, use_nullable = true)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct MediaResponse {
    pub volume: f64,
    pub sessions: Vec<AppMediaSession>,
    pub sources: Vec<AudioSource>,
    pub app_source: Option<AudioSource>,
}

impl MediaSessionState {
    pub fn new(event_emitter: Arc<crate::event_emitter::EventEmitter>) -> Self {
        Self {
            sessions: Mutex::new(Vec::new()),
            suppressed_apps: Mutex::new(HashSet::new()),
            system_volume: Mutex::new(1.0),
            event_emitter,
        }
    }

    pub fn emit_if_changed(&self, mut new_sessions: Vec<AppMediaSession>) {
        let mut guard = self.sessions.lock().unwrap();
        let old_sessions = &*guard;

        let old_map: std::collections::HashMap<&str, &AppMediaSession> = old_sessions.iter().map(|s| (s.app.as_str(), s)).collect();

        let mut has_changes = false;
        for new in &mut new_sessions {
            if let Some(old) = old_map.get(new.app.as_str()) {
                if new != *old {
                    has_changes = true;
                }
                if !new.content_changed(old) {
                    new.updated_at = old.updated_at;
                }
                if !new.activation_changed(old) {
                    new.activated_at = old.activated_at;
                }
            } else {
                has_changes = true;
            }
        }

        if !has_changes && new_sessions.len() != old_sessions.len() {
            has_changes = true;
        }

        if !has_changes {
            return;
        }
        *guard = new_sessions.clone();
        drop(guard);
        self.event_emitter.emit(crate::event_emitter::Event::MediaSessionsChanged(new_sessions));
    }
}

pub type SharedState = Arc<MediaSessionState>;
