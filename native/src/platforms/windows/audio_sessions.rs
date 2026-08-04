use std::collections::HashMap;
use std::sync::Mutex;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, AudioSessionStateExpired, DEVICE_STATE_ACTIVE, IAudioSessionControl2, IAudioSessionEnumerator, IAudioSessionManager2,
    IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows::core::{ComInterface, PWSTR};

pub struct WindowsAudioSessionManager {
    pid_cache: Mutex<HashMap<u32, Vec<String>>>,
}

impl Default for WindowsAudioSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsAudioSessionManager {
    pub fn new() -> Self {
        Self {
            pid_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_session_volume(
        &self,
        session: &windows::Media::Control::GlobalSystemMediaTransportControlsSession,
        volume: f64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_app = session.SourceAppUserModelId()?.to_string();
        let clamped_volume = volume.clamp(0.0, 1.0) as f32;

        if let Some((app_volume_ctrl, _, _)) = self.find_audio_session_for_app(&source_app) {
            unsafe {
                app_volume_ctrl.SetMasterVolume(clamped_volume, std::ptr::null())?;
            }
            tracing::info!("Set mixer volume for {} to {}", source_app, volume);
            Ok(())
        } else {
            Err(format!("No audio session found for {}", source_app).into())
        }
    }

    pub fn get_all_volumes(&self) -> HashMap<String, (f64, String, f64)> {
        let mut best_matches: HashMap<String, (i32, f64, String, f64)> = HashMap::new();

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(_) => return HashMap::new(),
            };

            let default_device_id = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .and_then(|d| d.GetId())
                .map(|id| {
                    let s = id.to_string().unwrap_or_default();
                    windows::Win32::System::Com::CoTaskMemFree(Some(id.0 as _));
                    s
                })
                .unwrap_or_default();

            if let Ok(endpoints_collection) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) {
                if let Ok(endpoints_count) = endpoints_collection.GetCount() {
                    for d_idx in 0..endpoints_count {
                        let device = match endpoints_collection.Item(d_idx) {
                            Ok(d) => d,
                            Err(_) => continue,
                        };

                        let id_pwstr = match device.GetId() {
                            Ok(id) => id,
                            Err(_) => continue,
                        };
                        let device_id = id_pwstr.to_string().unwrap_or_default();
                        windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.0 as _));
                        if device_id.is_empty() {
                            continue;
                        }

                        let session_manager: IAudioSessionManager2 = match device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) {
                            Ok(m) => m,
                            Err(_) => continue,
                        };

                        let session_enumerator = match session_manager.GetSessionEnumerator() {
                            Ok(e) => e,
                            Err(_) => continue,
                        };

                        let session_count = session_enumerator.GetCount().unwrap_or(0);
                        if session_count == 0 {
                            continue;
                        }

                        let device_vol = if let Ok(endpoint_volume) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                            endpoint_volume.GetMasterVolumeLevelScalar().unwrap_or(1.0) as f64
                        } else {
                            1.0
                        };

                        let is_default = device_id == default_device_id;

