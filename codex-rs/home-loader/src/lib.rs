//! Loads Codex home state needed by callers constructing Codex threads.

#![deny(private_bounds, private_interfaces, unreachable_pub)]

use codex_core::AgentsMdManager;
use codex_core::config::Config;
use codex_exec_server::EnvironmentManager;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;

/// Preloads AGENTS.md instructions for thread construction from the primary
/// selected environment.
///
/// Call this after the selected environments have been materialized and before
/// passing `config` to a thread start, resume, or fork operation. When no
/// environment is selected, this clears [`Config::user_instructions`].
///
/// This helper assumes user-level AGENTS.md instructions were loaded from the
/// configured `CODEX_HOME`. Callers that source user-level instructions from
/// another location should assemble [`Config::user_instructions`] themselves.
pub async fn load_thread_user_instructions(
    config: &mut Config,
    environment_manager: &EnvironmentManager,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<()> {
    let Some(primary_selection) = environments.first() else {
        config.user_instructions = None;
        return Ok(());
    };
    let environment = environment_manager
        .get_environment(&primary_selection.environment_id)
        .ok_or_else(|| {
            CodexErr::InvalidRequest(format!(
                "unknown turn environment id `{}`",
                primary_selection.environment_id
            ))
        })?;
    let mut warnings = Vec::new();
    let user_instructions = AgentsMdManager::new(config)
        .load_user_instructions(environment.as_ref(), &mut warnings)
        .await;
    config.startup_warnings.extend(warnings);
    config.user_instructions = user_instructions;
    Ok(())
}
