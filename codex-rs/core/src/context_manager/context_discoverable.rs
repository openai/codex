use crate::truncate::TruncationPolicy;
use crate::truncate::approx_token_count;
use crate::truncate::truncate_function_output_items_with_policy;
use crate::truncate::truncate_text;
use codex_protocol::models::ContentItem;
use codex_protocol::models::CustomToolCallOutput;
use codex_protocol::models::FunctionCallOutput;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::Message;
use codex_protocol::models::ResponseItem;
use sha2::Digest;
use sha2::Sha256;
use std::path::Path;

pub(super) trait ContextDiscoverable {
    fn discoverable_history_item(
        &self,
        policy: TruncationPolicy,
        codex_home: Option<&Path>,
        thread_id: Option<&str>,
        context_window_tokens: Option<i64>,
        current_usage_tokens: i64,
    ) -> ResponseItem;
}

impl ContextDiscoverable for FunctionCallOutput {
    fn discoverable_history_item(
        &self,
        policy: TruncationPolicy,
        _codex_home: Option<&Path>,
        _thread_id: Option<&str>,
        _context_window_tokens: Option<i64>,
        _current_usage_tokens: i64,
    ) -> ResponseItem {
        let body = match &self.output.body {
            FunctionCallOutputBody::Text(content) => {
                FunctionCallOutputBody::Text(truncate_text(content, policy))
            }
            FunctionCallOutputBody::ContentItems(items) => FunctionCallOutputBody::ContentItems(
                truncate_function_output_items_with_policy(items, policy),
            ),
        };

        ResponseItem::FunctionCallOutput(FunctionCallOutput {
            call_id: self.call_id.clone(),
            output: FunctionCallOutputPayload {
                body,
                success: self.output.success,
            },
        })
    }
}

impl ContextDiscoverable for CustomToolCallOutput {
    fn discoverable_history_item(
        &self,
        policy: TruncationPolicy,
        _codex_home: Option<&Path>,
        _thread_id: Option<&str>,
        _context_window_tokens: Option<i64>,
        _current_usage_tokens: i64,
    ) -> ResponseItem {
        ResponseItem::CustomToolCallOutput(CustomToolCallOutput {
            call_id: self.call_id.clone(),
            output: truncate_text(&self.output, policy),
        })
    }
}

impl ContextDiscoverable for Message {
    fn discoverable_history_item(
        &self,
        _policy: TruncationPolicy,
        codex_home: Option<&Path>,
        thread_id: Option<&str>,
        context_window_tokens: Option<i64>,
        current_usage_tokens: i64,
    ) -> ResponseItem {
        let (Some(codex_home), Some(thread_id), Some(context_window_tokens)) =
            (codex_home, thread_id, context_window_tokens)
        else {
            return ResponseItem::Message(self.clone());
        };

        if self.role != "user" {
            return ResponseItem::Message(self.clone());
        }

        let Some(user_text) = extract_message_text(&self.content) else {
            return ResponseItem::Message(self.clone());
        };

        let estimated_tokens = i64::try_from(approx_token_count(&user_text)).unwrap_or(i64::MAX);
        let offload_threshold = context_window_tokens.saturating_mul(95) / 100;
        if current_usage_tokens.saturating_add(estimated_tokens) <= offload_threshold {
            return ResponseItem::Message(self.clone());
        }

        let checksum = checksum_hex(&user_text);
        let output_path = codex_home
            .join("discovarable_items")
            .join(thread_id)
            .join("user_message")
            .join(checksum);

        if let Some(parent) = output_path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(
                path = %parent.display(),
                "failed to create discoverable message directory: {err}"
            );
            return ResponseItem::Message(self.clone());
        }

        if let Err(err) = std::fs::write(&output_path, &user_text) {
            tracing::warn!(
                path = %output_path.display(),
                "failed to write discoverable user message file: {err}"
            );
            return ResponseItem::Message(self.clone());
        }

        let replacement = format!(
            "User message was too large. Read it from <{}>",
            output_path.display()
        );
        let mut rewritten = self.clone();
        rewritten.content = vec![ContentItem::InputText { text: replacement }];
        ResponseItem::Message(rewritten)
    }
}

fn extract_message_text(content: &[ContentItem]) -> Option<String> {
    let parts: Vec<&str> = content
        .iter()
        .filter_map(|item| match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text }
                if !text.trim().is_empty() =>
            {
                Some(text.as_str())
            }
            ContentItem::InputText { .. } | ContentItem::OutputText { .. } => None,
            ContentItem::InputImage { .. } => None,
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

fn checksum_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
