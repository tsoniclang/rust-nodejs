use crate::{NodeError, NodeResult};

#[derive(Clone, Debug)]
pub struct HeapInfo {
    pub total_heap_size: f64,
    pub total_heap_size_executable: f64,
    pub total_physical_size: f64,
    pub total_available_size: f64,
    pub used_heap_size: f64,
    pub heap_size_limit: f64,
    pub malloced_memory: f64,
    pub peak_malloced_memory: f64,
    pub does_zap_garbage: f64,
    pub number_of_native_contexts: f64,
    pub number_of_detached_contexts: f64,
    pub total_global_handles_size: f64,
    pub used_global_handles_size: f64,
    pub external_memory: f64,
    pub total_allocated_bytes: f64,
}

pub fn get_heap_statistics() -> NodeResult<HeapInfo> {
    Err(NodeError::new(
        "ERR_PLATFORM_NOT_SUPPORTED",
        "node:v8.getHeapStatistics requires a V8 engine; native programs do not host V8",
    ))
}

/// Rejects a V8 flag request without changing native process state.
pub fn set_flags_from_string(_flags: &str) -> NodeResult<()> {
    Err(NodeError::new(
        "ERR_PLATFORM_NOT_SUPPORTED",
        "node:v8.setFlagsFromString requires a V8 engine; native programs do not host V8",
    ))
}
