use std::collections::HashSet;
use std::sync::Arc;

use codex_exec_server::EnvironmentManager;
use codex_exec_server::EnvironmentManagerArgs;
use codex_exec_server::ExecServerRuntimePaths;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::session::session::Session;
use crate::session::turn::build_prompt;
use crate::session::turn::built_tools;
use crate::thread_manager::ThreadManager;

/// Build the model-visible `input` list for a single debug turn.
#[doc(hidden)]
pub async fn build_prompt_input(
    mut config: Config,
    input: Vec<UserInput>,
) -> CodexResult<Vec<ResponseItem>> {
    config.ephemeral = true;

    let auth_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await;

    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        config.codex_self_exe.clone(),
        config.codex_linux_sandbox_exe.clone(),
    )?;

    let thread_manager = ThreadManager::new(
        &config,
        Arc::clone(&auth_manager),
        SessionSource::Exec,
        CollaborationModesConfig {
            default_mode_request_user_input: config
                .features
                .enabled(Feature::DefaultModeRequestUserInput),
        },
        Arc::new(EnvironmentManager::new(EnvironmentManagerArgs::from_env(
            local_runtime_paths,
        ))),
        /*analytics_events_client*/ None,
    );
    let thread = thread_manager.start_thread(config).await?;

    let output = build_prompt_input_from_session(thread.thread.codex.session.as_ref(), input).await;
    let shutdown = thread.thread.shutdown_and_wait().await;
    let _removed = thread_manager.remove_thread(&thread.thread_id).await;

    shutdown?;
    output
}

pub(crate) async fn build_prompt_input_from_session(
    sess: &Session,
    input: Vec<UserInput>,
) -> CodexResult<Vec<ResponseItem>> {
    let turn_context = sess.new_default_turn().await;
    sess.record_context_updates_and_set_reference_context_item(turn_context.as_ref())
        .await;

    if !input.is_empty() {
        let input_item = ResponseInputItem::from(input);
        let response_item = ResponseItem::from(input_item);
        sess.record_conversation_items(turn_context.as_ref(), std::slice::from_ref(&response_item))
            .await;
    }

    let prompt_input = sess
        .clone_history()
        .await
        .for_prompt(&turn_context.model_info.input_modalities);
    let router = built_tools(
        sess,
        turn_context.as_ref(),
        &prompt_input,
        &HashSet::new(),
        Some(turn_context.turn_skills.outcome.as_ref()),
        &CancellationToken::new(),
    )
    .await?;
    let base_instructions = sess.get_base_instructions().await;
    let prompt = build_prompt(
        prompt_input,
        router.as_ref(),
        turn_context.as_ref(),
        base_instructions,
    );

    Ok(prompt.get_formatted_input())
}
