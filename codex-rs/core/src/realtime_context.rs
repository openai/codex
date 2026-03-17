use crate::codex::Session;
use crate::compact::content_items_to_text;
use crate::event_mapping::is_contextual_user_message_content;
use crate::git_info::resolve_root_git_project_for_trust;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;
use chrono::Utc;
use codex_protocol::models::ResponseItem;
use codex_state::SortKey;
use codex_state::ThreadMetadata;
use dirs::home_dir;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::DirEntry;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tracing::debug;
use tracing::info;
use tracing::warn;

const STARTUP_CONTEXT_HEADER: &str = "Startup context from Codex.\nThis is background context about recent work and machine/workspace layout. It may be incomplete or stale. Use it to inform responses, and do not repeat it back unless relevant.";
const CURRENT_THREAD_SECTION_TOKEN_BUDGET: usize = 1_200;
const RECENT_WORK_SECTION_TOKEN_BUDGET: usize = 2_200;
const WORKSPACE_SECTION_TOKEN_BUDGET: usize = 1_600;
const NOTES_SECTION_TOKEN_BUDGET: usize = 300;
const MAX_CURRENT_THREAD_TURNS: usize = 2;
const MAX_RECENT_THREADS: usize = 40;
const MAX_RECENT_WORK_GROUPS: usize = 8;
const MAX_CURRENT_CWD_ASKS: usize = 8;
const MAX_OTHER_CWD_ASKS: usize = 5;
const MAX_ASK_CHARS: usize = 240;
const TREE_MAX_DEPTH: usize = 2;
const DIR_ENTRY_LIMIT: usize = 20;
const APPROX_BYTES_PER_TOKEN: usize = 4;
const NOISY_DIR_NAMES: &[&str] = &[
    ".git",
    ".next",
    ".pytest_cache",
    ".ruff_cache",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "out",
    "target",
];

pub(crate) async fn build_realtime_startup_context(
    sess: &Session,
    budget_tokens: usize,
) -> Option<String> {
    let config = sess.get_config().await;
    let cwd = config.cwd.clone();
    let history = sess.clone_history().await;
    let current_thread_section = build_current_thread_section(history.raw_items());
    let recent_threads = load_recent_threads(sess).await;
    let recent_work_section = build_recent_work_section(&cwd, &recent_threads);
    let workspace_section = build_workspace_section_with_user_root(&cwd, home_dir());

    if current_thread_section.is_none()
        && recent_work_section.is_none()
        && workspace_section.is_none()
    {
        debug!("realtime startup context unavailable; skipping injection");
        return None;
    }

    let mut parts = vec![STARTUP_CONTEXT_HEADER.to_string()];

    let has_current_thread_section = current_thread_section.is_some();
    let has_recent_work_section = recent_work_section.is_some();
    let has_workspace_section = workspace_section.is_some();

    if let Some(section) = format_section(
        "Current Thread",
        current_thread_section,
        CURRENT_THREAD_SECTION_TOKEN_BUDGET,
    ) {
        parts.push(section);
    }
    if let Some(section) = format_section(
        "Recent Work",
        recent_work_section,
        RECENT_WORK_SECTION_TOKEN_BUDGET,
    ) {
        parts.push(section);
    }
    if let Some(section) = format_section(
        "Machine / Workspace Map",
        workspace_section,
        WORKSPACE_SECTION_TOKEN_BUDGET,
    ) {
        parts.push(section);
    }
    if let Some(section) = format_section(
        "Notes",
        Some("Built at realtime startup from the current thread history, persisted thread metadata in the state DB, and a bounded local workspace scan. This excludes repo memory instructions, AGENTS files, project-doc prompt blends, and memory summaries.".to_string()),
        NOTES_SECTION_TOKEN_BUDGET,
    ) {
        parts.push(section);
    }

    let context = truncate_text(&parts.join("\n\n"), TruncationPolicy::Tokens(budget_tokens));
    debug!(
        approx_tokens = approx_token_count(&context),
        bytes = context.len(),
        has_current_thread_section,
        has_recent_work_section,
        has_workspace_section,
        "built realtime startup context"
    );
    info!("realtime startup context: {context}");
    Some(context)
}

async fn load_recent_threads(sess: &Session) -> Vec<ThreadMetadata> {
    let Some(state_db) = sess.services.state_db.as_ref() else {
        return Vec::new();
    };

    match state_db
        .list_threads(
            MAX_RECENT_THREADS,
            /*anchor*/ None,
            SortKey::UpdatedAt,
            &[],
            /*model_providers*/ None,
            /*archived_only*/ false,
            /*search_term*/ None,
        )
        .await
    {
        Ok(page) => page.items,
        Err(err) => {
            warn!("failed to load realtime startup threads from state db: {err}");
            Vec::new()
        }
    }
}

