use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;

use anyhow::{Context, Result};
use futures::StreamExt;
use tracing::{debug, error, info, instrument, warn};

use crate::config::Config;
use crate::config::Keybind;

const APP_ID: &str = "io.streamq.StreamQ";

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShortcutDefinition {
    action: String,
    label: String,
}

enum Command {
    Update(Vec<ShortcutDefinition>),
    Shutdown,
}

pub struct KeybindsManager {
    tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<Command>>>,
}

impl KeybindsManager {
    pub fn new(config: Config, event_emitter: std::sync::Arc<crate::event_emitter::EventEmitter>) -> Self {
        let manager = Self { tx: Mutex::new(None) };
        manager.initialize(config.keybinds, event_emitter);
        manager
    }

    fn should_use_portal() -> bool {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
        desktop.contains("hyprland") || desktop.contains("sway") || desktop.contains("kde") || desktop.contains("plasma")
    }

    #[instrument(skip(self, initial_binds, event_emitter))]
    fn initialize(&self, initial_binds: Vec<Keybind>, event_emitter: std::sync::Arc<crate::event_emitter::EventEmitter>) {
        if !Self::should_use_portal() {
            info!(
                desktop = %std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
                "Skipping GlobalShortcuts portal for current compositor"
            );
            return;
        }

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        *self.tx.lock().unwrap() = Some(tx);

        let initial_actions = Self::extract_actions(&initial_binds);

        thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    error!(error = %e, "Failed to build tokio runtime");
                    return;
                }
            };

            rt.block_on(async move {
                if let Err(e) = run_portal(rx, initial_actions, event_emitter).await {
                    error!(error = %format!("{e:#}"), "GlobalShortcuts portal error");
                }
            });
        });
    }

    fn extract_actions(keybinds: &[Keybind]) -> Vec<ShortcutDefinition> {
        let mut actions = Vec::new();
        for kb in keybinds {
            if let Some(action) = &kb.action {
                if action.is_empty() || actions.iter().any(|shortcut: &ShortcutDefinition| shortcut.action == *action) {
                    continue;
                }

                let label = kb.label.as_deref().filter(|label| !label.trim().is_empty()).unwrap_or(action);
                actions.push(ShortcutDefinition {
                    action: action.clone(),
                    label: label.to_string(),
                });
            }
        }
        actions
    }

    pub fn set_keybinds(&self, keybinds: Vec<Keybind>) {
        if !Self::should_use_portal() {
            return;
        }

        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let actions = Self::extract_actions(&keybinds);
            let count = actions.len();

            match tx.send(Command::Update(actions)) {
                Ok(()) => debug!(count, "Sent update command to GlobalShortcuts portal"),
                Err(e) => error!(error = %e, count, "Failed to send update command to GlobalShortcuts portal"),
            }
        }
    }

    #[instrument(skip(self))]
    pub fn cleanup(&self) {
        if let Some(tx) = self.tx.lock().unwrap().take() {
            info!("Sending shutdown command to GlobalShortcuts portal");
            let _ = tx.send(Command::Shutdown);
        }
    }
}

fn sanitize_dbus_id(id: &str) -> String {
    let mut safe = String::new();
    for (i, c) in id.chars().enumerate() {
        if c.is_ascii_alphabetic() || c == '_' || (i > 0 && c.is_ascii_digit()) {
            safe.push(c);
        } else if i == 0 && c.is_ascii_digit() {
            safe.push('_');
            safe.push(c);
        } else {
            safe.push('_');
        }
    }
    safe
}

