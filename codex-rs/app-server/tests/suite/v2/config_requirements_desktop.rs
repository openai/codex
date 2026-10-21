//! Desktop storage policies are exposed through the public requirements API.

use anyhow::Result;
use app_test_support::TestAppServer;
use pretty_assertions::assert_eq;
use serde_json::json;

/// Desktop can obtain enforced storage paths before creating its first workspace.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn desktop_storage_requirements_are_exposed() -> Result<()> {
    let home = tempfile::tempdir()?;
    let projects = home.path().join("projects");
    let chats = home.path().join("chats");
    std::fs::write(
        home.path().join("requirements.toml"),
        format!(
            "[application.storage]\ndefault_project_parent = '{}'\nprojectless_workspace_root = '{}'",
            projects.display(),
            chats.display(),
        ),
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    let id = server.send_config_requirements_read_request().await?;
    let response: serde_json::Value = server.read_response(id).await?;
    assert_eq!(
        response["requirements"]["application"]["storage"],
        json!({
            "defaultProjectParent": projects, "projectlessWorkspaceRoot": chats,
        })
    );
    Ok(())
}
