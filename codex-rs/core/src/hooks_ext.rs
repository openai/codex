//! Hook system integration for core.
//!
//! This module provides the integration layer between codex-core and codex-hooks,
//! handling hook initialization from configuration and providing helper functions
//! for running hooks at various points in the application lifecycle.
//!
//! ## Configuration
//!
//! Hooks are configured via `hooks.json` files in priority order:
//! 1. Project: `.codex/hooks.json`
//! 2. User: `~/.codex/hooks.json`
//!
//! See `codex_hooks::config::HooksJsonConfig` for the JSON format.

use std::path::Path;
use std::sync::Arc;

use codex_hooks::HookConfig;
use codex_hooks::HookContext;
use codex_hooks::HookEventData;
use codex_hooks::HookEventType;
use codex_hooks::HookExecutionResult;
use codex_hooks::HookExecutor;
use codex_hooks::HookMatcher;
use codex_hooks::HookType;
use codex_hooks::build_from_json_config;
use once_cell::sync::OnceCell;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::warn;

/// Global hook executor instance.
static HOOK_EXECUTOR: OnceCell<Arc<HookExecutor>> = OnceCell::new();

/// Initialize the hook system from JSON configuration.
///
/// This should be called once at application startup with the working directory.
/// Loads configuration from `.codex/hooks.json` (project) or `~/.codex/hooks.json` (user).
/// Subsequent calls will be ignored.
///
/// Returns `Ok(true)` if hooks were initialized, `Ok(false)` if already initialized or disabled.
pub fn init_hooks(cwd: &Path) -> Result<bool, codex_hooks::HookError> {
    // Load config from JSON files
    let config = codex_hooks::loader::load_hooks_config(cwd)?;

    if config.is_disabled() {
        debug!("Hooks are disabled via configuration");
        return Ok(false);
    }

    if config.hooks.is_empty() {
        debug!("No hooks configured");
        return Ok(false);
    }

    // Build registry and get shell prefix from JSON config
    let (registry, shell_prefix) = build_from_json_config(&config);

    // Create executor with shell prefix support
    let executor = HookExecutor::with_shell_prefix(Arc::new(registry), shell_prefix);

    if HOOK_EXECUTOR.set(Arc::new(executor)).is_err() {
        debug!("Hook executor already initialized");
        return Ok(false);
    }

    debug!("Hooks initialized from JSON configuration");
    Ok(true)
}

/// Initialize hooks from a pre-loaded JSON config.
///
/// Use this when you already have the config loaded (e.g., for testing).
pub fn init_hooks_from_config(
    config: &codex_hooks::config::HooksJsonConfig,
) -> Result<bool, codex_hooks::HookError> {
    if config.is_disabled() {
        debug!("Hooks are disabled via configuration");
        return Ok(false);
    }

    if config.hooks.is_empty() {
        debug!("No hooks configured");
        return Ok(false);
    }

    let (registry, shell_prefix) = build_from_json_config(config);
    let executor = HookExecutor::with_shell_prefix(Arc::new(registry), shell_prefix);

    if HOOK_EXECUTOR.set(Arc::new(executor)).is_err() {
        debug!("Hook executor already initialized");
        return Ok(false);
    }

    debug!("Hooks initialized from provided config");
    Ok(true)
}

/// Try to initialize hooks, logging any errors instead of propagating them.
///
/// This is useful for startup paths where hook initialization failure should not
/// prevent the application from running.
pub fn try_init_hooks(cwd: &Path) {
    match init_hooks(cwd) {
        Ok(true) => debug!("Hooks initialized successfully"),
        Ok(false) => debug!("Hooks not initialized (disabled or none configured)"),
        Err(e) => warn!("Failed to initialize hooks: {}", e),
    }
}

/// Get the global hook executor.
pub fn get_executor() -> Option<&'static Arc<HookExecutor>> {
    HOOK_EXECUTOR.get()
}

/// Check if hooks are enabled and initialized.
pub fn hooks_enabled() -> bool {
    HOOK_EXECUTOR.get().is_some()
}

