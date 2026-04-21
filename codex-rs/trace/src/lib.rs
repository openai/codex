//! Lightweight local trace capture for Codex debugging.
//!
//! Runtime code emits ordinary `tracing` events. This crate only provides a
//! localhost debug layer, raw payload references, and a reducer that writes the
//! `state.json` shape consumed by the standalone trace viewer.

mod local;
mod model;
mod payload;
mod raw_event;
mod reduce;

pub use local::CODEX_TRACE_ROOT_ENV;
pub use local::LOCAL_TRACE_TARGET;
pub use local::RawPayloadRef;
pub use local::local_layer_from_env;
pub use local::next_id;
pub use local::write_payload;
pub use local::write_payload_lazy;
pub use reduce::REDUCED_STATE_FILE_NAME;
pub use reduce::reduce_bundle_to_path;
