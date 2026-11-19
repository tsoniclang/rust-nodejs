use super::{
    data_clone_error, validate_graph, ClonedContainer, ClonedSlot, ClonedValue, MAXIMUM_ENTRIES,
    MAXIMUM_STRING_UNITS,
};
use crate::error::NodeResult;
use std::collections::{BTreeSet, HashSet};
use tsonic_rust_js::JsString;

pub(super) const FORMAT_VERSION: u8 = 3;

pub(crate) fn encode(value: &ClonedValue) -> NodeResult<Vec<u8>> {
    validate_graph(value)?;
    let mut output = vec![FORMAT_VERSION];
    write_count(&mut output, value.containers.len())?;
    encode_slot(&value.root, &mut output)?;
    for container in &value.containers {
        match container {
            ClonedContainer::Object(entries) => {
                output.push(0);
                write_count(&mut output, entries.len())?;
                for (key, value) in entries {
                    write_string(&mut output, key)?;
                    encode_slot(value, &mut output)?;
                }
            }
            ClonedContainer::Array { length, entries } => {
                output.push(1);
                write_count(&mut output, *length)?;
                write_count(&mut output, entries.len())?;
                for (index, value) in entries {
                    write_count(&mut output, *index)?;
                    encode_slot(value, &mut output)?;
                }
            }
            ClonedContainer::Record(entries) => {
                output.push(2);
                write_count(&mut output, entries.len())?;
                for (key, value) in entries {
                    write_native_string(&mut output, key)?;
                    encode_slot(value, &mut output)?;
                }
            }
        }
    }
    Ok(output)
}

pub(crate) fn decode(input: &[u8]) -> NodeResult<ClonedValue> {
    let mut reader = Reader::new(input);
    if reader.byte()? != FORMAT_VERSION {
        return Err(data_clone_error(
            "structured-clone payload version is unsupported",
        ));
    }
    let container_count = reader.count()?;
    let root = reader.slot()?;
    let mut containers = Vec::with_capacity(container_count);
    let mut entries = container_count;
    for _ in 0..container_count {
        match reader.byte()? {
            0 => {
                let count = reader.count()?;
                reserve_decoded_entries(count, &mut entries)?;
                let mut values = Vec::with_capacity(count);
                let mut keys = HashSet::with_capacity(count);
                for _ in 0..count {
                    let key = reader.string()?;
                    if !keys.insert(key.clone()) {
                        return Err(data_clone_error(
                            "structured-clone object contains a duplicate key",
                        ));
                    }
                    values.push((key, reader.slot()?));
                }
                containers.push(ClonedContainer::Object(values));
            }
            1 => {
                let length = reader.count()?;
                reserve_decoded_entries(length, &mut entries)?;
                let count = reader.count()?;
                if count > length {
                    return Err(data_clone_error(
                        "structured-clone array has more entries than its length",
                    ));
                }
                let mut values = Vec::with_capacity(count);
                let mut indexes = BTreeSet::new();
                for _ in 0..count {
                    let index = reader.count()?;
                    if index >= length || !indexes.insert(index) {
                        return Err(data_clone_error("structured-clone array index is invalid"));
                    }
                    values.push((index, reader.slot()?));
                }
                containers.push(ClonedContainer::Array {
                    length,
                    entries: values,
                });
            }
            2 => {
                let count = reader.count()?;
                reserve_decoded_entries(count, &mut entries)?;
                let mut values = Vec::with_capacity(count);
                let mut keys = HashSet::with_capacity(count);
                for _ in 0..count {
                    let key = reader.native_string()?;
                    if !keys.insert(key.clone()) {
                        return Err(data_clone_error(
                            "structured-clone record contains a duplicate key",
                        ));
                    }
                    values.push((key, reader.slot()?));
                }
                containers.push(ClonedContainer::Record(values));
            }
            _ => {
                return Err(data_clone_error(
                    "structured-clone payload contains an unknown container tag",
                ));
            }
        }
    }
    if !reader.is_complete() {
        return Err(data_clone_error(
            "structured-clone payload contains trailing bytes",
        ));
    }
    let value = ClonedValue { root, containers };
    validate_graph(&value)?;
    Ok(value)
}

