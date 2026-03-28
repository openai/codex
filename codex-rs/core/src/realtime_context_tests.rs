use super::build_recent_work_section;
use super::build_workspace_section_with_user_root;
use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::ThreadId;
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
        metadata_json: "{}".to_string(),
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

    let section =
        build_workspace_section_with_user_root(cwd.path(), None).expect("workspace section");
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
