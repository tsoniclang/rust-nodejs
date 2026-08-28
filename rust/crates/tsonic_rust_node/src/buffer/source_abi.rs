pub fn copy_open_number(
    source: &Buffer,
    target: &Buffer,
    target_start: f64,
    source_start: f64,
) -> NodeResult<f64> {
    copy_number(source, target, target_start, source_start, None)
}

pub fn copy_closed_number(
    source: &Buffer,
    target: &Buffer,
    target_start: f64,
    source_start: f64,
    source_end: f64,
) -> NodeResult<f64> {
    copy_number(source, target, target_start, source_start, Some(source_end))
}

fn copy_number(
    source: &Buffer,
    target: &Buffer,
    target_start: f64,
    source_start: f64,
    source_end: Option<f64>,
) -> NodeResult<f64> {
    let target_start = copy_index(target_start, "targetStart")?;
    let source_start = copy_index(source_start, "sourceStart")?;
    let source_end = source_end
        .map(|value| copy_index(value, "sourceEnd"))
        .transpose()?;
    if target_start >= target.len() {
        return Ok(0.0);
    }
    if source_start > source.len() {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "sourceStart is outside the source buffer",
        ));
    }
    Ok(source.copy(target, target_start, source_start, source_end)? as f64)
}

pub fn slice_open_number(buffer: &Buffer, start: f64) -> Buffer {
    view_number(buffer, start, None)
}

pub fn slice_closed_number(buffer: &Buffer, start: f64, end: f64) -> Buffer {
    view_number(buffer, start, Some(end))
}

fn view_number(buffer: &Buffer, start: f64, end: Option<f64>) -> Buffer {
    let start = relative_index(start, buffer.len());
    let end = end.map_or(buffer.len(), |value| relative_index(value, buffer.len()));
    let end = end.max(start);
    Buffer {
        storage: Rc::clone(&buffer.storage),
        offset: buffer.offset + start,
        len: end - start,
        identity: ObjectIdentity::new(),
    }
}

pub fn read_uint8_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_uint8(numeric_offset(offset)?)?))
}

pub fn read_int8_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_int8(numeric_offset(offset)?)?))
}

pub fn read_uint16_le_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_uint16_le(numeric_offset(offset)?)?))
}

pub fn read_uint16_be_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_uint16_be(numeric_offset(offset)?)?))
}

pub fn read_int16_le_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_int16_le(numeric_offset(offset)?)?))
}

pub fn read_int16_be_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_int16_be(numeric_offset(offset)?)?))
}

pub fn read_uint32_le_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_uint32_le(numeric_offset(offset)?)?))
}

pub fn read_uint32_be_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_uint32_be(numeric_offset(offset)?)?))
}

pub fn read_int32_le_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_int32_le(numeric_offset(offset)?)?))
}

pub fn read_int32_be_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_int32_be(numeric_offset(offset)?)?))
}

pub fn read_float_le_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_float_le(numeric_offset(offset)?)?))
}

pub fn read_float_be_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    Ok(f64::from(buffer.read_float_be(numeric_offset(offset)?)?))
}

pub fn read_double_le_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    buffer.read_double_le(numeric_offset(offset)?)
}

pub fn read_double_be_number(buffer: &Buffer, offset: f64) -> NodeResult<f64> {
    buffer.read_double_be(numeric_offset(offset)?)
}

pub fn write_uint8_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint8(unsigned_number::<u8>(value, u8::MAX as f64)?, offset)?;
    Ok((offset + 1) as f64)
}

pub fn write_int8_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_int8(signed_number::<i8>(value, i8::MIN as f64, i8::MAX as f64)?, offset)?;
    Ok((offset + 1) as f64)
}

pub fn write_uint16_le_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint16_le(unsigned_number::<u16>(value, u16::MAX as f64)?, offset)?;
    Ok((offset + 2) as f64)
}

pub fn write_uint16_be_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint16_be(unsigned_number::<u16>(value, u16::MAX as f64)?, offset)?;
    Ok((offset + 2) as f64)
}

pub fn write_int16_le_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_int16_le(signed_number::<i16>(value, i16::MIN as f64, i16::MAX as f64)?, offset)?;
    Ok((offset + 2) as f64)
}

pub fn write_int16_be_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_int16_be(signed_number::<i16>(value, i16::MIN as f64, i16::MAX as f64)?, offset)?;
    Ok((offset + 2) as f64)
}

pub fn write_uint32_le_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint32_le(unsigned_number::<u32>(value, u32::MAX as f64)?, offset)?;
    Ok((offset + 4) as f64)
}

pub fn write_uint32_be_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_uint32_be(unsigned_number::<u32>(value, u32::MAX as f64)?, offset)?;
    Ok((offset + 4) as f64)
}

pub fn write_int32_le_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_int32_le(signed_number::<i32>(value, i32::MIN as f64, i32::MAX as f64)?, offset)?;
    Ok((offset + 4) as f64)
}

pub fn write_int32_be_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_int32_be(signed_number::<i32>(value, i32::MIN as f64, i32::MAX as f64)?, offset)?;
    Ok((offset + 4) as f64)
}

pub fn write_float_le_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_float_le(value as f32, offset)?;
    Ok((offset + 4) as f64)
}

pub fn write_float_be_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_float_be(value as f32, offset)?;
    Ok((offset + 4) as f64)
}

pub fn write_double_le_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_double_le(value, offset)?;
    Ok((offset + 8) as f64)
}

pub fn write_double_be_number(buffer: &mut Buffer, value: f64, offset: f64) -> NodeResult<f64> {
    let offset = numeric_offset(offset)?;
    buffer.write_double_be(value, offset)?;
    Ok((offset + 8) as f64)
}

fn copy_index(value: f64, name: &str) -> NodeResult<usize> {
    let value = if value.is_finite() { value.floor() } else { 0.0 };
    if value < 0.0 || value > usize::MAX as f64 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            format!("{name} is outside the representable buffer range"),
        ));
    }
    Ok(value as usize)
}

fn relative_index(value: f64, len: usize) -> usize {
    if value.is_nan() || value == f64::NEG_INFINITY {
        return 0;
    }
    if value == f64::INFINITY {
        return len;
    }
    let value = value.trunc();
    if value < 0.0 {
        ((len as f64 + value).max(0.0).min(len as f64)) as usize
    } else {
        value.min(len as f64) as usize
    }
}

fn numeric_offset(value: f64) -> NodeResult<usize> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "offset must be a non-negative integer",
        ));
    }
    Ok(value as usize)
}

fn normalized_integer(value: f64, min: f64, max: f64) -> NodeResult<f64> {
    if value.is_nan() {
        return Ok(0.0);
    }
    if !value.is_finite() || value < min || value > max {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "value is outside the representable integer range",
        ));
    }
    Ok(value.trunc())
}

fn unsigned_number<T>(value: f64, max: f64) -> NodeResult<T>
where
    T: TryFrom<u64>,
{
    T::try_from(normalized_integer(value, 0.0, max)? as u64).map_err(|_| {
        NodeError::new(
            "ERR_OUT_OF_RANGE",
            "value is outside the representable unsigned integer range",
        )
    })
}

fn signed_number<T>(value: f64, min: f64, max: f64) -> NodeResult<T>
where
    T: TryFrom<i64>,
{
    T::try_from(normalized_integer(value, min, max)? as i64).map_err(|_| {
        NodeError::new(
            "ERR_OUT_OF_RANGE",
            "value is outside the representable signed integer range",
        )
    })
}
