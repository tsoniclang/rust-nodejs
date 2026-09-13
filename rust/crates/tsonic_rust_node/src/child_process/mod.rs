include!("types.rs");
include!("child.rs");
include!("commands.rs");
mod capture;
mod source_options;
pub use source_options::{spawn_sync_result_with_options, SpawnSyncOptions};
