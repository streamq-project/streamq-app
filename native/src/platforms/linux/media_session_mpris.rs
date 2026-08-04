use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use tracing::{debug, info, instrument, warn};
use zbus::zvariant::OwnedValue;
use zbus::{Connection, proxy};

use crate::error::NativeError;
use crate::models::media::{AppMediaSession, SharedState, now_ms};
use crate::thumbnails::Thumbnails;

pub const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

#[proxy(interface = "org.mpris.MediaPlayer2.Player", default_path = "/org/mpris/MediaPlayer2")]
trait MprisPlayer {
    fn pause(&self) -> zbus::Result<()>;
    fn play(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedValue>>;

    #[zbus(property)]
    fn volume(&self) -> zbus::Result<f64>;

    #[zbus(property)]
    fn set_volume(&self, value: f64) -> zbus::Result<()>;

    #[zbus(property)]
    fn can_control(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn position(&self) -> zbus::Result<i64>;
}

#[instrument(skip(conn, state, audio_manager), fields(player_count))]
pub async fn collect_media_sessions(
    conn: &Connection,
    state: &SharedState,
    audio_manager: &super::audio_sessions::AudioSessionManager,
    extract_thumbnails: bool,
) -> Result<Vec<AppMediaSession>> {
    let suppressed: HashSet<String> = state.suppressed_apps.lock().unwrap().clone();
    let previous: HashMap<String, AppMediaSession> = state.sessions.lock().unwrap().iter().map(|s| (s.app.clone(), s.clone())).collect();
    debug!(suppressed_apps = ?suppressed, "Collecting media sessions");

    let players = list_mpris_players(conn).await?;
    tracing::Span::current().record("player_count", players.len());

    let futures = players.into_iter().map(|name| {
        let conn = conn.clone();
        let suppressed = suppressed.clone();

        async move {
            let app = name.strip_prefix(MPRIS_PREFIX).unwrap_or(&name).to_string();
            let app_lower = app.to_lowercase();

            let proxy = match make_player_proxy(&conn, name.clone()).await {
                Ok(p) => p,
                Err(e) => {
                    warn!(error = %e, "Failed to create player proxy");
                    return None;
                }
            };

            let (playing, (title, artist, url, raw_art, duration), volume, can_control, position, pid_opt) = tokio::join!(
                is_playing(&proxy),
                metadata_strings(&proxy),
                get_volume(&proxy),
                can_control(&proxy),
                get_position(&proxy),
                get_pid(&conn, &name)
            );

            let pid = pid_opt.unwrap_or(0);
            let is_suppressed = suppressed.contains(&app_lower);

            let sink_info = audio_manager.get_sink_info(pid, &app);
            let device = sink_info.as_ref().map(|(d, _): &(String, f64)| d.clone());
            let device_volume = sink_info.map(|(_, v)| v);

            let art = if extract_thumbnails {
                resolve_art(&app, &title, &artist, raw_art.as_deref())
            } else {
                raw_art
            };

            debug!(
                app = app,
                is_playing = playing,
                is_suppressed = is_suppressed,
                can_control = can_control,
                device = ?device,
                device_volume = ?device_volume,
                "Processing player"
            );

            let now = now_ms();
            Some(AppMediaSession {
                app,
                title,
                artist,
                is_resumed: playing,
                is_suppressed,
                volume,
                device,
                device_volume,
                can_control: Some(can_control),
                updated_at: now,
                activated_at: now,
                url,
                art,
                position,
                duration,
            })
        }
    });

    let results = futures_util::future::join_all(futures).await;

    let mut sessions = Vec::new();
    let mut apps_playing: HashSet<String> = HashSet::new();

    for session in results.into_iter().flatten() {
        if session.is_resumed {
            apps_playing.insert(session.app.to_lowercase());
        }
        sessions.push(session);
    }

    {
        let mut suppressed_guard = state.suppressed_apps.lock().unwrap();
        suppressed_guard.retain(|app| !apps_playing.contains(app));
    }

    let previous_list: Vec<AppMediaSession> = previous.into_values().collect();
    Thumbnails::release_unreferenced(&previous_list, &sessions);

    Ok(sessions)
}

#[instrument(skip(state, audio_manager))]
pub async fn pause_async(
    apps: Vec<String>,
    state: &SharedState,
    is_dev: bool,
    extract_thumbnails: bool,
    audio_manager: &super::audio_sessions::AudioSessionManager,
) -> Result<Vec<AppMediaSession>> {
    let conn = Connection::session().await.context("Failed to connect to D-Bus session bus")?;

    let apps_lower: Vec<String> = apps.iter().map(|a| a.to_lowercase()).collect();
    let mut paused_sessions = Vec::new();
    let mut apps_to_suppress: Vec<String> = Vec::new();

    let players = list_mpris_players(&conn).await?;

    for name in players {
        let app = name.strip_prefix(MPRIS_PREFIX).unwrap_or(&name).to_string();

        if !apps_lower.contains(&app.to_lowercase()) {
            continue;
        }

        if is_streamq(app.clone(), is_dev) {
            continue;
        }

        let proxy = match make_player_proxy(&conn, name.clone()).await {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "Failed to create player proxy");
                continue;
            }
        };

        if is_playing(&proxy).await {
            match proxy.pause().await {
                Ok(_) => {
                    info!("Paused player");

                    let ((title, artist, url, raw_art, duration), volume, can_control, position, pid_opt) = tokio::join!(
                        metadata_strings(&proxy),
                        get_volume(&proxy),
                        can_control(&proxy),
                        get_position(&proxy),
                        get_pid(&conn, &name)
                    );

                    let pid = pid_opt.unwrap_or(0);
                    let sink_info = audio_manager.get_sink_info(pid, &app);
                    let device = sink_info.as_ref().map(|(d, _): &(String, f64)| d.clone());
                    let device_volume = sink_info.map(|(_, v)| v);

                    let art = if extract_thumbnails {
                        resolve_art(&app, &title, &artist, raw_art.as_deref())
                    } else {
                        raw_art
                    };

                    debug!(
                        app = app,
                        can_control = can_control,
                        device = ?device,
                        device_volume = ?device_volume,
                        "Processing player"
                    );

                    apps_to_suppress.push(app.to_lowercase());
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
                        can_control: Some(can_control),
                        updated_at: now,
                        activated_at: now,
                        url,
                        art,
                        position,
                        duration,
                    });
                }
                Err(e) => warn!(error = %e, "Failed to pause player"),
            }
        }
    }

    {
        let mut suppressed = state.suppressed_apps.lock().unwrap();
        for app in &apps_to_suppress {
            info!(app = %app, "Adding to suppressed list");
            suppressed.insert(app.clone());
        }
        debug!(suppressed_apps = ?suppressed, "Updated suppressed apps list");
    }

    Ok(paused_sessions)
}

