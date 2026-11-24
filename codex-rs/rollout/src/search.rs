use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::strip_user_message_prefix;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::pending;
use regex::Regex;
use regex::RegexBuilder;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::ARCHIVED_SESSIONS_SUBDIR;
use super::SESSIONS_SUBDIR;
use super::compression;
use crate::ResponseItemEnvelope;
use crate::RolloutItem;

mod compressed;
mod snippet_matcher;
use compressed::scan_compressed_rollout_matches;
pub use snippet_matcher::RolloutSnippetMatcher;

const MATCH_CONTEXT_BEFORE_CHARS: usize = 48;
const MATCH_CONTEXT_AFTER_CHARS: usize = 96;

/// Logical `.jsonl` paths discovered by search, regardless of physical representation.
pub type RolloutSearchMatches = HashSet<PathBuf>;

/// A potential match discovered by the rollout scanner.
#[derive(Debug, PartialEq, Eq)]
pub struct RolloutSearchCandidate {
    pub path: PathBuf,
}

pub async fn search_rollout_paths(
    rg_command: &Path,
    codex_home: &Path,
    archived: bool,
    search_term: &str,
) -> io::Result<HashSet<PathBuf>> {
    search_rollout_matches(rg_command, codex_home, archived, search_term).await
}

pub async fn search_rollout_matches(
    rg_command: &Path,
    codex_home: &Path,
    archived: bool,
    search_term: &str,
) -> io::Result<RolloutSearchMatches> {
    search_rollout_matches_with_progress(
        rg_command,
        codex_home,
        archived,
        search_term,
        /*progress*/ None,
        Box::pin(pending()),
    )
    .await
}

/// Search plain rollouts, then compressed rollouts, reporting candidates without blocking.
pub async fn search_rollout_matches_with_progress(
    rg_command: &Path,
    codex_home: &Path,
    archived: bool,
    search_term: &str,
    progress: Option<mpsc::Sender<RolloutSearchCandidate>>,
    cancelled: BoxFuture<'_, ()>,
) -> io::Result<RolloutSearchMatches> {
    let root = codex_home.join(if archived {
        ARCHIVED_SESSIONS_SUBDIR
    } else {
        SESSIONS_SUBDIR
    });
    let json_search_term = json_escaped_search_term(search_term)?;
    let cancelled = cancelled.shared();
    let mut cancellation: BoxFuture<'_, ()> = Box::pin(cancelled.clone());
    let mut matches = match ripgrep_rollout_paths(
        rg_command,
        &root,
        &json_search_term,
        progress.clone(),
        &mut cancellation,
    )
    .await?
    {
        Some(paths) => paths,
        None => tokio::select! {
            biased;
            () = cancelled.clone() => return Err(search_cancelled()),
            result = scan_plain_rollout_matches(&root, &json_search_term, progress.clone()) => result?,
        },
    };
    // Compression publishes the archive before removing its plain sibling. Discover archives
    // after the plain scan so a transition cannot fall between the two inventories.
    let compressed = tokio::select! {
        biased;
        () = cancelled => return Err(search_cancelled()),
        result = scan_compressed_rollout_matches(&root, search_term, progress) => result?,
    };
    matches.extend(compressed);
    Ok(matches)
}

fn search_cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "rollout search cancelled")
}

async fn ripgrep_rollout_paths(
    rg_command: &Path,
    root: &Path,
    search_term: &str,
    mut progress: Option<mpsc::Sender<RolloutSearchCandidate>>,
    cancelled: &mut BoxFuture<'_, ()>,
) -> io::Result<Option<HashSet<PathBuf>>> {
    let exists = tokio::select! {
        biased;
        () = &mut *cancelled => return Err(search_cancelled()),
        exists = tokio::fs::try_exists(root) => exists.unwrap_or(false),
    };
    if !exists {
        return Ok(Some(HashSet::new()));
    }

    let mut command = Command::from(codex_utils_process::background_command(rg_command));
    // A NUL-only file listing does not flush through ripgrep's line buffer. A one-match
    // count adds a newline after each NUL-delimited path and still stops at the first hit.
    command.args(["--count", "--max-count", "1", "--null"]);
    if progress.is_some() {
        command.arg("--line-buffered");
    }
    let mut child = match command
        .arg("--fixed-strings")
        .arg("--ignore-case")
        .arg("--no-ignore")
        .arg("--glob")
        .arg("*.jsonl")
        .arg("--")
        .arg(search_term)
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut stdout = BufReader::new(
        child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing ripgrep stdout"))?,
    );
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("missing ripgrep stderr"))?;
    let mut sink = tokio::io::sink();
    let read_paths = async {
        let mut paths = HashSet::new();
        let mut record = Vec::new();
        let mut count = [0; 2];
        while stdout.read_until(0, &mut record).await? != 0 {
            if record.pop() != Some(0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unterminated ripgrep path",
                ));
            }
            stdout.read_exact(&mut count).await?;
            if count != *b"1\n" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid ripgrep count",
                ));
            }
            let path = path_from_rg_bytes(&record);
            let path = if path.is_absolute() {
                path
            } else {
                root.join(path)
            };
            if paths.insert(path.clone()) {
                report_candidate(&mut progress, RolloutSearchCandidate { path });
            }
            record.clear();
        }
        Ok::<_, io::Error>(paths)
    };
    let (paths, stderr_bytes, status) = tokio::select! {
        biased;
        () = &mut *cancelled => {
            // Cancellation completes only after ripgrep is reaped.
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Err(search_cancelled());
        }
        result = async {
            tokio::try_join!(read_paths, tokio::io::copy(&mut stderr, &mut sink), child.wait())
        } => result?,
    };
    if !status.success() {
        if status.code() == Some(1) && stderr_bytes == 0 {
            return Ok(Some(HashSet::new()));
        }
        return Err(io::Error::other(format!(
            "ripgrep rollout search failed under {}",
            root.display()
        )));
    }

    Ok(Some(paths))
}

