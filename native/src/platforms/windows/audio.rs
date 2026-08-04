use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};

pub use super::audio_endpoints::get_audio_sources;
pub use super::audio_policy::{get_app_source, set_app_source};
pub use super::audio_watchers::{start_app_source_watcher, start_volume_listener};

pub fn get_system_volume() -> f64 {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator = match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
            Ok(e) => e,
            Err(_) => return 1.0,
        };

        let device = match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
            Ok(d) => d,
            Err(_) => return 1.0,
        };

        let endpoint_volume: IAudioEndpointVolume = match device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
            Ok(v) => v,
            Err(_) => return 1.0,
        };

        match endpoint_volume.GetMasterVolumeLevelScalar() {
            Ok(volume) => volume as f64,
            Err(_) => 1.0,
        }
    }
}

pub fn set_system_volume(volume: f64) -> Result<(), Box<dyn std::error::Error>> {
    let clamped_volume = volume.clamp(0.0, 1.0) as f32;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let endpoint_volume: IAudioEndpointVolume = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)?;

        endpoint_volume.SetMasterVolumeLevelScalar(clamped_volume, std::ptr::null())?;
    }

    tracing::info!("set_system_volume: Set volume to {}", clamped_volume);
    Ok(())
}
