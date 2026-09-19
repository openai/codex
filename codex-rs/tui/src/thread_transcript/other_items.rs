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
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::WebSearchAction;
use codex_utils_absolute_path::AbsolutePathBuf;
use ratatui::style::Stylize as _;
use std::sync::Arc;

pub(super) fn cells(item: ThreadItem, cwd: &AbsolutePathBuf) -> TranscriptCells {
    let mut cells: TranscriptCells = Vec::new();
    match item {
        ThreadItem::FileChange {
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
        ThreadItem::WebSearch(item) => cells.push(Arc::new(history_cell::new_web_search_call(
            item.id,
            item.query,
            item.action.unwrap_or(WebSearchAction::Other),
        ))),
        ThreadItem::ImageView { path, .. } => {
            cells.push(Arc::new(history_cell::new_view_image_tool_call(path)))
        }
        ThreadItem::ImageGeneration(item)
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
        ThreadItem::ImageGeneration(item) => {
            cells.push(Arc::new(history_cell::new_image_generation_call(
                item.id,
                &item.status,
                item.revised_prompt,
                item.saved_path,
            )));
        }
        item @ ThreadItem::CollabAgentToolCall { .. } => {
            if let Some(cell) = multi_agents::tool_call_history_cell(
                &item,
                /*cached_spawn_request*/ None,
                |_| multi_agents::AgentMetadata::default(),
            ) {
                cells.push(Arc::new(cell));
            }
        }
        item @ ThreadItem::SubAgentActivity { .. } => {
            if let Some(cell) = multi_agents::sub_agent_activity_history_cell(&item) {
                cells.push(Arc::new(cell));
            }
        }
        ThreadItem::EnteredReviewMode { review, .. } => {
            cells.push(Arc::new(history_cell::new_review_status_line(format!(
                ">> Code review started: {review} <<"
            ))));
        }
        ThreadItem::ExitedReviewMode { review, .. } => {
            cells.push(Arc::new(history_cell::new_review_status_line(format!(
                "<< Code review finished: {review} >>"
            ))));
        }
        ThreadItem::ContextCompaction { .. } => {
            cells.push(Arc::new(history_cell::new_info_event(
                "Context compacted".to_string(),
                /*hint*/ None,
            )));
        }
        // These items do not have richer history-cell presentations yet.
        ThreadItem::HookPrompt { fragments, .. } => {
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
        item @ ThreadItem::DynamicToolCall { .. } => {
            if let Some(cell) = history_cell::DynamicToolCallCell::from_item(item) {
                cells.push(Arc::new(cell));
            }
        }
        ThreadItem::UserMessage { .. }
        | ThreadItem::AgentMessage { .. }
        | ThreadItem::FunctionCallOutput { .. }
        | ThreadItem::Plan { .. }
        | ThreadItem::Reasoning { .. }
        | ThreadItem::CommandExecution { .. }
        | ThreadItem::McpToolCall { .. }
        | ThreadItem::Sleep(_) => {}
    }
    cells
}

#[cfg(test)]
#[path = "other_items_tests.rs"]
mod tests;
