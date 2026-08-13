//! Complete, Markdown-preserving conversation exports.

use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_protocol::ThreadId;
use codex_protocol::models::local_image_label_text;

use super::App;
use crate::app_event::TranscriptExportDestination;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::HistoryHydrationScope;
use crate::app_server_session::is_history_pagination_unsupported;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::ProposedPlanCell;
use crate::history_cell::ReasoningSummaryCell;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;
use crate::history_cell::raw_lines_from_source;
use crate::legacy_core::config::Config;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;

impl App {
    pub(super) async fn export_transcript(
        &mut self,
        app_server: &mut AppServerSession,
        destination: TranscriptExportDestination,
    ) -> Result<(), String> {
        let thread_id = self
            .chat_widget
            .thread_id()
            .ok_or_else(|| "No active conversation to export.".to_string())?;
        let visibility = if self.config.show_raw_agent_reasoning {
            RawReasoningVisibility::Visible
        } else {
            RawReasoningVisibility::Hidden
        };
        let cells = load_export_transcript(
            app_server,
            thread_id,
            visibility,
            Some(&self.config),
            self.transcript_cells.clone(),
        )
        .await?;
        let markdown = render_markdown_transcript(&cells)?;
        match destination {
            TranscriptExportDestination::Clipboard => {
                self.chat_widget.copy_transcript_to_clipboard(&markdown);
            }
            TranscriptExportDestination::File(path) => {
                let cwd = if self.app_server_target.uses_remote_workspace() {
                    self.launch_cwd.as_path()
                } else {
                    self.chat_widget.config_ref().cwd.as_path()
                };
                let path = write_transcript(cwd, &path, &markdown)?;
                self.chat_widget.add_info_message(
                    format!("Saved conversation to {}", path.display()),
                    /*hint*/ None,
                );
            }
        }
        Ok(())
    }
}

pub(super) async fn load_export_transcript(
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
    visibility: RawReasoningVisibility,
    config: Option<&Config>,
    visible_transcript: Vec<Arc<dyn HistoryCell>>,
) -> Result<Vec<Arc<dyn HistoryCell>>, String> {
    let mut thread = app_server
        .thread_read(thread_id, /*include_turns*/ false)
        .await
        .map_err(|error| format!("could not load conversation: {error}"))?;
    if thread.ephemeral {
        return Ok(visible_transcript);
    }
    if let Err(error) = app_server
        .hydrate_initial_thread_history(
            &mut thread,
            /*turn_cursor*/ None,
            /*item_cursor*/ None,
            /*config*/ None,
            HistoryHydrationScope::Complete,
        )
        .await
    {
        if matches!(
            error.downcast_ref::<TypedRequestError>(),
            Some(TypedRequestError::Server { source, .. })
                if is_history_pagination_unsupported(source)
        ) {
            match app_server
                .thread_read(thread_id, /*include_turns*/ true)
                .await
            {
                Ok(legacy) if !legacy.turns.is_empty() => thread = legacy,
                _ => return Ok(visible_transcript),
            }
        } else {
            return Err(format!("could not load conversation history: {error}"));
        }
    }
    let mut cells: Vec<Arc<dyn HistoryCell>> = Vec::new();
    for item in visible_export_items(thread.turns) {
        if let Some(cell) = export_activity_cell(&item) {
            cells.push(Arc::new(cell));
        } else {
            cells.extend(thread_items_to_transcript_cells(
                Some(thread_id),
                &thread.cwd,
                [item],
                visibility,
                config,
            ));
        }
    }
    Ok(cells)
}

fn export_activity_cell(item: &ThreadItem) -> Option<PlainHistoryCell> {
    let lines = match item {
        ThreadItem::FileChange {
            changes, status, ..
        } => {
            let mut lines =
                vec![format!("file changes: {status:?} · {} changes", changes.len()).into()];
            for change in changes {
                lines.push(format!("{:?}: {}", change.kind, change.path).into());
                lines.extend(change.diff.lines().map(|line| line.to_string().into()));
            }
            lines
        }
        ThreadItem::McpToolCall {
            server,
            tool,
            status,
            arguments,
            result,
            error,
            ..
        } => {
            let mut lines =
                vec![format!("mcp tool: {server}/{tool}({arguments}) · {status:?}").into()];
            if let Some(result) = result {
                for content in &result.content {
                    match serde_json::from_value::<rmcp::model::ContentBlock>(content.clone()) {
                        Ok(rmcp::model::ContentBlock::Text(text)) => {
                            lines.extend(raw_lines_from_source(&text.text));
                        }
                        Ok(rmcp::model::ContentBlock::Image(_)) => {
                            lines.push("<image content>".into());
                        }
                        Ok(rmcp::model::ContentBlock::Audio(_)) => {
                            lines.push("<audio content>".into());
                        }
                        Ok(rmcp::model::ContentBlock::Resource(_)) => {
                            let uri = content
                                .pointer("/resource/uri")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("<unknown embedded resource>");
                            lines.push(format!("embedded resource: {uri}").into());
                        }
                        Ok(rmcp::model::ContentBlock::ResourceLink(link)) => {
                            lines.push(format!("link: {}", link.uri).into());
                        }
                        _ => lines.push(content.to_string().into()),
                    }
                }
                if let Some(content) = &result.structured_content {
                    lines.push(format!("structured result: {content}").into());
                }
            }
            if let Some(error) = error {
                lines.extend(raw_lines_from_source(&format!("error: {}", error.message)));
            }
            lines
        }
        _ => return None,
    };
    Some(PlainHistoryCell::new(lines))
}

