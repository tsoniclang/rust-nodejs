use tsonic_rust_js::numeric::IndexInput;

pub fn to_string_range_number(
    buffer: &Buffer,
    encoding: &str,
    start: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    end: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<String> {
    buffer.to_string_range(
        Some(encoding),
        numeric_offset(start)?,
        Some(numeric_offset(end)?),
    )
}

pub fn to_string_from_number(
    buffer: &Buffer,
    encoding: &str,
    start: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<String> {
    buffer.to_string_range(Some(encoding), numeric_offset(start)?, None)
}

pub fn index_of_buffer_number(
    buffer: &Buffer,
    needle: &Buffer,
    byte_offset: impl tsonic_rust_runtime::conversions::IntegerInput<isize>,
) -> NodeResult<isize> {
    let byte_offset = byte_offset.checked_integer().ok_or_else(|| {
        NodeError::new("ERR_OUT_OF_RANGE", "byteOffset must be an integer")
    })?;
    Ok(needle.with_bytes(|bytes| {
        buffer
            .index_of(bytes, byte_offset)
            .and_then(|index| isize::try_from(index).ok())
            .unwrap_or(-1)
    }))
}

pub fn copy_open_number(
    source: &Buffer,
    target: &Buffer,
    target_start: impl IndexInput,
    source_start: impl IndexInput,
) -> NodeResult<usize> {
    copy_number(
        source,
        target,
        copy_index(target_start, "targetStart")?,
        copy_index(source_start, "sourceStart")?,
        None,
    )
}

pub fn copy_closed_number(
    source: &Buffer,
    target: &Buffer,
    target_start: impl IndexInput,
    source_start: impl IndexInput,
    source_end: impl IndexInput,
) -> NodeResult<usize> {
    copy_number(
        source,
        target,
        copy_index(target_start, "targetStart")?,
        copy_index(source_start, "sourceStart")?,
        Some(copy_index(source_end, "sourceEnd")?),
    )
}

fn copy_number(
    source: &Buffer,
    target: &Buffer,
    target_start: usize,
    source_start: usize,
    source_end: Option<usize>,
) -> NodeResult<usize> {
    if target_start >= target.len() {
        return Ok(0);
    }
    if source_start > source.len() {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "sourceStart is outside the source buffer",
        ));
    }
    source.copy(target, target_start, source_start, source_end)
}

pub fn slice_open_number(buffer: &Buffer, start: impl IndexInput) -> Buffer {
    Buffer {
        view: buffer.view.subarray_from(start),
    }
}

pub fn slice_closed_number(
    buffer: &Buffer,
    start: impl IndexInput,
    end: impl IndexInput,
) -> Buffer {
    Buffer {
        view: buffer.view.subarray_to(start, end),
    }
}

pub fn read_uint8_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<u8> {
    buffer.read_uint8(numeric_offset(offset)?)
}

pub fn read_int8_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<i8> {
    buffer.read_int8(numeric_offset(offset)?)
}

pub fn read_uint16_le_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<u16> {
    buffer.read_uint16_le(numeric_offset(offset)?)
}

pub fn read_uint16_be_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<u16> {
    buffer.read_uint16_be(numeric_offset(offset)?)
}

pub fn read_int16_le_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<i16> {
    buffer.read_int16_le(numeric_offset(offset)?)
}

pub fn read_int16_be_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<i16> {
    buffer.read_int16_be(numeric_offset(offset)?)
}

pub fn read_uint32_le_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<u32> {
    buffer.read_uint32_le(numeric_offset(offset)?)
}

pub fn read_uint32_be_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<u32> {
    buffer.read_uint32_be(numeric_offset(offset)?)
}

pub fn read_int32_le_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<i32> {
    buffer.read_int32_le(numeric_offset(offset)?)
}

pub fn read_int32_be_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<i32> {
    buffer.read_int32_be(numeric_offset(offset)?)
}

pub fn read_float_le_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<f32> {
    buffer.read_float_le(numeric_offset(offset)?)
}

pub fn read_float_be_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<f32> {
    buffer.read_float_be(numeric_offset(offset)?)
}

pub fn read_double_le_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<f64> {
    buffer.read_double_le(numeric_offset(offset)?)
}

pub fn read_double_be_number(
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<f64> {
    buffer.read_double_be(numeric_offset(offset)?)
}

pub fn write_uint8_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<u8>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint8(integer_value(value)?, offset)?;
    Ok(offset + 1)
}

pub fn write_int8_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<i8>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_int8(integer_value(value)?, offset)?;
    Ok(offset + 1)
}

pub fn write_uint16_le_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint16_le(integer_value(value)?, offset)?;
    Ok(offset + 2)
}

pub fn write_uint16_be_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint16_be(integer_value(value)?, offset)?;
    Ok(offset + 2)
}

pub fn write_int16_le_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<i16>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_int16_le(integer_value(value)?, offset)?;
    Ok(offset + 2)
}

pub fn write_int16_be_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<i16>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_int16_be(integer_value(value)?, offset)?;
    Ok(offset + 2)
}

pub fn write_uint32_le_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<u32>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint32_le(integer_value(value)?, offset)?;
    Ok(offset + 4)
}

pub fn write_uint32_be_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<u32>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint32_be(integer_value(value)?, offset)?;
    Ok(offset + 4)
}

pub fn write_int32_le_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_int32_le(integer_value(value)?, offset)?;
    Ok(offset + 4)
}

pub fn write_int32_be_number(
    buffer: &mut Buffer,
    value: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_int32_be(integer_value(value)?, offset)?;
    Ok(offset + 4)
}

pub fn write_float_le_number(
    buffer: &mut Buffer,
    value: f64,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_float_le(value as f32, offset)?;
    Ok(offset + 4)
}

pub fn write_float_be_number(
    buffer: &mut Buffer,
    value: f64,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_float_be(value as f32, offset)?;
    Ok(offset + 4)
}

pub fn write_double_le_number(
    buffer: &mut Buffer,
    value: f64,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_double_le(value, offset)?;
    Ok(offset + 8)
}

pub fn write_double_be_number(
    buffer: &mut Buffer,
    value: f64,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    let offset = numeric_offset(offset)?;
    buffer.write_double_be(value, offset)?;
    Ok(offset + 8)
}

fn copy_index(value: impl IndexInput, name: &str) -> NodeResult<usize> {
    value.floor_index().ok_or_else(|| {
        NodeError::new(
            "ERR_OUT_OF_RANGE",
            format!("{name} is outside the representable buffer range"),
        )
    })
}

#[inline]
fn numeric_offset(
    value: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
) -> NodeResult<usize> {
    value
        .checked_integer()
        .ok_or_else(|| NodeError::new("ERR_OUT_OF_RANGE", "offset must be a non-negative integer"))
}

#[inline]
fn integer_value<Output>(
    value: impl tsonic_rust_runtime::conversions::IntegerInput<Output>,
) -> NodeResult<Output> {
    value.truncated_integer().ok_or_else(|| {
        NodeError::new(
            "ERR_OUT_OF_RANGE",
            "value is outside the representable integer range",
        )
    })
}
