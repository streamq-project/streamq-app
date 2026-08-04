use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use libpulse_binding::{callbacks::ListResult, volume::Volume};

use crate::models::media::AudioSource;

use super::audio_pulse::{PulseResult, create_connected_context, disconnect, get_default_sink_name, wait_op};

pub fn get_audio_sources() -> PulseResult<Vec<AudioSource>> {
    let (mut ml, mut ctx) = create_connected_context()?;
    let default_sink_name = get_default_sink_name(&mut ml, &ctx)?;

    let sources = Arc::new(Mutex::new(Vec::<AudioSource>::new()));
    let seen = Arc::new(Mutex::new(HashSet::<String>::new()));

    let sources_clone = Arc::clone(&sources);
    let seen_clone = Arc::clone(&seen);
    let default_sink_name_clone = default_sink_name.clone();

    let op = ctx.introspect().get_sink_info_list(move |res| {
        if let ListResult::Item(info) = res {
            if let Some(name) = info.name.as_ref() {
                let sink_name = name.to_string();

                let mut seen_guard = seen_clone.lock().unwrap();
                if !seen_guard.insert(sink_name.clone()) {
                    return;
                }
                drop(seen_guard);

                let display_name = info.description.as_ref().map(|d| d.to_string()).unwrap_or_else(|| sink_name.clone());

                let avg = info.volume.avg();
                let linear_volume = avg.0 as f64 / Volume::NORMAL.0 as f64;

                sources_clone.lock().unwrap().push(AudioSource {
                    id: sink_name.clone(),
                    name: display_name,
                    is_default: sink_name == default_sink_name_clone,
                    volume: linear_volume,
                });
            }
        }
    });

    wait_op(&mut ml, &op);
    disconnect(&mut ctx);

    let mut out = sources.lock().unwrap().clone();
    out.sort_by(|a, b| b.is_default.cmp(&a.is_default).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));

    Ok(out)
}
