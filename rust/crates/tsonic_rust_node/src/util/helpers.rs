fn split_diff_units(value: &str) -> Vec<&str> {
    if value.contains('\n') {
        value.lines().collect()
    } else if value.is_empty() {
        Vec::new()
    } else {
        vec![value]
    }
}

fn next_arg<'a>(args: &'a [JsValue], index: &mut usize) -> &'a JsValue {
    let value = args.get(*index).unwrap_or(&JsValue::Undefined);
    *index += 1;
    value
}

fn set_parsed_arg(result: &mut ParseArgsResult, name: &str, value: String, multiple: bool) {
    if multiple {
        if let Some((_, values)) = result.values.iter_mut().find(|(key, _)| key == name) {
            values.push(value);
        } else {
            result.values.push((name.to_string(), vec![value]));
        }
    } else if let Some((_, values)) = result.values.iter_mut().find(|(key, _)| key == name) {
        *values = vec![value];
    } else {
        result.values.push((name.to_string(), vec![value]));
    }
}

// Node %s semantics: strings are emitted verbatim; every other value is
// rendered through inspection (which matches String() for primitives).
fn format_string(value: &JsValue) -> crate::error::NodeResult<String> {
    match value {
        JsValue::String(text) => text.to_utf8().map_err(|_| {
            crate::error::NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "JavaScript string cannot be represented by the native Rust string carrier",
            )
        }),
        other => Ok(other.inspect()),
    }
}

// Node %d semantics: the argument is coerced with Number() before being
// rendered with JS number formatting (NaN for values that do not coerce).
fn format_number(value: &JsValue) -> String {
    let number = match value {
        JsValue::Number(value) => *value,
        JsValue::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        JsValue::Null => 0.0,
        JsValue::String(text) => {
            match text.to_utf8() {
                Ok(text) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        0.0
                    } else {
                        trimmed.parse::<f64>().unwrap_or(f64::NAN)
                    }
                }
                Err(_) => f64::NAN,
            }
        }
        _ => f64::NAN,
    };
    JsValue::Number(number).inspect()
}
