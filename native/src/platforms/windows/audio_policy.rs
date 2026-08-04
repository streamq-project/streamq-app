use std::collections::HashSet;
use std::ffi::c_void;

use windows::Win32::Media::Audio::{
    EDataFlow, ERole, IAudioSessionControl2, IAudioSessionEnumerator, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator, eCommunications,
    eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoGetActivationFactory, RoInitialize};
use windows::core::{ComInterface, GUID, HRESULT, HSTRING, Interface};

use super::audio_endpoints::{endpoint_exists, get_audio_sources};
use super::audio_sessions::get_process_name;
use crate::error::NativeError;
use crate::models::media::AudioSource;

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq)]
struct IAudioPolicyConfigFactory21H2(windows::core::IInspectable);

unsafe impl Interface for IAudioPolicyConfigFactory21H2 {
    type Vtable = IAudioPolicyConfigFactoryVtbl;
}

unsafe impl ComInterface for IAudioPolicyConfigFactory21H2 {
    const IID: GUID = GUID::from_u128(0xab3d4648_e242_459f_b02f_541c70306324);
}

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq)]
struct IAudioPolicyConfigFactoryDownlevel(windows::core::IInspectable);

unsafe impl Interface for IAudioPolicyConfigFactoryDownlevel {
    type Vtable = IAudioPolicyConfigFactoryVtbl;
}

unsafe impl ComInterface for IAudioPolicyConfigFactoryDownlevel {
    const IID: GUID = GUID::from_u128(0x2a59116d_6c4f_45e0_a74f_707e3fef9258);
}

#[repr(C)]
#[allow(non_snake_case)]
struct IAudioPolicyConfigFactoryVtbl {
    pub base__: windows::core::IInspectable_Vtbl,
    pub __incomplete__add_CtxVolumeChange: usize,
    pub __incomplete__remove_CtxVolumeChanged: usize,
    pub __incomplete__add_RingerVibrateStateChanged: usize,
    pub __incomplete__remove_RingerVibrateStateChange: usize,
    pub __incomplete__SetVolumeGroupGainForId: usize,
    pub __incomplete__GetVolumeGroupGainForId: usize,
    pub __incomplete__GetActiveVolumeGroupForEndpointId: usize,
    pub __incomplete__GetVolumeGroupsForEndpoint: usize,
    pub __incomplete__GetCurrentVolumeContext: usize,
    pub __incomplete__SetVolumeGroupMuteForId: usize,
    pub __incomplete__GetVolumeGroupMuteForId: usize,
    pub __incomplete__SetRingerVibrateState: usize,
    pub __incomplete__GetRingerVibrateState: usize,
    pub __incomplete__SetPreferredChatApplication: usize,
    pub __incomplete__ResetPreferredChatApplication: usize,
    pub __incomplete__GetPreferredChatApplication: usize,
    pub __incomplete__GetCurrentChatApplications: usize,
    pub __incomplete__add_ChatContextChanged: usize,
    pub __incomplete__remove_ChatContextChanged: usize,
    pub SetPersistedDefaultAudioEndpoint:
        unsafe extern "system" fn(this: *mut c_void, processid: u32, flow: EDataFlow, role: ERole, deviceid: std::mem::MaybeUninit<HSTRING>) -> HRESULT,
    pub GetPersistedDefaultAudioEndpoint:
        unsafe extern "system" fn(this: *mut c_void, processid: u32, flow: EDataFlow, role: ERole, deviceid: *mut std::mem::MaybeUninit<HSTRING>) -> HRESULT,
    pub ClearAllPersistedApplicationDefaultEndpoints: usize,
}

const MMDEVAPI_TOKEN: &str = r"\\?\SWD#MMDEVAPI#";
const DEVINTERFACE_AUDIO_RENDER: &str = "#{e6327cad-dcec-4949-ae8a-991e976a79d2}";
const DEVINTERFACE_AUDIO_CAPTURE: &str = "#{2eef81be-33fa-4800-9670-1cd474972c3f}";

