use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartMarkerKind {
    RolloutLineTimestamp,
    RolloutLineIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum StartMarkerValue {
    Timestamp(String),
    LineIndex(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartMarker {
    pub kind: StartMarkerKind,
    pub value: StartMarkerValue,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RolloutStartSelectorKind {
    /// Find the first `event_msg` line where `payload.type == "user_message"` and
    /// `payload.message` contains `contains`.
    EventMsgUserMessageContains,
    /// Find the first `response_item` line where `payload.role == "user"` and the combined text
    /// from `payload.content[*].text` contains `contains`.
    ResponseItemUserMessageContains,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolloutInfo {
    pub filename: String,
    /// Preferred, deterministic selector for slicing the rollout when reproducing a case.
    pub start_selector: RolloutStartSelector,
    /// Debugging hint only; do not use this for correctness when slicing.
    pub start: StartMarker,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolloutStartSelector {
    pub kind: RolloutStartSelectorKind,
    pub contains: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitRemote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoGitInfo {
    /// Base commit to `git checkout` before applying `repo.patch`.
    pub commit: String,
    pub remotes: Vec<GitRemote>,
    pub canonical_remote: Option<String>,
    pub reproducible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproducible_reason: Option<String>,
    pub is_dirty: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub describe: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoInfo {
    /// Repository root as reported by `git rev-parse --show-toplevel` (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// Relative path from `repo.root` to capture-time cwd (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd_rel: Option<String>,
    pub git: RepoGitInfo,
    pub patch_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Notes {
    pub what_went_wrong: String,
    pub what_good_looks_like: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artifacts {
    pub include_logs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalCaseManifestV1 {
    pub version: String,
    pub case_id: String,
    pub created_at: String,
    pub conversation_id: String,
    pub source: String,
    pub rollout: RolloutInfo,
    pub repo: RepoInfo,
    pub notes: Notes,
    pub artifacts: Artifacts,
}

#[derive(Debug, Clone)]
pub struct CreateEvalCaseArgs {
    pub codex_home: PathBuf,
    pub conversation_id: String,
    pub rollout_path: PathBuf,
    pub start: StartMarker,
    pub repo_cwd: PathBuf,
    /// When present, derive `repo.patch` from the provided commit snapshot instead of the current
    /// working tree.
    pub repo_snapshot: Option<RepoSnapshot>,
    pub notes: Notes,
    pub include_logs: bool,
    pub logs_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSnapshot {
    pub base_sha: String,
    pub commit_sha: String,
}

#[derive(Debug, Clone)]
pub struct CreateEvalCaseResult {
    pub case_id: String,
    pub path: PathBuf,
}

fn sanitize_repo_slug(input: &str) -> String {
    // Keep it short + filesystem-friendly.
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for ch in input.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_dash = false;
            continue;
        }
        if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "repo".to_string()
    } else {
        out
    }
}

fn repo_slug(repo_cwd: &Path) -> String {
    // Prefer the git repo root name, but fall back to the cwd basename.
    let top_level = git_stdout(repo_cwd, &["rev-parse", "--show-toplevel"])
        .ok()
        .map(PathBuf::from);
    let basename = top_level
        .as_ref()
        .and_then(|p| p.file_name())
        .or_else(|| repo_cwd.file_name());
    let basename = basename
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());
    sanitize_repo_slug(&basename)
}

pub fn create_eval_case_bundle(args: &CreateEvalCaseArgs) -> anyhow::Result<CreateEvalCaseResult> {
    let created_at = OffsetDateTime::now_utc();
    let created_at_rfc3339 = created_at.format(&Rfc3339).context("format created_at")?;

    let ts_for_id = format!(
        "{:04}-{:02}-{:02}T{:02}-{:02}-{:02}",
        created_at.year(),
        u8::from(created_at.month()),
        created_at.day(),
        created_at.hour(),
        created_at.minute(),
        created_at.second()
    );

    // Short, human-scannable id: datetime + repo + 6-digit suffix.
    // Collision risk is low and acceptable for local bundles.
    let repo = repo_slug(&args.repo_cwd);
    let id6 = (Uuid::new_v4().as_u128() % 1_000_000) as u32;
    let case_id = format!("{ts_for_id}-{repo}-{id6:06}");

    let bundle_dir = args.codex_home.join("eval-case").join(&case_id);
    std::fs::create_dir_all(&bundle_dir)
        .with_context(|| format!("create eval bundle dir {}", bundle_dir.display()))?;

    let rollout_dst = bundle_dir.join("rollout.jsonl");
    std::fs::copy(&args.rollout_path, &rollout_dst).with_context(|| {
        format!(
            "copy rollout {} -> {}",
            args.rollout_path.display(),
            rollout_dst.display()
        )
    })?;

    let rollout_text = std::fs::read_to_string(&args.rollout_path).with_context(|| {
        format!(
            "read rollout for start selector {}",
            args.rollout_path.display()
        )
    })?;

    let (base_sha, patch) = match args.repo_snapshot.as_ref() {
        Some(snapshot) => {
            git_patch_between_commits(&args.repo_cwd, &snapshot.base_sha, &snapshot.commit_sha)
                .with_context(|| {
                    format!(
                        "generate patch for repo snapshot {}..{}",
                        snapshot.base_sha, snapshot.commit_sha
                    )
                })?
        }
        None => git_patch_against_head(&args.repo_cwd)?,
    };
    let patch_path = bundle_dir.join("repo.patch");
    std::fs::write(&patch_path, patch)
        .with_context(|| format!("write patch {}", patch_path.display()))?;

    if args.include_logs {
        let logs_path = bundle_dir.join("codex-logs.log");
        let bytes = args
            .logs_bytes
            .clone()
            .unwrap_or_else(|| Vec::with_capacity(0));
        std::fs::write(&logs_path, bytes)
            .with_context(|| format!("write logs {}", logs_path.display()))?;
    }

    let repo_root = git_stdout(&args.repo_cwd, &["rev-parse", "--show-toplevel"])
        .ok()
        .filter(|s| !s.is_empty());
    let cwd_rel = repo_root
        .as_ref()
        .and_then(|root| {
            let root = PathBuf::from(root);
            args.repo_cwd
                .strip_prefix(&root)
                .ok()
                .map(|p| p.display().to_string())
        })
        .filter(|s| !s.is_empty());

    let (git_remotes, remotes_ok) = match git_remotes(&args.repo_cwd) {
        Ok(remotes) => (remotes, true),
        Err(_) => (Vec::new(), false),
    };
    let canonical_remote = select_canonical_remote(&git_remotes);
    let git_is_dirty = git_is_dirty(&args.repo_cwd).unwrap_or(false);
    let git_describe = git_stdout(
        &args.repo_cwd,
        &["describe", "--tags", "--always", "--dirty"],
    )
    .ok()
    .filter(|s| !s.is_empty());

    let mut reproducible = true;
    let mut reproducible_reason: Option<String> = None;
    if !remotes_ok {
        reproducible = false;
        reproducible_reason = Some("not_a_git_repo".to_string());
    } else if canonical_remote.is_none() {
        reproducible = false;
        reproducible_reason = Some("no_git_remote".to_string());
    }
    if base_sha.len() != 40 || base_sha == "unknown" {
        reproducible = false;
        if reproducible_reason.is_none() {
            reproducible_reason = Some("unknown_commit".to_string());
        }
    }

    let start_selector = derive_rollout_start_selector(&args.start, &rollout_text);

    let manifest = EvalCaseManifestV1 {
        version: "v1".to_string(),
        case_id: case_id.clone(),
        created_at: created_at_rfc3339,
        conversation_id: args.conversation_id.clone(),
        source: "cli".to_string(),
        rollout: RolloutInfo {
            filename: "rollout.jsonl".to_string(),
            start_selector,
            start: args.start.clone(),
        },
        repo: RepoInfo {
            root: repo_root,
            cwd_rel,
            git: RepoGitInfo {
                commit: base_sha,
                remotes: git_remotes,
                canonical_remote,
                reproducible,
                reproducible_reason,
                is_dirty: git_is_dirty,
                describe: git_describe,
            },
            patch_filename: "repo.patch".to_string(),
        },
        notes: args.notes.clone(),
        artifacts: Artifacts {
            include_logs: args.include_logs,
        },
    };
    let manifest_path = bundle_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest).context("serialize manifest")?;
    std::fs::write(&manifest_path, format!("{manifest_json}\n"))
        .with_context(|| format!("write manifest {}", manifest_path.display()))?;

    Ok(CreateEvalCaseResult {
        case_id,
        path: bundle_dir,
    })
}

fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for part in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect()
}

fn should_show_start_message(message: &str) -> bool {
    // The rollout can include synthetic "user" messages (environment context, AGENTS
    // instructions) that should not be used as start selectors.
    let trimmed = message.trim_start();
    if trimmed.starts_with("<environment_context>") {
        return false;
    }
    if trimmed.starts_with("# AGENTS.md instructions") {
        return false;
    }
    true
}

fn parse_rfc3339(s: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(s, &Rfc3339).ok()
}

fn is_after_timestamp(timestamp: &str, after_timestamp: &str) -> bool {
    match (parse_rfc3339(timestamp), parse_rfc3339(after_timestamp)) {
        (Some(ts), Some(after)) => ts >= after,
        // Fall back to lexicographic compare; RFC3339 timestamps with consistent formatting
        // should still compare correctly.
        _ => timestamp >= after_timestamp,
    }
}

fn derive_rollout_start_selector(start: &StartMarker, rollout_text: &str) -> RolloutStartSelector {
    let after_timestamp = match &start.value {
        StartMarkerValue::Timestamp(ts) => Some(ts.clone()),
        StartMarkerValue::LineIndex(idx) => {
            let idx = usize::try_from(*idx).ok();
            idx.and_then(|i| rollout_text.lines().nth(i))
                .and_then(|line| {
                    serde_json::from_str::<serde_json::Value>(line)
                        .ok()
                        .and_then(|v| {
                            v.get("timestamp")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string)
                        })
                })
        }
    };

    let (kind, message) = if let Some(message) =
        find_first_event_msg_user_message(rollout_text, after_timestamp.as_deref())
    {
        (
            RolloutStartSelectorKind::EventMsgUserMessageContains,
            Some(message),
        )
    } else if let Some(message) =
        find_first_response_item_user_message(rollout_text, after_timestamp.as_deref())
    {
        (
            RolloutStartSelectorKind::ResponseItemUserMessageContains,
            Some(message),
        )
    } else {
        (RolloutStartSelectorKind::EventMsgUserMessageContains, None)
    };

    let contains = message
        .map(|m| truncate_chars(&normalize_whitespace(&m), 200))
        .unwrap_or_default();

    RolloutStartSelector {
        kind,
        contains,
        after_timestamp,
    }
}

fn find_first_event_msg_user_message(
    rollout_text: &str,
    after_timestamp: Option<&str>,
) -> Option<String> {
    rollout_text.lines().find_map(|line| {
        let v = serde_json::from_str::<serde_json::Value>(line).ok()?;
        let timestamp = v.get("timestamp")?.as_str()?;
        if let Some(after) = after_timestamp
            && !is_after_timestamp(timestamp, after)
        {
            return None;
        }

        let ty = v.get("type")?.as_str()?;
        if ty != "event_msg" {
            return None;
        }
        let payload = v.get("payload")?;
        let payload_ty = payload.get("type")?.as_str()?;
        if payload_ty != "user_message" {
            return None;
        }
        let message = payload.get("message")?.as_str()?;
        should_show_start_message(message).then(|| message.to_string())
    })
}

fn find_first_response_item_user_message(
    rollout_text: &str,
    after_timestamp: Option<&str>,
) -> Option<String> {
    rollout_text.lines().find_map(|line| {
        let v = serde_json::from_str::<serde_json::Value>(line).ok()?;
        let timestamp = v.get("timestamp")?.as_str()?;
        if let Some(after) = after_timestamp
            && !is_after_timestamp(timestamp, after)
        {
            return None;
        }

        let ty = v.get("type")?.as_str()?;
        if ty != "response_item" {
            return None;
        }
        let payload = v.get("payload")?;
        let role = payload.get("role")?.as_str()?;
        if role != "user" {
            return None;
        }
        let content = payload.get("content")?.as_array()?;
        let mut out = String::new();
        for item in content {
            let Some(text) = item.get("text").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(text);
        }
        let out = normalize_whitespace(out.as_str());
        (!out.is_empty() && should_show_start_message(&out)).then_some(out)
    })
}

pub fn find_rollout_start_index(
    rollout_text: &str,
    selector: &RolloutStartSelector,
) -> Option<usize> {
    if selector.contains.trim().is_empty() {
        return None;
    }

    let lines = rollout_text.lines().collect::<Vec<_>>();
    let matched_idx = lines.iter().enumerate().find_map(|(idx, line)| {
        let v = serde_json::from_str::<serde_json::Value>(line).ok()?;
        let timestamp = v.get("timestamp")?.as_str()?;
        if let Some(after) = selector.after_timestamp.as_deref()
            && !is_after_timestamp(timestamp, after)
        {
            return None;
        }

        match selector.kind {
            RolloutStartSelectorKind::EventMsgUserMessageContains => {
                let ty = v.get("type")?.as_str()?;
                if ty != "event_msg" {
                    return None;
                }
                let payload = v.get("payload")?;
                let payload_ty = payload.get("type")?.as_str()?;
                if payload_ty != "user_message" {
                    return None;
                }
                let message = payload.get("message")?.as_str()?;
                message.contains(selector.contains.as_str()).then_some(idx)
            }
            RolloutStartSelectorKind::ResponseItemUserMessageContains => {
                let ty = v.get("type")?.as_str()?;
                if ty != "response_item" {
                    return None;
                }
                let payload = v.get("payload")?;
                let role = payload.get("role")?.as_str()?;
                if role != "user" {
                    return None;
                }
                let content = payload.get("content")?.as_array()?;
                let mut out = String::new();
                for item in content {
                    let Some(text) = item.get("text").and_then(serde_json::Value::as_str) else {
                        continue;
                    };
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(text);
                }
                let out = normalize_whitespace(out.as_str());
                out.contains(selector.contains.as_str()).then_some(idx)
            }
        }
    })?;

    // Include immediately preceding turn context lines so replay has the right environment/config.
    let mut start_idx = matched_idx;
    while start_idx > 0 {
        let prev_line = lines.get(start_idx.saturating_sub(1))?;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(prev_line) else {
            break;
        };
        let Some(ty) = v.get("type").and_then(serde_json::Value::as_str) else {
            break;
        };
        if ty != "turn_context" {
            break;
        }
        start_idx = start_idx.saturating_sub(1);
    }

    Some(start_idx)
}

pub fn slice_rollout_from_selector(
    rollout_text: &str,
    selector: &RolloutStartSelector,
) -> Option<String> {
    let start = find_rollout_start_index(rollout_text, selector)?;
    let sliced = rollout_text
        .lines()
        .skip(start)
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!("{sliced}\n"))
}

fn git_patch_against_head(repo_cwd: &Path) -> anyhow::Result<(String, Vec<u8>)> {
    let base_sha =
        git_stdout(repo_cwd, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".to_string());

    let mut patch = Vec::new();
    if let Ok(mut bytes) = git_diff(
        repo_cwd,
        &["diff", "--no-textconv", "--no-ext-diff", "--binary", "HEAD"],
    ) {
        patch.append(&mut bytes);
    }

    let untracked =
        git_stdout(repo_cwd, &["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();
    for file in untracked.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let args = [
            "diff",
            "--no-textconv",
            "--no-ext-diff",
            "--binary",
            "--no-index",
            "--",
            null_device,
            file,
        ];
        if let Ok(mut bytes) = git_diff(repo_cwd, &args) {
            patch.append(&mut bytes);
        }
    }

    Ok((base_sha, patch))
}

fn git_remotes(repo_cwd: &Path) -> anyhow::Result<Vec<GitRemote>> {
    let output = git_stdout(repo_cwd, &["remote", "-v"])?;
    // Use a BTreeMap so iteration is deterministic.
    let mut by_name: std::collections::BTreeMap<String, (Vec<String>, Vec<String>)> =
        std::collections::BTreeMap::new();

    for line in output.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let Some(url) = parts.next() else {
            continue;
        };
        let Some(kind) = parts.next() else {
            continue;
        };

        let entry = by_name
            .entry(name.to_string())
            .or_insert_with(|| (Vec::new(), Vec::new()));
        match kind {
            "(fetch)" => entry.0.push(url.to_string()),
            "(push)" => entry.1.push(url.to_string()),
            _ => {}
        }
    }

    let remotes = by_name
        .into_iter()
        .filter_map(|(name, (mut fetch_urls, mut push_urls))| {
            fetch_urls.sort();
            push_urls.sort();
            let fetch_url = fetch_urls.into_iter().next()?;
            let push_url = push_urls
                .into_iter()
                .next()
                .unwrap_or_else(|| fetch_url.clone());
            Some(GitRemote {
                name,
                fetch_url,
                push_url,
            })
        })
        .collect::<Vec<_>>();
    Ok(remotes)
}

fn select_canonical_remote(remotes: &[GitRemote]) -> Option<String> {
    if let Some(origin) = remotes.iter().find(|r| r.name == "origin") {
        return Some(origin.fetch_url.clone());
    }
    remotes
        .iter()
        .min_by(|a, b| a.name.cmp(&b.name))
        .map(|r| r.fetch_url.clone())
}

fn git_is_dirty(repo_cwd: &Path) -> anyhow::Result<bool> {
    let diff_unstaged = Command::new("git")
        .args(["diff", "--quiet"])
        .current_dir(repo_cwd)
        .status()
        .context("run git diff --quiet")?;
    if !diff_unstaged.success() {
        return Ok(true);
    }

    let diff_staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_cwd)
        .status()
        .context("run git diff --cached --quiet")?;
    if !diff_staged.success() {
        return Ok(true);
    }

    let untracked =
        git_stdout(repo_cwd, &["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();
    Ok(untracked.lines().any(|s| !s.trim().is_empty()))
}

fn git_stdout(repo_cwd: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_cwd)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!("git {} failed with {}", args.join(" "), output.status);
    }
    let out = String::from_utf8(output.stdout).context("decode git stdout")?;
    Ok(out.trim().to_string())
}

fn git_diff(repo_cwd: &Path, args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_cwd)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;

    let exit_ok = output.status.code().is_some_and(|c| c == 0 || c == 1);
    if !exit_ok {
        anyhow::bail!("git {} failed with {}", args.join(" "), output.status);
    }
    Ok(output.stdout)
}

fn git_patch_between_commits(
    repo_cwd: &Path,
    base_sha: &str,
    commit_sha: &str,
) -> anyhow::Result<(String, Vec<u8>)> {
    let patch = git_diff(
        repo_cwd,
        &[
            "diff",
            "--no-textconv",
            "--no-ext-diff",
            "--binary",
            base_sha,
            commit_sha,
        ],
    )?;
    Ok((base_sha.to_string(), patch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn creates_bundle_with_manifest_and_rollout() {
        let codex_home = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let repo_root = repo_dir.path().join("my-repo");
        std::fs::create_dir_all(&repo_root).unwrap();
        std::fs::write(repo_root.join("README.md"), "hi\n").unwrap();
        let init_status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(init_status.success());
        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(add_status.success());
        let commit_status = Command::new("git")
            .args([
                "-c",
                "user.name=codex",
                "-c",
                "user.email=codex@example.com",
                "commit",
                "-m",
                "init",
                "-q",
            ])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(commit_status.success());

        std::fs::write(repo_root.join("README.md"), "changed\n").unwrap();

        let rollout_path = repo_root.join("rollout.jsonl");
        let rollout = [
            r#"{"timestamp":"2024-01-01T00:00:00Z","type":"token_count","payload":{"total":1}}"#,
            r#"{"timestamp":"2024-01-01T00:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"do the thing"}}"#,
            r#"{"timestamp":"2024-01-01T00:00:01Z","type":"event_msg","payload":{"type":"agent_message","message":"ok"}}"#,
        ]
        .join("\n");
        std::fs::write(&rollout_path, format!("{rollout}\n")).unwrap();

        let args = CreateEvalCaseArgs {
            codex_home: codex_home.path().to_path_buf(),
            conversation_id: "conv-1".to_string(),
            rollout_path,
            start: StartMarker {
                kind: StartMarkerKind::RolloutLineIndex,
                value: StartMarkerValue::LineIndex(1),
                display: "Start now".to_string(),
            },
            repo_cwd: repo_root,
            repo_snapshot: None,
            notes: Notes {
                what_went_wrong: "bad".to_string(),
                what_good_looks_like: "good".to_string(),
            },
            include_logs: true,
            logs_bytes: Some(b"logs".to_vec()),
        };

        let out = create_eval_case_bundle(&args).unwrap();
        assert!(!out.case_id.is_empty());
        assert!(out.path.exists());
        assert_eq!(out.path.file_name().unwrap(), out.case_id.as_str());
        assert!(out.path.starts_with(codex_home.path().join("eval-case")));
        assert!(out.case_id.contains("my-repo"));

        let manifest_text = std::fs::read_to_string(out.path.join("manifest.json")).unwrap();
        let manifest: EvalCaseManifestV1 = serde_json::from_str(&manifest_text).unwrap();
        assert_eq!(manifest.version, "v1");
        assert_eq!(manifest.conversation_id, "conv-1");
        assert_eq!(manifest.notes, args.notes);
        assert!(manifest.repo.git.commit != "unknown");
        assert_eq!(manifest.repo.git.canonical_remote, None);
        assert_eq!(manifest.repo.git.reproducible, false);
        assert_eq!(
            manifest.repo.git.reproducible_reason,
            Some("no_git_remote".to_string())
        );
        assert_eq!(manifest.repo.git.is_dirty, true);
        assert_eq!(manifest.artifacts.include_logs, true);
        assert!(!manifest.rollout.start_selector.contains.trim().is_empty());

        assert!(out.path.join("repo.patch").exists());
        assert!(out.path.join("rollout.jsonl").exists());
        assert_eq!(
            std::fs::read(out.path.join("codex-logs.log")).unwrap(),
            b"logs".to_vec()
        );
    }

    #[test]
    fn creates_bundle_from_repo_snapshot_commit() {
        let codex_home = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let repo_root = repo_dir.path().join("my-repo");
        std::fs::create_dir_all(&repo_root).unwrap();

        std::fs::write(repo_root.join("README.md"), "base\n").unwrap();
        let init_status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(init_status.success());
        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(add_status.success());
        let commit_status = Command::new("git")
            .args([
                "-c",
                "user.name=codex",
                "-c",
                "user.email=codex@example.com",
                "commit",
                "-m",
                "base",
                "-q",
            ])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(commit_status.success());

        let base_sha = git_stdout(&repo_root, &["rev-parse", "HEAD"]).unwrap();

        // Create a snapshot commit.
        std::fs::write(repo_root.join("README.md"), "snapshot\n").unwrap();
        std::fs::write(repo_root.join("snap.txt"), "snap\n").unwrap();
        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(add_status.success());
        let commit_status = Command::new("git")
            .args([
                "-c",
                "user.name=codex",
                "-c",
                "user.email=codex@example.com",
                "commit",
                "-m",
                "snapshot",
                "-q",
            ])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(commit_status.success());
        let snapshot_sha = git_stdout(&repo_root, &["rev-parse", "HEAD"]).unwrap();

        // Dirty the working tree after the snapshot commit; this should not affect repo.patch.
        std::fs::write(repo_root.join("README.md"), "worktree\n").unwrap();

        let rollout_path = repo_root.join("rollout.jsonl");
        let rollout = [
            r#"{"timestamp":"2024-01-01T00:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"snapshot run"}}"#,
            r#"{"timestamp":"2024-01-01T00:00:01Z","type":"event_msg","payload":{"type":"agent_message","message":"ok"}}"#,
        ]
        .join("\n");
        std::fs::write(&rollout_path, format!("{rollout}\n")).unwrap();

        let args = CreateEvalCaseArgs {
            codex_home: codex_home.path().to_path_buf(),
            conversation_id: "conv-2".to_string(),
            rollout_path,
            start: StartMarker {
                kind: StartMarkerKind::RolloutLineIndex,
                value: StartMarkerValue::LineIndex(0),
                display: "From: test".to_string(),
            },
            repo_cwd: repo_root,
            repo_snapshot: Some(RepoSnapshot {
                base_sha: base_sha.clone(),
                commit_sha: snapshot_sha,
            }),
            notes: Notes {
                what_went_wrong: "bad".to_string(),
                what_good_looks_like: "good".to_string(),
            },
            include_logs: false,
            logs_bytes: None,
        };

        let out = create_eval_case_bundle(&args).unwrap();
        assert_eq!(out.path.file_name().unwrap(), out.case_id.as_str());
        assert!(out.path.starts_with(codex_home.path().join("eval-case")));
        assert!(out.case_id.contains("my-repo"));
        let manifest_text = std::fs::read_to_string(out.path.join("manifest.json")).unwrap();
        let manifest: EvalCaseManifestV1 = serde_json::from_str(&manifest_text).unwrap();
        assert_eq!(manifest.repo.git.commit, base_sha);

        let patch_text =
            String::from_utf8(std::fs::read(out.path.join("repo.patch")).unwrap()).unwrap();
        assert!(
            patch_text.contains("snapshot"),
            "patch should include snapshot commit changes"
        );
        assert!(
            !patch_text.contains("worktree"),
            "patch should not include post-snapshot working tree changes"
        );
        assert!(patch_text.contains("snap.txt"));
    }

    #[test]
    fn slices_rollout_using_start_selector_even_when_line_numbers_shift() {
        let rollout = [
            r#"{"timestamp":"2024-01-01T00:00:00Z","type":"token_count","payload":{"total":1}}"#,
            r#"{"timestamp":"2024-01-01T00:00:00Z","type":"turn_context","payload":{"cwd":"/tmp","approval_policy":"never","sandbox_policy":"none","model":"x","summary":"none"}}"#,
            r#"{"timestamp":"2024-01-01T00:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"run the tests please"}}"#,
            r#"{"timestamp":"2024-01-01T00:00:01Z","type":"event_msg","payload":{"type":"agent_message","message":"ok"}}"#,
        ]
        .join("\n");

        let selector = RolloutStartSelector {
            kind: RolloutStartSelectorKind::EventMsgUserMessageContains,
            contains: "run the tests".to_string(),
            after_timestamp: Some("2024-01-01T00:00:00Z".to_string()),
        };

        let sliced = slice_rollout_from_selector(&rollout, &selector).unwrap();
        // Should include the turn_context that immediately precedes the matched user_message.
        assert!(
            sliced
                .lines()
                .next()
                .unwrap()
                .contains("\"type\":\"turn_context\"")
        );
        assert!(sliced.contains("\"type\":\"user_message\""));
        assert!(!sliced.contains("\"type\":\"token_count\""));
    }
}