#[instrument(skip(state), fields(apps_count = apps.len()))]
pub async fn resume_async(apps: Vec<String>, state: &SharedState) -> Result<()> {
    let conn = Connection::session().await.context("Failed to connect to D-Bus session bus")?;
    let apps_lower: Vec<String> = apps.iter().map(|a| a.to_lowercase()).collect();

    {
        let mut suppressed = state.suppressed_apps.lock().unwrap();
        suppressed.retain(|app| !apps_lower.contains(&app.to_lowercase()));
    }

    for name in list_mpris_players(&conn).await? {
        let app = name.strip_prefix(MPRIS_PREFIX).unwrap_or(&name).to_string();
        if apps_lower.contains(&app.to_lowercase()) {
            if let Ok(proxy) = make_player_proxy(&conn, name).await {
                let _ = proxy.play().await;
            }
        }
    }

    Ok(())
}

#[instrument]
pub async fn set_volume_async(app: String, volume: f64) -> Result<(), NativeError> {
    let conn = Connection::session().await?;
    let bus_name = format!("{}{}", MPRIS_PREFIX, app);
    let proxy = make_player_proxy(&conn, bus_name).await?;

    if !can_control(&proxy).await {
        return Err(NativeError::VolumeNotControllable);
    }

    let clamped_volume = volume.clamp(0.0, 1.0);
    proxy.set_volume(clamped_volume).await?;

    info!(volume = clamped_volume, "Set volume for player");
    Ok(())
}

pub async fn list_mpris_players(conn: &Connection) -> zbus::Result<Vec<String>> {
    use zbus::fdo::DBusProxy;
    let dbus = DBusProxy::new(conn).await?;
    let names = dbus.list_names().await?;
    Ok(names
        .into_iter()
        .filter(|n| n.starts_with(MPRIS_PREFIX))
        .filter(|n| !n.ends_with(".playerctld"))
        .filter(|n| !n.ends_with(".streamq"))
        .map(|n| n.to_string())
        .collect())
}

