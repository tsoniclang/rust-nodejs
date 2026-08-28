use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::rc::Rc;

use tsonic_rust_js::{JsArray, JsObject, JsString, JsValue};

use crate::error::{NodeError, NodeResult};

const FORMAT_VERSION: u8 = 1;
const MAXIMUM_DEPTH: usize = 128;
const MAXIMUM_ENTRIES: usize = 1 << 20;
const MAXIMUM_STRING_UNITS: usize = 1 << 24;

#[derive(Debug, Clone, PartialEq)]
pub struct ClonedValue {
    root: ClonedSlot,
    containers: Vec<ClonedContainer>,
}

#[derive(Debug, Clone, PartialEq)]
enum ClonedSlot {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(JsString),
    Reference(usize),
}

#[derive(Debug, Clone, PartialEq)]
enum ClonedContainer {
    Object(Vec<(JsString, ClonedSlot)>),
    Array {
        length: usize,
        entries: Vec<(usize, ClonedSlot)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceIdentity {
    Object(usize),
    Array(usize),
}

impl ClonedValue {
    pub fn from_js(value: &JsValue) -> NodeResult<Self> {
        let mut state = EncodingState::default();
        let root = clone_slot(value, 0, &mut state)?;
        let value = Self {
            root,
            containers: state.containers,
        };
        validate_graph(&value)?;
        Ok(value)
    }

    pub fn to_js(&self) -> JsValue {
        let containers = self
            .containers
            .iter()
            .map(|container| match container {
                ClonedContainer::Object(_) => JsValue::object(JsObject::new()),
                ClonedContainer::Array { length, .. } => {
                    JsValue::array(JsArray::with_length(*length))
                }
            })
            .collect::<Vec<_>>();

        for (index, container) in self.containers.iter().enumerate() {
            match container {
                ClonedContainer::Object(entries) => {
                    let object = containers[index]
                        .as_object()
                        .expect("validated structured-clone object");
                    let mut object = object.borrow_mut();
                    for (key, value) in entries {
                        object.set_exact(key.clone(), materialize_slot(value, &containers));
                    }
                }
                ClonedContainer::Array { entries, .. } => {
                    let array = containers[index]
                        .as_array()
                        .expect("validated structured-clone array");
                    for (entry_index, value) in entries {
                        array.set(*entry_index, materialize_slot(value, &containers));
                    }
                }
            }
        }

        materialize_slot(&self.root, &containers)
    }
}

fn clone_slot(
    value: &JsValue,
    depth: usize,
    state: &mut EncodingState,
) -> NodeResult<ClonedSlot> {
    if depth > MAXIMUM_DEPTH {
        return Err(data_clone_error("structured-clone depth exceeds the finite limit"));
    }
    match value {
        JsValue::Undefined => Ok(ClonedSlot::Undefined),
        JsValue::Null => Ok(ClonedSlot::Null),
        JsValue::Bool(value) => Ok(ClonedSlot::Bool(*value)),
        JsValue::Number(value) => Ok(ClonedSlot::Number(*value)),
        JsValue::String(value) => {
            reserve_string(value, state)?;
            Ok(ClonedSlot::String(value.clone()))
        }
        JsValue::Symbol(_) | JsValue::JsonProjection(_) => {
            Err(data_clone_error("value cannot be structured-cloned"))
        }
        JsValue::Object(object) => {
            let identity = SourceIdentity::Object(Rc::as_ptr(object) as usize);
            if let Some(index) = state.identities.get(&identity) {
                return Ok(ClonedSlot::Reference(*index));
            }
            reserve_entries(1, state)?;
            let index = state.containers.len();
            state.identities.insert(identity, index);
            state.containers.push(ClonedContainer::Object(Vec::new()));

            let source = object.try_borrow().map_err(|_| {
                data_clone_error("object is mutably borrowed during structured clone")
            })?;
            let source_entries = source.entries_exact();
            reserve_entries(source_entries.len(), state)?;
            let mut entries = Vec::with_capacity(source_entries.len());
            for (key, entry) in source_entries {
                reserve_string(&key, state)?;
                entries.push((key, clone_slot(&entry, depth + 1, state)?));
            }
            state.containers[index] = ClonedContainer::Object(entries);
            Ok(ClonedSlot::Reference(index))
        }
        JsValue::Array(values) => {
            let identity = SourceIdentity::Array(values.identity());
            if let Some(index) = state.identities.get(&identity) {
                return Ok(ClonedSlot::Reference(*index));
            }
            let length = values.len();
            reserve_entries(length.checked_add(1).ok_or_else(|| {
                data_clone_error("structured-clone array length overflowed")
            })?, state)?;
            let index = state.containers.len();
            state.identities.insert(identity, index);
            state.containers.push(ClonedContainer::Array {
                length,
                entries: Vec::new(),
            });

            let mut entries = Vec::new();
            for (entry_index, entry) in values.entries() {
                if let Some(entry) = entry {
                    entries.push((entry_index, clone_slot(&entry, depth + 1, state)?));
                }
            }
            state.containers[index] = ClonedContainer::Array { length, entries };
            Ok(ClonedSlot::Reference(index))
        }
    }
}

fn materialize_slot(value: &ClonedSlot, containers: &[JsValue]) -> JsValue {
    match value {
        ClonedSlot::Undefined => JsValue::Undefined,
        ClonedSlot::Null => JsValue::Null,
        ClonedSlot::Bool(value) => JsValue::Bool(*value),
        ClonedSlot::Number(value) => JsValue::Number(*value),
        ClonedSlot::String(value) => JsValue::String(value.clone()),
        ClonedSlot::Reference(index) => containers[*index].clone(),
    }
}

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
        }
    }
    Ok(output)
}

