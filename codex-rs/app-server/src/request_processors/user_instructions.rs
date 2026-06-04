use codex_core::AgentsMdManager;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;

/// Loads AGENTS.md instructions from the primary turn environment into
/// `config` before the app server passes it to the thread manager.
pub(super) async fn load_thread_user_instructions(
    thread_manager: &ThreadManager,
    config: &mut Config,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<()> {
    let Some(primary_selection) = environments.first() else {
        config.user_instructions = None;
        return Ok(());
    };
    let environment = thread_manager
        .environment_manager()
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
