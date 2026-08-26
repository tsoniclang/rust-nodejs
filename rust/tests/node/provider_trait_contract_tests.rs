use tsonic_rust_node::{
    buffer::Buffer,
    child_process::SpawnSyncResult,
    crypto::{Hash, Hmac},
    fs::{MakeDirectoryOptions, RmOptions, Stats},
    http::{IncomingMessage, ServerHandle, ServerResponseHandle},
    process::{MemoryUsage, ProcessEnv, ProcessWriteStream},
    timers::Timeout,
    url::{LegacyUrlObject, Url, UrlSearchParams},
    util::TextDecoder,
    NodeError,
};

fn require_clone<T: Clone>() {}
fn require_copy<T: Copy>() {}
fn require_default<T: Default>() {}

#[test]
fn provider_trait_contracts_are_implemented_by_the_runtime_carriers() {
    require_clone::<Buffer>();
    require_clone::<Hash>();
    require_clone::<Hmac>();
    require_clone::<IncomingMessage>();
    require_clone::<ServerHandle>();
    require_clone::<ServerResponseHandle>();
    require_clone::<MakeDirectoryOptions>();
    require_clone::<MemoryUsage>();
    require_clone::<NodeError>();
    require_clone::<ProcessEnv>();
    require_clone::<ProcessWriteStream>();
    require_clone::<RmOptions>();
    require_clone::<SpawnSyncResult>();
    require_clone::<Stats>();
    require_clone::<TextDecoder>();
    require_clone::<Timeout>();
    require_clone::<Url>();
    require_clone::<LegacyUrlObject>();
    require_clone::<UrlSearchParams>();

    require_copy::<MakeDirectoryOptions>();
    require_copy::<ProcessEnv>();
    require_copy::<ProcessWriteStream>();
    require_copy::<RmOptions>();

    require_default::<MakeDirectoryOptions>();
    require_default::<ProcessEnv>();
    require_default::<RmOptions>();
    require_default::<UrlSearchParams>();
}
