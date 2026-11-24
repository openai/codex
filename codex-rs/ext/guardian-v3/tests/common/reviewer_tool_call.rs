//! Invoke tools through the same bindings installed in Guardian reviewer threads.

use anyhow::Context;
use codex_guardian_v3::LiveRetrievalBinding;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_tools::ToolCall;
use codex_tools::ToolCallSource;
use codex_tools::ToolName;
use codex_tools::ToolPayload;
use serde_json::Value;
use std::sync::Arc;

pub(crate) async fn invoke(
    binding: &LiveRetrievalBinding,
    name: &str,
    arguments: Value,
) -> anyhow::Result<Value> {
    let tool = binding
        .tools()
        .into_iter()
        .find(|tool| tool.tool_name().name == name)
        .with_context(|| format!("missing reviewer tool {name}"))?;
    let call = reviewer_tool_call(&tool.tool_name(), &arguments, /*byte_limit*/ 8_000);
    let result = tool.handle(call.clone()).await?;
    Ok(result.code_mode_result(&call.payload).into_json())
}

pub(crate) fn reviewer_tool_call(
    name: &ToolName,
    arguments: &Value,
    byte_limit: i64,
) -> ToolCall<'static> {
    ToolCall {
        turn_id: "review".into(),
        call_id: name.name.clone(),
        tool_name: name.clone(),
        model: "test".into(),
        codex_turn_metadata: None,
        truncation_policy: TruncationPolicyConfig::bytes(byte_limit).into(),
        source: ToolCallSource::Direct,
        conversation_history: Default::default(),
        turn_item_emitter: Arc::new(codex_tools::NoopTurnItemEmitter),
        attachment_store: Arc::new(codex_attachment_store::InlineAttachmentStore),
        ephemeral: false,
        environments: Vec::new(),
        payload: ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    }
}
