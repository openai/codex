use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

use bm25::Document;
use bm25::Language;
use bm25::SearchEngineBuilder;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::RolloutItem;

use crate::INTERACTIVE_SESSION_SOURCES;
use crate::RolloutRecorder;
use crate::config::Config;
use crate::content_items_to_text;
use crate::find_thread_names_by_ids;
use crate::parse_turn_item;
use crate::rollout::list::Cursor;
use crate::rollout::list::ThreadItem;
use crate::rollout::list::ThreadSortKey;

const RECALL_PAGE_SIZE: usize = 128;
const RECALL_SNIPPET_MAX_CHARS: usize = 240;

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecallHit {
    pub thread_id: ThreadId,
    pub score: f32,
    pub thread_name: Option<String>,
    pub path: PathBuf,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub cwd: Option<PathBuf>,
    pub snippet: String,
}

pub async fn search_sessions(
    config: &Config,
    query: &str,
    current_thread_id: Option<ThreadId>,
    limit: usize,
) -> io::Result<Vec<SessionRecallHit>> {
    let query = query.trim();
    if query.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let mut items = list_all_threads(config, /*archived*/ false).await?;
    items.extend(list_all_threads(config, /*archived*/ true).await?);

    let thread_ids = items
        .iter()
        .filter_map(|item| item.thread_id)
        .filter(|thread_id| Some(*thread_id) != current_thread_id)
        .collect::<HashSet<_>>();
    let thread_names = find_thread_names_by_ids(&config.codex_home, &thread_ids)
        .await
        .unwrap_or_default();

    let mut entries = Vec::new();
    for item in items {
        let Some(thread_id) = item.thread_id else {
            continue;
        };
        if Some(thread_id) == current_thread_id {
            continue;
        }
        let Ok(history) = RolloutRecorder::get_rollout_history(item.path.as_path()).await else {
            continue;
        };

        let transcript = transcript_text(&history);
        let thread_name = thread_names.get(&thread_id).cloned();
        let search_text = build_search_text(&item, thread_name.as_deref(), &transcript);
        if search_text.trim().is_empty() {
            continue;
        }
        let snippet = select_snippet(query, &transcript, &item, thread_name.as_deref());
        entries.push((item, thread_id, thread_name, snippet, search_text));
    }

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let documents = entries
        .iter()
        .enumerate()
        .map(|(idx, (_, _, _, _, search_text))| Document::new(idx, search_text.clone()))
        .collect::<Vec<_>>();
    let search_engine =
        SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();

    Ok(search_engine
        .search(query, limit)
        .into_iter()
        .filter_map(|result| {
            entries
                .get(result.document.id)
                .map(
                    |(item, thread_id, thread_name, snippet, _)| SessionRecallHit {
                        thread_id: *thread_id,
                        score: result.score,
                        thread_name: thread_name.clone(),
                        path: item.path.clone(),
                        created_at: item.created_at.clone(),
                        updated_at: item.updated_at.clone(),
                        cwd: item.cwd.clone(),
                        snippet: snippet.clone(),
                    },
                )
        })
        .collect())
}

pub fn format_recall_draft(query: &str, hits: &[SessionRecallHit]) -> String {
    let mut draft = format!("Recalled context from previous Codex sessions for query: {query}\n\n");

    for (index, hit) in hits.iter().enumerate() {
        let title = hit
            .thread_name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(hit.snippet.as_str());
        let updated_at = hit.updated_at.as_deref().or(hit.created_at.as_deref());
        let cwd = hit
            .cwd
            .as_ref()
            .map(|cwd| cwd.display().to_string())
            .unwrap_or_else(|| "unknown cwd".to_string());
        let updated_line = updated_at
            .map(|updated_at| format!("Updated: {updated_at}\n"))
            .unwrap_or_default();

        draft.push_str(&format!(
            "{}. {title}\nThread: {}\n{}Cwd: {cwd}\nRollout: {}\nExcerpt: {}\n\n",
            index + 1,
            hit.thread_id,
            updated_line,
            hit.path.display(),
            hit.snippet
        ));
    }

    draft.push_str("Use this recalled context if it is relevant to my next message.");
    draft
}