fn build_recent_work_section(cwd: &Path, recent_threads: &[ThreadMetadata]) -> Option<String> {
    let mut groups: HashMap<PathBuf, Vec<&ThreadMetadata>> = HashMap::new();
    for entry in recent_threads {
        let group =
            resolve_root_git_project_for_trust(&entry.cwd).unwrap_or_else(|| entry.cwd.clone());
        groups.entry(group).or_default().push(entry);
    }

    let current_group =
        resolve_root_git_project_for_trust(cwd).unwrap_or_else(|| cwd.to_path_buf());
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left_group, left_entries), (right_group, right_entries)| {
        let left_latest = left_entries
            .iter()
            .map(|entry| entry.updated_at)
            .max()
            .unwrap_or_else(Utc::now);
        let right_latest = right_entries
            .iter()
            .map(|entry| entry.updated_at)
            .max()
            .unwrap_or_else(Utc::now);
        (
            *left_group != current_group,
            Reverse(left_latest),
            left_group.as_os_str(),
        )
            .cmp(&(
                *right_group != current_group,
                Reverse(right_latest),
                right_group.as_os_str(),
            ))
    });

    let sections = groups
        .into_iter()
        .take(MAX_RECENT_WORK_GROUPS)
        .filter_map(|(group, mut entries)| {
            entries.sort_by_key(|entry| Reverse(entry.updated_at));
            format_thread_group(&current_group, &group, entries)
        })
        .collect::<Vec<_>>();
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

fn build_current_thread_section(items: &[ResponseItem]) -> Option<String> {
    let mut turns = Vec::new();
    let mut current_user = Vec::new();
    let mut current_assistant = Vec::new();

    for item in items {
        match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                if is_contextual_user_message_content(content) {
                    continue;
                }
                let Some(text) = content_items_to_text(content)
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                else {
                    continue;
                };
                if !current_user.is_empty() || !current_assistant.is_empty() {
                    turns.push((
                        std::mem::take(&mut current_user),
                        std::mem::take(&mut current_assistant),
                    ));
                }
                current_user.push(text);
            }
            ResponseItem::Message { role, content, .. } if role == "assistant" => {
                let Some(text) = content_items_to_text(content)
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                else {
                    continue;
                };
                if current_user.is_empty() && current_assistant.is_empty() {
                    continue;
                }
                current_assistant.push(text);
            }
            _ => {}
        }
    }

    if !current_user.is_empty() || !current_assistant.is_empty() {
        turns.push((current_user, current_assistant));
    }

    let retained_turns = turns
        .into_iter()
        .rev()
        .take(MAX_CURRENT_THREAD_TURNS)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    if retained_turns.is_empty() {
        return None;
    }

    let mut lines = vec![
        "Most recent user/assistant turns from this exact thread. Use them for continuity when responding.".to_string(),
    ];

    let retained_turn_count = retained_turns.len();
    for (index, (user_messages, assistant_messages)) in retained_turns.into_iter().enumerate() {
        lines.push(String::new());
        if retained_turn_count == 1 || index + 1 == retained_turn_count {
            lines.push("### Latest turn".to_string());
        } else {
            lines.push(format!("### Prior turn {}", index + 1));
        }

        if !user_messages.is_empty() {
            lines.push("User:".to_string());
            lines.push(user_messages.join("\n\n"));
        }
        if !assistant_messages.is_empty() {
            lines.push(String::new());
            lines.push("Assistant:".to_string());
            lines.push(assistant_messages.join("\n\n"));
        }
    }

    Some(lines.join("\n"))
}

fn build_workspace_section_with_user_root(
    cwd: &Path,
    user_root: Option<PathBuf>,
) -> Option<String> {
    let git_root = resolve_root_git_project_for_trust(cwd);
    let cwd_tree = render_tree(cwd);
    let git_root_tree = git_root
        .as_ref()
        .filter(|git_root| git_root.as_path() != cwd)
        .and_then(|git_root| render_tree(git_root));
    let user_root_tree = user_root
        .as_ref()
        .filter(|user_root| user_root.as_path() != cwd)
        .filter(|user_root| {
            git_root
                .as_ref()
                .is_none_or(|git_root| git_root.as_path() != user_root.as_path())
        })
        .and_then(|user_root| render_tree(user_root));

    if cwd_tree.is_none() && git_root.is_none() && user_root_tree.is_none() {
        return None;
    }

    let mut lines = vec![
        format!("Current working directory: {}", cwd.display()),
        format!("Working directory name: {}", file_name_string(cwd)),
    ];

    if let Some(git_root) = &git_root {
        lines.push(format!("Git root: {}", git_root.display()));
        lines.push(format!("Git project: {}", file_name_string(git_root)));
    }
    if let Some(user_root) = &user_root {
        lines.push(format!("User root: {}", user_root.display()));
    }

    if let Some(tree) = cwd_tree {
        lines.push(String::new());
        lines.push("Working directory tree:".to_string());
        lines.extend(tree);
    }

    if let Some(tree) = git_root_tree {
        lines.push(String::new());
        lines.push("Git root tree:".to_string());
        lines.extend(tree);
    }

    if let Some(tree) = user_root_tree {
        lines.push(String::new());
        lines.push("User root tree:".to_string());
        lines.extend(tree);
    }

    Some(lines.join("\n"))
}

