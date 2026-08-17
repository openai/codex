use std::borrow::Cow;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;

use super::TranscriptPreviewLine;
use super::TranscriptPreviewSpeaker;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::HISTORY_ITEM_PAGE_LIMIT;
use crate::app_server_session::HISTORY_ITEM_SCAN_LIMIT;
use crate::app_server_session::HistoryHydrationScope;
use crate::app_server_session::ThreadParamsMode;
use crate::git_action_directives::parse_assistant_markdown;
use crate::inline_visualization::InlineVisualizationContext;
use crate::legacy_core::config::Config;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadItem;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_rollout::ReverseJsonlScanner;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutLine;
use codex_rollout::ScanOutcome;

const MAX_TRANSCRIPT_PREVIEW_LINES: usize = 6;
const TRANSCRIPT_PREVIEW_ITEMS_PAGE_SIZE: u32 = 6;
const MAX_LEGACY_TRANSCRIPT_PREVIEW_SCAN_BYTES: usize = 1024 * 1024;

/// Loads the newest transcript lines from bounded paginated items or a local legacy rollout tail.
pub(crate) async fn load_transcript_preview(
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
    config: Option<&Config>,
) -> std::io::Result<Vec<TranscriptPreviewLine>> {
    let mut thread = app_server
        .thread_read(thread_id, /*include_turns*/ false)
        .await
        .map_err(std::io::Error::other)?;
    let inline_visualization_context = config.and_then(|config| {
        ThreadId::from_string(&thread.id)
            .ok()
            .and_then(|thread_id| InlineVisualizationContext::from_config(config, thread_id))
    });
    let mut lines = Vec::with_capacity(MAX_TRANSCRIPT_PREVIEW_LINES);
    match thread.history_mode {
        ThreadHistoryMode::Legacy => {
            if matches!(app_server.thread_params_mode(), ThreadParamsMode::Embedded)
                && let Some(path) = thread.path.as_ref()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".jsonl"))
            {
                let path = path.clone();
                let cwd = thread.cwd.clone();
                let inline_visualization_context = inline_visualization_context.clone();
                let scanned = tokio::task::spawn_blocking(move || {
                    scan_legacy_transcript_preview(
                        path.as_path(),
                        cwd.as_path(),
                        inline_visualization_context.as_ref(),
                    )
                })
                .await;
                if let Ok(Ok(Some(mut scanned_lines))) = scanned {
                    scanned_lines.reverse();
                    return Ok(scanned_lines);
                }
            }

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
            append_transcript_preview_lines(
                &mut lines,
                thread
                    .turns
                    .iter()
                    .rev()
                    .flat_map(|turn| turn.items.iter().rev()),
                thread.cwd.as_path(),
                inline_visualization_context.as_ref(),
            );
        }
        ThreadHistoryMode::Paginated => {
            let cwd = thread.cwd.as_path();
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

/// Scans a bounded legacy rollout tail, falling back when its history cannot be proven complete.
fn scan_legacy_transcript_preview(
    path: &Path,
    cwd: &Path,
    inline_visualization_context: Option<&InlineVisualizationContext>,
) -> std::io::Result<Option<Vec<TranscriptPreviewLine>>> {
    let mut lines = Vec::with_capacity(MAX_TRANSCRIPT_PREVIEW_LINES);
    let file = File::open(path)?;
    let end_offset = file.metadata()?.len();
    let minimum_offset = end_offset.saturating_sub(MAX_LEGACY_TRANSCRIPT_PREVIEW_SCAN_BYTES as u64);
    let mut scanner = ReverseJsonlScanner::new_at(
        BoundedRolloutTailReader {
            file,
            minimum_offset,
        },
        end_offset,
    )?
    .with_max_record_bytes(MAX_LEGACY_TRANSCRIPT_PREVIEW_SCAN_BYTES);
    loop {
        let outcome = match scanner.scan_next::<RolloutLine>() {
            Ok(Some(outcome)) => outcome,
            Ok(None) => break,
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error),
        };
        let ScanOutcome::Parsed(line) = outcome else {
            continue;
        };
        match line.item {
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_)) => return Ok(None),
            RolloutItem::EventMsg(EventMsg::UserMessage(message)) => {
                append_transcript_preview_text(
                    &mut lines,
                    TranscriptPreviewSpeaker::User,
                    &message.message,
                    cwd,
                    inline_visualization_context,
                );
            }
            RolloutItem::EventMsg(EventMsg::AgentMessage(message)) => {
                append_transcript_preview_text(
                    &mut lines,
                    TranscriptPreviewSpeaker::Assistant,
                    &message.message,
                    cwd,
                    inline_visualization_context,
                );
            }
            _ => {}
        }
        if lines.len() == MAX_TRANSCRIPT_PREVIEW_LINES {
            break;
        }
    }

    Ok(Some(lines))
}

/// Prevents reverse scans from reading past the bounded suffix of a rollout.
struct BoundedRolloutTailReader {
    file: File,
    minimum_offset: u64,
}

impl Read for BoundedRolloutTailReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buffer)
    }
}

impl Seek for BoundedRolloutTailReader {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let offset = self.file.seek(position)?;
        if offset < self.minimum_offset {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "legacy transcript preview exceeded its scan budget",
            ));
        }
        Ok(offset)
    }
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