async fn list_all_threads(config: &Config, archived: bool) -> io::Result<Vec<ThreadItem>> {
    let mut items = Vec::new();
    let mut cursor: Option<Cursor> = None;

    loop {
        let page = if archived {
            RolloutRecorder::list_archived_threads(
                config,
                RECALL_PAGE_SIZE,
                cursor.as_ref(),
                ThreadSortKey::UpdatedAt,
                INTERACTIVE_SESSION_SOURCES.as_slice(),
                /*model_providers*/ None,
                &config.model_provider_id,
                /*search_term*/ None,
            )
            .await?
        } else {
            RolloutRecorder::list_threads(
                config,
                RECALL_PAGE_SIZE,
                cursor.as_ref(),
                ThreadSortKey::UpdatedAt,
                INTERACTIVE_SESSION_SOURCES.as_slice(),
                /*model_providers*/ None,
                &config.model_provider_id,
                /*search_term*/ None,
            )
            .await?
        };

        items.extend(page.items);
        let Some(next_cursor) = page.next_cursor else {
            break;
        };
        cursor = Some(next_cursor);
    }

    Ok(items)
}

fn transcript_text(history: &InitialHistory) -> String {
    history
        .get_rollout_items()
        .into_iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(response_item) => match parse_turn_item(&response_item) {
                Some(TurnItem::UserMessage(user)) => {
                    let message = user.message();
                    (!message.trim().is_empty()).then_some(message)
                }
                Some(TurnItem::AgentMessage(agent)) => {
                    let text = agent
                        .content
                        .into_iter()
                        .map(|content| match content {
                            codex_protocol::items::AgentMessageContent::Text { text } => text,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    (!text.trim().is_empty()).then_some(text)
                }
                Some(TurnItem::Plan(plan)) => {
                    let text = plan.text;
                    (!text.trim().is_empty()).then_some(text)
                }
                _ => match response_item {
                    codex_protocol::models::ResponseItem::Message { content, .. } => {
                        content_items_to_text(&content).filter(|text| !text.trim().is_empty())
                    }
                    _ => None,
                },
            },
            RolloutItem::EventMsg(ev) => match ev {
                codex_protocol::protocol::EventMsg::UserMessage(user) => {
                    let message = user.message;
                    (!message.trim().is_empty()).then_some(message)
                }
                _ => None,
            },
            RolloutItem::SessionMeta(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::Compacted(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn build_search_text(item: &ThreadItem, thread_name: Option<&str>, transcript: &str) -> String {
    let mut parts = Vec::new();
    if let Some(thread_name) = thread_name
        && !thread_name.trim().is_empty()
    {
        parts.push(thread_name.to_string());
    }
    if let Some(message) = item.first_user_message.as_deref()
        && !message.trim().is_empty()
    {
        parts.push(message.to_string());
    }
    if let Some(cwd) = item.cwd.as_ref() {
        parts.push(cwd.display().to_string());
    }
    if let Some(branch) = item.git_branch.as_deref()
        && !branch.trim().is_empty()
    {
        parts.push(branch.to_string());
    }
    if !transcript.trim().is_empty() {
        parts.push(transcript.to_string());
    }
    parts.join("\n\n")
}

fn select_snippet(
    query: &str,
    transcript: &str,
    item: &ThreadItem,
    thread_name: Option<&str>,
) -> String {
    let terms = query_terms(query);
    let candidate = transcript
        .lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && terms
                    .iter()
                    .any(|term| line.to_ascii_lowercase().contains(term.as_str()))
        })
        .or_else(|| {
            transcript
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
        })
        .or(item.first_user_message.as_deref())
        .or(thread_name)
        .unwrap_or("No text excerpt available.");

    truncate_snippet(candidate)
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(str::to_string)
        .collect()
}

fn truncate_snippet(text: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx == RECALL_SNIPPET_MAX_CHARS {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
#[path = "recall_tests.rs"]
mod tests;
