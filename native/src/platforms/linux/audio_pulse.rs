use libpulse_binding::{
    context::{Context, FlagSet as ContextFlagSet, State as ContextState},
    mainloop::standard::Mainloop,
    operation::{Operation, State as OpState},
    proplist::Proplist,
};
use std::sync::{Arc, Mutex};

pub const CLIENT_NAME: &str = "streamq-native";

pub type PulseResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn wait_ready(ml: &mut Mainloop, ctx: &Context) -> PulseResult<()> {
    loop {
        match ctx.get_state() {
            ContextState::Ready => return Ok(()),
            ContextState::Failed | ContextState::Terminated => {
                return Err("Pulse context failed/terminated".into());
            }
            _ => {
                ml.iterate(true);
            }
        }
    }
}

pub fn wait_op<T: ?Sized>(ml: &mut Mainloop, op: &Operation<T>) {
    while op.get_state() == OpState::Running {
        ml.iterate(true);
    }
}

pub fn create_proplist() -> PulseResult<Proplist> {
    let mut proplist = Proplist::new().ok_or("Failed to create proplist")?;
    proplist.set_str(libpulse_binding::proplist::properties::APPLICATION_NAME, CLIENT_NAME).ok();
    Ok(proplist)
}

pub fn create_connected_context() -> PulseResult<(Mainloop, Context)> {
    create_connected_context_named(CLIENT_NAME)
}

pub fn create_connected_context_named(context_name: &str) -> PulseResult<(Mainloop, Context)> {
    let proplist = create_proplist()?;

    let mut ml = Mainloop::new().ok_or("Failed to create mainloop")?;
    let mut ctx = Context::new_with_proplist(&ml, context_name, &proplist).ok_or("Failed to create context")?;

    ctx.connect(None, ContextFlagSet::NOFLAGS, None)?;
    wait_ready(&mut ml, &ctx)?;

    Ok((ml, ctx))
}

pub fn get_default_sink_name(ml: &mut Mainloop, ctx: &Context) -> PulseResult<String> {
    let out = Arc::new(Mutex::new(None::<String>));
    let out_clone = out.clone();

    let op = ctx.introspect().get_server_info(move |info| {
        if let Some(name) = info.default_sink_name.as_ref() {
            *out_clone.lock().unwrap() = Some(name.to_string());
        }
    });

    wait_op(ml, &op);

    let result = out.lock().unwrap().take();
    result.ok_or_else(|| "default sink name not found".into())
}

pub fn disconnect(ctx: &mut Context) {
    ctx.disconnect();
}