fn render_tree(root: &Path) -> Option<Vec<String>> {
    if !root.is_dir() {
        return None;
    }

    let mut lines = Vec::new();
    collect_tree_lines(root, /*depth*/ 0, &mut lines);
    (!lines.is_empty()).then_some(lines)
}

fn collect_tree_lines(dir: &Path, depth: usize, lines: &mut Vec<String>) {
    if depth >= TREE_MAX_DEPTH {
        return;
    }

    let entries = match read_sorted_entries(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let total_entries = entries.len();

    for entry in entries.into_iter().take(DIR_ENTRY_LIMIT) {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = file_name_string(&entry.path());
        let indent = "  ".repeat(depth);
        let suffix = if file_type.is_dir() { "/" } else { "" };
        lines.push(format!("{indent}- {name}{suffix}"));
        if file_type.is_dir() {
            collect_tree_lines(&entry.path(), depth + 1, lines);
        }
    }

    if total_entries > DIR_ENTRY_LIMIT {
        lines.push(format!(
            "{}- ... {} more entries",
            "  ".repeat(depth),
            total_entries - DIR_ENTRY_LIMIT
        ));
    }
}

fn read_sorted_entries(dir: &Path) -> io::Result<Vec<DirEntry>> {
    let mut entries = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| !is_noisy_name(&entry.file_name()))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        let left_is_dir = left
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false);
        let right_is_dir = right
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false);
        (!left_is_dir, file_name_string(&left.path()))
            .cmp(&(!right_is_dir, file_name_string(&right.path())))
    });
    Ok(entries)
}

fn is_noisy_name(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    name.starts_with('.') || NOISY_DIR_NAMES.iter().any(|noisy| *noisy == name)
}

fn format_section(title: &str, body: Option<String>, budget_tokens: usize) -> Option<String> {
    let body = body?;
    let body = body.trim();
    if body.is_empty() {
        return None;
    }

    Some(format!(
        "## {title}\n{}",
        truncate_text(body, TruncationPolicy::Tokens(budget_tokens))
    ))
}

fn format_thread_group(
    current_group: &Path,
    group: &Path,
    entries: Vec<&ThreadMetadata>,
) -> Option<String> {
    let latest = entries.first()?;
    let group_label = if resolve_root_git_project_for_trust(latest.cwd.as_path()).is_some() {
        format!("### Git repo: {}", group.display())
    } else {
        format!("### Directory: {}", group.display())
    };
    let mut lines = vec![
        group_label,
        format!("Recent sessions: {}", entries.len()),
        format!("Latest activity: {}", latest.updated_at.to_rfc3339()),
    ];

    if let Some(git_branch) = latest
        .git_branch
        .as_deref()
        .filter(|git_branch| !git_branch.is_empty())
    {
        lines.push(format!("Latest branch: {git_branch}"));
    }

    lines.push(String::new());
    lines.push("User asks:".to_string());

    let mut seen = HashSet::new();
    let max_asks = if group == current_group {
        MAX_CURRENT_CWD_ASKS
    } else {
        MAX_OTHER_CWD_ASKS
    };

    for entry in entries {
        let Some(first_user_message) = entry.first_user_message.as_deref() else {
            continue;
        };
        let ask = first_user_message
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let dedupe_key = format!("{}:{ask}", entry.cwd.display());
        if ask.is_empty() || !seen.insert(dedupe_key) {
            continue;
        }
        let ask = if ask.chars().count() > MAX_ASK_CHARS {
            format!(
                "{}...",
                ask.chars()
                    .take(MAX_ASK_CHARS.saturating_sub(3))
                    .collect::<String>()
            )
        } else {
            ask
        };
        lines.push(format!("- {}: {ask}", entry.cwd.display()));
        if seen.len() == max_asks {
            break;
        }
    }

    (lines.len() > 5).then(|| lines.join("\n"))
}

fn file_name_string(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

fn approx_token_count(text: &str) -> usize {
    text.len().div_ceil(APPROX_BYTES_PER_TOKEN)
}

#[cfg(test)]
#[path = "realtime_context_tests.rs"]
mod tests;