                        for i in 0..session_count {
                            let control: IAudioSessionControl2 = match session_enumerator.GetSession(i).ok().and_then(|s| s.cast().ok()) {
                                Some(c) => c,
                                None => continue,
                            };

                            let state = control.GetState().unwrap_or(AudioSessionStateExpired);
                            if state == AudioSessionStateExpired {
                                continue;
                            }

                            let pid = match control.GetProcessId() {
                                Ok(pid) if pid != 0 => pid,
                                _ => continue,
                            };

                            let app_volume_ctrl: ISimpleAudioVolume = match control.cast() {
                                Ok(v) => v,
                                Err(_) => continue,
                            };
                            let app_vol = app_volume_ctrl.GetMasterVolume().unwrap_or(1.0) as f64;

                            let score = if state == AudioSessionStateActive {
                                if is_default { 4 } else { 3 }
                            } else {
                                if is_default { 2 } else { 1 }
                            };

                            let aliases = self.get_aliases_for_pid(pid);
                            for alias in aliases {
                                let current_score = best_matches.get(&alias).map(|v| v.0).unwrap_or(0);
                                if score >= current_score {
                                    best_matches.insert(alias, (score, app_vol, device_id.clone(), device_vol));
                                }
                            }
                        }
                    }
                }
            }
        }

        best_matches.into_iter().map(|(k, v)| (k, (v.1, v.2, v.3))).collect()
    }

    fn find_audio_session_for_app(&self, source_app: &str) -> Option<(ISimpleAudioVolume, IAudioEndpointVolume, String)> {
        let source_app_lower = source_app.to_lowercase();

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(_) => return None,
            };

            let default_device_id = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .and_then(|d| d.GetId())
                .map(|id| {
                    let s = id.to_string().unwrap_or_default();
                    windows::Win32::System::Com::CoTaskMemFree(Some(id.0 as _));
                    s
                })
                .unwrap_or_default();

            let endpoints_collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).ok()?;
            let endpoints_count = endpoints_collection.GetCount().ok()?;

            let mut best_match = None;
            let mut fallback_match = None;

            for d_idx in 0..endpoints_count {
                let device = match endpoints_collection.Item(d_idx) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                let id_pwstr = match device.GetId() {
                    Ok(id) => id,
                    Err(_) => continue,
                };

                let device_id = id_pwstr.to_string().unwrap_or_default();
                windows::Win32::System::Com::CoTaskMemFree(Some(id_pwstr.0 as _));

                if device_id.is_empty() {
                    continue;
                }

                let session_manager: IAudioSessionManager2 = match device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let session_enumerator: IAudioSessionEnumerator = match session_manager.GetSessionEnumerator() {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let session_count = session_enumerator.GetCount().unwrap_or(0);

                for i in 0..session_count {
                    let control: IAudioSessionControl2 = match session_enumerator.GetSession(i).ok().and_then(|s| s.cast().ok()) {
                        Some(c) => c,
                        None => continue,
                    };

                    let state = control.GetState().unwrap_or(AudioSessionStateExpired);
                    if state == AudioSessionStateExpired {
                        continue;
                    }

                    let pid = match control.GetProcessId() {
                        Ok(pid) if pid != 0 => pid,
                        _ => continue,
                    };

                    let aliases = self.get_aliases_for_pid(pid);
                    if aliases.contains(&source_app_lower) {
                        if let Ok(app_volume) = control.cast::<ISimpleAudioVolume>() {
                            if let Ok(endpoint_volume) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                                let is_default = device_id == default_device_id;

                                if state == AudioSessionStateActive {
                                    if is_default {
                                        return Some((app_volume, endpoint_volume, device_id));
                                    } else {
                                        best_match = Some((app_volume, endpoint_volume, device_id.clone()));
                                    }
                                } else if best_match.is_none() {
                                    if is_default || fallback_match.is_none() {
                                        fallback_match = Some((app_volume, endpoint_volume, device_id.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            best_match.or(fallback_match)
        }
    }

    fn get_aliases_for_pid(&self, pid: u32) -> Vec<String> {
        {
            let cache = self.pid_cache.lock().unwrap();
            if let Some(aliases) = cache.get(&pid) {
                return aliases.clone();
            }
        }

        let mut aliases = Vec::new();

        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut len = 0;
                let _ = GetApplicationUserModelId(handle, &mut len, PWSTR::null());
                if len > 0 {
                    let mut buf: Vec<u16> = vec![0; len as usize];
                    if GetApplicationUserModelId(handle, &mut len, PWSTR(buf.as_mut_ptr())).is_ok() {
                        let aumid = String::from_utf16_lossy(&buf[..((len - 1) as usize)]);
                        aliases.push(aumid.to_lowercase());
                    }
                }

                let mut path_buf: [u16; 260] = [0; 260];
                let mut path_len = 260u32;
                if QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), PWSTR(path_buf.as_mut_ptr()), &mut path_len).is_ok() {
                    let full_path = String::from_utf16_lossy(&path_buf[..path_len as usize]).to_lowercase();
                    let file_name = full_path.split('\\').last().unwrap_or(&full_path).to_string();
                    let file_name_no_ext = file_name.replace(".exe", "");

                    aliases.push(full_path);
                    aliases.push(file_name);
                    aliases.push(file_name_no_ext);
                }

                let _ = CloseHandle(handle);
            }
        }

        if !aliases.is_empty() {
            self.pid_cache.lock().unwrap().insert(pid, aliases.clone());
        }

        aliases
    }
}

pub fn get_process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut path_buf: [u16; 260] = [0; 260];
        let mut path_len = 260u32;

        if QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), PWSTR(path_buf.as_mut_ptr()), &mut path_len).is_ok() {
            let path = String::from_utf16_lossy(&path_buf[..path_len as usize]);
            let name = path.split('\\').last().unwrap_or(&path).to_lowercase();
            let _ = CloseHandle(handle);
            return Some(name);
        }

        let _ = CloseHandle(handle);
    }
    None
}
