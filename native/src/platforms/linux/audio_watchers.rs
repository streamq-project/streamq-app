use std::sync::{Arc, Mutex};

use libpulse_binding::context::subscribe::{Facility, InterestMaskSet, Operation as SubOp};

use super::audio_endpoints::get_audio_sources;
use super::audio_policy::get_app_source;
use super::audio_pulse::{create_connected_context, wait_op};
use crate::models::media::AudioSource;

pub struct VolumeListener {
    state: Arc<crate::models::media::MediaSessionState>,
    refresh_tx: tokio::sync::mpsc::UnboundedSender<()>,
    is_dev: bool,
    audio_manager: Arc<crate::platforms::linux::audio_sessions::AudioSessionManager>,
}

impl VolumeListener {
    pub fn new(
        state: Arc<crate::models::media::MediaSessionState>,
        refresh_tx: tokio::sync::mpsc::UnboundedSender<()>,
        is_dev: bool,
        audio_manager: Arc<crate::platforms::linux::audio_sessions::AudioSessionManager>,
    ) -> Self {
        Self {
            state,
            refresh_tx,
            is_dev,
            audio_manager,
        }
    }

    pub fn run(&self) {
        tracing::info!("VolumeListener: Starting volume listener thread");

        let (mut ml, mut ctx) = match create_connected_context() {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("VolumeListener: Failed to initialize PulseAudio context: {:?}", e);
                return;
            }
        };

        let state_clone = Arc::clone(&self.state);

        let volume_dirty = Arc::new(Mutex::new(true));
        let volume_dirty_cb = Arc::clone(&volume_dirty);

        let app_source_dirty = Arc::new(Mutex::new(true));
        let app_source_dirty_cb = Arc::clone(&app_source_dirty);

        ctx.set_subscribe_callback(Some(Box::new(move |facility, op, _idx| {
            if let (Some(f), Some(o)) = (facility, op) {
                let is_volume_event = (f == Facility::Sink || f == Facility::Server) && o == SubOp::Changed;
                let is_app_route_event = f == Facility::SinkInput && (o == SubOp::New || o == SubOp::Changed || o == SubOp::Removed);

                if is_volume_event || is_app_route_event {
                    tracing::debug!("VolumeListener: event {:?} {:?}", f, o);
                }

                if is_volume_event {
                    *volume_dirty_cb.lock().unwrap() = true;
                }

                if is_app_route_event {
                    *app_source_dirty_cb.lock().unwrap() = true;
                }
            }
        })));

        let op = ctx.subscribe(InterestMaskSet::SINK | InterestMaskSet::SERVER | InterestMaskSet::SINK_INPUT, |_| {});
        wait_op(&mut ml, &op);

        tracing::info!("VolumeListener: Volume listener started successfully");

        self.audio_manager.update_cache();

        let mut last_app_source: Option<Option<AudioSource>> = None;

        loop {
            ml.iterate(true);

            let should_emit_sources = {
                let mut dirty = volume_dirty.lock().unwrap();
                let out = *dirty;
                if out {
                    *dirty = false;
                }
                out
            };

            if should_emit_sources {
                match get_audio_sources() {
                    Ok(sources) => {
                        if let Some(default_source) = sources.iter().find(|s| s.is_default) {
                            *state_clone.system_volume.lock().unwrap() = default_source.volume;
                        }

                        state_clone.event_emitter.emit(crate::event_emitter::Event::SourcesChanged(sources));
                    }
                    Err(e) => {
                        tracing::warn!("VolumeListener: failed to collect audio sources after volume event: {:?}", e);
                    }
                }
            }

            let should_check_app_source = {
                let mut dirty = app_source_dirty.lock().unwrap();
                let out = *dirty;
                if out {
                    *dirty = false;
                }
                out
            };

            if should_check_app_source {
                self.audio_manager.update_cache();

                let _ = self.refresh_tx.send(());

                match get_app_source(self.is_dev) {
                    Ok(current) => {
                        if last_app_source.as_ref() != Some(&current) {
                            last_app_source = Some(current.clone());
                            state_clone.event_emitter.emit(crate::event_emitter::Event::AppAudioSourceChanged(current));
                        }
                    }
                    Err(e) => {
                        tracing::debug!("VolumeListener: Failed to resolve app source: {:?}", e);
                    }
                }
            }
        }
    }
}

pub fn start_volume_listener(
    state: Arc<crate::models::media::MediaSessionState>,
    refresh_tx: tokio::sync::mpsc::UnboundedSender<()>,
    is_dev: bool,
    audio_manager: Arc<crate::platforms::linux::audio_sessions::AudioSessionManager>,
) {
    let listener = VolumeListener::new(state, refresh_tx, is_dev, audio_manager);
    std::thread::spawn(move || {
        listener.run();
    });
}
