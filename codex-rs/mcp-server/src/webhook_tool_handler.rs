//! Webhook tool handler implementation.

use anyhow::Result;
use codex_core::integrations::WebhookClient;
use codex_core::integrations::WebhookPayload;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::TextContent;
use serde_json::Value;
use tracing::debug;
use tracing::info;

pub(crate) async fn handle_webhook_tool_call(arguments: Option<Value>) -> Result<CallToolResult> {
    let args = arguments.ok_or_else(|| anyhow::anyhow!("Missing arguments"))?;
    debug!("Webhook tool call with args: {:?}", args);
    let payload: WebhookPayload = serde_json::from_value(args)
        .map_err(|e| anyhow::anyhow!("Invalid webhook payload: {}", e))?;
    info!(
        "Executing webhook: service={:?}, action={}",
        payload.service, payload.action
    );
    let client = WebhookClient::new();
    match client.execute(payload).await {
        Ok(response) => {
            let result_text = if response.success {
                format!(
                    "笨・Webhook call succeeded (status: {})\n\nResponse:\n{}",
                    response.status, response.text
                )
            } else {
                format!(
                    "笞・・Webhook call failed (status: {})\n\nResponse:\n{}",
                    response.status, response.text
                )
            };
            Ok(CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: result_text,
                    annotations: None,
                })],
                is_error: Some(!response.success),
                structured_content: response.body.map(|b| vec![b]),
            })
        }
        Err(e) => {
            let error_text = format!("笶・Webhook call error: {}", e);
            Ok(CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: error_text,
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            })
        }
    }
}
