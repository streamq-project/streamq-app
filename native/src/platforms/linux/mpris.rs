use napi::bindgen_prelude::spawn;
use std::{
    sync::{Arc, Mutex, RwLock},
    time::Instant,
};
use tracing::{error, info};

use mpris_server::{LoopStatus, Metadata, PlaybackStatus, PlayerInterface, Property, RootInterface, Server, Time, TrackId, zbus, zbus::fdo};

use crate::config::Config;
use crate::models::mpris::{MprisMetadata, MprisPlaybackState};

const MPRIS_APP_NAME: &str = "streamq";
const MPRIS_IDENTITY: &str = "StreamQ";

#[derive(Clone)]
struct PlayerState {
    status: PlaybackStatus,
    metadata: Metadata,
    position: f64,
    position_updated_at: Instant,
    volume: f64,
    shuffle: bool,
    loop_status: LoopStatus,
}

impl PlayerState {
    fn current_position(&self) -> f64 {
        self.position
            + if matches!(self.status, PlaybackStatus::Playing) {
                self.position_updated_at.elapsed().as_secs_f64()
            } else {
                0.0
            }
    }
}

struct StreamQPlayer {
    state: Arc<RwLock<PlayerState>>,
    event_emitter: Arc<crate::event_emitter::EventEmitter>,
}

impl StreamQPlayer {
    fn emit_action(&self, action: crate::models::mpris::MediaAction) {
        if let Ok(value) = serde_json::to_value(action) {
            self.event_emitter.emit(crate::event_emitter::Event::MediaAction(value));
        }
    }
}

impl RootInterface for StreamQPlayer {
    async fn identity(&self) -> fdo::Result<String> {
        Ok(MPRIS_IDENTITY.to_owned())
    }
    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok(MPRIS_APP_NAME.to_owned())
    }
    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn raise(&self) -> fdo::Result<()> {
        Ok(())
    }
    async fn quit(&self) -> fdo::Result<()> {
        Ok(())
    }
    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }
    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }
}

impl PlayerInterface for StreamQPlayer {
    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(self.state.read().unwrap().status)
    }
    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(self.state.read().unwrap().metadata.clone())
    }
    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }
    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }
    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }
    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }
    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
    async fn play(&self) -> fdo::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::Play);
        Ok(())
    }
    async fn pause(&self) -> fdo::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::Pause);
        Ok(())
    }
    async fn play_pause(&self) -> fdo::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::Playpause);
        Ok(())
    }
    async fn next(&self) -> fdo::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::Next);
        Ok(())
    }
    async fn previous(&self) -> fdo::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::Previous);
        Ok(())
    }
    async fn stop(&self) -> fdo::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::Stop);
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::Seek {
            offset: offset.as_micros() as f64 / 1_000_000.0,
        });
        Ok(())
    }
    async fn set_position(&self, _track_id: TrackId, position: Time) -> fdo::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::SeekTo {
            position: position.as_micros() as f64 / 1_000_000.0,
        });
        Ok(())
    }
    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Ok(())
    }
    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(self.state.read().unwrap().loop_status)
    }
    async fn set_loop_status(&self, loop_status: LoopStatus) -> zbus::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::SetRepeat {
            state: loop_status.as_str().to_ascii_lowercase(),
        });
        Ok(())
    }
    async fn rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }
    async fn set_rate(&self, _rate: f64) -> zbus::Result<()> {
        Ok(())
    }
    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(self.state.read().unwrap().shuffle)
    }
    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::SetShuffle { state: shuffle });
        Ok(())
    }
    async fn volume(&self) -> fdo::Result<f64> {
        Ok(self.state.read().unwrap().volume)
    }
    async fn set_volume(&self, volume: f64) -> zbus::Result<()> {
        self.emit_action(crate::models::mpris::MediaAction::SetVolume { value: volume });
        Ok(())
    }
    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros((self.state.read().unwrap().current_position() * 1_000_000.0) as i64))
    }
    async fn minimum_rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }
    async fn maximum_rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }
    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

enum MprisCommand {
    UpdateMetadata(Option<MprisMetadata>),
    UpdatePlaybackState(MprisPlaybackState),
}

pub struct MprisManager {
    tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<MprisCommand>>>,
    event_emitter: Arc<crate::event_emitter::EventEmitter>,
}

impl MprisManager {
    pub fn new(_config: Config, event_emitter: Arc<crate::event_emitter::EventEmitter>) -> Self {
        Self {
            tx: Mutex::new(None),
            event_emitter,
        }
    }

