use std::path::PathBuf;

use codex_core::NewConversation;
use codex_core::config::Config as CodexConfig;
use codex_core::config::ConfigOverrides;
use mcp_types::RequestId;

use crate::conversation_loop::run_conversation_loop;
use crate::json_to_toml::json_to_toml;
use crate::mcp_protocol::ConversationCreateArgs;
use crate::mcp_protocol::ConversationCreateResult;
use crate::mcp_protocol::ConversationId;
use crate::mcp_protocol::ToolCallResponseResult;
use crate::message_processor::MessageProcessor;

pub(crate) async fn handle_create_conversation(
    message_processor: &MessageProcessor,
    id: RequestId,
    args: ConversationCreateArgs,
) {
    // Build ConfigOverrides from args
    let ConversationCreateArgs {
        prompt: _, // not used here; creation only establishes the session
        model,
        cwd,
        approval_policy,
        sandbox,
        config,
        profile,
        base_instructions,
    } = args;

    // Convert config overrides JSON into CLI-style TOML overrides
    let cli_overrides: Vec<(String, toml::Value)> = match config {
        Some(v) => match v.as_object() {
            Some(map) => map
                .into_iter()
                .map(|(k, v)| (k.clone(), json_to_toml(v.clone())))
                .collect(),
            None => Vec::new(),
        },
        None => Vec::new(),
    };

    let overrides = ConfigOverrides {
        model: Some(model.clone()),
        cwd: Some(PathBuf::from(cwd)),
        approval_policy,
        sandbox_mode: sandbox,
        model_provider: None,
        config_profile: profile,
        codex_linux_sandbox_exe: None,
        base_instructions,
        include_plan_tool: None,
        include_apply_patch_tool: None,
        disable_response_storage: None,
        show_raw_agent_reasoning: None,
    };

    let cfg: CodexConfig = match CodexConfig::load_with_cli_overrides(cli_overrides, overrides) {
        Ok(cfg) => cfg,
        Err(e) => {
            message_processor
                .send_response_with_optional_error(
                    id,
                    Some(ToolCallResponseResult::ConversationCreate(
                        ConversationCreateResult::Error {
                            message: format!("Failed to load config: {e}"),
                        },
                    )),
                    Some(true),
                )
                .await;
            return;
        }
    };

    // Initialize Codex session via server API
    let NewConversation {
        conversation_id: session_id,
        conversation,
        session_configured,
    } = match message_processor
        .get_conversation_manager()
        .new_conversation(cfg)
        .await
    {
        Ok(conv) => conv,
        Err(e) => {
            message_processor
                .send_response_with_optional_error(
                    id,
                    Some(ToolCallResponseResult::ConversationCreate(
                        ConversationCreateResult::Error {
                            message: format!("Failed to initialize session: {e}"),
                        },
                    )),
                    Some(true),
                )
                .await;
            return;
        }
    };

    let effective_model = session_configured.model.clone();

    // Run the conversation loop in the background so this request can return immediately.
    let outgoing = message_processor.outgoing();
    let spawn_id = id.clone();
    tokio::spawn(async move {
        run_conversation_loop(conversation.clone(), outgoing, spawn_id).await;
    });

    // Reply with the new conversation id and effective model
    message_processor
        .send_response_with_optional_error(
            id,
            Some(ToolCallResponseResult::ConversationCreate(
                ConversationCreateResult::Ok {
                    conversation_id: ConversationId(session_id),
                    model: effective_model,
                },
            )),
            Some(false),
        )
        .await;
}
