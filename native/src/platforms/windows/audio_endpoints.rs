use std::ffi::c_void;

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{DEVICE_STATE_ACTIVE, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ};

use crate::models::media::AudioSource;
unsafe fn pwstr_to_string_and_free(ptr: windows::core::PWSTR) -> Result<String, Box<dyn std::error::Error>> {
    if ptr.is_null() {
        return Err("Received null PWSTR".into());
    }
    let out = unsafe { ptr.to_string()? };
    unsafe { CoTaskMemFree(Some(ptr.0 as *const c_void)) };
    Ok(out)
}

unsafe fn get_device_friendly_name(device: &IMMDevice) -> Option<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.ok()?;

    let mut pv = match unsafe { store.GetValue(&PKEY_Device_FriendlyName) } {
        Ok(v) => v,
        Err(_) => return None,
    };

    let result = match unsafe { PropVariantToStringAlloc(&pv) } {
        Ok(pwstr) => {
            let s = unsafe { pwstr.to_string() }.ok();
            unsafe { CoTaskMemFree(Some(pwstr.0 as *const c_void)) };
            s
        }
        Err(_) => None,
    };

    let _ = unsafe { PropVariantClear(&mut pv as *mut _) };
    result
}

pub fn get_default_endpoint_id() -> Result<String, Box<dyn std::error::Error>> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let default_device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        pwstr_to_string_and_free(default_device.GetId()?)
    }
}

pub fn endpoint_exists(endpoint_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let endpoint_wide: Vec<u16> = endpoint_id.encode_utf16().chain(std::iter::once(0)).collect();
        Ok(enumerator.GetDevice(windows::core::PCWSTR(endpoint_wide.as_ptr())).is_ok())
    }
}

pub fn get_audio_sources() -> Result<Vec<AudioSource>, Box<dyn std::error::Error>> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let default_id = get_default_endpoint_id()?;

        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let count = collection.GetCount()?;

        let mut out = Vec::with_capacity(count as usize);

        for i in 0..count {
            let device = collection.Item(i)?;
            let id = pwstr_to_string_and_free(device.GetId()?)?;
            let friendly_name = get_device_friendly_name(&device).unwrap_or_else(|| id.clone());

            let volume = if let Ok(endpoint_volume) = device.Activate::<windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume>(CLSCTX_ALL, None) {
                endpoint_volume.GetMasterVolumeLevelScalar().unwrap_or(1.0) as f64
            } else {
                1.0
            };

            out.push(AudioSource {
                id: id.clone(),
                name: friendly_name,
                is_default: id.eq_ignore_ascii_case(&default_id),
                volume,
            });
        }

        out.sort_by(|a, b| b.is_default.cmp(&a.is_default).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));

        Ok(out)
    }
}
