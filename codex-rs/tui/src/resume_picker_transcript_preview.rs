use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;

use super::TranscriptPreviewLine;
use super::TranscriptPreviewSpeaker;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::HISTORY_ITEM_PAGE_LIMIT;
use crate::app_server_session::HISTORY_ITEM_SCAN_LIMIT;
use crate::app_server_session::HistoryHydrationScope;
use crate::git_action_directives::parse_assistant_markdown;
use crate::inline_visualization::InlineVisualizationContext;
use crate::legacy_core::config::Config;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadItem;
use codex_protocol::ThreadId;

const MAX_TRANSCRIPT_PREVIEW_LINES: usize = 6;
const TRANSCRIPT_PREVIEW_ITEMS_PAGE_SIZE: u32 = 6;

/// Loads the newest nonblank transcript lines within a bounded paginated-item scan.
pub(crate) async fn load_transcript_preview(
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
    config: Option<&Config>,
) -> std::io::Result<Vec<TranscriptPreviewLine>> {
    let mut thread = app_server
        .thread_read(thread_id, /*include_turns*/ false)
        .await
        .map_err(std::io::Error::other)?;
    if thread.history_mode == ThreadHistoryMode::Legacy {
        app_server
            .hydrate_initial_thread_history(
                &mut thread,
                /*turn_cursor*/ None,
                /*item_cursor*/ None,
                /*config*/ None,
                HistoryHydrationScope::Initial,
            )
            .await
            .map_err(std::io::Error::other)?;
    }
    let cwd = thread.cwd.as_path();
    let inline_visualization_context = config.and_then(|config| {
        ThreadId::from_string(&thread.id)
            .ok()
            .and_then(|thread_id| InlineVisualizationContext::from_config(config, thread_id))
    });
    let mut lines = Vec::with_capacity(MAX_TRANSCRIPT_PREVIEW_LINES);
    match thread.history_mode {
        ThreadHistoryMode::Legacy => {
            append_transcript_preview_lines(
                &mut lines,
                thread
                    .turns
                    .iter()
                    .rev()
                    .flat_map(|turn| turn.items.iter().rev()),
                cwd,
                inline_visualization_context.as_ref(),
            );
        }
        ThreadHistoryMode::Paginated => {
            let mut cursor = None;
            let mut seen_cursors = HashSet::new();
            let mut scanned_items = 0_usize;
            loop {
                let remaining_items = HISTORY_ITEM_SCAN_LIMIT.saturating_sub(scanned_items);
                let page_size = if cursor.is_none() {
                    TRANSCRIPT_PREVIEW_ITEMS_PAGE_SIZE
                } else {
                    HISTORY_ITEM_PAGE_LIMIT
                }
                .min(remaining_items as u32);
                if page_size == 0 {
                    break;
                }
                let page = app_server
                    .thread_items_page(thread_id, /*turn_id*/ None, cursor.clone(), page_size)
                    .await
                    .map_err(std::io::Error::other)?;
                scanned_items = scanned_items.saturating_add(page.data.len());
                append_transcript_preview_lines(
                    &mut lines,
                    page.data
                        .iter()
                        .take(remaining_items)
                        .map(|entry| &entry.item),
                    cwd,
                    inline_visualization_context.as_ref(),
                );
                if lines.len() == MAX_TRANSCRIPT_PREVIEW_LINES
                    || scanned_items >= HISTORY_ITEM_SCAN_LIMIT
                {
                    break;
                }
                let Some(next_cursor) = page
                    .next_cursor
                    .filter(|next| seen_cursors.insert(next.clone()))
                else {
                    break;
                };
                cursor = Some(next_cursor);
            }
        }
    }

    lines.reverse();
    Ok(lines)
}

/// Appends the newest preview lines from items already ordered newest-first.
fn append_transcript_preview_lines<'a>(
    lines: &mut Vec<TranscriptPreviewLine>,
    items: impl Iterator<Item = &'a ThreadItem>,
    cwd: &Path,
    inline_visualization_context: Option<&InlineVisualizationContext>,
) {
    for item in items {
        match item {
            ThreadItem::UserMessage { content, .. } => {
                let text = content
                    .iter()
                    .filter_map(|input| match input {
                        codex_app_server_protocol::UserInput::Text { text, .. } => {
                            Some(text.as_str())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                append_transcript_preview_text(
                    lines,
                    TranscriptPreviewSpeaker::User,
                    &text,
                    cwd,
                    inline_visualization_context,
                );
            }
            ThreadItem::AgentMessage { text, .. } => {
                append_transcript_preview_text(
                    lines,
                    TranscriptPreviewSpeaker::Assistant,
                    text,
                    cwd,
                    inline_visualization_context,
                );
            }
            _ => continue,
        }
        if lines.len() == MAX_TRANSCRIPT_PREVIEW_LINES {
            break;
        }
    }
}

/// Appends a message's newest nonblank lines while preserving assistant display rewrites.
fn append_transcript_preview_text(
    lines: &mut Vec<TranscriptPreviewLine>,
    speaker: TranscriptPreviewSpeaker,
    text: &str,
    cwd: &Path,
    inline_visualization_context: Option<&InlineVisualizationContext>,
) {
    let visible_markdown;
    let text = match speaker {
        TranscriptPreviewSpeaker::User => Cow::Borrowed(text),
        TranscriptPreviewSpeaker::Assistant => {
            visible_markdown = parse_assistant_markdown(text, cwd).visible_markdown;
            let rewritten = crate::inline_visualization::rewrite_inline_visualizations(
                &visible_markdown,
                inline_visualization_context,
            );
            let mut text = rewritten.markdown;
            for (placeholder, link) in &rewritten.trusted_file_links {
                text = Cow::Owned(text.replace(
                    &format!(
                        "{}  \n[{}]({placeholder})",
                        link.markdown_label, link.markdown_destination_label
                    ),
                    &format!("{}  \n{}", link.display_label, link.destination),
                ));
            }
            text
        }
    };

    let remaining = MAX_TRANSCRIPT_PREVIEW_LINES - lines.len();
    lines.extend(
        text.lines()
            .rev()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .take(remaining)
            .map(|text| TranscriptPreviewLine {
                speaker,
                text: text.to_string(),
            }),
    );
}

#[cfg(test)]
#[path = "resume_picker_transcript_preview_tests.rs"]
mod tests;
