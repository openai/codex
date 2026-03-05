use crate::codex::Session;
use crate::git_info::resolve_root_git_project_for_trust;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;
use codex_state::Stage1Output;
use std::ffi::OsStr;
use std::fs::DirEntry;
use std::io;
use std::path::Path;
use tracing::debug;
use tracing::warn;

const STARTUP_CONTEXT_HEADER: &str = "Startup context from Codex.\nThis is background context about the user, recent work, and machine/workspace layout. It may be incomplete or stale. Use it to inform responses, and do not repeat it back unless relevant.";
const USER_SECTION_TOKEN_BUDGET: usize = 800;
const RECENT_WORK_SECTION_TOKEN_BUDGET: usize = 2_200;
const WORKSPACE_SECTION_TOKEN_BUDGET: usize = 1_600;
const NOTES_SECTION_TOKEN_BUDGET: usize = 300;
const MAX_STAGE1_OUTPUTS: usize = 3;
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
    let memories = load_global_memories(sess).await;
    let user_section = build_user_section(&memories);
    let recent_work_section = build_recent_work_section(&memories);
    let workspace_section = build_workspace_section(&cwd);

    if user_section.is_none() && recent_work_section.is_none() && workspace_section.is_none() {
        debug!("realtime startup context unavailable; skipping injection");
        return None;
    }

    let notes_section = build_notes_section();
    let mut parts = vec![STARTUP_CONTEXT_HEADER.to_string()];

    let has_user_section = user_section.is_some();
    let has_recent_work_section = recent_work_section.is_some();
    let has_workspace_section = workspace_section.is_some();

    if let Some(section) = format_section("User", user_section, USER_SECTION_TOKEN_BUDGET) {
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
    if let Some(section) = format_section("Notes", Some(notes_section), NOTES_SECTION_TOKEN_BUDGET)
    {
        parts.push(section);
    }

    let context = truncate_text(&parts.join("\n\n"), TruncationPolicy::Tokens(budget_tokens));
    debug!(
        approx_tokens = approx_token_count(&context),
        bytes = context.len(),
        has_user_section,
        has_recent_work_section,
        has_workspace_section,
        "built realtime startup context"
    );
    Some(context)
}

#[derive(Default)]
struct GlobalMemories {
    entries: Vec<Stage1Output>,
}

async fn load_global_memories(sess: &Session) -> GlobalMemories {
    let Some(state_db) = sess.services.state_db.as_ref() else {
        return GlobalMemories::default();
    };

    match state_db
        .list_stage1_outputs_for_global(MAX_STAGE1_OUTPUTS)
        .await
    {
        Ok(entries) => GlobalMemories { entries },
        Err(err) => {
            warn!("failed to load realtime startup memories from state db: {err}");
            GlobalMemories::default()
        }
    }
}

fn build_user_section(memories: &GlobalMemories) -> Option<String> {
    let sections = memories
        .entries
        .iter()
        .filter_map(|entry| format_memory_entry(entry, &entry.raw_memory))
        .collect::<Vec<_>>();
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

fn build_recent_work_section(memories: &GlobalMemories) -> Option<String> {
    let sections = memories
        .entries
        .iter()
        .filter_map(|entry| format_memory_entry(entry, &entry.rollout_summary))
        .collect::<Vec<_>>();
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

fn build_workspace_section(cwd: &Path) -> Option<String> {
    let git_root = resolve_root_git_project_for_trust(cwd);
    let cwd_tree = render_tree(cwd);
    let git_root_tree = git_root
        .as_ref()
        .filter(|git_root| git_root.as_path() != cwd)
        .and_then(|git_root| render_tree(git_root));
    let parent_layout = git_root
        .as_ref()
        .and_then(|_| cwd.parent())
        .and_then(|parent| render_parent_layout(parent, cwd.file_name()));

    if cwd_tree.is_none() && git_root.is_none() && parent_layout.is_none() {
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

    if let Some(layout) = parent_layout {
        lines.push(String::new());
        lines.push("Parent workspace layout:".to_string());
        lines.extend(layout);
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

fn render_parent_layout(parent: &Path, current: Option<&OsStr>) -> Option<Vec<String>> {
    let entries = read_sorted_entries(parent).ok()?;
    if entries.len() <= 1 {
        return None;
    }

    let mut lines = Vec::new();
    for entry in entries.into_iter().take(DIR_ENTRY_LIMIT) {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let mut label = name.to_string_lossy().into_owned();
        if file_type.is_dir() {
            label.push('/');
        }
        if current.is_some_and(|current| current == name) {
            label.push_str(" (current)");
        }
        lines.push(format!("- {label}"));
    }
    (!lines.is_empty()).then_some(lines)
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

fn build_notes_section() -> String {
    "Built at realtime startup from persisted global Codex memories in the state DB and a bounded local workspace scan. This excludes repo memory instructions, AGENTS files, and project-doc prompt blends.".to_string()
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

fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

fn format_memory_entry(entry: &Stage1Output, body: &str) -> Option<String> {
    let body = normalize_text(body);
    if body.is_empty() {
        return None;
    }

    let mut lines = vec![
        format!("### {}", entry.source_updated_at.to_rfc3339()),
        format!("cwd: {}", entry.cwd.display()),
    ];
    if let Some(git_branch) = entry.git_branch.as_deref() {
        lines.push(format!("git_branch: {git_branch}"));
    }
    lines.push(String::new());
    lines.push(body);
    Some(lines.join("\n"))
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
    use super::GlobalMemories;
    use super::build_recent_work_section;
    use super::build_user_section;
    use super::build_workspace_section;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_state::Stage1Output;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn stage1_output(raw_memory: &str, rollout_summary: &str) -> Stage1Output {
        Stage1Output {
            thread_id: ThreadId::new(),
            rollout_path: PathBuf::from("/tmp/rollout.jsonl"),
            source_updated_at: Utc
                .timestamp_opt(1_709_251_200, 0)
                .single()
                .expect("valid timestamp"),
            raw_memory: raw_memory.to_string(),
            rollout_summary: rollout_summary.to_string(),
            rollout_slug: Some("slug".to_string()),
            cwd: PathBuf::from("/tmp/workspace"),
            git_branch: Some("main".to_string()),
            generated_at: Utc
                .timestamp_opt(1_709_251_260, 0)
                .single()
                .expect("valid timestamp"),
        }
    }

    #[test]
    fn workspace_section_requires_meaningful_structure() {
        let cwd = TempDir::new().expect("tempdir");
        assert_eq!(build_workspace_section(cwd.path()), None);
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
    fn recent_work_section_uses_rollout_summaries() {
        let memories = GlobalMemories {
            entries: vec![stage1_output("user memory", "recent")],
        };

        let section = build_recent_work_section(&memories).expect("recent work section");
        assert!(section.contains("cwd: /tmp/workspace"));
        assert!(section.contains("recent"));
    }

    #[test]
    fn user_section_uses_raw_memories() {
        let memories = GlobalMemories {
            entries: vec![stage1_output("prefers concise updates", "recent")],
        };

        let section = build_user_section(&memories).expect("user section");
        assert!(section.contains("prefers concise updates"));
        assert!(section.contains("git_branch: main"));
    }
}
