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

/// Visits fields intended for one sink and allowed by the supplied policy.
///
/// This helper is the safe path for sinks that serialize fields in their
/// visitor. The explicit field-use marker selects the intended fields; the
/// policy gate then rejects unsafe metadata before the wrapped visitor can
/// materialize the value.
pub fn visit_fields_for_use<E, V>(
    event: &E,
    field_use: FieldUse,
    policy: FieldPolicy,
    visitor: &mut V,
) where
    E: Observation,
    V: ObservationFieldVisitor,
{
    let mut visitor = PolicyFilteredVisitor {
        field_use,
        policy,
        inner: visitor,
    };
    event.visit_fields(&mut visitor);
}

struct PolicyFilteredVisitor<'a, V> {
    field_use: FieldUse,
    policy: FieldPolicy,
    inner: &'a mut V,
}

impl<V> ObservationFieldVisitor for PolicyFilteredVisitor<'_, V>
where
    V: ObservationFieldVisitor,
{
    fn field<T: Serialize + ?Sized>(&mut self, name: &'static str, meta: FieldMeta, value: &T) {
        if meta.is_used_by(self.field_use) && self.policy.allows(meta) {
            self.inner.field(name, meta, value);
        }
    }
}

/// Policy metadata attached to a single observation field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldMeta {
    /// How much detail a sink must be allowed to read before consuming the field.
    pub detail: DetailLevel,
    /// Semantic/privacy class for the field.
    pub class: DataClass,
    /// Exact sinks or projections that are intended to consume the field.
    pub uses: &'static [FieldUse],
}

impl FieldMeta {
    /// Creates metadata for a field that is not consumed by any exact sink.
    pub const fn new(detail: DetailLevel, class: DataClass) -> Self {
        Self {
            detail,
            class,
            uses: &[],
        }
    }

    /// Creates metadata for a field with explicit sink-use markers.
    pub const fn with_uses(
        detail: DetailLevel,
        class: DataClass,
        uses: &'static [FieldUse],
    ) -> Self {
        Self {
            detail,
            class,
            uses,
        }
    }

    /// Returns true when the field was explicitly marked for `field_use`.
    pub fn is_used_by(self, field_use: FieldUse) -> bool {
        self.uses.contains(&field_use)
    }
}

/// Decides whether a sink may read an observation field.
///
/// Policies are checked before serialization. This matters because denied
/// fields may contain content, secrets, or large trace payloads that remote
/// sinks must not materialize even transiently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldPolicy {
    max_detail: DetailLevel,
    allowed_classes: &'static [DataClass],
}

impl FieldPolicy {
    /// Creates a policy that permits fields at or below the configured detail
    /// limit and whose data class is present in the allowed class list.
    pub const fn new(max_detail: DetailLevel, allowed_classes: &'static [DataClass]) -> Self {
        Self {
            max_detail,
            allowed_classes,
        }
    }

    /// Returns true when a sink may inspect and serialize a field.
    pub fn allows(self, meta: FieldMeta) -> bool {
        let detail_allowed = match self.max_detail {
            DetailLevel::Basic => matches!(meta.detail, DetailLevel::Basic),
            DetailLevel::Detailed => {
                matches!(meta.detail, DetailLevel::Basic | DetailLevel::Detailed)
            }
            DetailLevel::Trace => true,
        };

        detail_allowed && self.allowed_classes.contains(&meta.class)
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

/// Exact sink or projection that is intended to consume a field.
///
/// This marker is separate from `DetailLevel` and `DataClass`: it expresses
/// intent, while detail/class remain guardrails that a sink policy enforces
/// before it serializes or exports the selected field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldUse {
    /// Remote product analytics.
    Analytics,
    /// OpenTelemetry events, logs, or metrics.
    Otel,
    /// Local rollout trace bundles.
    RolloutTrace,
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
                uses: &[],
            }
        );
    }

    #[test]
    fn field_policy_requires_allowed_detail_and_class() {
        let policy = FieldPolicy::new(
            DetailLevel::Basic,
            &[DataClass::Identifier, DataClass::Operational],
        );
        let cases = [
            (
                FieldMeta::new(DetailLevel::Basic, DataClass::Identifier),
                true,
            ),
            (
                FieldMeta::new(DetailLevel::Basic, DataClass::Operational),
                true,
            ),
            (
                FieldMeta::new(DetailLevel::Detailed, DataClass::Operational),
                false,
            ),
            (
                FieldMeta::new(DetailLevel::Basic, DataClass::Content),
                false,
            ),
            (
                FieldMeta::new(DetailLevel::Basic, DataClass::SecretRisk),
                false,
            ),
        ];

        assert_eq!(
            cases.map(|(meta, _expected)| policy.allows(meta)),
            cases.map(|(_meta, expected)| expected)
        );
    }
}