fn visible_export_items(turns: Vec<Turn>) -> Vec<ThreadItem> {
    let mut visible = Vec::new();
    let mut review_mode = false;
    let mut previous_turn = None;

    for turn in turns {
        let hidden_nested_review_turn = previous_turn.as_ref().is_some_and(|previous| {
            crate::app_backtrack::is_hidden_nested_review_turn(previous, &turn)
        });
        for item in turn.items.iter().cloned() {
            match item {
                ThreadItem::EnteredReviewMode { .. } | ThreadItem::ExitedReviewMode { .. } => {
                    review_mode = matches!(item, ThreadItem::EnteredReviewMode { .. });
                    visible.push(item);
                }
                ThreadItem::UserMessage { .. } if review_mode || hidden_nested_review_turn => {}
                _ => visible.push(item),
            }
        }
        previous_turn = Some(turn);
    }

    visible
}

fn render_markdown_transcript(cells: &[Arc<dyn HistoryCell>]) -> Result<String, String> {
    let mut markdown = String::from("# Codex conversation\n");
    for cell in cells {
        let lines = if let Some(user) = cell.as_any().downcast_ref::<UserHistoryCell>() {
            let (message, _) =
                crate::ide_context::extract_prompt_request_with_offset(&user.message);
            let message = crate::history_cell::sanitize_user_text(message.into());
            let mut lines = raw_lines_from_source(&message);
            let image_count = user.local_image_paths.len() + user.remote_image_urls.len();
            let image_labels = (0..image_count)
                .map(|index| local_image_label_text(index + 1))
                .filter(|label| !message.contains(label))
                .collect::<Vec<_>>();
            if !image_labels.is_empty() && !lines.is_empty() {
                lines.push("".into());
            }
            lines.extend(image_labels.into_iter().map(Into::into));
            lines
        } else {
            cell.raw_lines()
        };
        if lines.is_empty()
            || cell.as_any().is::<SessionInfoCell>()
            || cell.as_any().is::<PlainHistoryCell>()
                && lines.first().is_some_and(|line| {
                    let text = line.to_string();
                    [
                        "• Saved conversation to ",
                        "• Copied conversation to clipboard",
                        "■ Export failed: ",
                        "■ Copy failed: ",
                    ]
                    .iter()
                    .any(|prefix| text.starts_with(prefix))
                })
        {
            continue;
        }
        let (heading, indent) = if cell.as_any().is::<UserHistoryCell>() {
            ("User", false)
        } else if cell.as_any().is::<AgentMarkdownCell>() {
            ("Assistant", false)
        } else if cell.as_any().is::<ProposedPlanCell>() {
            ("Plan", false)
        } else if cell.as_any().is::<ReasoningSummaryCell>() {
            ("Reasoning", false)
        } else {
            ("Activity", true)
        };
        markdown.push_str(&format!("\n## {heading}\n\n"));
        for line in lines {
            if indent {
                markdown.push_str("    ");
            }
            for span in line.spans {
                markdown.push_str(&crate::history_cell::sanitize_user_text(span.content));
            }
            markdown.push('\n');
        }
    }
    if markdown != "# Codex conversation\n" {
        Ok(markdown)
    } else {
        Err("No conversation content to export.".to_string())
    }
}

fn write_transcript(cwd: &Path, requested_path: &Path, markdown: &str) -> Result<PathBuf, String> {
    let path = if let Ok(relative) = requested_path.strip_prefix("~") {
        dirs::home_dir()
            .ok_or_else(|| "could not determine the home directory".to_string())?
            .join(relative)
    } else if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        cwd.join(requested_path)
    };
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(cwd))
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    file.write_all(markdown.as_bytes())
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    file.persist_noclobber(&path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
#[path = "transcript_export_tests.rs"]
mod tests;
