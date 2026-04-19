//! Canonical Codex observation event definitions.

use crate::Observation;
use serde::Serialize;

/// How an app/tool/plugin capability was selected by the user or system.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvocationType {
    /// The user explicitly mentioned or selected the capability.
    Explicit,
    /// Codex inferred that the capability should be used.
    Implicit,
}

/// Observation emitted when Codex uses an app connector during a turn.
#[derive(Observation)]
#[observation(name = "app.used", crate = "crate")]
pub struct AppUsed<'a> {
    /// Model slug active for the turn where the app was used.
    #[obs(level = "basic", class = "operational")]
    pub model_slug: &'a str,

    /// Thread that owns the turn.
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    /// Turn where the app was used.
    #[obs(level = "basic", class = "identifier")]
    pub turn_id: &'a str,

    /// Stable connector identifier when the app is backed by a connector.
    #[obs(level = "basic", class = "identifier")]
    pub connector_id: Option<&'a str>,

    /// User-facing app name.
    #[obs(level = "basic", class = "operational")]
    pub app_name: Option<&'a str>,

    /// Whether usage was explicit or implicit.
    #[obs(level = "basic", class = "operational")]
    pub invocation_type: Option<InvocationType>,
}
