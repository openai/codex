//! Shared semantic observation API for Codex runtime facts.
//!
//! Observations describe facts that occurred in Codex. Destination-specific
//! systems such as analytics, rollout trace, OTEL events, and OTEL metrics
//! consume observations through sinks and reducers.
//!
//! Field metadata is required because sinks must decide whether they may read a
//! field before serializing or exporting it. Missing field annotations are a
//! compile-time error:
//!
//! ```compile_fail
//! use codex_observability::Observation;
//!
//! #[derive(Observation)]
//! #[observation(name = "example.missing_field_meta")]
//! struct MissingFieldMeta {
//!     field: &'static str,
//! }
//! ```
//!
//! Observation names are also required:
//!
//! ```compile_fail
//! use codex_observability::Observation;
//!
//! #[derive(Observation)]
//! struct MissingObservationName {
//!     #[obs(level = "basic", class = "operational")]
//!     field: &'static str,
//! }
//! ```

pub mod events;

pub use codex_observability_derive::Observation;
use serde::Serialize;

/// A runtime fact emitted by Codex.
///
/// Implementations visit every exported field together with its field metadata.
/// Sinks use that metadata to apply destination-specific policy before
/// serialization, storage, or export.
pub trait Observation {
    /// Stable semantic event name, for example `turn.started`.
    const NAME: &'static str;

    /// Visits the fields that make up this observation.
    fn visit_fields<V: ObservationFieldVisitor>(&self, visitor: &mut V);
}

/// Receives observation fields after policy metadata has been attached.
///
/// Implementations should inspect `meta` before serializing `value`. This keeps
/// remote sinks from accidentally materializing local-only content fields.
pub trait ObservationFieldVisitor {
    /// Visits one field from an observation.
    fn field<T: Serialize + ?Sized>(&mut self, name: &'static str, meta: FieldMeta, value: &T);
}

/// Consumes observations.
///
/// A sink may serialize immediately, reduce into another event shape, or fan
/// out to additional sinks. The trait is generic so callers can pass borrowed
/// typed observations without allocating an intermediate event object.
pub trait ObservationSink {
    /// Observes a single typed event.
    fn observe<E: Observation>(&self, event: &E);
}

/// Policy metadata attached to a single observation field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldMeta {
    /// How much detail a sink must be allowed to read before consuming the field.
    pub detail: DetailLevel,
    /// Semantic/privacy class for the field.
    pub class: DataClass,
}

impl FieldMeta {
    /// Creates metadata for a field.
    pub const fn new(detail: DetailLevel, class: DataClass) -> Self {
        Self { detail, class }
    }
}

/// Coarse detail level for a field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetailLevel {
    /// Lifecycle, identifiers, status, counts, model/config, and timing.
    Basic,
    /// Bounded previews and richer runtime summaries.
    Detailed,
    /// Raw or near-raw diagnostic evidence intended for local traces.
    Trace,
}

/// Semantic/privacy class for a field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataClass {
    /// Thread IDs, turn IDs, call IDs, and similar correlation identifiers.
    Identifier,
    /// Status, duration, model, provider, token counts, and tool kind.
    Operational,
    /// Working directory, repository, OS/runtime, or client metadata.
    Environment,
    /// User text, assistant text, command text, tool output, or model payloads.
    Content,
    /// Headers, environment values, auth-like payloads, or raw request blobs.
    SecretRisk,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn field_meta_preserves_detail_and_class() {
        assert_eq!(
            FieldMeta::new(DetailLevel::Trace, DataClass::Content),
            FieldMeta {
                detail: DetailLevel::Trace,
                class: DataClass::Content,
            }
        );
    }
}
