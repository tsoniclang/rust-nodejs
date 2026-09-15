use crate::{NodeError, NodeResult};

pub fn set_flags_from_string(_flags: &str) -> NodeResult<()> {
    Err(NodeError::new(
        "ERR_PLATFORM_NOT_SUPPORTED",
        "node:v8.setFlagsFromString requires a V8 engine; native programs do not host V8",
    ))
}