fn encode_slot(value: &ClonedSlot, output: &mut Vec<u8>) -> NodeResult<()> {
    match value {
        ClonedSlot::Null => output.push(1),
        ClonedSlot::Bool(false) => output.push(2),
        ClonedSlot::Bool(true) => output.push(3),
        ClonedSlot::Number(value) => {
            output.push(4);
            output.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        ClonedSlot::Integer(value) => {
            output.push(8);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::UnsignedInteger(value) => {
            output.push(9);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::Int8(value) => {
            output.push(10);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::Uint8(value) => {
            output.push(11);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::Int16(value) => {
            output.push(12);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::Uint16(value) => {
            output.push(13);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::Int32(value) => {
            output.push(14);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::Uint32(value) => {
            output.push(15);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::NativeInt(value) => {
            output.push(16);
            output.extend_from_slice(&(*value as i64).to_be_bytes());
        }
        ClonedSlot::NativeUint(value) => {
            output.push(17);
            output.extend_from_slice(&(*value as u64).to_be_bytes());
        }
        ClonedSlot::Float32(value) => {
            output.push(18);
            output.extend_from_slice(&value.to_be_bytes());
        }
        ClonedSlot::NativeString(value) => {
            output.push(7);
            write_native_string(output, value)?;
        }
        ClonedSlot::String(value) => {
            output.push(5);
            write_string(output, value)?;
        }
        ClonedSlot::Reference(index) => {
            output.push(6);
            write_count(output, *index)?;
        }
    }
    Ok(())
}

fn write_count(output: &mut Vec<u8>, value: usize) -> NodeResult<()> {
    let value = u32::try_from(value)
        .map_err(|_| data_clone_error("structured-clone count exceeds the finite limit"))?;
    output.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn write_string(output: &mut Vec<u8>, value: &JsString) -> NodeResult<()> {
    if value.len() > MAXIMUM_STRING_UNITS {
        return Err(data_clone_error(
            "structured-clone string exceeds the finite limit",
        ));
    }
    write_count(output, value.len())?;
    for unit in value.units() {
        output.extend_from_slice(&unit.to_be_bytes());
    }
    Ok(())
}

fn write_native_string(output: &mut Vec<u8>, value: &str) -> NodeResult<()> {
    if value.len() > MAXIMUM_STRING_UNITS {
        return Err(data_clone_error(
            "structured-clone native string exceeds the finite limit",
        ));
    }
    write_count(output, value.len())?;
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn reserve_decoded_entries(count: usize, entries: &mut usize) -> NodeResult<()> {
    *entries = entries
        .checked_add(count)
        .ok_or_else(|| data_clone_error("structured-clone entry count overflowed"))?;
    if *entries > MAXIMUM_ENTRIES {
        return Err(data_clone_error(
            "structured-clone entry count exceeds the finite limit",
        ));
    }
    Ok(())
}

struct Reader<'a> {
    input: &'a [u8],
    position: usize,
    string_units: usize,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            position: 0,
            string_units: 0,
        }
    }

    fn is_complete(&self) -> bool {
        self.position == self.input.len()
    }

    fn bytes(&mut self, count: usize) -> NodeResult<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| data_clone_error("structured-clone payload position overflowed"))?;
        if end > self.input.len() {
            return Err(data_clone_error("structured-clone payload is truncated"));
        }
        let result = &self.input[self.position..end];
        self.position = end;
        Ok(result)
    }

    fn byte(&mut self) -> NodeResult<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn u32(&mut self) -> NodeResult<u32> {
        let bytes: [u8; 4] = self.bytes(4)?.try_into().expect("exact byte count");
        Ok(u32::from_be_bytes(bytes))
    }

    fn u64(&mut self) -> NodeResult<u64> {
        let bytes: [u8; 8] = self.bytes(8)?.try_into().expect("exact byte count");
        Ok(u64::from_be_bytes(bytes))
    }

    fn count(&mut self) -> NodeResult<usize> {
        let value = usize::try_from(self.u32()?)
            .map_err(|_| data_clone_error("structured-clone count is not representable"))?;
        if value > MAXIMUM_ENTRIES {
            return Err(data_clone_error(
                "structured-clone count exceeds the finite limit",
            ));
        }
        Ok(value)
    }

    fn string(&mut self) -> NodeResult<JsString> {
        let count = self.count()?;
        self.string_units = self
            .string_units
            .checked_add(count)
            .ok_or_else(|| data_clone_error("structured-clone string budget overflowed"))?;
        if self.string_units > MAXIMUM_STRING_UNITS {
            return Err(data_clone_error(
                "structured-clone string budget exceeds the finite limit",
            ));
        }
        let mut units = Vec::with_capacity(count);
        for _ in 0..count {
            let bytes: [u8; 2] = self.bytes(2)?.try_into().expect("exact byte count");
            units.push(u16::from_be_bytes(bytes));
        }
        Ok(JsString::from_units(units))
    }

    fn native_string(&mut self) -> NodeResult<String> {
        let length = self.count()?;
        self.string_units = self
            .string_units
            .checked_add(length)
            .filter(|total| *total <= MAXIMUM_STRING_UNITS)
            .ok_or_else(|| {
                data_clone_error("structured-clone string budget exceeds the finite limit")
            })?;
        let text = std::str::from_utf8(self.bytes(length)?)
            .map_err(|_| data_clone_error("structured-clone native string is not UTF-8"))?;
        Ok(text.to_owned())
    }

    fn slot(&mut self) -> NodeResult<ClonedSlot> {
        match self.byte()? {
            1 => Ok(ClonedSlot::Null),
            2 => Ok(ClonedSlot::Bool(false)),
            3 => Ok(ClonedSlot::Bool(true)),
            4 => Ok(ClonedSlot::Number(f64::from_bits(self.u64()?))),
            8 => Ok(ClonedSlot::Integer(i64::from_be_bytes(
                self.bytes(8)?.try_into().expect("exact byte count"),
            ))),
            9 => Ok(ClonedSlot::UnsignedInteger(self.u64()?)),
            10 => Ok(ClonedSlot::Int8(i8::from_be_bytes(
                self.bytes(1)?.try_into().expect("exact byte count"),
            ))),
            11 => Ok(ClonedSlot::Uint8(u8::from_be_bytes(
                self.bytes(1)?.try_into().expect("exact byte count"),
            ))),
            12 => Ok(ClonedSlot::Int16(i16::from_be_bytes(
                self.bytes(2)?.try_into().expect("exact byte count"),
            ))),
            13 => Ok(ClonedSlot::Uint16(u16::from_be_bytes(
                self.bytes(2)?.try_into().expect("exact byte count"),
            ))),
            14 => Ok(ClonedSlot::Int32(i32::from_be_bytes(
                self.bytes(4)?.try_into().expect("exact byte count"),
            ))),
            15 => Ok(ClonedSlot::Uint32(u32::from_be_bytes(
                self.bytes(4)?.try_into().expect("exact byte count"),
            ))),
            16 => Ok(ClonedSlot::NativeInt(
                isize::try_from(i64::from_be_bytes(
                    self.bytes(8)?.try_into().expect("exact byte count"),
                ))
                .map_err(|_| data_clone_error("native signed integer is not representable"))?,
            )),
            17 => Ok(ClonedSlot::NativeUint(
                usize::try_from(self.u64()?).map_err(|_| {
                    data_clone_error("native unsigned integer is not representable")
                })?,
            )),
            18 => Ok(ClonedSlot::Float32(f32::from_be_bytes(
                self.bytes(4)?.try_into().expect("exact byte count"),
            ))),
            5 => Ok(ClonedSlot::String(self.string()?)),
            6 => Ok(ClonedSlot::Reference(self.count()?)),
            7 => Ok(ClonedSlot::NativeString(self.native_string()?)),
            _ => Err(data_clone_error(
                "structured-clone payload contains an unknown value tag",
            )),
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/node/structured_clone_wire_tests.rs"]
mod tests;
