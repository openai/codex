use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

use super::ARCHIVED_SESSIONS_SUBDIR;
use super::SESSIONS_SUBDIR;
use super::list::ThreadItem;

const MATCH_CONTEXT_BEFORE_CHARS: usize = 48;
const MATCH_CONTEXT_AFTER_CHARS: usize = 96;

/// Compact search-specific context attached only to thread discovery results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadSearchPreview {
    ContentMatch { snippet: String },
}

pub struct ThreadSearchItem {
    pub item: ThreadItem,
    pub search_preview: ThreadSearchPreview,
}

pub struct ThreadSearchMatches {
    content_preview_by_path: HashMap<PathBuf, ThreadSearchPreview>,
}

impl ThreadSearchMatches {
    pub async fn load(codex_home: &Path, archived: bool, search_term: &str) -> io::Result<Self> {
        let root = codex_home.join(if archived {
            ARCHIVED_SESSIONS_SUBDIR
        } else {
            SESSIONS_SUBDIR
        });
        let content_preview_by_path = ripgrep_rollout_matches(root.as_path(), search_term).await?;
        Ok(Self {
            content_preview_by_path,
        })
    }

    pub fn matching_items(&self, items: Vec<ThreadItem>) -> Vec<ThreadSearchItem> {
        items
            .into_iter()
            .filter_map(|item| {
                self.content_preview_by_path
                    .get(item.path.as_path())
                    .cloned()
                    .map(|search_preview| ThreadSearchItem {
                        item,
                        search_preview,
                    })
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RipgrepEvent {
    Match {
        data: RipgrepMatchData,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct RipgrepMatchData {
    path: RipgrepText,
    lines: RipgrepText,
}

#[derive(Debug, Deserialize)]
struct RipgrepText {
    text: Option<String>,
}

async fn ripgrep_rollout_matches(
    root: &Path,
    search_term: &str,
) -> io::Result<HashMap<PathBuf, ThreadSearchPreview>> {
    if !tokio::fs::try_exists(root).await.unwrap_or(false) {
        return Ok(HashMap::new());
    }

    let output = match Command::new("rg")
        .arg("--json")
        .arg("--fixed-strings")
        .arg("--no-ignore")
        .arg("--glob")
        .arg("*.jsonl")
        .arg("--")
        .arg(search_term)
        .arg(root)
        .output()
        .await
    {
        Ok(output) => output,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return scan_rollout_matches(root, search_term).await;
        }
        Err(err) => return Err(err),
    };
    if !output.status.success() {
        if output.status.code() == Some(1) && output.stderr.is_empty() {
            return Ok(HashMap::new());
        }

        return Err(io::Error::other(format!(
            "ripgrep rollout search failed under {}",
            root.display()
        )));
    }

    let mut matches = HashMap::new();
    for line in String::from_utf8_lossy(output.stdout.as_slice()).lines() {
        let Ok(RipgrepEvent::Match { data }) = serde_json::from_str::<RipgrepEvent>(line) else {
            continue;
        };
        let (Some(path), Some(jsonl_line)) = (data.path.text, data.lines.text) else {
            continue;
        };
        let path = PathBuf::from(path);
        let path = if path.is_absolute() {
            path
        } else {
            root.join(path)
        };
        if matches.contains_key(path.as_path()) {
            continue;
        }
        let Some(preview) = content_match_preview(jsonl_line.as_str(), search_term) else {
            continue;
        };
        matches.insert(path, preview);
    }

    Ok(matches)
}

async fn scan_rollout_matches(
    root: &Path,
    search_term: &str,
) -> io::Result<HashMap<PathBuf, ThreadSearchPreview>> {
    let mut matches = HashMap::new();
    let mut dirs = vec![root.to_path_buf()];

    while let Some(dir) = dirs.pop() {
        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                dirs.push(path);
                continue;
            }
            if !file_type.is_file()
                || path.extension().and_then(|extension| extension.to_str()) != Some("jsonl")
            {
                continue;
            }
            if let Some(preview) = first_matching_preview(path.as_path(), search_term).await? {
                matches.insert(path, preview);
            }
        }
    }

    Ok(matches)
}

async fn first_matching_preview(
    path: &Path,
    search_term: &str,
) -> io::Result<Option<ThreadSearchPreview>> {
    let file = tokio::fs::File::open(path).await?;
    let mut lines = tokio::io::BufReader::new(file).lines();
    while let Some(line) = lines.next_line().await? {
        if line.contains(search_term)
            && let Some(preview) = content_match_preview(line.as_str(), search_term)
        {
            return Ok(Some(preview));
        }
    }
    Ok(None)
}

fn content_match_preview(jsonl_line: &str, search_term: &str) -> Option<ThreadSearchPreview> {
    let rollout_line = serde_json::from_str::<RolloutLine>(jsonl_line.trim()).ok()?;
    conversation_text_from_item(&rollout_line.item)
        .into_iter()
        .find_map(|text| match text {
            ConversationText::User(text) | ConversationText::Assistant(text) => {
                excerpt_around_match(text.as_str(), search_term)
            }
        })
        .map(|snippet| ThreadSearchPreview::ContentMatch { snippet })
}

enum ConversationText {
    User(String),
    Assistant(String),
}

fn conversation_text_from_item(item: &RolloutItem) -> Vec<ConversationText> {
    match item {
        RolloutItem::EventMsg(EventMsg::UserMessage(user)) => {
            let text = strip_user_message_prefix(user.message.as_str());
            if text.is_empty() {
                Vec::new()
            } else {
                vec![ConversationText::User(text.to_string())]
            }
        }
        RolloutItem::EventMsg(EventMsg::AgentMessage(agent)) => {
            if agent.message.trim().is_empty() {
                Vec::new()
            } else {
                vec![ConversationText::Assistant(
                    agent.message.trim().to_string(),
                )]
            }
        }
        RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) => {
            let text = content
                .iter()
                .filter_map(content_item_text)
                .collect::<Vec<_>>()
                .join(" ");
            if text.trim().is_empty() {
                Vec::new()
            } else if role == "user" {
                vec![ConversationText::User(text)]
            } else if role == "assistant" {
                vec![ConversationText::Assistant(text)]
            } else {
                Vec::new()
            }
        }
        RolloutItem::SessionMeta(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_)
        | RolloutItem::ResponseItem(_)
        | RolloutItem::Compacted(_) => Vec::new(),
    }
}

fn content_item_text(item: &ContentItem) -> Option<&str> {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text } => Some(text.as_str()),
        ContentItem::InputImage { .. } => None,
    }
}