#[cfg(unix)]
fn path_from_rg_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    Path::new(std::ffi::OsStr::from_bytes(bytes)).to_path_buf()
}

#[cfg(not(unix))]
fn path_from_rg_bytes(bytes: &[u8]) -> PathBuf {
    String::from_utf8_lossy(bytes).into_owned().into()
}

fn report_candidate(
    progress: &mut Option<mpsc::Sender<RolloutSearchCandidate>>,
    candidate: RolloutSearchCandidate,
) {
    if let Some(sender) = progress
        && matches!(
            sender.try_send(candidate),
            Err(mpsc::error::TrySendError::Closed(_))
        )
    {
        *progress = None;
    }
}

async fn scan_plain_rollout_matches(
    root: &Path,
    json_search_term: &str,
    mut progress: Option<mpsc::Sender<RolloutSearchCandidate>>,
) -> io::Result<RolloutSearchMatches> {
    let mut matches = HashSet::new();
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
                continue;
            }
            if rollout_contains(rollout_file.path(), &json_search_term).await? {
                let path = rollout_file.into_path();
                report_candidate(&mut progress, RolloutSearchCandidate { path: path.clone() });
                matches.insert(path);
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

fn json_escaped_search_term(search_term: &str) -> io::Result<String> {
    let serialized = serde_json::to_string(search_term).map_err(io::Error::other)?;
    Ok(serialized[1..serialized.len() - 1].to_string())
}

fn case_insensitive_literal_regex(search_term: impl AsRef<str>) -> io::Result<Regex> {
    #[cfg(test)]
    tests::REGEX_COMPILATIONS.set(tests::REGEX_COMPILATIONS.get() + 1);
    RegexBuilder::new(regex::escape(search_term.as_ref()).as_str())
        .case_insensitive(true)
        .build()
        .map_err(io::Error::other)
}

fn content_match_snippet(jsonl_line: &str, search_term: &Regex) -> Option<String> {
    let rollout_line = crate::parse_rollout_line(jsonl_line.trim()).ok()?;
    let text = conversation_text_from_item(&rollout_line.item)?;
    excerpt_around_match(text.as_str(), search_term)
}

fn conversation_text_from_item(item: &RolloutItem) -> Option<String> {
    match item {
        RolloutItem::EventMsg {
            event: EventMsg::UserMessage(user),
            ..
        } => {
            let text = strip_user_message_prefix(user.message.as_str());
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        RolloutItem::EventMsg {
            event: EventMsg::AgentMessage(agent),
            ..
        } => {
            if agent.message.trim().is_empty() {
                None
            } else {
                Some(agent.message.trim().to_string())
            }
        }
        RolloutItem::ResponseItem(ResponseItemEnvelope {
            item: ResponseItem::Message { role, content, .. },
            ..
        }) => {
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
        RolloutItem::DynamicToolsUpdated { .. }
        | RolloutItem::SessionMeta(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg { .. }
        | RolloutItem::ResponseItem(_)
        | RolloutItem::InterAgentCommunication(_)
        | RolloutItem::InterAgentCommunicationMetadata { .. }
        | RolloutItem::Compacted(_)
        | RolloutItem::RealtimeItem(_)
        | RolloutItem::RetainedContext(_)
        | RolloutItem::SecurityRiskScore(_)
        | RolloutItem::TokenUsageRecord(_)
        | RolloutItem::WorldState(_) => None,
    }
}

fn content_item_text(item: &ContentItem) -> Option<&str> {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text, .. } => {
            Some(text.as_str())
        }
        ContentItem::InputImage { .. } | ContentItem::InputAudio { .. } => None,
    }
}

fn excerpt_around_match(text: &str, search_term: &Regex) -> Option<String> {
    let matched = search_term.find(text)?;
    let excerpt_start = char_start_before(text, matched.start(), MATCH_CONTEXT_BEFORE_CHARS);
    let excerpt_end = char_end_after(text, matched.end(), MATCH_CONTEXT_AFTER_CHARS);
    let excerpt = normalize_preview_text(&text[excerpt_start..excerpt_end]);
    if excerpt.is_empty() {
        return None;
    }

    let mut snippet = String::new();
    if excerpt_start > 0 {
        snippet.push_str("... ");
    }
    snippet.push_str(&excerpt);
    if excerpt_end < text.len() {
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

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
