use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core_api::load_thread_user_instructions as preload_thread_user_instructions;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;

/// Preloads AGENTS.md instructions from the primary selected environment
/// before app-server passes the config to the thread manager.
pub(super) async fn load_thread_user_instructions(
    thread_manager: &ThreadManager,
    config: &mut Config,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<()> {
    preload_thread_user_instructions(
        config,
        thread_manager.environment_manager().as_ref(),
        environments,
    )
    .await
}
