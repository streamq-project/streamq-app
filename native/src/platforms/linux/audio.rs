use std::sync::{Arc, Mutex};

use libpulse_binding::{callbacks::ListResult, volume::Volume};

use super::audio_pulse::{create_connected_context, disconnect, get_default_sink_name, wait_op};

pub use super::audio_endpoints::get_audio_sources;
pub use super::audio_policy::{get_app_source, set_app_source};
pub use super::audio_watchers::start_volume_listener;

pub fn get_system_volume() -> f64 {
    tracing::debug!("get_system_volume: Getting system volume via PulseAudio");

    let (mut ml, mut ctx) = match create_connected_context() {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("get_system_volume: Failed to initialize context: {:?}", e);
            return 1.0;
        }
    };

    let sink_name = match get_default_sink_name(&mut ml, &ctx) {
        Ok(name) => name,
        Err(e) => {
            tracing::error!("get_system_volume: Failed to get default sink: {:?}", e);
            disconnect(&mut ctx);
            return 1.0;
        }
    };

    let volume_linear = Arc::new(Mutex::new(None::<f64>));
    let volume_linear_clone = Arc::clone(&volume_linear);

    let op = ctx.introspect().get_sink_info_by_name(&sink_name, move |res| {
        if let ListResult::Item(info) = res {
            let avg = info.volume.avg();
            let linear = avg.0 as f64 / Volume::NORMAL.0 as f64;
            *volume_linear_clone.lock().unwrap() = Some(linear);
        }
    });

    wait_op(&mut ml, &op);
    disconnect(&mut ctx);

    let result = volume_linear.lock().unwrap().take();

    match result {
        Some(vol) => vol,
        None => {
            tracing::error!("get_system_volume: Failed to get volume");
            1.0
        }
    }
}

pub fn set_system_volume(volume: f64) -> Result<(), Box<dyn std::error::Error>> {
    tracing::debug!("set_system_volume: Setting system volume to {}", volume);

    let linear = volume.clamp(0.0, 1.5);

    let (mut ml, mut ctx) = create_connected_context()?;
    let sink_name = get_default_sink_name(&mut ml, &ctx)?;

    let channels = Arc::new(Mutex::new(None::<u8>));
    let channels_clone = Arc::clone(&channels);

    let op = ctx.introspect().get_sink_info_by_name(&sink_name, move |res| {
        if let ListResult::Item(info) = res {
            *channels_clone.lock().unwrap() = Some(info.channel_map.len() as u8);
        }
    });

    wait_op(&mut ml, &op);

    let channels_count = channels.lock().unwrap().take().ok_or("Failed to get channel count")?;

    let mut vols = libpulse_binding::volume::ChannelVolumes::default();
    let vol = Volume((linear * Volume::NORMAL.0 as f64) as u32);
    vols.set(channels_count, vol);

    let op = ctx.introspect().set_sink_volume_by_name(&sink_name, &vols, None);
    wait_op(&mut ml, &op);

    disconnect(&mut ctx);

    tracing::info!("set_system_volume: Set volume to {}", linear);
    Ok(())
}
