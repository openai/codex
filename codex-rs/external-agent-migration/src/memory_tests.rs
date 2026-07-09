use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn discovers_arbitrary_project_markdown_and_resolves_cwd_from_session_metadata() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(".external-agent");
    let project_root = root.path().join("source/project-a");
    let project_memory = external_agent_home.join("projects/opaque-project-key/memory");
    fs::create_dir_all(project_memory.join("topics")).expect("create memory directories");
    fs::create_dir_all(&project_root).expect("create project root");
    let project_root = fs::canonicalize(project_root).expect("canonicalize project root");
    fs::write(project_memory.join("MEMORY.md"), "index").expect("write index");
    fs::write(project_memory.join("release-process.md"), "release notes")
        .expect("write arbitrary topic");
    fs::write(project_memory.join("topics/database.md"), "database notes")
        .expect("write nested topic");
    fs::write(project_memory.join("ignored.txt"), "not markdown").expect("write ignored file");
    fs::write(
        external_agent_home.join("projects/opaque-project-key/session.jsonl"),
        format!("not json\n{}\n", serde_json::json!({ "cwd": project_root })),
    )
    .expect("write session metadata");

    let discovered = discover_external_memory_files_with_managed_root(
        &external_agent_home,
        &[],
        /*managed_settings_root*/ None,
    )
    .expect("discover memories");

    assert_eq!(
        discovered
            .iter()
            .map(|memory| (
                memory.project_key.as_str(),
                memory.cwd.as_deref(),
                memory.relative_path.as_path()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "opaque-project-key",
                Some(project_root.as_path()),
                Path::new("MEMORY.md")
            ),
            (
                "opaque-project-key",
                Some(project_root.as_path()),
                Path::new("release-process.md")
            ),
            (
                "opaque-project-key",
                Some(project_root.as_path()),
                Path::new("topics/database.md")
            ),
        ]
    );
}

#[test]
fn discovers_user_and_project_custom_directories_with_project_precedence() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(".external-agent");
    let repo_root = root.path().join("repo");
    let user_memory = root.path().join("user-memory");
    let shared_memory = root.path().join("shared-memory");
    let local_memory = root.path().join("local-memory");
    for directory in [
        &external_agent_home,
        &repo_root.join(".claude"),
        &user_memory,
        &shared_memory,
        &local_memory,
    ] {
        fs::create_dir_all(directory).expect("create fixture directory");
    }
    fs::write(user_memory.join("MEMORY.md"), "user memory").expect("write user memory");
    fs::write(shared_memory.join("shared.md"), "shared memory").expect("write shared memory");
    fs::write(local_memory.join("local-topic.md"), "local memory").expect("write local memory");
    fs::write(
        external_agent_home.join("settings.json"),
        serde_json::json!({ "autoMemoryDirectory": user_memory }).to_string(),
    )
    .expect("write user settings");
    fs::write(
        repo_root.join(".claude/settings.json"),
        serde_json::json!({ "autoMemoryDirectory": shared_memory }).to_string(),
    )
    .expect("write shared settings");
    fs::write(
        repo_root.join(".claude/settings.local.json"),
        serde_json::json!({ "autoMemoryDirectory": local_memory }).to_string(),
    )
    .expect("write local settings");

    let discovered = discover_external_memory_files_with_managed_root(
        &external_agent_home,
        std::slice::from_ref(&repo_root),
        /*managed_settings_root*/ None,
    )
    .expect("discover custom memories");

    assert_eq!(discovered.len(), 2);
    assert!(discovered.iter().any(|memory| {
        memory.project_key == CUSTOM_MEMORY_SCOPE
            && memory.cwd.is_none()
            && memory.source_path == user_memory.join("MEMORY.md")
    }));
    assert!(discovered.iter().any(|memory| {
        memory.project_key == format!("project:{}", repo_root.display())
            && memory.cwd.as_deref() == Some(repo_root.as_path())
            && memory.source_path == local_memory.join("local-topic.md")
    }));
    assert!(
        discovered
            .iter()
            .all(|memory| memory.source_path != shared_memory.join("shared.md"))
    );
}

#[test]
fn does_not_require_documentation_example_topic_names() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(".external-agent");
    let project_memory = external_agent_home.join("projects/project/memory");
    fs::create_dir_all(&project_memory).expect("create project memory");
    fs::write(project_memory.join("team-conventions.md"), "conventions")
        .expect("write arbitrary memory topic");

    let discovered = discover_external_memory_files_with_managed_root(
        &external_agent_home,
        &[],
        /*managed_settings_root*/ None,
    )
    .expect("discover memories");

    assert_eq!(
        discovered
            .iter()
            .map(|memory| memory.relative_path.as_path())
            .collect::<Vec<_>>(),
        vec![Path::new("team-conventions.md")]
    );
}
