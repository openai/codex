use std::sync::Arc;

use codex_hooks::PromptHookRequest;
use codex_hooks::PromptHookRunner;
use codex_models_manager::ModelsManagerConfig;
use codex_models_manager::manager::SharedModelsManager;
use codex_otel::SessionTelemetry;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_rollout_trace::InferenceTraceContext;
use futures::StreamExt;
use serde_json::json;

use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::config::Config;
use crate::stream_events_utils::raw_assistant_output_text_from_item;

const PROMPT_HOOK_BASE_INSTRUCTIONS: &str = r#"You evaluate a Codex prompt hook.

The user message contains:
1. The hook author's instructions.
2. The hook input JSON.

Decide whether the hook input satisfies the hook author's instructions.

Return only JSON:
{"ok": true}
or
{"ok": false, "reason": "concise actionable reason"}

Use ok:false only when the hook criteria fail. Do not answer the user's task. Do not include Markdown or extra text."#;

pub(crate) fn build_prompt_hook_runner(
    model_client: ModelClient,
    models_manager: SharedModelsManager,
    config: &Config,
    session_telemetry: SessionTelemetry,
    service_tier: Option<String>,
) -> PromptHookRunner {
    let models_manager_config = config.to_models_manager_config();
    PromptHookRunner::new(move |request| {
        let model_client = model_client.clone();
        let models_manager = Arc::clone(&models_manager);
        let models_manager_config = models_manager_config.clone();
        let session_telemetry = session_telemetry.clone();
        let service_tier = service_tier.clone();
        async move {
            run_prompt_hook(
                model_client,
                models_manager,
                models_manager_config,
                session_telemetry,
                service_tier,
                request,
            )
            .await
        }
    })
}

/// Run the hook as an isolated model request: resolve the configured model,
/// send the rendered hook prompt as the only user message, disable all
/// tools/personality/reasoning summaries, and constrain the final answer to the
/// small `{ ok, reason }` contract that `codex-hooks` maps back into existing
/// hook output semantics.
async fn run_prompt_hook(
    model_client: ModelClient,
    models_manager: SharedModelsManager,
    models_manager_config: ModelsManagerConfig,
    session_telemetry: SessionTelemetry,
    service_tier: Option<String>,
    request: PromptHookRequest,
) -> anyhow::Result<String> {
    let model_info = models_manager
        .get_model_info(request.model.as_str(), &models_manager_config)
        .await;
    let prompt = Prompt {
        input: vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: request.prompt,
            }],
            phase: None,
        }],
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions: BaseInstructions {
            text: PROMPT_HOOK_BASE_INSTRUCTIONS.to_string(),
        },
        personality: None,
        output_schema: Some(hook_output_schema()),
        output_schema_strict: false,
    };

    let disabled_trace = InferenceTraceContext::disabled();
    let mut client_session = model_client.new_isolated_session();
    let mut stream = client_session
        .stream(
            &prompt,
            &model_info,
            &session_telemetry,
            /*effort*/ None,
            ReasoningSummaryConfig::None,
            service_tier,
            /*turn_metadata_header*/ None,
            &disabled_trace,
        )
        .await?;
    let mut delta_text = String::new();
    let mut item_texts = Vec::new();
    while let Some(event) = stream.next().await {
        match event? {
            ResponseEvent::OutputItemDone(item) => {
                if let Some(text) = raw_assistant_output_text_from_item(&item) {
                    item_texts.push(text);
                }
            }
            ResponseEvent::OutputTextDelta(delta) => delta_text.push_str(delta.as_str()),
            ResponseEvent::Completed { .. } => break,
            ResponseEvent::Created
            | ResponseEvent::OutputItemAdded(_)
            | ResponseEvent::ServerModel(_)
            | ResponseEvent::ModelVerifications(_)
            | ResponseEvent::ServerReasoningIncluded(_)
            | ResponseEvent::ToolCallInputDelta { .. }
            | ResponseEvent::ReasoningSummaryDelta { .. }
            | ResponseEvent::ReasoningContentDelta { .. }
            | ResponseEvent::ReasoningSummaryPartAdded { .. }
            | ResponseEvent::RateLimits(_)
            | ResponseEvent::ModelsEtag(_) => {}
        }
    }

    if item_texts.is_empty() {
        Ok(delta_text)
    } else {
        Ok(item_texts.join(""))
    }
}

pub(crate) fn hook_output_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "ok": {
                "type": "boolean"
            },
            "reason": {
                "type": "string"
            }
        },
        "required": ["ok"],
        "additionalProperties": false
    })
}
