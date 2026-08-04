use futures::executor::block_on;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use tracing::{debug, info, instrument};
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};

use crate::error::NativeError;
use crate::models::media::{AppMediaSession, SharedState, now_ms};
use crate::thumbnails::Thumbnails;

#[instrument(skip(state, audio_manager))]
pub fn handle_playback_update(
    state: &SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: &super::audio_sessions::WindowsAudioSessionManager,
) -> Result<(), NativeError> {
    let manager: windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager = block_on(
        windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(|e| NativeError::MediaSession(format!("Failed to create async operation: {}", e)))?,
    )
    .map_err(|e| NativeError::MediaSession(format!("Failed to request session manager: {}", e)))?;

    let sessions = manager
        .GetSessions()
        .map_err(|e| NativeError::MediaSession(format!("Failed to get sessions: {}", e)))?;

    let previous: HashMap<String, AppMediaSession> = state.sessions.lock().unwrap().iter().map(|s| (s.app.clone(), s.clone())).collect();

    let suppressed = state.suppressed_apps.lock().unwrap();
    let mut media_sessions: Vec<AppMediaSession> = Vec::new();
    let mut apps_playing: HashSet<String> = HashSet::new();
    let mut seen_apps: HashSet<String> = HashSet::new();

    let volumes_map = audio_manager.get_all_volumes();

    for i in 0..sessions.Size().unwrap_or(0) {
        let Ok(session) = sessions.GetAt(i) else {
            continue;
        };

        let Ok(app_id) = session.SourceAppUserModelId() else {
            continue;
        };
        let source_app = app_id.to_string().to_lowercase();

        if !seen_apps.insert(source_app.clone()) {
            continue;
        }

        let is_playing = session
            .GetPlaybackInfo()
            .and_then(|info| info.PlaybackStatus())
            .map(|status| status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
            .unwrap_or(false);

        debug!(
            app = %session.SourceAppUserModelId().unwrap(),
            is_playing = is_playing,
            is_streamq = is_streamq(source_app.clone(), is_dev),
            "Playback changed"
        );

        if is_streamq(source_app.clone(), is_dev) {
            continue;
        }

        if is_playing {
            apps_playing.insert(source_app.clone());
        }

        let app = app_id.to_string();
        let (title, artist, art) = if let Ok(op) = session.TryGetMediaPropertiesAsync() {
            if let Ok(media) = block_on(op) {
                let title = media.Title().ok().map(|s| s.to_string());
                let artist = media.Artist().ok().map(|s| s.to_string());
                let art = if extract_thumbnails {
                    resolve_art(&app, &title, &artist, media.Thumbnail().ok())
                } else {
                    None
                };
                (title, artist, art)
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        };

        let (position, duration) = if let Ok(timeline) = session.GetTimelineProperties() {
            (
                timeline.Position().ok().map(|p| p.Duration as f64 / 10000.0),
                timeline.EndTime().ok().map(|e| e.Duration as f64 / 10000.0),
            )
        } else {
            (None, None)
        };

        let is_suppressed = suppressed.contains(&source_app);

        let (volume, device, device_volume) = volumes_map
            .get(&source_app)
            .map(|(v, d, dv): &(f64, String, f64)| (Some(*v), Some(d.clone()), Some(*dv)))
            .unwrap_or((None, None, None));

        let now = now_ms();
        media_sessions.push(AppMediaSession {
            app,
            title,
            artist,
            is_resumed: is_playing,
            is_suppressed,
            volume,
            device,
            device_volume,
            can_control: None,
            updated_at: now,
            activated_at: now,
            url: None,
            art,
            position,
            duration,
        });
    }
    drop(suppressed);

    {
        let mut suppressed = state.suppressed_apps.lock().unwrap();
        suppressed.retain(|app| !apps_playing.contains(app));
    }

    let previous_list: Vec<AppMediaSession> = previous.into_values().collect();
    Thumbnails::release_unreferenced(&previous_list, &media_sessions);

    state.emit_if_changed(media_sessions);
    Ok(())
}

#[instrument(skip(state, audio_manager))]
pub fn pause(
    apps: Vec<String>,
    state: &SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: &super::audio_sessions::WindowsAudioSessionManager,
) -> Result<Vec<AppMediaSession>, NativeError> {
    let manager: windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager = block_on(
        windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(|e| NativeError::MediaSession(format!("Failed to create async operation: {}", e)))?,
    )
    .map_err(|e| NativeError::MediaSession(format!("Failed to request session manager: {}", e)))?;

    let sessions = manager
        .GetSessions()
        .map_err(|e| NativeError::MediaSession(format!("Failed to get sessions: {}", e)))?;

    let apps_lower: Vec<String> = apps.iter().map(|a| a.to_lowercase()).collect();
    let mut paused_sessions = vec![];
    let mut suppressed = state.suppressed_apps.lock().unwrap();

    let volumes_map = audio_manager.get_all_volumes();

    for i in 0..sessions.Size().unwrap_or(0) {
        let Ok(session) = sessions.GetAt(i) else {
            continue;
        };

        let Ok(app_id) = session.SourceAppUserModelId() else {
            continue;
        };
        let app = app_id.to_string();
        let source_app = app.to_lowercase();

        if !apps_lower.contains(&source_app) {
            continue;
        }

        let is_playing = session
            .GetPlaybackInfo()
            .and_then(|info| info.PlaybackStatus())
            .map(|status| status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
            .unwrap_or(false);

        if is_streamq(source_app.clone(), is_dev) {
            continue;
        }

        if is_playing {
            let pause_succeeded = session.TryPauseAsync().ok().and_then(|operation| block_on(operation).ok()).unwrap_or(false);

            if pause_succeeded {
                let (title, artist, art) = if let Ok(op) = session.TryGetMediaPropertiesAsync() {
                    if let Ok(media) = block_on(op) {
                        let title = media.Title().ok().map(|s| s.to_string());
                        let artist = media.Artist().ok().map(|s| s.to_string());
                        let art = if extract_thumbnails {
                            resolve_art(&app, &title, &artist, media.Thumbnail().ok())
                        } else {
                            None
                        };
                        (title, artist, art)
                    } else {
                        (None, None, None)
                    }
                } else {
                    (None, None, None)
                };

                let (position, duration) = if let Ok(timeline) = session.GetTimelineProperties() {
                    (
                        timeline.Position().ok().map(|p| p.Duration as f64 / 10000.0),
                        timeline.EndTime().ok().map(|e| e.Duration as f64 / 10000.0),
                    )
                } else {
                    (None, None)
                };

                info!(app = %app_id, "Paused media session");
                suppressed.insert(app.to_lowercase());

                let (volume, device, device_volume) = volumes_map
                    .get(&source_app)
                    .map(|(v, d, dv): &(f64, String, f64)| (Some(*v), Some(d.clone()), Some(*dv)))
                    .unwrap_or((None, None, None));

                let now = now_ms();
                paused_sessions.push(AppMediaSession {
                    app,
                    title,
                    artist,
                    is_resumed: false,
                    is_suppressed: true,
                    volume,
                    device,
                    device_volume,
                    can_control: None,
                    updated_at: now,
                    activated_at: now,
                    url: None,
                    art,
                    position,
                    duration,
                });
            }
        }
    }
    drop(suppressed);

    handle_playback_update(state, is_dev, extract_thumbnails, audio_manager)?;

    Ok(paused_sessions)
}

#[instrument(skip(state, audio_manager))]
pub fn resume(
    apps: Vec<String>,
    state: &SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: &super::audio_sessions::WindowsAudioSessionManager,
) -> Result<(), NativeError> {
    let manager: windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager = block_on(
        windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(|e| NativeError::MediaSession(format!("Failed to create async operation: {}", e)))?,
    )
    .map_err(|e| NativeError::MediaSession(format!("Failed to request session manager: {}", e)))?;

    let sessions = manager
        .GetSessions()
        .map_err(|e| NativeError::MediaSession(format!("Failed to get sessions: {}", e)))?;

    let apps_lower: Vec<String> = apps.iter().map(|a| a.to_lowercase()).collect();

    {
        let mut suppressed = state.suppressed_apps.lock().unwrap();
        debug!(
            apps = ?apps_lower,
            suppressed_before = ?suppressed,
            "Removing apps from suppressed list"
        );
        suppressed.retain(|app| !apps_lower.contains(&app.to_lowercase()));
    }

    for i in 0..sessions.Size().unwrap_or(0) {
        let Ok(session) = sessions.GetAt(i) else {
            continue;
        };

        let Ok(app_id) = session.SourceAppUserModelId() else {
            continue;
        };
        let app = app_id.to_string();
        let source_app = app.to_lowercase();

        if apps_lower.contains(&source_app) {
            let play_succeeded = session.TryPlayAsync().ok().and_then(|operation| block_on(operation).ok()).unwrap_or(false);

            if play_succeeded {
                info!(app = %app, "Resumed media session");
            }
        }
    }

    handle_playback_update(state, is_dev, extract_thumbnails, audio_manager)?;
    Ok(())
}

#[instrument(skip(_state, audio_manager))]
pub fn set_volume(
    app: String,
    volume: f64,
    _state: &SharedState,
    audio_manager: &super::audio_sessions::WindowsAudioSessionManager,
) -> Result<(), NativeError> {
    if !(0.0..=1.0).contains(&volume) {
        return Err(NativeError::InvalidVolume(volume));
    }

    let manager: windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager = block_on(
        windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(|e| NativeError::MediaSession(format!("Failed to create async operation: {}", e)))?,
    )
    .map_err(|e| NativeError::MediaSession(format!("Failed to request session manager: {}", e)))?;

    let sessions = manager
        .GetSessions()
        .map_err(|e| NativeError::MediaSession(format!("Failed to get sessions: {}", e)))?;

    let app_lower = app.to_lowercase();

    for i in 0..sessions.Size().unwrap_or(0) {
        let Ok(session) = sessions.GetAt(i) else {
            continue;
        };
        let Ok(app_id) = session.SourceAppUserModelId() else {
            continue;
        };
        let source_app = app_id.to_string().to_lowercase();

        if source_app == app_lower {
            audio_manager
                .set_session_volume(&session, volume)
                .map_err(|e| NativeError::MediaSession(format!("Failed to set volume: {}", e)))?;

            info!(app = %app, volume = volume, "Set volume for session");
            return Ok(());
        }
    }

    Err(NativeError::PlayerNotFound(app))
}

fn is_streamq(app: String, is_dev: bool) -> bool {
    if is_dev { app == "electron.exe" } else { app.contains("streamq") }
}

fn resolve_art(app: &str, title: &Option<String>, artist: &Option<String>, thumbnail: Option<IRandomAccessStreamReference>) -> Option<String> {
    let (url, path) = Thumbnails::path_for(app, title.as_deref(), artist.as_deref(), ".jpg")?;

    if path.is_file() {
        return Some(url);
    }

    save_thumbnail(thumbnail, &path)?;
    Some(url)
}

fn save_thumbnail(thumbnail: Option<IRandomAccessStreamReference>, path: &std::path::Path) -> Option<()> {
    let thumbnail = thumbnail?;
    let stream = thumbnail.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()? as usize;
    if size == 0 {
        return None;
    }

    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size as u32).ok()?.get().ok()?;

    let mut buffer = vec![0u8; size];
    reader.ReadBytes(&mut buffer).ok()?;
    let _ = reader.Close();
    let _ = stream.Close();

    let mut file = std::fs::File::create(path).ok()?;
    file.write_all(&buffer).ok()?;
    Some(())
}
