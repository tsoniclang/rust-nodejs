use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

use tsonic_rust_js::{JsArray, JsObject, JsString, JsValue};

use crate::error::{NodeError, NodeResult};

mod wire;
pub(crate) use wire::{decode, encode};
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
    Null,
    Bool(bool),
    Number(f64),
    Integer(i64),
    UnsignedInteger(u64),
    Int8(i8),
    Uint8(u8),
    Int16(i16),
    Uint16(u16),
    Int32(i32),
    Uint32(u32),
    NativeInt(isize),
    NativeUint(usize),
    Float32(f32),
    NativeString(String),
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

fn clone_slot(value: &JsValue, depth: usize, state: &mut EncodingState) -> NodeResult<ClonedSlot> {
    if depth > MAXIMUM_DEPTH {
        return Err(data_clone_error(
            "structured-clone depth exceeds the finite limit",
        ));
    }
    match value {
        JsValue::Null => Ok(ClonedSlot::Null),
        JsValue::Bool(value) => Ok(ClonedSlot::Bool(*value)),
        JsValue::Number(value) => Ok(ClonedSlot::Number(*value)),
        JsValue::Integer(value) => Ok(ClonedSlot::Integer(*value)),
        JsValue::UnsignedInteger(value) => Ok(ClonedSlot::UnsignedInteger(*value)),
        JsValue::Int8(value) => Ok(ClonedSlot::Int8(*value)),
        JsValue::Uint8(value) => Ok(ClonedSlot::Uint8(*value)),
        JsValue::Int16(value) => Ok(ClonedSlot::Int16(*value)),
        JsValue::Uint16(value) => Ok(ClonedSlot::Uint16(*value)),
        JsValue::Int32(value) => Ok(ClonedSlot::Int32(*value)),
        JsValue::Uint32(value) => Ok(ClonedSlot::Uint32(*value)),
        JsValue::NativeInt(value) => Ok(ClonedSlot::NativeInt(*value)),
        JsValue::NativeUint(value) => Ok(ClonedSlot::NativeUint(*value)),
        JsValue::Float32(value) => Ok(ClonedSlot::Float32(*value)),
        JsValue::String(value) => {
            reserve_native_string(value.len(), state)?;
            Ok(ClonedSlot::NativeString(value.clone()))
        }
        JsValue::Utf16String(value) => {
            reserve_string(value, state)?;
            Ok(ClonedSlot::String(value.clone()))
        }
        JsValue::Symbol(_) | JsValue::Closed(_) | JsValue::JsonProjection(_) => {
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
            reserve_entries(
                length
                    .checked_add(1)
                    .ok_or_else(|| data_clone_error("structured-clone array length overflowed"))?,
                state,
            )?;
            let index = state.containers.len();
            state.identities.insert(identity, index);
            state.containers.push(ClonedContainer::Array {
                length,
                entries: Vec::new(),
            });

            let mut entries = Vec::new();
            for (entry_index, (_, entry)) in values.entries().enumerate() {
                entries.push((entry_index, clone_slot(&entry, depth + 1, state)?));
            }
            state.containers[index] = ClonedContainer::Array { length, entries };
            Ok(ClonedSlot::Reference(index))
        }
    }
}

fn materialize_slot(value: &ClonedSlot, containers: &[JsValue]) -> JsValue {
    match value {
        ClonedSlot::Null => JsValue::Null,
        ClonedSlot::Bool(value) => JsValue::Bool(*value),
        ClonedSlot::Number(value) => JsValue::Number(*value),
        ClonedSlot::Integer(value) => JsValue::Integer(*value),
        ClonedSlot::UnsignedInteger(value) => JsValue::UnsignedInteger(*value),
        ClonedSlot::Int8(value) => JsValue::Int8(*value),
        ClonedSlot::Uint8(value) => JsValue::Uint8(*value),
        ClonedSlot::Int16(value) => JsValue::Int16(*value),
        ClonedSlot::Uint16(value) => JsValue::Uint16(*value),
        ClonedSlot::Int32(value) => JsValue::Int32(*value),
        ClonedSlot::Uint32(value) => JsValue::Uint32(*value),
        ClonedSlot::NativeInt(value) => JsValue::NativeInt(*value),
        ClonedSlot::NativeUint(value) => JsValue::NativeUint(*value),
        ClonedSlot::Float32(value) => JsValue::Float32(*value),
        ClonedSlot::NativeString(value) => JsValue::String(value.clone()),
        ClonedSlot::String(value) => JsValue::Utf16String(value.clone()),
        ClonedSlot::Reference(index) => containers[*index].clone(),
    }
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
                        return Err(data_clone_error("structured-clone array index is invalid"));
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
    reserve_native_string(value.len(), state)
}

fn reserve_native_string(length: usize, state: &mut EncodingState) -> NodeResult<()> {
    state.string_units = state
        .string_units
        .checked_add(length)
        .ok_or_else(|| data_clone_error("structured-clone string budget overflowed"))?;
    if state.string_units > MAXIMUM_STRING_UNITS {
        return Err(data_clone_error(
            "structured-clone string budget exceeds the finite limit",
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
