use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn detect_does_not_offer_memory_for_an_unsupported_source() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(".cursor");
    let codex_home = root.path().join(".codex");
    let project_root = external_agent_home.join("projects/project-a");
    let project_memory = project_root.join("memory");
    let project_cwd = root.path().join("project-a-cwd");
    fs::create_dir_all(&project_memory).expect("create project memory");
    fs::create_dir_all(&project_cwd).expect("create project cwd");
    fs::write(project_memory.join("MEMORY.md"), "project memory").expect("write project memory");
    fs::write(
        project_root.join("session.jsonl"),
        serde_json::json!({
            "type": "user",
            "cwd": project_cwd,
            "timestamp": "2026-07-13T00:00:00Z",
            "message": { "content": "remember this" },
        })
        .to_string(),
    )
    .expect("write project session");
    let mut service = service_for_paths(external_agent_home, codex_home);
    service.source = ExternalAgentSource::Cur;

    let items = service
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            include_memory: true,
            cwds: None,
        })
        .await
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}
