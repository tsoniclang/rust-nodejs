pub fn hrtime_open_number() -> JsArray<f64> {
    let (seconds, nanoseconds) = current_hrtime_number();
    JsArray::from_dense(vec![seconds, nanoseconds])
}

pub fn hrtime_since_number(previous: &JsArray<f64>) -> NodeResult<JsArray<f64>> {
    let values = previous.values();
    if values.len() != 2 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "previous hrtime must contain exactly two values",
        ));
    }
    let previous_seconds = values[0].ok_or_else(|| {
        NodeError::new("ERR_INVALID_ARG_VALUE", "previous hrtime seconds are absent")
    })?;
    let previous_nanoseconds = values[1].ok_or_else(|| {
        NodeError::new(
            "ERR_INVALID_ARG_VALUE",
            "previous hrtime nanoseconds are absent",
        )
    })?;
    let (mut seconds, mut nanoseconds) = current_hrtime_number();
    seconds -= previous_seconds;
    nanoseconds -= previous_nanoseconds;
    if nanoseconds < 0.0 {
        seconds -= 1.0;
        nanoseconds += 1_000_000_000.0;
    }

    Ok(JsArray::from_dense(vec![seconds, nanoseconds]))
}

fn current_hrtime_number() -> (f64, f64) {
    let elapsed = START.get_or_init(Instant::now).elapsed();
    (
        elapsed.as_secs_f64().floor(),
        f64::from(elapsed.subsec_nanos()),
    )
}
