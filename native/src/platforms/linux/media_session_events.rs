use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument, warn};
use zbus::Connection;

use crate::models::media::SharedState;

use super::media_session_mpris::{MPRIS_PREFIX, collect_media_sessions, list_mpris_players};

pub fn start_monitor(
    state: SharedState,
    refresh_rx: UnboundedReceiver<()>,
    audio_manager: Arc<super::audio_sessions::AudioSessionManager>,
    extract_thumbnails: bool,
) {
    std::thread::spawn(move || {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            rt.block_on(async move {
                if let Err(e) = run_monitor(&state, refresh_rx, audio_manager, extract_thumbnails).await {
                    error!(error = %e, "MPRIS2 monitor stopped");
                }
            });
        } else {
            error!("Failed to create tokio runtime for MPRIS monitor");
        }
    });
}

#[instrument(skip(state, refresh_rx, audio_manager))]
async fn run_monitor(
    state: &SharedState,
    mut refresh_rx: UnboundedReceiver<()>,
    audio_manager: Arc<super::audio_sessions::AudioSessionManager>,
    extract_thumbnails: bool,
) -> Result<()> {
    use futures::StreamExt;
    use zbus::fdo::DBusProxy;

    let conn = Connection::session().await.context("Failed to connect to D-Bus session bus")?;
    info!("Connected to D-Bus session bus for MPRIS2 monitoring");

    let dbus = DBusProxy::new(&conn).await?;
    let mut name_owner_changed = dbus.receive_name_owner_changed().await?;

    let mut active_watchers: HashMap<String, JoinHandle<()>> = HashMap::new();

    update_media_sessions(&conn, state, &audio_manager, extract_thumbnails).await;

    let names = list_mpris_players(&conn).await?;
    for name in &names {
        let handle = spawn_player_watcher(conn.clone(), name.clone(), state.clone(), audio_manager.clone(), extract_thumbnails);
        active_watchers.insert(name.clone(), handle);
    }

    loop {
        tokio::select! {
            recv_result = refresh_rx.recv() => {
                if recv_result.is_some() {
                    debug!("Manual refresh triggered via channel");
                    update_media_sessions(&conn, state, &audio_manager, extract_thumbnails).await;
                } else {
                    debug!("Refresh channel closed, stopping monitor");
                    break;
                }
            }

            signal_opt = name_owner_changed.next() => {
                match signal_opt {
                    Some(signal) => {
                        let args = match signal.args() {
                            Ok(a) => a,
                            Err(e) => {
                                warn!(error = %e, "Failed to parse NameOwnerChanged args");
                                continue;
                            }
                        };

                        let name_bus = args.name();
                        let name_str = name_bus.as_str();

                        if !name_str.starts_with(MPRIS_PREFIX) {
                            continue;
                        }

                        let new_owner = args.new_owner();
                        if new_owner.as_deref().unwrap_or("").is_empty() {
                            info!(player = %name_str, "MPRIS2 player removed");

                            if let Some(handle) = active_watchers.remove(name_str) {
                                handle.abort();
                            }

                            update_media_sessions(&conn, state, &audio_manager, extract_thumbnails).await;
                        } else {
                            info!(player = %name_str, "MPRIS2 player appeared");

                            if let Some(old_handle) = active_watchers.remove(name_str) {
                                old_handle.abort();
                            }

                            let handle = spawn_player_watcher(conn.clone(), name_str.to_string(), state.clone(), audio_manager.clone(), extract_thumbnails);
                            active_watchers.insert(name_str.to_string(), handle);

                            update_media_sessions(&conn, state, &audio_manager, extract_thumbnails).await;
                        }
                    }
                    None => {
                        warn!("NameOwnerChanged signal stream ended");
                        break;
                    }
                }
            }
        }
    }

    for (_, handle) in active_watchers {
        handle.abort();
    }

    Ok(())
}

fn spawn_player_watcher(
    conn: Connection,
    bus_name: String,
    state: SharedState,
    audio_manager: Arc<super::audio_sessions::AudioSessionManager>,
    extract_thumbnails: bool,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = watch_player(&conn, bus_name, &state, &audio_manager, extract_thumbnails).await {
            debug!(error = %e, "Player watcher ended");
        }
    })
}

#[instrument(skip(conn, state, audio_manager))]
async fn watch_player(
    conn: &Connection,
    bus_name: String,
    state: &SharedState,
    audio_manager: &super::audio_sessions::AudioSessionManager,
    extract_thumbnails: bool,
) -> Result<()> {
    use futures::StreamExt;
    use zbus::fdo::PropertiesProxy;

    let props = PropertiesProxy::builder(conn)
        .destination(bus_name.as_str())?
        .path("/org/mpris/MediaPlayer2")?
        .build()
        .await?;

    let mut changed = props.receive_properties_changed().await?;
    debug!("Watching PropertiesChanged");

    while let Some(signal) = changed.next().await {
        let args = match signal.args() {
            Ok(a) => a,
            Err(_) => continue,
        };

        if args.interface_name() != "org.mpris.MediaPlayer2.Player" {
            continue;
        }

        let changed_props = args.changed_properties();
        if changed_props.len() == 1 && changed_props.contains_key("Position") {
            continue;
        }

        update_media_sessions(conn, state, audio_manager, extract_thumbnails).await;
    }

    Ok(())
}

#[instrument(skip(conn, state, audio_manager))]
async fn update_media_sessions(conn: &Connection, state: &SharedState, audio_manager: &super::audio_sessions::AudioSessionManager, extract_thumbnails: bool) {
    let media_sessions = match collect_media_sessions(conn, state, audio_manager, extract_thumbnails).await {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "Failed to query MPRIS2 sessions");
            return;
        }
    };

    state.emit_if_changed(media_sessions);
}