#[instrument(name = "portal_worker", skip(rx, initial_actions, event_emitter))]
async fn run_portal(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Command>,
    initial_actions: Vec<ShortcutDefinition>,
    event_emitter: std::sync::Arc<crate::event_emitter::EventEmitter>,
) -> Result<()> {
    use ashpd::desktop::global_shortcuts::GlobalShortcuts;

    let connection = zbus::Connection::session().await.context("Failed to connect to D-Bus session bus")?;
    let app_id = ashpd::AppID::try_from(APP_ID).context("Invalid portal application ID")?;

    match ashpd::register_host_app_with_connection(connection.clone(), app_id).await {
        Ok(()) => info!(app_id = APP_ID, "Registered application ID with xdg-desktop-portal"),
        Err(ashpd::Error::PortalNotFound(_)) => {
            info!("Host portal Registry is unavailable; using legacy application ID detection");
        }
        Err(e) => return Err(e).context("Failed to register application ID with portal"),
    }

    let proxy = GlobalShortcuts::with_connection(connection)
        .await
        .context("Failed to create GlobalShortcuts proxy")?;
    info!("Connected to xdg-desktop-portal GlobalShortcuts");

    let mut activated_stream = proxy.receive_activated().await?;

    let mut session = proxy.create_session(ashpd::desktop::CreateSessionOptions::default()).await?;
    info!("GlobalShortcuts session created");

    let mut known_map: HashMap<String, String> = HashMap::new();
    let mut active_actions = initial_actions.clone();
    let mut session_bound = false;

    if !initial_actions.is_empty() {
        session_bound = true;
        known_map = bind_shortcuts(&proxy, &session, &initial_actions).await.unwrap_or_default();
    }

    loop {
        tokio::select! {
            cmd = rx.recv() => {
                match cmd {
                    Some(Command::Update(new_actions)) => {
                        if new_actions == active_actions {
                            debug!(count = new_actions.len(), "GlobalShortcuts actions are unchanged; skipping update");
                            continue;
                        }

                        info!(count = new_actions.len(), "Updating GlobalShortcuts session with new actions");

                        if session_bound {
                            if let Err(e) = session.close().await {
                                warn!(error = %e, "Failed to close previous GlobalShortcuts session");
                            }

                            session = proxy.create_session(ashpd::desktop::CreateSessionOptions::default()).await?;
                            info!("GlobalShortcuts session recreated");
                            session_bound = false;
                            known_map.clear();
                        }

                        active_actions = new_actions;

                        if !active_actions.is_empty() {
                            session_bound = true;
                            if let Ok(new_map) = bind_shortcuts(&proxy, &session, &active_actions).await {
                                known_map = new_map;
                            }
                        }
                    }
                    Some(Command::Shutdown) | None => {
                        info!("Shutting down GlobalShortcuts portal");
                        let _ = session.close().await;
                        break;
                    }
                }
            }

            event = activated_stream.next() => {
                match event {
                    Some(activated) => {
                        let safe_id = activated.shortcut_id();

                        if let Some(original_id) = known_map.get(safe_id) {
                            if active_actions.iter().any(|shortcut| shortcut.action == *original_id) {
                                info!(shortcut_id = %original_id, "Shortcut activated");
                                event_emitter.emit(crate::event_emitter::Event::KeybindPressed(original_id.clone()));
                            } else {
                                debug!(shortcut_id = %original_id, "Ignored deactivated shortcut");
                            }
                        } else {
                            warn!("Unknown shortcut activated: {}", safe_id);
                        }
                    }
                    None => {
                        warn!("GlobalShortcuts activated stream ended");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

#[instrument(skip(proxy, session, actions))]
async fn bind_shortcuts(
    proxy: &ashpd::desktop::global_shortcuts::GlobalShortcuts,
    session: &ashpd::desktop::Session<ashpd::desktop::global_shortcuts::GlobalShortcuts>,
    actions: &[ShortcutDefinition],
) -> Result<HashMap<String, String>> {
    use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, NewShortcut};

    let mut id_map = HashMap::new();
    let mut shortcuts = Vec::new();

    for shortcut in actions {
        let safe_id = sanitize_dbus_id(&shortcut.action);
        id_map.insert(safe_id.clone(), shortcut.action.clone());
        shortcuts.push(NewShortcut::new(&safe_id, &shortcut.label));
    }

    let bind_result = proxy.bind_shortcuts(session, &shortcuts, None, BindShortcutsOptions::default()).await;

    match bind_result {
        Ok(response) => {
            let resp = response.response().context("Failed to parse bind_shortcuts response")?;
            let bound = resp.shortcuts();
            info!(count = bound.len(), "Successfully registered shortcuts");
            for s in bound {
                debug!(id = %s.id(), trigger = %s.trigger_description(), "Registered shortcut");
            }
            Ok(id_map)
        }
        Err(e) => {
            error!(error = %e, "Failed to bind shortcuts");
            Err(e).context("bind_shortcuts failed")
        }
    }
}