fn to_policy_device_id(endpoint_id: &str) -> String {
    format!("{MMDEVAPI_TOKEN}{endpoint_id}{DEVINTERFACE_AUDIO_RENDER}")
}

fn unpack_policy_device_id(device_id: &str) -> String {
    let mut out = device_id.to_string();

    if out.starts_with(MMDEVAPI_TOKEN) {
        out = out[MMDEVAPI_TOKEN.len()..].to_string();
    }
    if out.ends_with(DEVINTERFACE_AUDIO_RENDER) {
        out = out[..out.len() - DEVINTERFACE_AUDIO_RENDER.len()].to_string();
    }
    if out.ends_with(DEVINTERFACE_AUDIO_CAPTURE) {
        out = out[..out.len() - DEVINTERFACE_AUDIO_CAPTURE.len()].to_string();
    }

    out
}

unsafe fn ro_get_activation_factory<T: ComInterface>(class_name: &HSTRING) -> windows::core::Result<T> {
    unsafe { RoGetActivationFactory::<T>(class_name) }
}

unsafe fn set_persisted_endpoint_for_all_roles<T: Interface<Vtable = IAudioPolicyConfigFactoryVtbl>>(
    factory: &T,
    process_id: u32,
    device_id: &HSTRING,
) -> windows::core::Result<()> {
    unsafe {
        (factory.vtable().SetPersistedDefaultAudioEndpoint)(factory.as_raw(), process_id, eRender, eConsole, std::mem::MaybeUninit::new(device_id.clone()))
            .ok()?;
        (factory.vtable().SetPersistedDefaultAudioEndpoint)(
            factory.as_raw(),
            process_id,
            eRender,
            eMultimedia,
            std::mem::MaybeUninit::new(device_id.clone()),
        )
        .ok()?;
        (factory.vtable().SetPersistedDefaultAudioEndpoint)(
            factory.as_raw(),
            process_id,
            eRender,
            eCommunications,
            std::mem::MaybeUninit::new(device_id.clone()),
        )
        .ok()?;
    }
    Ok(())
}

unsafe fn get_persisted_endpoint_for_role<T: Interface<Vtable = IAudioPolicyConfigFactoryVtbl>>(
    factory: &T,
    process_id: u32,
    role: ERole,
) -> windows::core::Result<Option<String>> {
    let mut raw_device_id = std::mem::MaybeUninit::<HSTRING>::zeroed();
    let result = unsafe { (factory.vtable().GetPersistedDefaultAudioEndpoint)(factory.as_raw(), process_id, eRender, role, &mut raw_device_id as *mut _) };

    if result.is_err() {
        return Ok(None);
    }

    let value = unsafe { raw_device_id.assume_init() }.to_string();
    if value.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(unpack_policy_device_id(&value)))
    }
}

unsafe fn get_persisted_endpoint_for_process<T: Interface<Vtable = IAudioPolicyConfigFactoryVtbl>>(
    factory: &T,
    process_id: u32,
) -> windows::core::Result<Option<String>> {
    if let Some(v) = unsafe { get_persisted_endpoint_for_role(factory, process_id, eMultimedia) }? {
        return Ok(Some(v));
    }
    if let Some(v) = unsafe { get_persisted_endpoint_for_role(factory, process_id, eConsole) }? {
        return Ok(Some(v));
    }
    if let Some(v) = unsafe { get_persisted_endpoint_for_role(factory, process_id, eCommunications) }? {
        return Ok(Some(v));
    }

    Ok(None)
}