async fn make_player_proxy(conn: &Connection, bus_name: String) -> zbus::Result<MprisPlayerProxy<'_>> {
    use zbus::names::OwnedBusName;
    let dest: OwnedBusName = bus_name.try_into().map_err(|e: zbus::names::Error| zbus::Error::Names(e))?;
    MprisPlayerProxy::builder(conn).destination(dest)?.build().await
}

async fn is_playing(proxy: &MprisPlayerProxy<'_>) -> bool {
    matches!(proxy.playback_status().await.as_deref(), Ok("Playing"))
}

async fn metadata_strings(proxy: &MprisPlayerProxy<'_>) -> (Option<String>, Option<String>, Option<String>, Option<String>, Option<f64>) {
    let meta = match proxy.metadata().await {
        Ok(m) => m,
        Err(_) => return (None, None, None, None, None),
    };

    let title = extract_str(&meta, "xesam:title");
    let artist = extract_str(&meta, "xesam:artist");
    let url = extract_str(&meta, "xesam:url");
    let art = extract_str(&meta, "mpris:artUrl");
    let duration = extract_i64(&meta, "mpris:length").map(|v| v as f64 / 1000.0);
    (title, artist, url, art, duration)
}

fn extract_str(meta: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    use std::ops::Deref;
    use zbus::zvariant::Value;
    match meta.get(key)?.deref() {
        Value::Str(s) => Some(s.to_string()),
        Value::Array(arr) => arr.first().and_then(|v| if let Value::Str(s) = v { Some(s.to_string()) } else { None }),
        _ => None,
    }
}

fn extract_i64(meta: &HashMap<String, OwnedValue>, key: &str) -> Option<i64> {
    use std::ops::Deref;
    use zbus::zvariant::Value;
    match meta.get(key)?.deref() {
        Value::I64(v) => Some(*v),
        Value::U64(v) => Some(*v as i64),
        Value::I32(v) => Some(*v as i64),
        Value::U32(v) => Some(*v as i64),
        _ => None,
    }
}

async fn get_volume(proxy: &MprisPlayerProxy<'_>) -> Option<f64> {
    proxy.volume().await.ok().map(|v| v.clamp(0.0, 1.0))
}

async fn get_position(proxy: &MprisPlayerProxy<'_>) -> Option<f64> {
    proxy.position().await.ok().map(|v| v as f64 / 1000.0)
}

async fn can_control(proxy: &MprisPlayerProxy<'_>) -> bool {
    let can_control = proxy.can_control().await.unwrap_or(false);
    let volume_readable = proxy.volume().await.is_ok();
    can_control && volume_readable
}

fn is_streamq(app: String, is_dev: bool) -> bool {
    if is_dev { app == "electron" } else { app.contains("streamq") }
}

fn resolve_art(app: &str, title: &Option<String>, artist: &Option<String>, art_url: Option<&str>) -> Option<String> {
    let art_url = art_url.filter(|s| !s.is_empty())?;

    if art_url.starts_with("streamq-local://") {
        return Some(art_url.to_string());
    }

    let Some(source_path) = local_file_path(art_url) else {
        return Some(art_url.to_string());
    };

    if !source_path.is_file() {
        return None;
    }

    let ext = source_path.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    let (url, dest) = Thumbnails::path_for(app, title.as_deref(), artist.as_deref(), &ext)?;

    if dest.is_file() {
        return Some(url);
    }

    std::fs::copy(&source_path, &dest).ok()?;
    Some(url)
}

fn local_file_path(art_url: &str) -> Option<std::path::PathBuf> {
    if let Ok(url) = url::Url::parse(art_url) {
        if url.scheme() == "file" {
            return url.to_file_path().ok();
        }
        return None;
    }

    let path = std::path::Path::new(art_url);
    if path.is_absolute() && !art_url.contains("://") {
        return Some(path.to_path_buf());
    }

    None
}

async fn get_pid(conn: &Connection, bus_name: &str) -> Option<u32> {
    use zbus::fdo::DBusProxy;
    let dbus = DBusProxy::new(conn).await.ok()?;
    let name: zbus::names::BusName = bus_name.try_into().ok()?;
    dbus.get_connection_unix_process_id(name).await.ok()
}
