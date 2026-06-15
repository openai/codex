use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use regex::Regex;
use regex::RegexBuilder;
use tokio::process::Command;

use super::ARCHIVED_SESSIONS_SUBDIR;
use super::SESSIONS_SUBDIR;
use super::compression;

const MATCH_CONTEXT_BEFORE_CHARS: usize = 48;
const MATCH_CONTEXT_AFTER_CHARS: usize = 96;

/// Search matches keyed by the canonical `.jsonl` path for each rollout.
pub type RolloutSearchMatches = HashMap<PathBuf, Option<String>>;

pub async fn search_rollout_paths(
    rg_command: &Path,
    codex_home: &Path,
    archived: bool,
    search_term: &str,
) -> io::Result<HashSet<PathBuf>> {
    Ok(
        search_rollout_matches(rg_command, codex_home, archived, search_term)
            .await?
            .into_keys()
            .collect(),
    )
}

pub async fn search_rollout_matches(
    rg_command: &Path,
    codex_home: &Path,
    archived: bool,
    search_term: &str,
) -> io::Result<RolloutSearchMatches> {
    let root = codex_home.join(if archived {
        ARCHIVED_SESSIONS_SUBDIR
    } else {
        SESSIONS_SUBDIR
    });
    let json_search_term = json_escaped_search_term(search_term)?;
    let Some(plain_matches) =
        ripgrep_rollout_paths(rg_command, root.as_path(), json_search_term.as_str()).await?
    else {
        return scan_rollout_matches(root.as_path(), json_search_term.as_str(), search_term).await;
    };
    let mut matches: RolloutSearchMatches =
        plain_matches.into_iter().map(|path| (path, None)).collect();
    matches.extend(scan_compressed_rollout_matches(root.as_path(), search_term).await?);
    Ok(matches)
}

async fn ripgrep_rollout_paths(
    rg_command: &Path,
    root: &Path,
    search_term: &str,
) -> io::Result<Option<HashSet<PathBuf>>> {
    if !tokio::fs::try_exists(root).await.unwrap_or(false) {
        return Ok(Some(HashSet::new()));
    }

    let output = match Command::new(rg_command)
        .arg("-l")
        .arg("--fixed-strings")
        .arg("--ignore-case")
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
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    if !output.status.success() {
        if output.status.code() == Some(1) && output.stderr.is_empty() {
            return Ok(Some(HashSet::new()));
        }

        return Err(io::Error::other(format!(
            "ripgrep rollout search failed under {}",
            root.display()
        )));
    }

    let mut matches = HashSet::new();
    for line in String::from_utf8_lossy(output.stdout.as_slice()).lines() {
        let path = PathBuf::from(line);
        let path = if path.is_absolute() {
            path
        } else {
            root.join(path)
        };
        matches.insert(path);
    }

    Ok(Some(matches))
}

async fn scan_rollout_matches(
    root: &Path,
    json_search_term: &str,
    search_term: &str,
) -> io::Result<RolloutSearchMatches> {
    let mut matches = HashMap::new();
    let mut dirs = vec![root.to_path_buf()];
    let json_search_term = case_insensitive_literal_regex(json_search_term)?;

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
            if !file_type.is_file() {
                continue;
            }
            let Some(rollout_file) = compression::RolloutFile::from_path(path) else {
                continue;
            };
            if rollout_file.is_compressed() {
                if let Some(snippet) =
                    first_rollout_content_match_snippet(rollout_file.path(), search_term).await?
                {
                    matches.insert(
                        compression::plain_rollout_path(rollout_file.path()),
                        Some(snippet),
                    );
                }
                continue;
            }
            if rollout_contains(rollout_file.path(), &json_search_term).await? {
                matches.insert(rollout_file.into_path(), None);
            }
        }
    }

    Ok(matches)
}

async fn rollout_contains(path: &Path, search_term: &Regex) -> io::Result<bool> {
    let mut lines = compression::open_rollout_line_reader(path).await?;
    while let Some(line) = lines.next_line().await? {
        if search_term.is_match(line.as_str()) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn first_rollout_content_match_snippet(
    path: &Path,
    search_term: &str,
) -> io::Result<Option<String>> {
    let mut lines = compression::open_rollout_line_reader(path).await?;
    let json_search_term = case_insensitive_literal_regex(json_escaped_search_term(search_term)?)?;
    let search_term = case_insensitive_literal_regex(search_term)?;
    while let Some(line) = lines.next_line().await? {
        if json_search_term.is_match(line.as_str())
            && let Some(snippet) = content_match_snippet(line.as_str(), &search_term)
        {
            return Ok(Some(snippet));
        }
    }
    Ok(None)
}

async fn scan_compressed_rollout_matches(
    root: &Path,
    search_term: &str,
) -> io::Result<RolloutSearchMatches> {
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
            if !file_type.is_file() {
                continue;
            }
            let Some(rollout_file) = compression::RolloutFile::from_path(path) else {
                continue;
            };
            if !rollout_file.is_compressed() {
                continue;
            }
            if let Some(snippet) =
                first_rollout_content_match_snippet(rollout_file.path(), search_term).await?
            {
                matches.insert(
                    compression::plain_rollout_path(rollout_file.path()),
                    Some(snippet),
                );
            }
        }
    }

    Ok(matches)
}

fn json_escaped_search_term(search_term: &str) -> io::Result<String> {
    let serialized = serde_json::to_string(search_term).map_err(io::Error::other)?;
    Ok(serialized[1..serialized.len() - 1].to_string())
}

fn case_insensitive_literal_regex(search_term: impl AsRef<str>) -> io::Result<Regex> {
    RegexBuilder::new(regex::escape(search_term.as_ref()).as_str())
        .case_insensitive(true)
        .build()
        .map_err(io::Error::other)
}

fn content_match_snippet(jsonl_line: &str, search_term: &Regex) -> Option<String> {
    let rollout_line = serde_json::from_str::<RolloutLine>(jsonl_line.trim()).ok()?;
    let text = conversation_text_from_item(&rollout_line.item)?;
    excerpt_around_match(text.as_str(), search_term)
}

fn conversation_text_from_item(item: &RolloutItem) -> Option<String> {
    match item {
        RolloutItem::EventMsg(EventMsg::UserMessage(user)) => {
            let text = strip_user_message_prefix(user.message.as_str());
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        RolloutItem::EventMsg(EventMsg::AgentMessage(agent)) => {
            if agent.message.trim().is_empty() {
                None
            } else {
                Some(agent.message.trim().to_string())
            }
        }
        RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) => {
            let text = content
                .iter()
                .filter_map(content_item_text)
                .collect::<Vec<_>>()
                .join(" ");
            if text.trim().is_empty() || (role != "user" && role != "assistant") {
                None
            } else {
                Some(text)
            }
        }
        RolloutItem::SessionMeta(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_)
        | RolloutItem::ResponseItem(_)
        | RolloutItem::InterAgentCommunication(_)
        | RolloutItem::Compacted(_) => None,
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

fn excerpt_around_match(text: &str, search_term: &Regex) -> Option<String> {
    let normalized = normalize_preview_text(text);
    let matched = search_term.find(normalized.as_str())?;
    let match_start = matched.start();
    let match_end = matched.end();
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
