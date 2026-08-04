use std::sync::{Arc, Mutex};

use libpulse_binding::callbacks::ListResult;

use super::audio_endpoints::get_audio_sources;
use super::audio_pulse::{PulseResult, create_connected_context, disconnect, wait_op};
use crate::error::NativeError;
use crate::models::media::AudioSource;

fn is_streamq_app_name(value: &str, is_dev: bool) -> bool {
    let app = value.to_lowercase();

    if is_dev { app == "electron" } else { app.contains("streamq") }
}

fn is_streamq_app_by_props(current_pid: &str, pid_prop: Option<&str>, binary_prop: Option<&str>, app_name_prop: Option<&str>, is_dev: bool) -> bool {
    if let Some(v) = pid_prop {
        if v == current_pid {
            return true;
        }
    }

    if let Some(v) = binary_prop {
        if is_streamq_app_name(v, is_dev) {
            return true;
        }
    }

    if let Some(v) = app_name_prop {
        if is_streamq_app_name(v, is_dev) {
            return true;
        }
    }

    false
}

pub fn set_app_source(source: &str, is_dev: bool) -> Result<(), NativeError> {
    let (mut ml, mut ctx) = create_connected_context().map_err(|e| NativeError::Generic(e.to_string()))?;
    let target_sink = source.to_string();

    let sink_exists = Arc::new(Mutex::new(false));
    let sink_exists_clone = Arc::clone(&sink_exists);
    let target_sink_clone = target_sink.clone();

    let op = ctx.introspect().get_sink_info_list(move |res| {
        if let ListResult::Item(info) = res {
            if let Some(name) = info.name.as_ref() {
                if name.as_ref() == target_sink_clone {
                    *sink_exists_clone.lock().unwrap() = true;
                }
            }
        }
    });
    wait_op(&mut ml, &op);

    if !*sink_exists.lock().unwrap() {
        disconnect(&mut ctx);
        return Err(NativeError::AudioSourceNotFound(source.to_string()));
    }

    let current_pid = std::process::id().to_string();
    let input_indexes = Arc::new(Mutex::new(Vec::<u32>::new()));
    let input_indexes_clone = Arc::clone(&input_indexes);

    let op = ctx.introspect().get_sink_input_info_list(move |res| {
        if let ListResult::Item(info) = res {
            if is_streamq_app_by_props(
                &current_pid,
                info.proplist.get_str("application.process.id").as_deref(),
                info.proplist.get_str("application.process.binary").as_deref(),
                info.proplist.get_str("application.name").as_deref(),
                is_dev,
            ) {
                input_indexes_clone.lock().unwrap().push(info.index);
            }
        }
    });
    wait_op(&mut ml, &op);

    let indexes = input_indexes.lock().unwrap().clone();
    if indexes.is_empty() {
        disconnect(&mut ctx);
        return Err(NativeError::AppAudioStreamNotFound);
    }

    for idx in indexes {
        let op = ctx.introspect().move_sink_input_by_name(idx, &target_sink, None);
        wait_op(&mut ml, &op);
    }

    disconnect(&mut ctx);
    Ok(())
}

pub fn get_app_source(is_dev: bool) -> PulseResult<Option<AudioSource>> {
    let (mut ml, mut ctx) = create_connected_context()?;
    let current_pid = std::process::id().to_string();

    let sink_index = Arc::new(Mutex::new(None::<u32>));
    let sink_index_clone = Arc::clone(&sink_index);

    let op = ctx.introspect().get_sink_input_info_list(move |res| {
        if let ListResult::Item(info) = res {
            if is_streamq_app_by_props(
                &current_pid,
                info.proplist.get_str("application.process.id").as_deref(),
                info.proplist.get_str("application.process.binary").as_deref(),
                info.proplist.get_str("application.name").as_deref(),
                is_dev,
            ) {
                let mut guard = sink_index_clone.lock().unwrap();
                if guard.is_none() {
                    *guard = Some(info.sink);
                }
            }
        }
    });
    wait_op(&mut ml, &op);

    let Some(sink_index) = sink_index.lock().unwrap().take() else {
        disconnect(&mut ctx);
        return Ok(None);
    };

    let sink_name = Arc::new(Mutex::new(None::<String>));
    let sink_name_clone = Arc::clone(&sink_name);

    let op = ctx.introspect().get_sink_info_by_index(sink_index, move |res| {
        if let ListResult::Item(info) = res {
            if let Some(name) = info.name.as_ref() {
                *sink_name_clone.lock().unwrap() = Some(name.to_string());
            }
        }
    });
    wait_op(&mut ml, &op);
    disconnect(&mut ctx);

    let Some(sink_name) = sink_name.lock().unwrap().take() else {
        return Ok(None);
    };

    Ok(get_audio_sources().ok().and_then(|sources| sources.into_iter().find(|s| s.id == sink_name)))
}
