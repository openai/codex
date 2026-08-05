use codex_core_plugins::PluginsManager;
use codex_core_plugins::store::PluginStore;
use codex_plugin::PluginId;
use codex_protocol::auth::AuthMode;
use codex_protocol::protocol::Product;
use codex_skills_extension::HostSkillsLoadInput;
use codex_skills_extension::HostSkillsService;
use pretty_assertions::assert_eq;

use super::test_support::load_plugins_config;
use super::test_support::write_file;

const PLUGIN_CONFIG_NAME: &str = "sample@openai-curated-remote";
const REMOTE_PLUGIN_ID: &str = "plugins~Plugin_sample";

#[tokio::test]
async fn host_skills_service_reuses_plugin_manager_skill_snapshot() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let plugin_root = codex_home
        .path()
        .join("plugins/cache/openai-curated-remote/sample/local");
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","description":"sample plugin"}"#,
    );
    let skill_path = plugin_root.join("skills/SKILL.md");
    write_file(&skill_path, "---\nname: search\ndescription: first\n---\n");
    write_file(
        &codex_home.path().join("config.toml"),
        &format!(
            r#"[features]
plugins = true
remote_plugin = true

[plugins."{PLUGIN_CONFIG_NAME}"]
enabled = true
"#,
        ),
    );
    let plugin_id = PluginId::parse(PLUGIN_CONFIG_NAME).expect("remote plugin id should parse");
    PluginStore::new(codex_home.path().to_path_buf())
        .write_remote_plugin_id(&plugin_id, REMOTE_PLUGIN_ID)
        .expect("persist remote plugin id");
    let config = load_plugins_config(codex_home.path()).await;
    let plugins_input = config.plugins_config_input();
    let plugins_manager = PluginsManager::new_with_options(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    );
    let plugin_outcome = plugins_manager.plugins_for_config(&plugins_input).await;

    write_file(&skill_path, "---\nname: search\ndescription: second\n---\n");

    let skills_input = HostSkillsLoadInput::new(
        config.cwd.clone(),
        plugin_outcome.effective_plugin_skill_roots(),
        config.config_layer_stack.clone(),
        /*bundled_skills_enabled*/ false,
    )
    .with_plugin_skill_snapshots(plugins_manager.plugin_skill_snapshots_for_config(&plugins_input));
    let skills_service = HostSkillsService::new(
        config.codex_home.clone(),
        /*bundled_skills_enabled*/ false,
    );
    let snapshot = skills_service
        .snapshot_for_config(&skills_input, /*fs*/ None)
        .await;

    assert_eq!(
        snapshot
            .outcome()
            .skills
            .iter()
            .filter(|skill| skill.plugin_id.as_deref() == Some(PLUGIN_CONFIG_NAME))
            .map(|skill| {
                (
                    skill.description.as_str(),
                    skill.plugin_id.as_deref(),
                    skill.remote_plugin_id.as_deref(),
                )
            })
            .collect::<Vec<_>>(),
        vec![("first", Some(PLUGIN_CONFIG_NAME), Some(REMOTE_PLUGIN_ID),)]
    );
}
