//! Exercises EMA registration ownership through real configuration layers.

use anyhow::Result;
use codex_config::LoaderOverrides;
use codex_config::McpServerAuth;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::config::ConfigBuilder;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use test_case::test_case;

#[test_case("oauth"; "OAuth")]
#[test_case("chatgpt"; "ChatGPT")]
#[tokio::test]
async fn trusted_project_cannot_downgrade_enterprise_auth(project_auth: &str) -> Result<()> {
    let home = tempdir()?;
    let workspace = tempdir()?;
    std::fs::create_dir_all(workspace.path().join(".git"))?;
    std::fs::create_dir_all(workspace.path().join(".codex"))?;
    set_project_trust_level(home.path(), workspace.path(), TrustLevel::Trusted)?;
    let managed_config = r#"
[features]
use_xaa = true
[mcp_enterprise_managed_auth.idp]
issuer = "https://idp.example"
client_id = "idp-client"
[mcp_servers.enterprise]
url = "https://resource.example/mcp"
auth = "ema_auth"
scopes = ["files.read"]
oauth_resource = "https://resource.example/mcp"
[mcp_servers.enterprise.oauth]
client_id = "mcp-client"
"#;
    let builder = ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(workspace.path().to_path_buf()))
        .cloud_config_bundle(
            CloudConfigBundleFixture::enterprise_config(managed_config.to_string())
                .add_enterprise_requirement(
                    "[mcp_servers.enterprise.identity]\nurl = \"https://resource.example/mcp\"\n",
                )
                .into_loader(),
        );
    assert_eq!(
        builder.clone().build().await?.mcp_servers.get()["enterprise"].auth,
        McpServerAuth::EmaAuth,
    );
    std::fs::write(
        workspace.path().join(".codex/config.toml"),
        format!("[mcp_servers.enterprise]\nauth = {project_auth:?}\n"),
    )?;
    let error = builder
        .build()
        .await
        .expect_err("a trusted project must not downgrade EMA before MCP startup");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    let message = format!("{error:#}");
    assert!(
        message.ends_with("one non-project config layer"),
        "{message}"
    );
    Ok(())
}
