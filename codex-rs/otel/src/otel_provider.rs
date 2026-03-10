//! Compatibility shim for `codex_otel::otel_provider`.

pub use crate::provider::*;
pub use crate::trace_context::traceparent_context_from_env;
