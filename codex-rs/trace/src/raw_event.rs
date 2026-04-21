//! Shared raw-event primitives used by the reduced state model.
//!
//! The local OTEL/tracing wire format remains intentionally untyped. This
//! module only gives `state.json` objects a documented sequence-number type.

/// Monotonic order assigned while replaying one trace event log.
pub(crate) type RawEventSeq = u64;
