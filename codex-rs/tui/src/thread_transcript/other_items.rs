//! Restore historical tool and notice cells without changing live application state.
//!
//! Formatting stays in the same history cells used by live events. Only completed
//! patches render as applied changes; other statuses retain their actual outcome.

use super::TranscriptCells;
use crate::app_server_approval_conversions::file_update_changes_to_display;
use crate::history_cell;
use crate::history_cell::PlainHistoryCell;
use crate::multi_agents;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::WebSearchAction;
use codex_utils_absolute_path::AbsolutePathBuf;
use ratatui::style::Stylize as _;
use std::sync::Arc;

pub(super) fn cells(item: ThreadItemPayload, cwd: &AbsolutePathBuf) -> TranscriptCells {
    let mut cells: TranscriptCells = Vec::new();
    match item {
        ThreadItemPayload::FileChange {
            id,
            changes,
            status,
        } => match status {
            PatchApplyStatus::Completed if !changes.is_empty() => {
                cells.push(Arc::new(
                    history_cell::new_patch_event(
                        file_update_changes_to_display(changes),
                        cwd.as_path(),
                    )
                    .with_activity_id(id),
                ));
            }
            PatchApplyStatus::Completed => {}
            PatchApplyStatus::Failed => {
                cells.push(Arc::new(history_cell::new_patch_apply_failure(
                    String::new(),
                )));
            }
            PatchApplyStatus::InProgress | PatchApplyStatus::Declined => {
                let message = if status == PatchApplyStatus::Declined {
                    "Patch application declined"
                } else {
                    "Patch application in progress"
                };
                cells.push(Arc::new(history_cell::new_info_event(
                    message.to_string(),
                    /*hint*/ None,
                )));
            }
        },
        ThreadItemPayload::WebSearch(item) => {
            cells.push(Arc::new(history_cell::new_web_search_call(
                item.id,
                item.query,
                item.action.unwrap_or(WebSearchAction::Other),
            )))
        }
        ThreadItemPayload::ImageView { id, path } => {
            cells.push(Arc::new(history_cell::new_view_image_tool_call(id, path)))
        }
        ThreadItemPayload::ImageGeneration(item)
            if !matches!(item.status.as_str(), "completed" | "failed") =>
        {
            let status = if item.status.is_empty() {
                "status unavailable"
            } else {
                &item.status
            };
            cells.push(Arc::new(history_cell::new_info_event(
                format!("Image generation · {status}"),
                /*hint*/ None,
            )));
        }
        ThreadItemPayload::ImageGeneration(item) => {
            cells.push(Arc::new(history_cell::new_image_generation_call(
                item.id,
                &item.status,
                item.revised_prompt,
                item.saved_path,
            )));
        }
        item @ ThreadItemPayload::CollabAgentToolCall { .. } => {
            if let Some(cell) = multi_agents::tool_call_history_cell(
                &item,
                /*cached_spawn_request*/ None,
                |_| multi_agents::AgentMetadata::default(),
            ) {
                cells.push(Arc::new(cell));
            }
        }
        item @ ThreadItemPayload::SubAgentActivity { .. } => {
            if let Some(cell) = multi_agents::sub_agent_activity_history_cell(&item) {
                cells.push(Arc::new(cell));
            }
        }
        ThreadItemPayload::EnteredReviewMode { review, .. } => {
            cells.push(Arc::new(history_cell::new_review_status_line(format!(
                ">> Code review started: {review} <<"
            ))));
        }
        ThreadItemPayload::ExitedReviewMode { review, .. } => {
            cells.push(Arc::new(history_cell::new_review_status_line(format!(
                "<< Code review finished: {review} >>"
            ))));
        }
        ThreadItemPayload::ContextCompaction { .. } => {
            cells.push(Arc::new(history_cell::new_info_event(
                "Context compacted".to_string(),
                /*hint*/ None,
            )));
        }
        // These items do not have richer history-cell presentations yet.
        ThreadItemPayload::HookPrompt { fragments, .. } => {
            if !fragments.is_empty() {
                cells.push(Arc::new(PlainHistoryCell::new(
                    fragments
                        .into_iter()
                        .map(|fragment| {
                            vec![
                                "hook prompt: ".dim(),
                                fragment.text.trim().to_string().into(),
                            ]
                            .into()
                        })
                        .collect(),
                )));
            }
        }
        item @ ThreadItemPayload::DynamicToolCall { .. } => {
            if let Some(cell) = history_cell::DynamicToolCallCell::from_item(item) {
                cells.push(Arc::new(cell));
            }
        }
        ThreadItemPayload::UserMessage { .. }
        | ThreadItemPayload::AgentMessage { .. }
        | ThreadItemPayload::FunctionCallOutput { .. }
        | ThreadItemPayload::Plan { .. }
        | ThreadItemPayload::Reasoning { .. }
        | ThreadItemPayload::CommandExecution { .. }
        | ThreadItemPayload::McpToolCall { .. }
        | ThreadItemPayload::Sleep(_) => {}
    }
    cells
}

#[cfg(test)]
#[path = "other_items_tests.rs"]
mod tests;
