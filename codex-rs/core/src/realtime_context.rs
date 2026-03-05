use crate::codex::Session;
use crate::git_info::resolve_root_git_project_for_trust;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;
use chrono::Utc;
use codex_state::SortKey;
use codex_state::ThreadMetadata;
use dirs::home_dir;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::DirEntry;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tracing::debug;
use tracing::warn;

const STARTUP_CONTEXT_HEADER: &str = "Startup context from Codex.\nThis is background context about recent work and machine/workspace layout. It may be incomplete or stale. Use it to inform responses, and do not repeat it back unless relevant.";
const RECENT_WORK_SECTION_TOKEN_BUDGET: usize = 2_200;
const WORKSPACE_SECTION_TOKEN_BUDGET: usize = 1_600;
const NOTES_SECTION_TOKEN_BUDGET: usize = 300;
const MAX_RECENT_THREADS: usize = 40;
const MAX_RECENT_WORK_GROUPS: usize = 8;
const MAX_RECENT_WORK_ENTRIES_PER_GROUP: usize = 4;
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
    let recent_threads = load_recent_threads(sess).await;
    let recent_work_section = build_recent_work_section(&cwd, &recent_threads);
    let workspace_section = build_workspace_section(&cwd);

    if recent_work_section.is_none() && workspace_section.is_none() {
        debug!("realtime startup context unavailable; skipping injection");
        return None;
    }

    let mut parts = vec![STARTUP_CONTEXT_HEADER.to_string()];

    let has_recent_work_section = recent_work_section.is_some();
    let has_workspace_section = workspace_section.is_some();

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
        Some("Built at realtime startup from persisted thread metadata in the state DB and a bounded local workspace scan. This excludes repo memory instructions, AGENTS files, project-doc prompt blends, and memory summaries.".to_string()),
        NOTES_SECTION_TOKEN_BUDGET,
    ) {
        parts.push(section);
    }

    let context = truncate_text(&parts.join("\n\n"), TruncationPolicy::Tokens(budget_tokens));
    debug!(
        approx_tokens = approx_token_count(&context),
        bytes = context.len(),
        has_recent_work_section,
        has_workspace_section,
        "built realtime startup context"
    );
    Some(context)
}

async fn load_recent_threads(sess: &Session) -> Vec<ThreadMetadata> {
    let Some(state_db) = sess.services.state_db.as_ref() else {
        return Vec::new();
    };

    match state_db
        .list_threads(
            MAX_RECENT_THREADS,
            None,
            SortKey::UpdatedAt,
            &[],
            None,
            false,
            None,
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
        groups.entry(entry.cwd.clone()).or_default().push(entry);
    }

    let current_cwd = cwd.to_path_buf();
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left_cwd, left_entries), (right_cwd, right_entries)| {
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
            *left_cwd != current_cwd,
            Reverse(left_latest),
            left_cwd.as_os_str(),
        )
            .cmp(&(
                *right_cwd != current_cwd,
                Reverse(right_latest),
                right_cwd.as_os_str(),
            ))
    });

    let sections = groups
        .into_iter()
        .take(MAX_RECENT_WORK_GROUPS)
        .filter_map(|(group_cwd, mut entries)| {
            entries.sort_by_key(|entry| Reverse(entry.updated_at));
            format_thread_group(&group_cwd, entries)
        })
        .collect::<Vec<_>>();
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

