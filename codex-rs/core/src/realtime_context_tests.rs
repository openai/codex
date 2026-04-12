use super::build_current_thread_section;
use super::build_recent_work_section;
use super::build_workspace_section_with_user_root;
use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_state::ThreadMetadata;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
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
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
        model_provider: "test-provider".to_string(),
        model: Some("gpt-5".to_string()),
        reasoning_effort: None,
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

fn message(role: &str, content: ContentItem) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: vec![content],
        end_turn: None,
        phase: None,
    }
}

fn user_message(text: impl Into<String>) -> ResponseItem {
    message("user", ContentItem::InputText { text: text.into() })
}

fn assistant_message(text: impl Into<String>) -> ResponseItem {
    message("assistant", ContentItem::OutputText { text: text.into() })
}

fn long_turn_text(index: usize) -> String {
    format!(
        "turn-{index}-start {} turn-{index}-middle {} turn-{index}-end",
        "head filler ".repeat(160),
        "tail filler ".repeat(240),
    )
}

#[test]
fn current_thread_section_includes_short_turns_newest_first_until_budget() {
    let items = vec![
        user_message("user turn 1"),
        assistant_message("assistant turn 1"),
        user_message("user turn 2"),
        assistant_message("assistant turn 2"),
        user_message("user turn 3"),
        assistant_message("assistant turn 3"),
        user_message("user turn 4"),
        assistant_message("assistant turn 4"),
    ];

    assert_eq!(
        build_current_thread_section(&items),
        Some(
            r#"Most recent user/assistant turns from this exact thread. Use them for continuity when responding.

### Latest turn
User:
user turn 4

Assistant:
assistant turn 4

### Previous turn 1
User:
user turn 3

Assistant:
assistant turn 3

### Previous turn 2
User:
user turn 2

Assistant:
assistant turn 2

### Previous turn 3
User:
user turn 1

Assistant:
assistant turn 1"#
                .to_string()
        )
    );
}

#[test]
fn current_thread_turn_truncation_preserves_start_and_end() {
    let items = vec![user_message(long_turn_text(/*index*/ 0))];
    let section = build_current_thread_section(&items).expect("current thread section");

    assert_eq!(
        (
            section.contains("turn-0-start"),
            section.contains("turn-0-middle"),
            section.contains("turn-0-end"),
            section.contains("tokens truncated"),
        ),
        (true, false, true, true),
    );
}

#[test]
fn current_thread_section_keeps_latest_turns_when_history_exceeds_budget() {
    let mut items = Vec::new();
    for index in 1..=8 {
        items.push(user_message(long_turn_text(index)));
        items.push(assistant_message(format!("assistant turn {index}")));
    }

    let section = build_current_thread_section(&items).expect("current thread section");

    assert_eq!(
        (
            section.contains("turn-8-start"),
            section.contains("turn-8-end"),
            section.contains("### Previous turn 2"),
            section.contains("turn-1-start"),
            section.contains("turn-1-end"),
        ),
        (true, true, true, false, false),
    );
}

#[test]
fn workspace_section_requires_meaningful_structure() {
    let cwd = TempDir::new().expect("tempdir");
    assert_eq!(
        build_workspace_section_with_user_root(cwd.path(), /*user_root*/ None),
        None
    );
}

#[test]
fn workspace_section_includes_tree_when_entries_exist() {
    let cwd = TempDir::new().expect("tempdir");
    fs::create_dir(cwd.path().join("docs")).expect("create docs dir");
    fs::write(cwd.path().join("README.md"), "hello").expect("write readme");

    let section = build_workspace_section_with_user_root(cwd.path(), /*user_root*/ None)
        .expect("workspace section");
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
    assert!(!section.contains("- .zshrc"));
}

#[test]
fn recent_work_section_groups_threads_by_cwd() {
    let root = TempDir::new().expect("tempdir");
    let repo = root.path().join("repo");
    let workspace_a = repo.join("workspace-a");
    let workspace_b = repo.join("workspace-b");
    let outside = root.path().join("outside");

    fs::create_dir(&repo).expect("create repo dir");
    Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["init"])
        .current_dir(&repo)
        .output()
        .expect("git init");
    fs::create_dir_all(&workspace_a).expect("create workspace a");
    fs::create_dir_all(&workspace_b).expect("create workspace b");
    fs::create_dir_all(&outside).expect("create outside dir");

    let recent_threads = vec![
        thread_metadata(
            workspace_a.to_string_lossy().as_ref(),
            "Investigate realtime startup context",
            "Log the startup context before sending it",
        ),
        thread_metadata(
            workspace_b.to_string_lossy().as_ref(),
            "Trim websocket startup payload",
            "Remove memories from the realtime startup context",
        ),
        thread_metadata(outside.to_string_lossy().as_ref(), "", "Inspect flaky test"),
    ];
    let current_cwd = workspace_a;
    let repo = fs::canonicalize(repo).expect("canonicalize repo");

    let section = build_recent_work_section(current_cwd.as_path(), &recent_threads)
        .expect("recent work section");
    assert!(section.contains(&format!("### Git repo: {}", repo.display())));
    assert!(section.contains("Recent sessions: 2"));
    assert!(section.contains("User asks:"));
    assert!(section.contains(&format!(
        "- {}: Log the startup context before sending it",
        current_cwd.display()
    )));
    assert!(section.contains(&format!("### Directory: {}", outside.display())));
    assert!(section.contains(&format!("- {}: Inspect flaky test", outside.display())));
}