fn strip_user_message_prefix(text: &str) -> &str {
    match text.find(USER_MESSAGE_BEGIN) {
        Some(idx) => text[idx + USER_MESSAGE_BEGIN.len()..].trim(),
        None => text.trim(),
    }
}

fn excerpt_around_match(text: &str, search_term: &str) -> Option<String> {
    let normalized = normalize_preview_text(text);
    let match_start = normalized.find(search_term)?;
    let match_end = match_start.saturating_add(search_term.len());
    let excerpt_start =
        char_start_before(normalized.as_str(), match_start, MATCH_CONTEXT_BEFORE_CHARS);
    let excerpt_end = char_end_after(normalized.as_str(), match_end, MATCH_CONTEXT_AFTER_CHARS);
    let excerpt = normalized[excerpt_start..excerpt_end].trim();
    if excerpt.is_empty() {
        return None;
    }

    let mut snippet = String::new();
    if excerpt_start > 0 {
        snippet.push_str("... ");
    }
    snippet.push_str(excerpt);
    if excerpt_end < normalized.len() {
        snippet.push_str(" ...");
    }
    Some(snippet)
}

fn normalize_preview_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn char_start_before(text: &str, byte_index: usize, chars_before: usize) -> usize {
    text[..byte_index]
        .char_indices()
        .rev()
        .nth(chars_before)
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn char_end_after(text: &str, byte_index: usize, chars_after: usize) -> usize {
    text[byte_index..]
        .char_indices()
        .nth(chars_after)
        .map(|(offset, _)| byte_index.saturating_add(offset))
        .unwrap_or(text.len())
}
