use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use pretty_assertions::assert_eq;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

fn write_skill(skills_root: &Path, dir: &str, name: &str, description: &str) -> Result<()> {
    let skill_dir = skills_root.join(dir);
    fs::create_dir_all(&skill_dir)?;
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    fs::write(skill_dir.join("SKILL.md"), contents)?;
    Ok(())
}

#[tokio::test]
async fn skills_list_includes_additional_roots() -> Result<()> {
    let codex_home = tempfile::tempdir()?;
    let cwd = tempfile::tempdir()?;
    let additional_root = tempfile::tempdir()?;
    write_skill(
        additional_root.path(),
        "desktop-extra",
        "desktop-extra-skill",
        "from additional root",
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.path().to_path_buf()],
            additional_roots: Some(vec![additional_root.path().to_path_buf()]),
            force_reload: true,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let SkillsListResponse { data } = to_response(response)?;

    assert_eq!(data.len(), 1);
    let skill_names: Vec<&str> = data[0]
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect();
    assert!(
        skill_names.contains(&"desktop-extra-skill"),
        "expected additional root skill, got {skill_names:?}"
    );

    let expected_skill_path =
        fs::canonicalize(additional_root.path().join("desktop-extra/SKILL.md"))?;
    let skill = data[0]
        .skills
        .iter()
        .find(|skill| skill.name == "desktop-extra-skill")
        .ok_or_else(|| anyhow::anyhow!("skill from additional root should exist"))?;
    assert_eq!(skill.path, expected_skill_path);

    Ok(())
}
