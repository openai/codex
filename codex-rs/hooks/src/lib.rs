mod config_rules;
mod declarations;
mod engine;
pub(crate) mod events;
mod legacy_notify;
mod output_spill;
mod registry;
mod schema;
mod types;

use codex_protocol::protocol::HookEventName;

pub use config_rules::hook_states_from_stack;
pub use declarations::PluginHookDeclaration;
pub use declarations::plugin_hook_declarations;
pub use engine::HookListEntry;
/// Hook event names as they appear in hooks JSON and config files.
pub const HOOK_EVENT_NAMES: [&str; 8] = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
];

/// Hook event names whose matcher fields are meaningful during dispatch.
///
/// Other events can appear in hooks JSON, but Codex ignores their matcher
/// fields because those events do not dispatch against a tool, compaction
/// trigger, or session-start source.
pub const HOOK_EVENT_NAMES_WITH_MATCHERS: [&str; 6] = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "SessionStart",
];

pub use events::compact::PostCompactRequest;
pub use events::compact::PreCompactOutcome;
pub use events::compact::PreCompactRequest;
pub use events::compact::StatelessHookOutcome;
pub use events::permission_request::PermissionRequestDecision;
pub use events::permission_request::PermissionRequestOutcome;
pub use events::permission_request::PermissionRequestRequest;
pub use events::post_tool_use::PostToolUseOutcome;
pub use events::post_tool_use::PostToolUseRequest;
pub use events::pre_tool_use::PreToolUseOutcome;
pub use events::pre_tool_use::PreToolUseRequest;
pub use events::session_start::SessionStartOutcome;
pub use events::session_start::SessionStartRequest;
pub use events::session_start::SessionStartSource;
pub use events::stop::StopOutcome;
pub use events::stop::StopRequest;
pub use events::user_prompt_submit::UserPromptSubmitOutcome;
pub use events::user_prompt_submit::UserPromptSubmitRequest;
pub use legacy_notify::legacy_notify_json;
pub use legacy_notify::notify_hook;
pub use registry::HookListOutcome;
pub use registry::Hooks;
pub use registry::HooksConfig;
pub use registry::command_from_argv;
pub use registry::list_hooks;
pub use schema::write_schema_fixtures;
pub use types::Hook;
pub use types::HookEvent;
pub use types::HookEventAfterAgent;
pub use types::HookPayload;
pub use types::HookResponse;
pub use types::HookResult;

/// Returns the hook event label used in persisted hook-state keys.
pub fn hook_event_key_label(event_name: HookEventName) -> &'static str {
    match event_name {
        HookEventName::PreToolUse => "pre_tool_use",
        HookEventName::PermissionRequest => "permission_request",
        HookEventName::PostToolUse => "post_tool_use",
        HookEventName::PreCompact => "pre_compact",
        HookEventName::PostCompact => "post_compact",
        HookEventName::SessionStart => "session_start",
        HookEventName::UserPromptSubmit => "user_prompt_submit",
        HookEventName::Stop => "stop",
    }
}

/// Builds the persisted config-state key for one discovered hook handler.
pub fn hook_key(
    key_source: &str,
    event_name: HookEventName,
    group_index: usize,
    handler_index: usize,
) -> String {
    format!(
        "{key_source}:{}:{group_index}:{handler_index}",
        hook_event_key_label(event_name)
    )
}
