//! Project individual persisted items into rich history cells without changing live state.

use std::sync::Arc;

use crate::app_server_session::AppServerSession;
use crate::app_server_session::HistoryHydrationScope;
use crate::git_action_directives::parse_assistant_markdown;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::PrefixedWrappedHistoryCell;
use crate::history_cell::ReasoningSummaryCell;
use crate::history_cell::UserHistoryCell;
use crate::history_cell::split_reasoning_summary_parts;
use crate::inline_visualization::InlineVisualizationContext;
use crate::legacy_core::config::Config;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;
use codex_protocol::items::UserMessageItem;
use codex_utils_absolute_path::AbsolutePathBuf;
use ratatui::style::Stylize as _;

mod other_items;
pub(crate) mod tools;

pub(crate) type TranscriptCells = Vec<Arc<dyn HistoryCell>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RawReasoningVisibility {
    Hidden,
    Visible,
}

pub(crate) async fn load_session_transcript(
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
    raw_reasoning_visibility: RawReasoningVisibility,
    config: Option<&Config>,
) -> std::io::Result<TranscriptCells> {
    let mut thread = app_server
        .thread_read(thread_id, /*include_turns*/ false)
        .await
        .map_err(std::io::Error::other)?;
    app_server
        .hydrate_initial_thread_history(
            &mut thread,
            /*turn_cursor*/ None,
            /*item_cursor*/ None,
            /*config*/ None,
            /*local_settings*/ None,
            HistoryHydrationScope::Complete,
        )
        .await
        .map_err(std::io::Error::other)?;
    Ok(thread_to_transcript_cells(
        thread,
        raw_reasoning_visibility,
        config,
    ))
}

pub(crate) fn thread_to_transcript_cells(
    thread: Thread,
    raw_reasoning_visibility: RawReasoningVisibility,
    config: Option<&Config>,
) -> TranscriptCells {
    let cwd = thread.cwd;
    let thread_id = ThreadId::from_string(&thread.id).ok();
    let mut cells = thread_items_to_transcript_cells(
        thread_id,
        &cwd,
        thread.turns.into_iter().flat_map(|turn| turn.items),
        raw_reasoning_visibility,
        config,
    );
    if cells.is_empty() {
        cells.push(Arc::new(PlainHistoryCell::new(vec![
            "No transcript content available".italic().dim().into(),
        ])));
    }
    cells
}

pub(crate) fn thread_items_to_transcript_cells(
    thread_id: Option<ThreadId>,
    cwd: &AbsolutePathBuf,
    items: impl IntoIterator<Item = ThreadItem>,
    raw_reasoning_visibility: RawReasoningVisibility,
    config: Option<&Config>,
) -> TranscriptCells {
    let inline_visualization_context = config.and_then(|config| {
        thread_id.and_then(|thread_id| InlineVisualizationContext::from_config(config, thread_id))
    });
    let mut cells: TranscriptCells = Vec::new();
    for item in items {
        if let Some(cell) = tools::historical_tool_fallback(&item) {
            cells.push(Arc::new(cell));
        } else {
            cells.extend(item_to_cells(
                item,
                cwd,
                raw_reasoning_visibility,
                inline_visualization_context.clone(),
            ));
        }
    }
    cells
}

/// Project one item without changing the active widget or its turn lifecycle.
fn item_to_cells(
    item: ThreadItem,
    cwd: &AbsolutePathBuf,
    raw_reasoning_visibility: RawReasoningVisibility,
    inline_visualization_context: Option<InlineVisualizationContext>,
) -> TranscriptCells {
    let mut cells: TranscriptCells = Vec::new();
    match item {
        ThreadItem::UserMessage {
            id,
            client_id,
            content,
        } => {
            if content.iter().any(|input| {
                matches!(
                    input,
                    UserInput::Audio { .. } | UserInput::LocalAudio { .. }
                )
            }) {
                tracing::warn!(
                    user_message_id = id,
                    "audio user inputs are not supported by the TUI and will be omitted"
                );
            }
            let item = UserMessageItem {
                id,
                client_id,
                content: content
                    .into_iter()
                    .map(codex_app_server_protocol::UserInput::into_core)
                    .collect(),
            };
            let message = item.message();
            let reply_text = crate::async_question_reply::display_text(&message);
            let text_elements = if reply_text.is_some() {
                Vec::new()
            } else {
                item.text_elements()
            };
            cells.push(Arc::new(UserHistoryCell {
                spoken: false,
                message: reply_text.unwrap_or(message),
                text_elements,
                local_image_paths: item.local_image_paths(),
                remote_image_urls: item.image_urls(),
            }));
        }
        ThreadItem::AgentMessage { text, .. } => {
            let parsed = parse_assistant_markdown(&text, cwd.as_path());
            if !parsed.visible_markdown.trim().is_empty() {
                cells.push(Arc::new(AgentMarkdownCell::new_with_inline_visualizations(
                    parsed.visible_markdown,
                    cwd.as_path(),
                    inline_visualization_context,
                )));
            }
        }
        ThreadItem::FunctionCallOutput {
            name,
            namespace,
            output,
            ..
        } => {
            if let Some((source_thread_id, prompt)) =
                crate::dynamic_tools::parse_delegated_tool_output(
                    &name,
                    namespace.as_deref(),
                    &output,
                )
            {
                cells.push(Arc::new(PrefixedWrappedHistoryCell::new(
                    format!("Sent by Codex from task {source_thread_id}\n{prompt}"),
                    "• ".dim(),
                    "  ",
                )));
            }
        }
        ThreadItem::Plan { text, .. } => {
            if !text.trim().is_empty() {
                cells.push(Arc::new(crate::history_cell::new_proposed_plan(
                    text,
                    cwd.as_path(),
                )));
            }
        }
        ThreadItem::Reasoning {
            summary, content, ..
        } => {
            let (header, text) =
                if matches!(raw_reasoning_visibility, RawReasoningVisibility::Visible)
                    && !content.is_empty()
                {
                    ("Reasoning".to_string(), content.join("\n\n"))
                } else {
                    split_reasoning_summary_parts(&summary)
                };
            if !text.trim().is_empty() {
                cells.push(Arc::new(ReasoningSummaryCell::new(
                    header,
                    text,
                    cwd.as_path(),
                    /*transcript_only*/ false,
                )));
            }
        }
        item @ ThreadItem::CommandExecution { .. } => {
            if let Some(command) = tools::CommandHistory::from_item(item) {
                cells.push(Arc::new(command.into_cell()));
            }
        }
        item @ ThreadItem::McpToolCall { .. } => {
            if let Some(call) = tools::McpHistory::from_item(item) {
                cells.push(Arc::new(call.into_cell()));
            }
        }
        other => cells.extend(other_items::cells(other, cwd)),
    }
    cells
}
