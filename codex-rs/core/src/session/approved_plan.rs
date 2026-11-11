//! Prepares an approved plan using the fresh thread's configured executor and model.

use std::time::Duration;

use codex_file_system::EnvironmentAccess;
use codex_file_system::GetMetadataOptions;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_bytes_for_tokens;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;

use super::session::Session;
use super::turn_context::TurnContext;

const ATTACHMENT_READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FALLBACK_TOKENS: usize = 10_000;
const MAX_FALLBACK_ITEMS: usize = 32;

impl Session {
    pub(crate) async fn prepare_approved_plan(
        &self,
        path: Option<&PathUri>,
        markdown: &str,
    ) -> Result<(String, Vec<ResponseItem>)> {
        if markdown.trim().is_empty() {
            return Err(CodexErr::InvalidRequest(
                "The approved plan is empty.".into(),
            ));
        }
        if self.active_turn.lock().await.is_some() {
            return Err(CodexErr::InvalidRequest(
                "The thread already has an active turn.".into(),
            ));
        }
        let turn = self.new_inject_items_context().await;
        if let Some(path) = path
            && let Ok(Some(path)) = tokio::time::timeout(
                ATTACHMENT_READ_TIMEOUT,
                readable_path(&turn, path, markdown),
            )
            .await
        {
            return Ok((
                format!("Read the approved plan at {path} and implement it."),
                Vec::new(),
            ));
        }
        Ok((
            "Implement the plan.".to_string(),
            fallback_items(&turn, markdown)?,
        ))
    }
}

fn fallback_items(turn: &TurnContext, mut markdown: &str) -> Result<Vec<ResponseItem>> {
    let model = turn.model_info();
    let context_budget = model
        .usable_context_window()
        .map_or(MAX_FALLBACK_TOKENS, |window| {
            usize::try_from(window / 4)
                .unwrap_or_default()
                .min(MAX_FALLBACK_TOKENS)
        });
    let policy = TruncationPolicy::from(model.truncation_policy);
    let chunk_bytes = policy
        .byte_budget()
        .min(approx_bytes_for_tokens(context_budget));
    let too_large = || {
        CodexErr::InvalidRequest(
            "The executor could not read the plan attachment and the complete plan is too large to send as context. Implement it in the original thread.".into(),
        )
    };
    if markdown.len() > approx_bytes_for_tokens(context_budget) || chunk_bytes == 0 {
        return Err(too_large());
    }

    let mut items = Vec::new();
    while !markdown.is_empty() {
        let end = markdown.floor_char_boundary(chunk_bytes.min(markdown.len()));
        if end == 0 || items.len() == MAX_FALLBACK_ITEMS {
            return Err(too_large());
        }
        let (chunk, rest) = markdown.split_at(end);
        items.push(ResponseItem::FunctionCallOutput {
            id: None,
            call_id: None,
            namespace: Some("agent".to_string()),
            name: Some("proposed_plan".to_string()),
            output: FunctionCallOutputPayload::from_text(chunk.to_string()),
            internal_chat_message_metadata_passthrough: None,
        });
        markdown = rest;
    }
    Ok(items)
}

async fn readable_path(turn: &TurnContext, path: &PathUri, markdown: &str) -> Option<String> {
    let environment = turn.initial_environments.primary()?;
    let convention = environment.cwd().infer_path_convention()?;
    let native = LegacyAppPathString::from_path_uri(path, convention).ok()?;
    if native.to_path_uri(convention).ok().as_ref() != Some(path) {
        return None;
    }
    let native = native.into_string();
    if native.chars().any(char::is_control) {
        return None;
    }

    let grants = turn.granted_permissions(&environment.selection().environment_id);
    let fs = environment.fs_accessor(grants);
    let metadata = fs
        .get_metadata(
            path,
            GetMetadataOptions {
                follow_symlinks: false,
            },
        )
        .await
        .ok()?;
    if !metadata.is_file || metadata.is_symlink || metadata.size != markdown.len() as u64 {
        return None;
    }
    let mut remaining = markdown.as_bytes();
    let mut stream = fs.read_file_stream(path).await.ok()?;
    while let Some(chunk) = stream.next().await {
        remaining = remaining.strip_prefix(chunk.ok()?.as_ref())?;
    }
    remaining.is_empty().then_some(native)
}