pub(crate) fn decode(input: &[u8]) -> NodeResult<ClonedValue> {
    let mut reader = Reader::new(input);
    if reader.byte()? != FORMAT_VERSION {
        return Err(data_clone_error("structured-clone payload version is unsupported"));
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
                        return Err(data_clone_error(
                            "structured-clone array index is invalid",
                        ));
                    }
                    values.push((index, reader.slot()?));
                }
                containers.push(ClonedContainer::Array {
                    length,
                    entries: values,
                });
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
        ClonedSlot::Undefined => output.push(0),
        ClonedSlot::Null => output.push(1),
        ClonedSlot::Bool(false) => output.push(2),
        ClonedSlot::Bool(true) => output.push(3),
        ClonedSlot::Number(value) => {
            output.push(4);
            output.extend_from_slice(&value.to_bits().to_be_bytes());
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

fn validate_graph(value: &ClonedValue) -> NodeResult<()> {
    let container_count = value.containers.len();
    if container_count > MAXIMUM_ENTRIES {
        return Err(data_clone_error(
            "structured-clone container count exceeds the finite limit",
        ));
    }
    let mut pending = VecDeque::new();
    collect_reference(&value.root, container_count, &mut pending)?;
    let mut reachable = BTreeSet::new();
    while let Some(index) = pending.pop_front() {
        if !reachable.insert(index) {
            continue;
        }
        match &value.containers[index] {
            ClonedContainer::Object(entries) => {
                for (_, slot) in entries {
                    collect_reference(slot, container_count, &mut pending)?;
                }
            }
            ClonedContainer::Array { length, entries } => {
                if *length > MAXIMUM_ENTRIES || entries.len() > *length {
                    return Err(data_clone_error(
                        "structured-clone array length exceeds the finite limit",
                    ));
                }
                let mut indexes = BTreeSet::new();
                for (entry_index, slot) in entries {
                    if *entry_index >= *length || !indexes.insert(*entry_index) {
                        return Err(data_clone_error(
                            "structured-clone array index is invalid",
                        ));
                    }
                    collect_reference(slot, container_count, &mut pending)?;
                }
            }
        }
    }
    if reachable.len() != container_count {
        return Err(data_clone_error(
            "structured-clone payload contains an unreachable container",
        ));
    }
    Ok(())
}

fn collect_reference(
    value: &ClonedSlot,
    container_count: usize,
    pending: &mut VecDeque<usize>,
) -> NodeResult<()> {
    if let ClonedSlot::Reference(index) = value {
        if *index >= container_count {
            return Err(data_clone_error(
                "structured-clone reference is outside the container table",
            ));
        }
        pending.push_back(*index);
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

fn reserve_entries(count: usize, state: &mut EncodingState) -> NodeResult<()> {
    state.entries = state
        .entries
        .checked_add(count)
        .ok_or_else(|| data_clone_error("structured-clone entry count overflowed"))?;
    if state.entries > MAXIMUM_ENTRIES {
        return Err(data_clone_error(
            "structured-clone entry count exceeds the finite limit",
        ));
    }
    Ok(())
}

fn reserve_string(value: &JsString, state: &mut EncodingState) -> NodeResult<()> {
    state.string_units = state
        .string_units
        .checked_add(value.len())
        .ok_or_else(|| data_clone_error("structured-clone string budget overflowed"))?;
    if state.string_units > MAXIMUM_STRING_UNITS {
        return Err(data_clone_error(
            "structured-clone string budget exceeds the finite limit",
        ));
    }
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

fn data_clone_error(message: &str) -> NodeError {
    NodeError::new("DATA_CLONE_ERR", message)
}

#[derive(Default)]
struct EncodingState {
    identities: BTreeMap<SourceIdentity, usize>,
    containers: Vec<ClonedContainer>,
    entries: usize,
    string_units: usize,
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
        let value = usize::try_from(self.u32()).expect("u32 must fit usize");
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

    fn slot(&mut self) -> NodeResult<ClonedSlot> {
        match self.byte()? {
            0 => Ok(ClonedSlot::Undefined),
            1 => Ok(ClonedSlot::Null),
            2 => Ok(ClonedSlot::Bool(false)),
            3 => Ok(ClonedSlot::Bool(true)),
            4 => Ok(ClonedSlot::Number(f64::from_bits(self.u64()?))),
            5 => Ok(ClonedSlot::String(self.string()?)),
            6 => Ok(ClonedSlot::Reference(self.count()?)),
            _ => Err(data_clone_error(
                "structured-clone payload contains an unknown value tag",
            )),
        }
    }
}
