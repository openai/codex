use std::collections::HashSet;
use std::sync::Arc;

use codex_exec_server::EnvironmentManager;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::build_prompt;
use crate::codex::built_tools;
use crate::config::Config;
use crate::thread_manager::ThreadManager;

/// Build the model-visible `input` list for a single debug turn.
#[doc(hidden)]
pub async fn build_prompt_input(
    mut config: Config,
    input: Vec<UserInput>,
) -> CodexResult<Vec<ResponseItem>> {
    config.ephemeral = true;

    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        /*enable_codex_api_key_env*/ false,
        config.cli_auth_credentials_store_mode,
    );
    auth_manager.set_forced_chatgpt_workspace_id(config.forced_chatgpt_workspace_id.clone());

    let thread_manager = ThreadManager::new(
        &config,
        Arc::clone(&auth_manager),
        SessionSource::Exec,
        CollaborationModesConfig {
            default_mode_request_user_input: config
                .features
                .enabled(Feature::DefaultModeRequestUserInput),
        },
        Arc::new(EnvironmentManager::from_env()),
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use codex_exec_server::EnvironmentManager;
    use codex_exec_server::EnvironmentMode;
    use codex_features::Feature;
    use codex_login::AuthManager;
    use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::user_input::UserInput;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tokio_util::sync::CancellationToken;

    use crate::config::test_config;
    use crate::thread_manager::ThreadManager;

    use super::build_prompt_input;

    async fn build_model_visible_tool_names(
        mut config: crate::config::Config,
        environment_mode: EnvironmentMode,
    ) -> Vec<String> {
        config.ephemeral = true;

        let auth_manager = AuthManager::shared(
            config.codex_home.clone(),
            /*enable_codex_api_key_env*/ false,
            config.cli_auth_credentials_store_mode,
        );
        auth_manager.set_forced_chatgpt_workspace_id(config.forced_chatgpt_workspace_id.clone());

        let thread_manager = ThreadManager::new(
            &config,
            Arc::clone(&auth_manager),
            SessionSource::Exec,
            CollaborationModesConfig {
                default_mode_request_user_input: config
                    .features
                    .enabled(Feature::DefaultModeRequestUserInput),
            },
            Arc::new(EnvironmentManager::from_mode(environment_mode)),
        );
        let thread = thread_manager
            .start_thread(config)
            .await
            .expect("start thread");
        let session = thread.thread.codex.session.as_ref();
        let turn_context = session.new_default_turn().await;
        session
            .record_context_updates_and_set_reference_context_item(turn_context.as_ref())
            .await;

        let prompt_input = session
            .clone_history()
            .await
            .for_prompt(&turn_context.model_info.input_modalities);
        let router = super::built_tools(
            session,
            turn_context.as_ref(),
            &prompt_input,
            &HashSet::new(),
            Some(turn_context.turn_skills.outcome.as_ref()),
            &CancellationToken::new(),
        )
        .await
        .expect("build tools");
        let tool_names = router
            .model_visible_specs()
            .iter()
            .map(|spec| spec.name().to_string())
            .collect::<Vec<_>>();

        let shutdown = thread.thread.shutdown_and_wait().await;
        let _removed = thread_manager.remove_thread(&thread.thread_id).await;
        shutdown.expect("shutdown thread");

        tool_names
    }

    #[tokio::test]
    async fn build_prompt_input_includes_context_and_user_message() {
        let codex_home = tempfile::tempdir().expect("create codex home");
        let cwd = tempfile::tempdir().expect("create cwd");
        let mut config = test_config();
        config.codex_home = codex_home.path().to_path_buf();
        config.cwd = AbsolutePathBuf::try_from(cwd.path().to_path_buf()).expect("absolute cwd");
        config.user_instructions = Some("Project-specific test instructions".to_string());

        let input = build_prompt_input(
            config,
            vec![UserInput::Text {
                text: "hello from debug prompt".to_string(),
                text_elements: Vec::new(),
            }],
        )
        .await
        .expect("build prompt input");

        let expected_user_message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hello from debug prompt".to_string(),
            }],
            end_turn: None,
            phase: None,
        };
        assert_eq!(input.last(), Some(&expected_user_message));
        assert!(input.iter().any(|item| {
            let ResponseItem::Message { content, .. } = item else {
                return false;
            };

            content.iter().any(|content_item| {
                let (ContentItem::InputText { text } | ContentItem::OutputText { text }) =
                    content_item
                else {
                    return false;
                };
                text.contains("Project-specific test instructions")
            })
        }));
    }

    #[tokio::test]
    async fn disabled_environment_omits_environment_backed_tools_from_prompt() {
        let codex_home = tempfile::tempdir().expect("create codex home");
        let cwd = tempfile::tempdir().expect("create cwd");
        let mut config = test_config();
        config.codex_home = codex_home.path().to_path_buf();
        config.cwd = AbsolutePathBuf::try_from(cwd.path().to_path_buf()).expect("absolute cwd");
        let _ = config.features.enable(Feature::UnifiedExec);

        let local_tool_names =
            build_model_visible_tool_names(config.clone(), EnvironmentMode::Local).await;
        let disabled_tool_names =
            build_model_visible_tool_names(config, EnvironmentMode::Disabled).await;

        let environment_backed_tools = ["exec_command", "write_stdin", "apply_patch", "view_image"];
        for tool_name in environment_backed_tools {
            assert!(
                local_tool_names.iter().any(|name| name == tool_name),
                "expected local environment to expose {tool_name}, got {local_tool_names:?}",
            );
            assert!(
                disabled_tool_names.iter().all(|name| name != tool_name),
                "expected disabled environment to hide {tool_name}, got {disabled_tool_names:?}",
            );
        }

        assert!(disabled_tool_names.iter().any(|name| name == "update_plan"));
    }
}
