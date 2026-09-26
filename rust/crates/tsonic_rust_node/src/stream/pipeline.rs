pub trait WritableTarget: Clone {
    fn writable_handle(&self) -> Writable;
}

impl WritableTarget for Writable {
    fn writable_handle(&self) -> Writable {
        self.clone()
    }
}

pub(crate) fn install_pipeline<W>(readable: Readable, destination: W) -> NodeResult<()>
where
    W: WritableTarget + Clone + 'static,
{
    let writable = destination.writable_handle();
    let finish_writable = writable.clone();
    readable.on_end_internal(move || {
        finish_writable.end_checked()
    });

    let flow_readable = readable.clone();
    let flow_writable = writable.clone();
    readable.on_data_internal(move |chunk| {
        if flow_writable.write_buffer(&chunk)? {
            return Ok(());
        }
        flow_readable.pause();
        let resume_readable = flow_readable.clone();
        flow_writable.on_drain_internal(move || {
            resume_readable.resume();
            Ok(())
        });
        Ok(())
    })?;
    Ok(())
}

pub fn pipeline<W: WritableTarget + Clone + 'static>(
    readable: &Readable,
    writable: &W,
) -> NodeResult<()> {
    install_pipeline(readable.clone(), writable.clone())
}

pub fn finished(readable: &Readable, writable: &Writable) -> bool {
    readable.is_ended() && writable.is_ended()
}

pub fn finished_with_options(
    readable: &Readable,
    writable: &Writable,
    options: &FinishedOptions,
) -> bool {
    if options.error && (readable.errored().is_some() || writable.errored().is_some()) {
        return false;
    }
    if options.readable && !readable.is_ended() {
        return false;
    }
    if options.writable && !writable.is_ended() {
        return false;
    }
    true
}

pub fn is_readable(readable: &Readable) -> bool {
    readable.readable()
}

pub fn is_writable(writable: &Writable) -> bool {
    writable.writable()
}

pub fn is_errored(readable: &Readable, writable: &Writable) -> bool {
    readable.errored().is_some() || writable.errored().is_some()
}

pub fn is_destroyed(readable: &Readable, writable: &Writable) -> bool {
    readable.destroyed() || writable.destroyed()
}

pub fn compose(readable: Readable, next: impl Fn(Readable) -> Readable) -> Readable {
    readable.compose(next)
}

pub fn add_abort_signal(readable: &Readable, signal_aborted: bool) {
    if signal_aborted {
        readable.destroy_with_error("aborted");
    }
}