fn collect_related_app_audio_pids() -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let current_pid = std::process::id();
        let current_process_name = get_process_name(current_pid);

        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let session_manager: IAudioSessionManager2 = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)?;
        let session_enumerator: IAudioSessionEnumerator = session_manager.GetSessionEnumerator()?;
        let session_count = session_enumerator.GetCount()?;

        let mut pids = HashSet::<u32>::new();
        pids.insert(current_pid);

        for i in 0..session_count {
            let control: IAudioSessionControl2 = session_enumerator.GetSession(i)?.cast()?;
            let pid = match control.GetProcessId() {
                Ok(pid) => pid,
                Err(_) => continue,
            };

            if pid == 0 {
                continue;
            }

            if pid == current_pid {
                pids.insert(pid);
                continue;
            }

            if let (Some(cur_name), Some(session_name)) = (current_process_name.as_deref(), get_process_name(pid)) {
                if session_name.as_str() == cur_name {
                    pids.insert(pid);
                }
            }
        }

        Ok(pids.into_iter().collect())
    }
}

pub fn get_app_source() -> Result<Option<AudioSource>, Box<dyn std::error::Error>> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = RoInitialize(RO_INIT_MULTITHREADED);

        let target_pids = collect_related_app_audio_pids()?;
        let class_name = HSTRING::from("Windows.Media.Internal.AudioPolicyConfig");

        let mut selected_id: Option<String> = None;

        if let Ok(factory) = ro_get_activation_factory::<IAudioPolicyConfigFactory21H2>(&class_name) {
            for pid in &target_pids {
                if let Some(endpoint) = get_persisted_endpoint_for_process(&factory, *pid)? {
                    selected_id = Some(endpoint);
                    break;
                }
            }
        } else if let Ok(factory) = ro_get_activation_factory::<IAudioPolicyConfigFactoryDownlevel>(&class_name) {
            for pid in &target_pids {
                if let Some(endpoint) = get_persisted_endpoint_for_process(&factory, *pid)? {
                    selected_id = Some(endpoint);
                    break;
                }
            }
        }

        let Some(endpoint_id) = selected_id else {
            return Ok(None);
        };

        Ok(get_audio_sources()
            .unwrap_or_default()
            .into_iter()
            .find(|s| s.id.eq_ignore_ascii_case(&endpoint_id)))
    }
}

pub fn set_app_source(source: &str) -> Result<(), NativeError> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = RoInitialize(RO_INIT_MULTITHREADED);

        let exists = endpoint_exists(source).map_err(|e| NativeError::Generic(e.to_string()))?;
        if !exists {
            return Err(NativeError::AudioSourceNotFound(source.to_string()));
        }

        let target_pids = collect_related_app_audio_pids().map_err(|e| NativeError::Generic(e.to_string()))?;
        if target_pids.is_empty() {
            return Err(NativeError::AppAudioStreamNotFound);
        }

        let class_name = HSTRING::from("Windows.Media.Internal.AudioPolicyConfig");
        let policy_device_id = HSTRING::from(to_policy_device_id(source));

        if let Ok(factory) = ro_get_activation_factory::<IAudioPolicyConfigFactory21H2>(&class_name) {
            for pid in &target_pids {
                set_persisted_endpoint_for_all_roles(&factory, *pid, &policy_device_id).map_err(|e| NativeError::Generic(e.to_string()))?;
            }
            tracing::info!(
                "set_app_source: Applied per-process endpoint routing (21H2 interface), pids={:?}, endpoint={}",
                target_pids,
                source
            );
            return Ok(());
        }

        if let Ok(factory) = ro_get_activation_factory::<IAudioPolicyConfigFactoryDownlevel>(&class_name) {
            for pid in &target_pids {
                set_persisted_endpoint_for_all_roles(&factory, *pid, &policy_device_id).map_err(|e| NativeError::Generic(e.to_string()))?;
            }
            tracing::info!(
                "set_app_source: Applied per-process endpoint routing (downlevel interface), pids={:?}, endpoint={}",
                target_pids,
                source
            );
            return Ok(());
        }
    }

    Err(NativeError::OperationFailed(
        "Failed to acquire AudioPolicyConfigFactory for per-process routing".into(),
    ))
}