/// Run PreToolUse hooks.
///
/// Returns the execution result containing permission decisions and any updated input.
pub async fn run_pre_tool_use_hooks(
    tool_name: &str,
    tool_input: &serde_json::Value,
    tool_use_id: &str,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::pre_tool_use(
        tool_name.to_string(),
        tool_input.clone(),
        tool_use_id.to_string(),
    );

    let result = executor
        .run_hooks(
            HookEventType::PreToolUse,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run PostToolUse hooks.
///
/// Returns any additional context to inject into the conversation.
pub async fn run_post_tool_use_hooks(
    tool_name: &str,
    tool_input: &serde_json::Value,
    tool_use_id: &str,
    tool_response: &str,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::post_tool_use(
        tool_name.to_string(),
        tool_input.clone(),
        tool_use_id.to_string(),
        tool_response.to_string(),
    );

    let result = executor
        .run_hooks(
            HookEventType::PostToolUse,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run PostToolUseFailure hooks.
///
/// Called when a tool execution fails. Returns any additional context to inject.
pub async fn run_post_tool_use_failure_hooks(
    tool_name: &str,
    tool_input: &serde_json::Value,
    tool_use_id: &str,
    error_message: &str,
    is_interrupt: bool,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::post_tool_use_failure(
        tool_name.to_string(),
        tool_input.clone(),
        tool_use_id.to_string(),
        error_message.to_string(),
        is_interrupt,
    );

    let result = executor
        .run_hooks(
            HookEventType::PostToolUseFailure,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run SessionStart hooks.
pub async fn run_session_start_hooks(
    source: &str,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::SessionStart {
        source: source.to_string(),
    };

    let result = executor
        .run_hooks(
            HookEventType::SessionStart,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run SessionEnd hooks.
pub async fn run_session_end_hooks(
    reason: &str,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::SessionEnd {
        reason: reason.to_string(),
    };

    let result = executor
        .run_hooks(
            HookEventType::SessionEnd,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run UserPromptSubmit hooks.
pub async fn run_user_prompt_submit_hooks(
    prompt: &str,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::UserPromptSubmit {
        prompt: prompt.to_string(),
    };

    let result = executor
        .run_hooks(
            HookEventType::UserPromptSubmit,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run Stop hooks.
pub async fn run_stop_hooks(
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::Stop {
        stop_hook_active: true,
    };

    let result = executor
        .run_hooks(
            HookEventType::Stop,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run SubagentStart hooks.
pub async fn run_subagent_start_hooks(
    agent_id: &str,
    agent_type: &str,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::SubagentStart {
        agent_id: agent_id.to_string(),
        agent_type: agent_type.to_string(),
    };

    let result = executor
        .run_hooks(
            HookEventType::SubagentStart,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run SubagentStop hooks.
pub async fn run_subagent_stop_hooks(
    agent_id: &str,
    agent_transcript_path: &str,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::SubagentStop {
        stop_hook_active: true,
        agent_id: agent_id.to_string(),
        agent_transcript_path: agent_transcript_path.to_string(),
    };

    let result = executor
        .run_hooks(
            HookEventType::SubagentStop,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run PreCompact hooks.
pub async fn run_pre_compact_hooks(
    trigger: &str,
    custom_instructions: Option<&str>,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::PreCompact {
        trigger: trigger.to_string(),
        custom_instructions: custom_instructions.map(|s| s.to_string()),
    };

    let result = executor
        .run_hooks(
            HookEventType::PreCompact,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run Notification hooks.
pub async fn run_notification_hooks(
    message: &str,
    title: &str,
    notification_type: &str,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::Notification {
        message: message.to_string(),
        title: title.to_string(),
        notification_type: notification_type.to_string(),
    };

    let result = executor
        .run_hooks(
            HookEventType::Notification,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Run PermissionRequest hooks.
pub async fn run_permission_request_hooks(
    tool_name: &str,
    tool_input: &serde_json::Value,
    tool_use_id: &str,
    permission_suggestions: Vec<serde_json::Value>,
    context: HookContext,
    cwd: &Path,
    session_id: Option<&str>,
) -> Option<HookExecutionResult> {
    let executor = HOOK_EXECUTOR.get()?;

    let event_data = HookEventData::permission_request(
        tool_name.to_string(),
        tool_input.clone(),
        tool_use_id.to_string(),
        permission_suggestions,
    );

    let result = executor
        .run_hooks(
            HookEventType::PermissionRequest,
            event_data,
            context,
            session_id,
            cwd,
            CancellationToken::new(),
        )
        .await;

    Some(result)
}

/// Create a hook context from session information.
pub fn make_hook_context(
    session_id: &str,
    transcript_path: &str,
    cwd: &str,
    permission_mode: serde_json::Value,
) -> HookContext {
    HookContext {
        session_id: session_id.to_string(),
        transcript_path: transcript_path.to_string(),
        cwd: cwd.to_string(),
        permission_mode,
    }
}

/// Clear session hooks when a session ends.
pub fn clear_session_hooks(session_id: &str) {
    if let Some(executor) = HOOK_EXECUTOR.get() {
        executor.registry().clear_session_hooks(session_id);
    }
}

/// Register a callback hook at runtime.
///
/// This allows programmatic registration of native Rust callbacks.
pub fn register_callback_hook<C>(
    session_id: Option<&str>,
    event_type: HookEventType,
    matcher: &str,
    callback: C,
) where
    C: codex_hooks::HookCallback + 'static,
{
    if let Some(executor) = HOOK_EXECUTOR.get() {
        let hook_config = HookConfig {
            hook_type: HookType::Callback {
                callback: Arc::new(callback),
                timeout_ms: Some(60_000),
            },
            on_success: None,
        };

        let hook_matcher = HookMatcher {
            matcher: matcher.to_string(),
            hooks: vec![hook_config],
        };

        if let Some(session_id) = session_id {
            executor
                .registry()
                .register_session_hook(session_id, event_type, hook_matcher);
        } else {
            executor
                .registry()
                .register_global(event_type, vec![hook_matcher]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_hooks::config::HooksJsonConfig;

    #[test]
    fn test_build_registry_from_empty_config() {
        let config = HooksJsonConfig::default();
        let (registry, shell_prefix) = build_from_json_config(&config);

        // Should have no hooks registered
        assert!(!registry.has_hooks(HookEventType::PreToolUse, None));
        assert!(!registry.has_hooks(HookEventType::SessionStart, None));
        assert!(shell_prefix.is_none());
    }

    #[test]
    fn test_build_registry_from_json_config() {
        let json = r#"{
            "disableAllHooks": false,
            "shellPrefix": "/opt/wrapper.sh",
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            {"type": "command", "command": "echo test", "timeout": 30}
                        ]
                    }
                ]
            }
        }"#;

        let config: HooksJsonConfig = serde_json::from_str(json).unwrap();
        let (registry, shell_prefix) = build_from_json_config(&config);

        assert!(registry.has_hooks(HookEventType::PreToolUse, None));
        assert_eq!(shell_prefix, Some("/opt/wrapper.sh".to_string()));
    }

    #[test]
    fn test_make_hook_context() {
        let context = make_hook_context(
            "session-123",
            "/tmp/transcript.json",
            "/home/user",
            serde_json::json!({"mode": "ask"}),
        );

        assert_eq!(context.session_id, "session-123");
        assert_eq!(context.transcript_path, "/tmp/transcript.json");
        assert_eq!(context.cwd, "/home/user");
    }
}
