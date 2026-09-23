pub fn hrtime_open() -> NodeResult<JsArray<i64>> {
    let (seconds, nanoseconds) = current_hrtime()?;
    Ok(JsArray::from_dense(vec![seconds, nanoseconds]))
}

pub fn hrtime_since(previous: &JsArray<i64>) -> NodeResult<JsArray<i64>> {
    if previous.len() != 2 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "previous hrtime must contain exactly two values",
        ));
    }
    let previous_seconds = previous.get(0).ok_or_else(hrtime_range_error)?;
    let previous_nanoseconds = previous.get(1).ok_or_else(hrtime_range_error)?;
    if !(0..1_000_000_000).contains(&previous_nanoseconds) {
        return Err(hrtime_range_error());
    }
    let (mut seconds, mut nanoseconds) = current_hrtime()?;
    seconds = seconds.checked_sub(previous_seconds).ok_or_else(hrtime_range_error)?;
    nanoseconds -= previous_nanoseconds;
    if nanoseconds < 0 {
        seconds = seconds.checked_sub(1).ok_or_else(hrtime_range_error)?;
        nanoseconds += 1_000_000_000;
    }

    Ok(JsArray::from_dense(vec![seconds, nanoseconds]))
}

fn current_hrtime() -> NodeResult<(i64, i64)> {
    let (seconds, nanoseconds) = hrtime(None);
    Ok((i64::try_from(seconds).map_err(|_| hrtime_range_error())?, i64::from(nanoseconds)))
}

fn hrtime_range_error() -> NodeError {
    NodeError::new("ERR_OUT_OF_RANGE", "hrtime exceeds its native integer range")
}