fn build_workspace_section(cwd: &Path) -> Option<String> {
    build_workspace_section_with_user_root(cwd, home_dir())
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
        format!("Working directory name: {}", display_name(cwd)),
    ];

    if let Some(git_root) = &git_root {
        lines.push(format!("Git root: {}", git_root.display()));
        lines.push(format!("Git project: {}", display_name(git_root)));
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
    collect_tree_lines(root, 0, &mut lines);
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
    NOISY_DIR_NAMES.iter().any(|noisy| *noisy == name)
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

fn format_thread_group(cwd: &Path, entries: Vec<&ThreadMetadata>) -> Option<String> {
    let latest = entries.first()?;
    let mut lines = vec![
        format!("### {}", cwd.display()),
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

    for entry in entries.into_iter().take(MAX_RECENT_WORK_ENTRIES_PER_GROUP) {
        let summary = if !entry.title.trim().is_empty() {
            entry.title.trim()
        } else if let Some(first_user_message) = entry.first_user_message.as_deref() {
            first_user_message.trim()
        } else {
            continue;
        };
        if !summary.is_empty() {
            lines.push(format!("- {summary}"));
        }
    }

    (lines.len() > 4).then(|| lines.join("\n"))
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
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
mod tests {
    use super::build_recent_work_section;
    use super::build_workspace_section;
    use super::build_workspace_section_with_user_root;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_state::ThreadMetadata;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn thread_metadata(cwd: &str, title: &str, first_user_message: &str) -> ThreadMetadata {
        ThreadMetadata {
            id: ThreadId::new(),
            rollout_path: PathBuf::from("/tmp/rollout.jsonl"),
            created_at: Utc
                .timestamp_opt(1_709_251_100, 0)
                .single()
                .expect("valid timestamp"),
            updated_at: Utc
                .timestamp_opt(1_709_251_200, 0)
                .single()
                .expect("valid timestamp"),
            source: "cli".to_string(),
            agent_nickname: None,
            agent_role: None,
            model_provider: "test-provider".to_string(),
            cwd: PathBuf::from(cwd),
            cli_version: "test".to_string(),
            title: title.to_string(),
            sandbox_policy: "workspace-write".to_string(),
            approval_mode: "never".to_string(),
            tokens_used: 0,
            first_user_message: Some(first_user_message.to_string()),
            archived_at: None,
            git_sha: None,
            git_branch: Some("main".to_string()),
            git_origin_url: None,
        }
    }

    #[test]
    fn workspace_section_requires_meaningful_structure() {
        let cwd = TempDir::new().expect("tempdir");
        assert_eq!(
            build_workspace_section_with_user_root(cwd.path(), None),
            None
        );
    }

    #[test]
    fn workspace_section_includes_tree_when_entries_exist() {
        let cwd = TempDir::new().expect("tempdir");
        fs::create_dir(cwd.path().join("docs")).expect("create docs dir");
        fs::write(cwd.path().join("README.md"), "hello").expect("write readme");

        let section = build_workspace_section(cwd.path()).expect("workspace section");
        assert!(section.contains("Working directory tree:"));
        assert!(section.contains("- docs/"));
        assert!(section.contains("- README.md"));
    }

    #[test]
    fn workspace_section_includes_user_root_tree_when_distinct() {
        let root = TempDir::new().expect("tempdir");
        let cwd = root.path().join("cwd");
        let git_root = root.path().join("git");
        let user_root = root.path().join("home");

        fs::create_dir_all(cwd.join("docs")).expect("create cwd docs dir");
        fs::write(cwd.join("README.md"), "hello").expect("write cwd readme");
        fs::create_dir_all(git_root.join(".git")).expect("create git dir");
        fs::write(git_root.join("Cargo.toml"), "[workspace]").expect("write git root marker");
        fs::create_dir_all(user_root.join("code")).expect("create user root child");
        fs::write(user_root.join(".zshrc"), "export TEST=1").expect("write home file");

        let section = build_workspace_section_with_user_root(cwd.as_path(), Some(user_root))
            .expect("workspace section");
        assert!(section.contains("User root tree:"));
        assert!(section.contains("- code/"));
        assert!(section.contains("- .zshrc"));
    }

    #[test]
    fn recent_work_section_groups_threads_by_cwd() {
        let recent_threads = vec![
            thread_metadata(
                "/tmp/workspace-a",
                "Investigate realtime startup context",
                "ignored",
            ),
            thread_metadata(
                "/tmp/workspace-a",
                "Trim websocket startup payload",
                "ignored",
            ),
            thread_metadata("/tmp/workspace-b", "", "Inspect flaky test"),
        ];
        let current_cwd = PathBuf::from("/tmp/workspace-a");

        let section = build_recent_work_section(current_cwd.as_path(), &recent_threads)
            .expect("recent work section");
        assert!(section.contains("### /tmp/workspace-a"));
        assert!(section.contains("Recent sessions: 2"));
        assert!(section.contains("Investigate realtime startup context"));
        assert!(section.contains("### /tmp/workspace-b"));
        assert!(section.contains("Inspect flaky test"));
    }
}
