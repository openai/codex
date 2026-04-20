//! Canonical Codex observation event definitions.

use crate::Observation;
use serde::Serialize;

mod compaction;
mod review;
mod thread;
mod turn;

pub use compaction::*;
pub use review::*;
pub use thread::*;
pub use turn::*;

/// How an app/tool/plugin capability was selected by the user or system.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvocationType {
    /// The user explicitly mentioned or selected the capability.
    Explicit,
    /// Codex inferred that the capability should be used.
    Implicit,
}

/// Status reported after a hook run reaches a terminal state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookRunStatus {
    /// The hook completed successfully.
    Completed,
    /// The hook failed.
    Failed,
    /// The hook blocked the triggering action.
    Blocked,
    /// The hook stopped execution.
    Stopped,
}

/// Plugin lifecycle state after a plugin management operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Plugin was installed.
    Installed,
    /// Plugin was uninstalled.
    Uninstalled,
    /// Plugin was enabled.
    Enabled,
    /// Plugin was disabled.
    Disabled,
}

/// Observation emitted when an app connector is mentioned during a turn.
#[derive(Observation)]
#[observation(name = "app.mentioned", crate = "crate", uses = ["analytics"])]
pub struct AppMentioned<'a> {
    /// Model slug active for the turn where the app was mentioned.
    #[obs(level = "basic", class = "operational")]
    pub model_slug: &'a str,

    /// Thread that owns the turn.
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    /// Turn where the app was mentioned.
    #[obs(level = "basic", class = "identifier")]
    pub turn_id: &'a str,

    /// Stable connector identifier when the app is backed by a connector.
    #[obs(level = "basic", class = "identifier")]
    pub connector_id: Option<&'a str>,

    /// User-facing app name.
    #[obs(level = "basic", class = "operational")]
    pub app_name: Option<&'a str>,

    /// Whether the mention was explicit or inferred.
    #[obs(level = "basic", class = "operational")]
    pub invocation_type: Option<InvocationType>,
}

/// Observation emitted when Codex uses an app connector during a turn.
#[derive(Observation)]
#[observation(name = "app.used", crate = "crate", uses = ["analytics"])]
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

/// Observation emitted after a configured hook run completes.
#[derive(Observation)]
#[observation(name = "hook.run_completed", crate = "crate", uses = ["analytics"])]
pub struct HookRunCompleted<'a> {
    /// Model slug active for the turn where the hook ran.
    #[obs(level = "basic", class = "operational")]
    pub model_slug: &'a str,

    /// Thread that owns the turn.
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    /// Turn where the hook ran.
    #[obs(level = "basic", class = "identifier")]
    pub turn_id: &'a str,

    /// Hook lifecycle point, for example PostToolUse.
    #[obs(level = "basic", class = "operational")]
    pub hook_name: &'a str,

    /// Source that configured the hook, for example user or project.
    #[obs(level = "basic", class = "operational")]
    pub hook_source: &'static str,

    /// Final hook run status.
    #[obs(level = "basic", class = "operational")]
    pub status: HookRunStatus,
}

/// Observation emitted when a plugin capability is used during a turn.
#[derive(Observation)]
#[observation(name = "plugin.used", crate = "crate", uses = ["analytics"])]
pub struct PluginUsed<'a> {
    /// Model slug active for the turn where the plugin was used.
    #[obs(level = "basic", class = "operational")]
    pub model_slug: &'a str,

    /// Thread that owns the turn.
    #[obs(level = "basic", class = "identifier")]
    pub thread_id: &'a str,

    /// Turn where the plugin was used.
    #[obs(level = "basic", class = "identifier")]
    pub turn_id: &'a str,

    /// Stable plugin identifier.
    #[obs(level = "basic", class = "identifier")]
    pub plugin_id: &'a str,

    /// Plugin name component.
    #[obs(level = "basic", class = "operational")]
    pub plugin_name: &'a str,

    /// Marketplace or namespace component.
    #[obs(level = "basic", class = "operational")]
    pub marketplace_name: &'a str,

    /// Whether the plugin exposes skills.
    #[obs(level = "basic", class = "operational")]
    pub has_skills: Option<bool>,

    /// Number of MCP servers exposed by the plugin.
    #[obs(level = "basic", class = "operational")]
    pub mcp_server_count: Option<usize>,

    /// Connector identifiers exposed by the plugin.
    #[obs(level = "basic", class = "identifier")]
    pub connector_ids: Option<&'a [String]>,
}

/// Observation emitted after a plugin lifecycle state changes.
#[derive(Observation)]
#[observation(name = "plugin.state_changed", crate = "crate", uses = ["analytics"])]
pub struct PluginStateChanged<'a> {
    /// Stable plugin identifier.
    #[obs(level = "basic", class = "identifier")]
    pub plugin_id: &'a str,

    /// Plugin name component.
    #[obs(level = "basic", class = "operational")]
    pub plugin_name: &'a str,

    /// Marketplace or namespace component.
    #[obs(level = "basic", class = "operational")]
    pub marketplace_name: &'a str,

    /// Whether the plugin exposes skills.
    #[obs(level = "basic", class = "operational")]
    pub has_skills: Option<bool>,

    /// Number of MCP servers exposed by the plugin.
    #[obs(level = "basic", class = "operational")]
    pub mcp_server_count: Option<usize>,

    /// Connector identifiers exposed by the plugin.
    #[obs(level = "basic", class = "identifier")]
    pub connector_ids: Option<&'a [String]>,

    /// New plugin lifecycle state.
    #[obs(level = "basic", class = "operational")]
    pub state: PluginState,
}
