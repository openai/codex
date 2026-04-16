use super::build_current_thread_section;
use super::build_recent_work_section;
use super::build_workspace_section_with_user_root;
use super::format_startup_context_blob;
use chrono::TimeZone;
use chrono::Utc;
use codex_git_utils::GitSha;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_thread_store::StoredThread;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn stored_thread(cwd: &str, title: &str, first_user_message: &str) -> StoredThread {
    StoredThread {
        thread_id: ThreadId::new(),
        rollout_path: Some(PathBuf::from("/tmp/rollout.jsonl")),
        forked_from_id: None,
        preview: first_user_message.to_string(),
        name: (!title.is_empty()).then(|| title.to_string()),
        model_provider: "test-provider".to_string(),
        model: Some("gpt-5".to_string()),
        reasoning_effort: None,
        created_at: Utc
            .timestamp_opt(1_709_251_100, 0)
            .single()
            .expect("valid timestamp"),
        updated_at: Utc
            .timestamp_opt(1_709_251_200, 0)
            .single()
            .expect("valid timestamp"),
        archived_at: None,
        cwd: PathBuf::from(cwd),
        cli_version: "test".to_string(),
        source: SessionSource::Cli,
        agent_nickname: None,
        agent_role: None,
        agent_path: None,
        git_info: Some(GitInfo {
            commit_hash: Some(GitSha::new("abcdef")),
            branch: Some("main".to_string()),
            repository_url: None,
        }),
        approval_mode: AskForApproval::Never,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        token_usage: None,
        first_user_message: Some(first_user_message.to_string()),
        history: None,
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
fn startup_context_blob_is_wrapped_in_tags_and_fits_budget() {
    let body = format!(
        "Startup context from Codex.\n{}\n{}",
        "recent work ".repeat(1_200),
        "workspace tree ".repeat(800),
    );

    let wrapped = format_startup_context_blob(&body, /*budget_tokens*/ 200);

    assert!(wrapped.starts_with("<startup_context>\n"));
    assert!(wrapped.ends_with("\n</startup_context>"));
    assert!(wrapped.contains("Startup context from Codex."));
    assert!(wrapped.contains("tokens truncated"));
    assert!(wrapped.len().div_ceil(4) <= 200);
}

#[tokio::test]
async fn workspace_section_requires_meaningful_structure() {
    let cwd = TempDir::new().expect("tempdir");
    assert_eq!(
        build_workspace_section_with_user_root(&cwd.path().abs(), /*user_root*/ None).await,
        None
    );
}

#[tokio::test]
async fn workspace_section_includes_tree_when_entries_exist() {
    let cwd = TempDir::new().expect("tempdir");
    fs::create_dir(cwd.path().join("docs")).expect("create docs dir");
    fs::write(cwd.path().join("README.md"), "hello").expect("write readme");

    let section =
        build_workspace_section_with_user_root(&cwd.path().abs(), /*user_root*/ None)
            .await
            .expect("workspace section");
    assert!(section.contains("Working directory tree:"));
    assert!(section.contains("- docs/"));
    assert!(section.contains("- README.md"));
}

#[tokio::test]
async fn workspace_section_includes_user_root_tree_when_distinct() {
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

    let section = build_workspace_section_with_user_root(&cwd.abs(), Some(user_root))
        .await
        .expect("workspace section");
    assert!(section.contains("User root tree:"));
    assert!(section.contains("- code/"));
    assert!(!section.contains("- .zshrc"));
}

#[tokio::test]
async fn recent_work_section_groups_threads_by_cwd() {
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
        stored_thread(
            workspace_a.to_string_lossy().as_ref(),
            "Investigate realtime startup context",
            "Log the startup context before sending it",
        ),
        stored_thread(
            workspace_b.to_string_lossy().as_ref(),
            "Trim websocket startup payload",
            "Remove memories from the realtime startup context",
        ),
        stored_thread(outside.to_string_lossy().as_ref(), "", "Inspect flaky test"),
    ];
    let current_cwd = workspace_a;
    let repo = repo.abs();

    let section = build_recent_work_section(&current_cwd.abs(), &recent_threads)
        .await
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
