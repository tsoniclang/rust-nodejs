pub(crate) fn install_pipeline<E: From<NodeError> + 'static, W>(
    readable: Readable<E>,
    destination: W,
) -> Result<(), E>
where
    W: WritableTarget<E> + Clone + 'static,
{
    let writable = destination.writable_handle();
    let finish_writable = writable.clone();
    readable.on_end_internal(move || finish_writable.end().map(|_| ()));

    let flow_readable = readable.clone();
    let flow_writable = writable.clone();
    readable.on_data_internal(move |chunk| {
        if flow_writable.write_buffer(&chunk)? {
            return Ok(());
        }
        flow_readable.pause();
        let resume_readable = flow_readable.clone();
        flow_writable.on_drain_internal(move || resume_readable.resume().map(|_| ()));
        Ok(())
    })?;
    Ok(())
}

pub fn pipeline<E: From<NodeError> + 'static, W: WritableTarget<E> + Clone + 'static>(
    readable: &Readable<E>,
    writable: &W,
) -> Result<(), E> {
    install_pipeline(readable.clone(), writable.clone())
}

pub fn finished<E: From<NodeError> + 'static>(
    readable: &Readable<E>,
    writable: &Writable<E>,
) -> bool {
    readable.is_ended() && writable.is_ended()
}

pub fn finished_with_options<E: From<NodeError> + 'static>(
    readable: &Readable<E>,
    writable: &Writable<E>,
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

pub fn is_readable<E: From<NodeError> + 'static>(readable: &Readable<E>) -> bool {
    readable.readable()
}

pub fn is_writable<E: From<NodeError> + 'static>(writable: &Writable<E>) -> bool {
    writable.writable()
}

pub fn is_errored<E: From<NodeError> + 'static>(
    readable: &Readable<E>,
    writable: &Writable<E>,
) -> bool {
    readable.errored().is_some() || writable.errored().is_some()
}

pub fn is_destroyed<E: From<NodeError> + 'static>(
    readable: &Readable<E>,
    writable: &Writable<E>,
) -> bool {
    readable.destroyed() || writable.destroyed()
}

pub fn compose<E: From<NodeError> + 'static>(
    readable: Readable<E>,
    next: impl Fn(Readable<E>) -> Readable<E>,
) -> Readable<E> {
    readable.compose(next)
}

pub fn add_abort_signal<E: From<NodeError> + 'static>(
    readable: &Readable<E>,
    signal_aborted: bool,
) -> Result<(), E> {
    if signal_aborted {
        readable.destroy_with_error("aborted")?;
    }
    Ok(())
}