    pub fn init(&self) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        *self.tx.lock().unwrap() = Some(tx);
        let event_emitter = self.event_emitter.clone();

        spawn(async move {
            let state = Arc::new(RwLock::new(PlayerState {
                status: PlaybackStatus::Stopped,
                metadata: Metadata::new(),
                position: 0.0,
                position_updated_at: Instant::now(),
                volume: 1.0,
                shuffle: false,
                loop_status: LoopStatus::None,
            }));

            let player = StreamQPlayer {
                state: state.clone(),
                event_emitter,
            };

            let server = match Server::new(MPRIS_APP_NAME, player).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to start MPRIS server: {}", e);
                    return;
                }
            };

            info!("MPRIS D-Bus server started successfully");

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    MprisCommand::UpdateMetadata(Some(meta)) => {
                        let track_id = format!("/io/streamq/track/{}", meta.id);
                        let mut builder = Metadata::builder()
                            .trackid(TrackId::try_from(track_id).expect("MPRIS track ID must be valid"))
                            .title(meta.title.as_str())
                            .album(meta.album.as_str());

                        if !meta.artist.is_empty() {
                            builder = builder.artist(vec![meta.artist.as_str()]);
                        }

                        if let Some(length) = meta.length {
                            builder = builder.length(Time::from_millis(length as i64));
                        }

                        if let Some(art) = meta.art_url {
                            builder = builder.art_url(art);
                        }

                        if let Some(url) = meta.url {
                            builder = builder.url(url);
                        }

                        let metadata = builder.build();
                        let mut changes = vec![Property::Metadata(metadata.clone())];

                        {
                            let mut state_lock = state.write().unwrap();
                            state_lock.metadata = metadata;

                            if let Some(vol) = meta.volume {
                                state_lock.volume = vol;
                                changes.push(Property::Volume(vol));
                            }
                        }

                        let _ = server.properties_changed(changes).await;
                    }
                    MprisCommand::UpdateMetadata(None) => {
                        let metadata = Metadata::new();
                        {
                            let mut state = state.write().unwrap();
                            state.status = PlaybackStatus::Stopped;
                            state.metadata = metadata.clone();
                            state.position = 0.0;
                            state.position_updated_at = Instant::now();
                            state.shuffle = false;
                            state.loop_status = LoopStatus::None;
                        }
                        let _ = server
                            .properties_changed([
                                Property::PlaybackStatus(PlaybackStatus::Stopped),
                                Property::Metadata(metadata),
                                Property::Shuffle(false),
                                Property::LoopStatus(LoopStatus::None),
                            ])
                            .await;
                    }
                    MprisCommand::UpdatePlaybackState(update) => {
                        let mut changes = Vec::new();

                        {
                            let mut state = state.write().unwrap();

                            if let Some(position) = update.position {
                                state.position = position;
                                state.position_updated_at = Instant::now();
                            }

                            if let Some(status) = update.status {
                                if update.position.is_none() {
                                    state.position = state.current_position();
                                    state.position_updated_at = Instant::now();
                                }
                                state.status = match status.as_str() {
                                    "playing" => PlaybackStatus::Playing,
                                    "paused" => PlaybackStatus::Paused,
                                    _ => PlaybackStatus::Stopped,
                                };
                                changes.push(Property::PlaybackStatus(state.status));
                            }

                            if let Some(volume) = update.volume {
                                state.volume = volume;
                                changes.push(Property::Volume(volume));
                            }

                            if let Some(shuffle) = update.shuffle {
                                state.shuffle = shuffle;
                                changes.push(Property::Shuffle(shuffle));
                            }

                            if let Some(repeat) = update.repeat {
                                state.loop_status = match repeat.as_str() {
                                    "track" => LoopStatus::Track,
                                    "playlist" => LoopStatus::Playlist,
                                    _ => LoopStatus::None,
                                };
                                changes.push(Property::LoopStatus(state.loop_status));
                            }
                        }

                        if !changes.is_empty() {
                            let _ = server.properties_changed(changes).await;
                        }
                    }
                }
            }
        });
    }

    pub fn update_metadata(&self, metadata: Option<MprisMetadata>) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(MprisCommand::UpdateMetadata(metadata));
        }
    }

    pub fn update_playback_state(&self, state: MprisPlaybackState) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(MprisCommand::UpdatePlaybackState(state));
        }
    }
}
