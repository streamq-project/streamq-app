use super::audio_pulse::{create_connected_context, disconnect, wait_op};
use libpulse_binding::{callbacks::ListResult, volume::Volume};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct AppStreamInfo {
    pub device: String,
    pub volume: f64,
}

#[derive(Debug, Clone, Default)]
pub struct AudioStreamCache {
    pub by_pid: HashMap<u32, AppStreamInfo>,
    pub by_name: HashMap<String, AppStreamInfo>,
}

pub struct AudioSessionManager {
    cache: Mutex<AudioStreamCache>,
    alias_cache: Mutex<HashMap<u32, Vec<String>>>,
}

impl Default for AudioSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSessionManager {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(AudioStreamCache::default()),
            alias_cache: Mutex::new(HashMap::new()),
        }
    }

    fn get_proc_aliases(&self, target_pid: u32) -> Vec<String> {
        if target_pid == 0 {
            return Vec::new();
        }

        {
            let cache_guard = self.alias_cache.lock().unwrap();
            if let Some(aliases) = cache_guard.get(&target_pid) {
                return aliases.clone();
            }
        }

        let mut proc_names = Vec::new();

        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{}/stat", target_pid)) {
            if let Some(start) = stat.find('(') {
                if let Some(end) = stat.rfind(')') {
                    let comm = stat[start + 1..end].to_lowercase();
                    let comm = comm.strip_suffix("-bin").unwrap_or(&comm).to_string();
                    if !comm.is_empty() {
                        proc_names.push(comm);
                    }
                }
            }
        }

        if let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{}/cmdline", target_pid)) {
            if let Some(first) = cmdline.split('\0').next() {
                if let Some(file_name) = std::path::Path::new(first).file_name() {
                    let name = file_name.to_string_lossy().to_lowercase();
                    let name = name.strip_suffix("-bin").unwrap_or(&name).to_string();
                    if !name.is_empty() && !proc_names.contains(&name) {
                        proc_names.push(name);
                    }
                }
            }
        }

        if let Ok(flatpak_info) = std::fs::read_to_string(format!("/proc/{}/root/.flatpak-info", target_pid)) {
            for line in flatpak_info.lines() {
                if line.starts_with("name=") {
                    let app_id = line.trim_start_matches("name=").to_lowercase();
                    if !app_id.is_empty() && !proc_names.contains(&app_id) {
                        proc_names.push(app_id);
                    }
                    break;
                }
            }
        }

        self.alias_cache.lock().unwrap().insert(target_pid, proc_names.clone());
        proc_names
    }

    pub fn update_cache(&self) {
        let start = std::time::Instant::now();
        tracing::debug!("Starting audio stream cache update");

        let (mut ml, mut ctx) = match create_connected_context() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("update_cache: Failed to connect to PulseAudio: {:?}", e);
                return;
            }
        };

        let sinks = Arc::new(Mutex::new(HashMap::<u32, (String, f64)>::new()));
        let sinks_clone = Arc::clone(&sinks);
        let op_sinks = ctx.introspect().get_sink_info_list(move |res| {
            if let ListResult::Item(info) = res {
                if let Some(name) = info.name.as_ref() {
                    let linear = info.volume.avg().0 as f64 / Volume::NORMAL.0 as f64;
                    sinks_clone.lock().unwrap().insert(info.index, (name.to_string(), linear));
                }
            }
        });
        wait_op(&mut ml, &op_sinks);
        let sinks_map = sinks.lock().unwrap().clone();

        #[derive(Clone, Debug)]
        struct ClientData {
            pid: Option<String>,
            binary: Option<String>,
            name: Option<String>,
            app_id: Option<String>,
        }
        let clients = Arc::new(Mutex::new(HashMap::<u32, ClientData>::new()));
        let clients_clone = Arc::clone(&clients);
        let op_clients = ctx.introspect().get_client_info_list(move |res| {
            if let ListResult::Item(info) = res {
                let pid = info.proplist.get_str("application.process.id").map(|s| s.to_string());
                let binary = info.proplist.get_str("application.process.binary").map(|s| s.to_string());
                let name = info.proplist.get_str("application.name").map(|s| s.to_string());
                let app_id = info.proplist.get_str("pipewire.access.portal.app_id").map(|s| s.to_string());
                clients_clone.lock().unwrap().insert(info.index, ClientData { pid, binary, name, app_id });
            }
        });
        wait_op(&mut ml, &op_clients);
        let clients_map = clients.lock().unwrap().clone();

        struct SinkInputData {
            sink: u32,
            client: Option<u32>,
            pid: Option<String>,
            binary: Option<String>,
            name: Option<String>,
            app_id: Option<String>,
        }
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let inputs_clone = Arc::clone(&inputs);
        let op_inputs = ctx.introspect().get_sink_input_info_list(move |res| {
            if let ListResult::Item(info) = res {
                let pid = info.proplist.get_str("application.process.id").map(|s| s.to_string());
                let binary = info.proplist.get_str("application.process.binary").map(|s| s.to_string());
                let name = info.proplist.get_str("application.name").map(|s| s.to_string());
                let app_id = info.proplist.get_str("pipewire.access.portal.app_id").map(|s| s.to_string());
                inputs_clone.lock().unwrap().push(SinkInputData {
                    sink: info.sink,
                    client: info.client,
                    pid,
                    binary,
                    name,
                    app_id,
                });
            }
        });
        wait_op(&mut ml, &op_inputs);
        let inputs_list = inputs.lock().unwrap();

        disconnect(&mut ctx);

        tracing::debug!(
            sinks_count = sinks_map.len(),
            clients_count = clients_map.len(),
            inputs_count = inputs_list.len(),
            "Collected PulseAudio data"
        );

        let mut new_cache = AudioStreamCache::default();

        for input in inputs_list.iter() {
            let sink_idx = input.sink;
            if let Some((dev_name, dev_vol)) = sinks_map.get(&sink_idx) {
                let info = AppStreamInfo {
                    device: dev_name.clone(),
                    volume: *dev_vol,
                };

                let mut pid_resolved = None;
                if let Some(pid_str) = &input.pid {
                    if let Ok(p) = pid_str.parse::<u32>() {
                        pid_resolved = Some(p);
                    }
                } else if let Some(client_id) = input.client {
                    if let Some(client_data) = clients_map.get(&client_id) {
                        if let Some(pid_str) = &client_data.pid {
                            if let Ok(p) = pid_str.parse::<u32>() {
                                pid_resolved = Some(p);
                            }
                        }
                    }
                }

                if let Some(p) = pid_resolved {
                    new_cache.by_pid.insert(p, info.clone());
                }

                let mut names_to_add = Vec::new();
                if let Some(n) = &input.name {
                    names_to_add.push(n.to_lowercase());
                }
                if let Some(b) = &input.binary {
                    names_to_add.push(b.to_lowercase());
                }
                if let Some(a) = &input.app_id {
                    names_to_add.push(a.to_lowercase());
                }

                if let Some(client_id) = input.client {
                    if let Some(client_data) = clients_map.get(&client_id) {
                        if let Some(n) = &client_data.name {
                            names_to_add.push(n.to_lowercase());
                        }
                        if let Some(b) = &client_data.binary {
                            names_to_add.push(b.to_lowercase());
                        }
                        if let Some(a) = &client_data.app_id {
                            names_to_add.push(a.to_lowercase());
                        }
                    }
                }

                for name in names_to_add {
                    new_cache.by_name.insert(name, info.clone());
                }
            }
        }

        let pids_cached = new_cache.by_pid.len();
        let names_cached = new_cache.by_name.len();

        *self.cache.lock().unwrap() = new_cache;

        tracing::debug!(
            elapsed_ms = start.elapsed().as_millis(),
            pids_cached = pids_cached,
            names_cached = names_cached,
            "Audio stream cache updated successfully"
        );
    }

    pub fn get_sink_info(&self, target_pid: u32, app_name: &str) -> Option<(String, f64)> {
        let cache = self.cache.lock().unwrap();

        if target_pid > 0 {
            if let Some(info) = cache.by_pid.get(&target_pid) {
                return Some((info.device.clone(), info.volume));
            }
        }

        let app_lower = app_name.to_lowercase();
        let base_app = app_lower.split('.').next().unwrap_or(&app_lower);

        for (name, info) in &cache.by_name {
            if name.contains(base_app) || base_app.contains(name) {
                return Some((info.device.clone(), info.volume));
            }
        }

        if target_pid > 0 {
            let proc_names = self.get_proc_aliases(target_pid);
            for pn in proc_names {
                for (name, info) in &cache.by_name {
                    if name.contains(&pn) || pn.contains(name) {
                        return Some((info.device.clone(), info.volume));
                    }
                }
            }
        }

        tracing::warn!(
            target_pid = target_pid,
            app_name = app_name,
            cache_pids = ?cache.by_pid.keys().collect::<Vec<_>>(),
            cache_names = ?cache.by_name.keys().collect::<Vec<_>>(),
            "Failed to find sink info in cache"
        );

        None
    }
}
