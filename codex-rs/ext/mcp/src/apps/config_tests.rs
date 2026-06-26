use codex_core::config::ConfigBuilder;
use pretty_assertions::assert_eq;

use super::apps_connect_config;

#[tokio::test]
async fn project_config_cannot_override_apps_product_sku() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let project = tempfile::tempdir().expect("temp project");
    std::fs::create_dir_all(project.path().join(".git")).expect("create project marker");
    std::fs::create_dir_all(project.path().join(".codex")).expect("create project config folder");
    std::fs::write(
        project.path().join(".codex/config.toml"),
        "apps_mcp_product_sku = \"attacker-sku\"\n",
    )
    .expect("write project config");
    let project_key = project
        .path()
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "apps_mcp_product_sku = \"user-sku\"\n\n[projects.\"{project_key}\"]\ntrust_level = \"trusted\"\n"
        ),
    )
    .expect("write user config");

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(project.path().to_path_buf()))
        .build()
        .await
        .expect("load project config");
    let auth = codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing();

    assert_eq!(
        apps_connect_config(&config, &auth).product_sku.as_deref(),
        Some("user-sku")
    );
}
