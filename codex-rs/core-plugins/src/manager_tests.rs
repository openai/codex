use super::*;
use crate::LoadedPlugin;
use crate::OPENAI_API_CURATED_MARKETPLACE_NAME;
use crate::OPENAI_CURATED_MARKETPLACE_NAME;
use crate::PluginLoadOutcome;
use crate::ToolSuggestDiscoverablePlugin;
use crate::ToolSuggestPluginDiscoveryInput;
use crate::installed_marketplaces::marketplace_install_root;
use crate::loader::load_plugin_skills;
use crate::loader::load_plugins_from_layer_stack;
use crate::loader::refresh_non_curated_plugin_cache;
use crate::loader::refresh_non_curated_plugin_cache_force_reinstall;
use crate::marketplace::MarketplacePluginInstallPolicy;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::remote::REMOTE_WORKSPACE_MARKETPLACE_NAME;
use crate::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME;
use crate::remote::RecommendedPlugin;
use crate::remote::RemoteInstalledPlugin;
use crate::startup_sync::curated_plugins_repo_path;
use crate::test_support::TEST_CURATED_PLUGIN_CACHE_VERSION;
use crate::test_support::TEST_CURATED_PLUGIN_SHA;
use crate::test_support::load_plugins_config as load_plugins_config_input;
use crate::test_support::write_curated_plugin;
use crate::test_support::write_curated_plugin_sha_with as write_curated_plugin_sha;
use crate::test_support::write_file;
use crate::test_support::write_openai_api_curated_marketplace;
use crate::test_support::write_openai_curated_marketplace;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::AppToolApproval;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::McpServerConfig;
use codex_config::McpServerOAuthConfig;
use codex_config::McpServerToolConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core_skills::PluginSkillSnapshots;
use codex_core_skills::SkillsLoadInput;
use codex_core_skills::SkillsService;
use codex_core_skills::config_rules::SkillConfigRules;
use codex_login::CodexAuth;
use codex_plugin::AppDeclaration;
use codex_plugin::PluginId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::Product;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use toml::Value;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const MAX_CAPABILITY_SUMMARY_DESCRIPTION_LEN: usize = 1024;

#[test]
fn plugins_manager_tracks_auth_mode() {
    let tmp = TempDir::new().unwrap();
    let manager = PluginsManager::new(tmp.path().to_path_buf());

    assert_eq!(manager.auth_mode(), None);
    assert!(manager.set_auth_mode(Some(AuthMode::ApiKey)));
    assert_eq!(manager.auth_mode(), Some(AuthMode::ApiKey));
    assert!(!manager.set_auth_mode(Some(AuthMode::ApiKey)));
    assert!(manager.set_auth_mode(Some(AuthMode::ChatgptAuthTokens)));
    assert_eq!(manager.auth_mode(), Some(AuthMode::ChatgptAuthTokens));
    assert!(manager.set_auth_mode(/*auth_mode*/ None));
    assert_eq!(manager.auth_mode(), None);

    let manager_with_auth = PluginsManager::new_with_options(
        tmp.path().join("auth"),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    );
    assert_eq!(manager_with_auth.auth_mode(), Some(AuthMode::Chatgpt));
}

fn write_auth_projection_plugin(codex_home: &Path, name: &str, include_app: bool) {
    let plugin_root = codex_home
        .join("plugins/cache")
        .join("test")
        .join(name)
        .join("local");
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        &format!(r#"{{"name":"{name}"}}"#),
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        &format!(
            r#"{{
  "mcpServers": {{
    "{name}": {{
      "type": "stdio",
      "command": "{name}-mcp"
    }}
  }}
}}"#
        ),
    );
    if include_app {
        write_auth_projection_app(codex_home, name, name);
    }
}

fn write_auth_projection_app(codex_home: &Path, plugin_name: &str, app_name: &str) {
    let plugin_root = codex_home
        .join("plugins/cache")
        .join("test")
        .join(plugin_name)
        .join("local");
    write_file(
        &plugin_root.join(".app.json"),
        &format!(r#"{{"apps":{{"{app_name}":{{"id":"connector_{plugin_name}"}}}}}}"#),
    );
}

fn app_declaration(name: &str, connector_id: &str) -> AppDeclaration {
    AppDeclaration {
        name: name.to_string(),
        connector_id: AppConnectorId(connector_id.to_string()),
        category: None,
    }
}

async fn auth_projection_config(codex_home: &Path) -> PluginsConfigInput {
    let config_toml = r#"[features]
plugins = true

[plugins."sample@test"]
enabled = true

[plugins."docs@test"]
enabled = true
"#
    .to_string();
    write_file(&codex_home.join(CONFIG_TOML_FILE), &config_toml);
    load_config(codex_home, codex_home).await
}

fn sorted_effective_mcp_server_names(outcome: &PluginLoadOutcome) -> Vec<String> {
    let mut names = outcome
        .effective_mcp_servers()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[tokio::test]
async fn plugin_auth_projection_hides_apps_without_chatgpt_auth() {
    let codex_home = TempDir::new().unwrap();
    write_auth_projection_plugin(codex_home.path(), "sample", /*include_app*/ true);
    write_auth_projection_plugin(codex_home.path(), "docs", /*include_app*/ false);
    let config = auth_projection_config(codex_home.path()).await;
    let manager = PluginsManager::new_with_options(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::ApiKey),
    );

    let outcome = manager.plugins_for_config(&config).await;

    assert!(outcome.effective_apps().is_empty());
    assert_eq!(
        sorted_effective_mcp_server_names(&outcome),
        vec!["docs".to_string(), "sample".to_string()]
    );
    let sample = outcome
        .capability_summaries()
        .iter()
        .find(|plugin| plugin.config_name == "sample@test")
        .expect("sample plugin summary should exist");
    assert_eq!(sample.mcp_server_names, vec!["sample".to_string()]);
    assert!(sample.app_connector_ids.is_empty());
}

#[tokio::test]
async fn plugin_auth_projection_hides_matching_mcp_with_chatgpt_apps_route() {
    let codex_home = TempDir::new().unwrap();
    write_auth_projection_plugin(codex_home.path(), "sample", /*include_app*/ true);
    write_auth_projection_plugin(codex_home.path(), "docs", /*include_app*/ false);
    let config = auth_projection_config(codex_home.path()).await;
    let manager = PluginsManager::new_with_options(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    );

    let outcome = manager.plugins_for_config(&config).await;

    assert_eq!(
        outcome.effective_apps(),
        vec![AppConnectorId("connector_sample".to_string())]
    );
    assert_eq!(
        sorted_effective_mcp_server_names(&outcome),
        vec!["docs".to_string()]
    );
    let sample = outcome
        .capability_summaries()
        .iter()
        .find(|plugin| plugin.config_name == "sample@test")
        .expect("sample plugin summary should exist");
    assert!(sample.mcp_server_names.is_empty());
    assert_eq!(
        sample.app_connector_ids,
        vec![AppConnectorId("connector_sample".to_string())]
    );
    let docs = outcome
        .capability_summaries()
        .iter()
        .find(|plugin| plugin.config_name == "docs@test")
        .expect("docs plugin summary should exist");
    assert_eq!(docs.mcp_server_names, vec!["docs".to_string()]);
    assert!(docs.app_connector_ids.is_empty());
}

#[tokio::test]
async fn plugin_auth_projection_hides_dual_surface_mcp_with_agent_identity_apps_route() {
    let codex_home = TempDir::new().unwrap();
    write_auth_projection_plugin(codex_home.path(), "sample", /*include_app*/ true);
    write_auth_projection_plugin(codex_home.path(), "docs", /*include_app*/ false);
    let config = auth_projection_config(codex_home.path()).await;
    let manager = PluginsManager::new_with_options(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::AgentIdentity),
    );

    let outcome = manager.plugins_for_config(&config).await;

    assert_eq!(
        outcome.effective_apps(),
        vec![AppConnectorId("connector_sample".to_string())]
    );
    assert_eq!(
        sorted_effective_mcp_server_names(&outcome),
        vec!["docs".to_string()]
    );
}

#[tokio::test]
async fn plugin_auth_projection_keeps_non_conflicting_mcp_with_chatgpt_apps_route() {
    let codex_home = TempDir::new().unwrap();
    write_auth_projection_plugin(codex_home.path(), "sample", /*include_app*/ false);
    write_auth_projection_app(codex_home.path(), "sample", "sample_app");
    write_auth_projection_plugin(codex_home.path(), "docs", /*include_app*/ false);
    let config = auth_projection_config(codex_home.path()).await;
    let manager = PluginsManager::new_with_options(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    );

    let outcome = manager.plugins_for_config(&config).await;

    assert_eq!(
        outcome.effective_apps(),
        vec![AppConnectorId("connector_sample".to_string())]
    );
    assert_eq!(
        sorted_effective_mcp_server_names(&outcome),
        vec!["docs".to_string(), "sample".to_string()]
    );
    let sample = outcome
        .capability_summaries()
        .iter()
        .find(|plugin| plugin.config_name == "sample@test")
        .expect("sample plugin summary should exist");
    assert_eq!(sample.mcp_server_names, vec!["sample".to_string()]);
    assert_eq!(
        sample.app_connector_ids,
        vec![AppConnectorId("connector_sample".to_string())]
    );
}

#[tokio::test]
async fn plugin_auth_projection_preserves_duplicate_connector_declaration_names() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test")
        .join("sample")
        .join("local");
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "foo": {
      "type": "stdio",
      "command": "foo-mcp"
    },
    "foo2": {
      "type": "stdio",
      "command": "foo2-mcp"
    },
    "other": {
      "type": "stdio",
      "command": "other-mcp"
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "foo": {
      "id": "connector_shared"
    },
    "foo2": {
      "id": "connector_shared"
    }
  }
}"#,
    );
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample@test"]
enabled = true
"#,
    );
    let config = load_config(codex_home.path(), codex_home.path()).await;
    let manager = PluginsManager::new_with_options(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    );

    let outcome = manager.plugins_for_config(&config).await;

    assert_eq!(
        outcome.effective_apps(),
        vec![AppConnectorId("connector_shared".to_string())]
    );
    assert_eq!(
        sorted_effective_mcp_server_names(&outcome),
        vec!["other".to_string()]
    );
    let sample = outcome
        .capability_summaries()
        .iter()
        .find(|plugin| plugin.config_name == "sample@test")
        .expect("sample plugin summary should exist");
    assert_eq!(sample.mcp_server_names, vec!["other".to_string()]);
    assert_eq!(
        sample.app_connector_ids,
        vec![AppConnectorId("connector_shared".to_string())]
    );
}

#[tokio::test]
async fn plugin_auth_projection_reprojects_cached_plugins_when_auth_changes() {
    let codex_home = TempDir::new().unwrap();
    write_auth_projection_plugin(codex_home.path(), "sample", /*include_app*/ true);
    write_auth_projection_plugin(codex_home.path(), "docs", /*include_app*/ false);
    let config = auth_projection_config(codex_home.path()).await;
    let manager = PluginsManager::new_with_options(
        codex_home.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    );

    let chatgpt_outcome = manager.plugins_for_config(&config).await;
    assert_eq!(
        sorted_effective_mcp_server_names(&chatgpt_outcome),
        vec!["docs".to_string()]
    );
    assert_eq!(
        chatgpt_outcome.effective_apps(),
        vec![AppConnectorId("connector_sample".to_string())]
    );
    assert_eq!(
        chatgpt_outcome.capability_summaries(),
        &[
            PluginCapabilitySummary {
                config_name: "docs@test".to_string(),
                display_name: "docs".to_string(),
                description: None,
                has_skills: false,
                mcp_server_names: vec!["docs".to_string()],
                app_connector_ids: Vec::new(),
            },
            PluginCapabilitySummary {
                config_name: "sample@test".to_string(),
                display_name: "sample".to_string(),
                description: None,
                has_skills: false,
                mcp_server_names: Vec::new(),
                app_connector_ids: vec![AppConnectorId("connector_sample".to_string())],
            },
        ]
    );

    assert!(manager.set_auth_mode(Some(AuthMode::ApiKey)));
    let api_key_outcome = manager.plugins_for_config(&config).await;

    assert_eq!(
        sorted_effective_mcp_server_names(&api_key_outcome),
        vec!["docs".to_string(), "sample".to_string()]
    );
    assert!(api_key_outcome.effective_apps().is_empty());
    assert_eq!(
        api_key_outcome.capability_summaries(),
        &[
            PluginCapabilitySummary {
                config_name: "docs@test".to_string(),
                display_name: "docs".to_string(),
                description: None,
                has_skills: false,
                mcp_server_names: vec!["docs".to_string()],
                app_connector_ids: Vec::new(),
            },
            PluginCapabilitySummary {
                config_name: "sample@test".to_string(),
                display_name: "sample".to_string(),
                description: None,
                has_skills: false,
                mcp_server_names: vec!["sample".to_string()],
                app_connector_ids: Vec::new(),
            },
        ]
    );
}

fn write_plugin_with_version(
    root: &Path,
    dir_name: &str,
    manifest_name: &str,
    manifest_version: Option<&str>,
) {
    let plugin_root = root.join(dir_name);
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::create_dir_all(plugin_root.join("skills")).unwrap();
    let version = manifest_version
        .map(|manifest_version| format!(r#","version":"{manifest_version}""#))
        .unwrap_or_default();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"{manifest_name}"{version}}}"#),
    )
    .unwrap();
    fs::write(plugin_root.join("skills/SKILL.md"), "skill").unwrap();
    fs::write(plugin_root.join(".mcp.json"), r#"{"mcpServers":{}}"#).unwrap();
}

fn write_plugin(root: &Path, dir_name: &str, manifest_name: &str) {
    write_plugin_with_version(
        root,
        dir_name,
        manifest_name,
        /*manifest_version*/ None,
    );
}

fn init_git_repo(repo: &Path) {
    run_git(repo, &["init"]);
    run_git(repo, &["config", "user.email", "codex-test@example.com"]);
    run_git(repo, &["config", "user.name", "Codex Test"]);
    run_git(repo, &["add", "."]);
    run_git(repo, &["commit", "-m", "initial"]);
}

fn run_git(repo: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("git should run: {err}"));
    assert!(
        output.status.success(),
        "git -C {} {} failed\nstdout:\n{}\nstderr:\n{}",
        repo.display(),
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn plugin_config_toml(enabled: bool, plugins_feature_enabled: bool) -> String {
    let mut root = toml::map::Map::new();

    let mut features = toml::map::Map::new();
    features.insert(
        "plugins".to_string(),
        Value::Boolean(plugins_feature_enabled),
    );
    root.insert("features".to_string(), Value::Table(features));

    let mut plugin = toml::map::Map::new();
    plugin.insert("enabled".to_string(), Value::Boolean(enabled));

    let mut plugins = toml::map::Map::new();
    plugins.insert("sample@test".to_string(), Value::Table(plugin));
    root.insert("plugins".to_string(), Value::Table(plugins));

    toml::to_string(&Value::Table(root)).expect("plugin test config should serialize")
}

async fn load_plugins_from_config(
    config_toml: &str,
    codex_home: &Path,
    auth_mode: Option<AuthMode>,
) -> PluginLoadOutcome {
    write_file(&codex_home.join(CONFIG_TOML_FILE), config_toml);
    let config = load_config(codex_home, codex_home).await;
    PluginsManager::new_with_options(codex_home.to_path_buf(), Some(Product::Codex), auth_mode)
        .plugins_for_config(&config)
        .await
}

async fn load_config(codex_home: &Path, cwd: &Path) -> PluginsConfigInput {
    load_plugins_config_input(codex_home, cwd).await
}

fn remote_installed_linear_plugin() -> RemoteInstalledPlugin {
    remote_installed_plugin("linear")
}

fn remote_installed_plugin(name: &str) -> RemoteInstalledPlugin {
    remote_installed_plugin_in_marketplace(name, REMOTE_GLOBAL_MARKETPLACE_NAME)
}

fn remote_installed_plugin_in_marketplace(
    name: &str,
    marketplace_name: &str,
) -> RemoteInstalledPlugin {
    RemoteInstalledPlugin {
        marketplace_name: marketplace_name.to_string(),
        id: format!("plugins~Plugin_{name}"),
        name: name.to_string(),
        enabled: true,
        install_policy: codex_app_server_protocol::PluginInstallPolicy::Available,
        auth_policy: codex_app_server_protocol::PluginAuthPolicy::OnUse,
        availability: codex_app_server_protocol::PluginAvailability::Available,
        interface: None,
        keywords: Vec::new(),
    }
}

fn write_cached_plugin(codex_home: &Path, marketplace_name: &str, plugin_name: &str) {
    write_plugin_with_version(
        &codex_home
            .join("plugins/cache")
            .join(marketplace_name)
            .join(plugin_name),
        "local",
        plugin_name,
        /*manifest_version*/ Some("local"),
    );
}

#[tokio::test]
async fn load_plugins_loads_default_skills_and_mcp_servers() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "sample",
  "description": "Plugin that includes the sample MCP server and Skills"
}"#,
    );
    write_file(
        &plugin_root.join("skills/sample-search/SKILL.md"),
        "---\nname: sample-search\ndescription: search sample data\n---\n",
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "sample": {
      "type": "http",
      "url": "https://sample.example/mcp",
      "oauth": {
        "clientId": "client-id",
        "callbackPort": 3118
      }
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "example": {
      "id": "connector_example"
    }
  }
}"#,
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
        codex_home.path(),
        Some(AuthMode::Chatgpt),
    )
    .await;

    assert_eq!(
        outcome.plugins(),
        vec![LoadedPlugin {
            config_name: "sample@test".to_string(),
            manifest_name: Some("sample".to_string()),
            plugin_namespace: Some("sample".to_string()),
            manifest_description: Some(
                "Plugin that includes the sample MCP server and Skills".to_string(),
            ),
            root: AbsolutePathBuf::try_from(plugin_root.clone()).unwrap(),
            enabled: true,
            skill_roots: vec![plugin_root.join("skills").abs()],
            disabled_skill_paths: HashSet::new(),
            has_enabled_skills: true,
            mcp_servers: HashMap::from([(
                "sample".to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: "https://sample.example/mcp".to_string(),
                        bearer_token_env_var: None,
                        http_headers: None,
                        env_http_headers: None,
                    },
                    environment_id: "local".to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    disabled_reason: None,
                    startup_timeout_sec: None,
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: Some(McpServerOAuthConfig {
                        client_id: Some("client-id".to_string()),
                    }),
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            )]),
            apps: vec![app_declaration("example", "connector_example")],
            hook_sources: Vec::new(),
            hook_load_warnings: Vec::new(),
            error: None,
        }]
    );
    assert_eq!(
        outcome.capability_summaries(),
        &[PluginCapabilitySummary {
            config_name: "sample@test".to_string(),
            display_name: "sample".to_string(),
            description: Some("Plugin that includes the sample MCP server and Skills".to_string(),),
            has_skills: true,
            mcp_server_names: vec!["sample".to_string()],
            app_connector_ids: vec![AppConnectorId("connector_example".to_string())],
        }]
    );
    assert_eq!(
        outcome.effective_skill_roots(),
        vec![plugin_root.join("skills").abs()]
    );
    assert_eq!(outcome.effective_mcp_servers().len(), 1);
    assert_eq!(
        outcome.effective_apps(),
        vec![AppConnectorId("connector_example".to_string())]
    );
}

#[tokio::test]
async fn load_plugins_loads_manifest_mcp_server_objects() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/counter-sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "counter-sample",
  "version": "1.1.1",
  "description": "Plugin that declares MCP servers in the manifest",
  "mcpServers": {
    "counter": {
      "type": "http",
      "url": "https://sample.example/counter/mcp"
    }
  }
}"#,
    );

    let config_toml = r#"
[features]
plugins = true

[plugins."counter-sample@test"]
enabled = true
"#;
    let outcome =
        load_plugins_from_config(config_toml, codex_home.path(), /*auth_mode*/ None).await;

    assert_eq!(outcome.plugins()[0].error, None);
    assert_eq!(
        outcome.plugins()[0].mcp_servers,
        HashMap::from([(
            "counter".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://sample.example/counter/mcp".to_string(),
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                environment_id: "local".to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            },
        )])
    );
}

#[tokio::test]
async fn load_plugins_applies_plugin_mcp_server_policy() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "sample"
}"#,
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "sample": {
      "type": "http",
      "url": "https://sample.example/mcp",
      "default_tools_approval_mode": "prompt",
      "enabled_tools": ["read", "search"],
      "tools": {
        "search": { "approval_mode": "prompt" }
      }
    }
  }
}"#,
    );
    let config_toml = r#"
[features]
plugins = true

[plugins."sample@test"]
enabled = true

[plugins."sample@test".mcp_servers.sample]
enabled = false
default_tools_approval_mode = "approve"
enabled_tools = ["search"]
disabled_tools = ["delete"]

[plugins."sample@test".mcp_servers.sample.tools.search]
approval_mode = "approve"
"#;

    let outcome =
        load_plugins_from_config(config_toml, codex_home.path(), /*auth_mode*/ None).await;
    let server = outcome.plugins()[0]
        .mcp_servers
        .get("sample")
        .expect("sample server");

    assert!(!server.enabled);
    assert_eq!(
        server.default_tools_approval_mode,
        Some(AppToolApproval::Approve)
    );
    assert_eq!(server.enabled_tools, Some(vec!["search".to_string()]));
    assert_eq!(server.disabled_tools, Some(vec!["delete".to_string()]));
    assert_eq!(
        server.tools.get("search"),
        Some(&McpServerToolConfig {
            approval_mode: Some(AppToolApproval::Approve),
        })
    );
}

#[tokio::test]
async fn remote_installed_cache_ignores_plugins_missing_local_cache() {
    let codex_home = TempDir::new().unwrap();
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );

    let config = load_config(codex_home.path(), codex_home.path()).await;
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    manager.write_remote_installed_plugins_cache(vec![remote_installed_linear_plugin()]);

    let outcome = manager.plugins_for_config(&config).await;
    assert_eq!(outcome, PluginLoadOutcome::default());
}

#[tokio::test]
async fn remote_installed_cache_prefers_local_curated_conflicts_when_remote_plugin_disabled() {
    let codex_home = TempDir::new().unwrap();
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = false

[plugins."linear@openai-curated"]
enabled = true

[plugins."calendar@openai-curated"]
enabled = true
"#,
    );
    write_cached_plugin(codex_home.path(), "openai-curated", "linear");
    write_cached_plugin(codex_home.path(), "openai-curated", "calendar");
    write_cached_plugin(codex_home.path(), "openai-curated-remote", "linear");
    write_cached_plugin(codex_home.path(), "openai-curated-remote", "remote-only");

    let config = load_config(codex_home.path(), codex_home.path()).await;
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    manager.write_remote_installed_plugins_cache(vec![
        remote_installed_plugin("linear"),
        remote_installed_plugin("remote-only"),
    ]);

    let outcome = manager.plugins_for_config(&config).await;
    assert_eq!(
        outcome
            .plugins()
            .iter()
            .map(|plugin| plugin.config_name.clone())
            .collect::<Vec<_>>(),
        vec![
            "calendar@openai-curated".to_string(),
            "linear@openai-curated".to_string(),
            "remote-only@openai-curated-remote".to_string(),
        ]
    );
}

#[tokio::test]
async fn remote_installed_cache_prefers_remote_curated_conflicts_when_remote_plugin_enabled() {
    let codex_home = TempDir::new().unwrap();
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true

[plugins."linear@openai-curated"]
enabled = true

[plugins."linear@openai-api-curated"]
enabled = true

[plugins."calendar@openai-curated"]
enabled = true
"#,
    );
    write_cached_plugin(codex_home.path(), "openai-curated", "linear");
    write_cached_plugin(codex_home.path(), "openai-api-curated", "linear");
    write_cached_plugin(codex_home.path(), "openai-curated", "calendar");
    write_cached_plugin(codex_home.path(), "openai-curated-remote", "linear");
    write_cached_plugin(codex_home.path(), "openai-curated-remote", "remote-only");

    let config = load_config(codex_home.path(), codex_home.path()).await;
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    manager.write_remote_installed_plugins_cache(vec![
        remote_installed_plugin("linear"),
        remote_installed_plugin("remote-only"),
    ]);

    let outcome = manager.plugins_for_config(&config).await;
    assert_eq!(
        outcome
            .plugins()
            .iter()
            .map(|plugin| plugin.config_name.clone())
            .collect::<Vec<_>>(),
        vec![
            "calendar@openai-curated".to_string(),
            "linear@openai-curated-remote".to_string(),
            "remote-only@openai-curated-remote".to_string(),
        ]
    );
}

#[tokio::test]
async fn build_remote_installed_plugin_marketplaces_from_cache_uses_remote_metadata() {
    let codex_home = TempDir::new().unwrap();
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    let mut plugin = remote_installed_linear_plugin();
    plugin.install_policy = codex_app_server_protocol::PluginInstallPolicy::InstalledByDefault;
    plugin.auth_policy = codex_app_server_protocol::PluginAuthPolicy::OnInstall;
    plugin.interface = Some(codex_app_server_protocol::PluginInterface {
        display_name: Some("Linear".to_string()),
        short_description: Some("Track remote work".to_string()),
        long_description: None,
        developer_name: None,
        category: None,
        capabilities: Vec::new(),
        website_url: None,
        privacy_policy_url: None,
        terms_of_service_url: None,
        default_prompt: None,
        brand_color: Some("#111111".to_string()),
        composer_icon: None,
        composer_icon_url: None,
        logo: None,
        logo_url: None,
        screenshots: Vec::new(),
        screenshot_urls: Vec::new(),
    });
    plugin.keywords = vec!["issues".to_string()];
    manager.write_remote_installed_plugins_cache(vec![plugin]);

    let marketplaces = manager
        .build_remote_installed_plugin_marketplaces_from_cache(&[REMOTE_GLOBAL_MARKETPLACE_NAME])
        .expect("remote installed cache should be present");
    assert_eq!(marketplaces.len(), 1);
    assert_eq!(marketplaces[0].name, "openai-curated-remote");
    assert_eq!(marketplaces[0].display_name, "OpenAI Curated Remote");
    assert_eq!(marketplaces[0].plugins.len(), 1);
    let plugin = &marketplaces[0].plugins[0];
    assert_eq!(plugin.id, "linear@openai-curated-remote");
    assert_eq!(plugin.remote_plugin_id, "plugins~Plugin_linear");
    assert_eq!(plugin.name, "linear");
    assert_eq!(plugin.installed, true);
    assert_eq!(plugin.enabled, true);
    assert_eq!(
        plugin.install_policy,
        codex_app_server_protocol::PluginInstallPolicy::InstalledByDefault
    );
    assert_eq!(
        plugin.auth_policy,
        codex_app_server_protocol::PluginAuthPolicy::OnInstall
    );
    assert_eq!(plugin.keywords, vec!["issues".to_string()]);
    assert_eq!(
        plugin
            .interface
            .as_ref()
            .and_then(|interface| interface.display_name.as_deref()),
        Some("Linear")
    );
    assert_eq!(
        plugin
            .interface
            .as_ref()
            .and_then(|interface| interface.short_description.as_deref()),
        Some("Track remote work")
    );
    assert_eq!(
        manager
            .build_remote_installed_plugin_marketplaces_from_cache(&[
                REMOTE_WORKSPACE_MARKETPLACE_NAME
            ])
            .expect("remote installed cache should be present"),
        Vec::new()
    );
}

#[tokio::test]
async fn build_remote_installed_plugin_marketplaces_from_cache_filters_by_marketplace_name() {
    let codex_home = TempDir::new().unwrap();
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    manager.write_remote_installed_plugins_cache(vec![
        remote_installed_plugin_in_marketplace(
            "workspace-linear",
            REMOTE_WORKSPACE_MARKETPLACE_NAME,
        ),
        remote_installed_plugin_in_marketplace(
            "shared-linear",
            REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME,
        ),
    ]);

    let marketplaces = manager
        .build_remote_installed_plugin_marketplaces_from_cache(&[REMOTE_WORKSPACE_MARKETPLACE_NAME])
        .expect("remote installed cache should be present");

    assert_eq!(marketplaces.len(), 1);
    assert_eq!(marketplaces[0].name, REMOTE_WORKSPACE_MARKETPLACE_NAME);
    assert_eq!(
        marketplaces[0]
            .plugins
            .iter()
            .map(|plugin| plugin.id.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace-linear@workspace-directory"]
    );
}

#[tokio::test]
async fn load_plugins_resolves_disabled_skill_names_against_loaded_plugin_skills() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");
    let skill_path = plugin_root.join("skills/sample-search/SKILL.md");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );
    write_file(
        &skill_path,
        "---\nname: sample-search\ndescription: search sample data\n---\n",
    );

    let config_toml = r#"[features]
plugins = true

[[skills.config]]
name = "sample:sample-search"
enabled = false

[plugins."sample@test"]
enabled = true
"#;
    let outcome =
        load_plugins_from_config(config_toml, codex_home.path(), /*auth_mode*/ None).await;
    let skill_path = std::fs::canonicalize(skill_path)
        .expect("skill path should canonicalize")
        .abs();

    assert_eq!(
        outcome.plugins()[0].disabled_skill_paths,
        HashSet::from([skill_path])
    );
    assert!(!outcome.plugins()[0].has_enabled_skills);
    assert!(outcome.capability_summaries().is_empty());
}

#[tokio::test]
async fn load_plugins_ignores_unknown_disabled_skill_names() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );
    write_file(
        &plugin_root.join("skills/sample-search/SKILL.md"),
        "---\nname: sample-search\ndescription: search sample data\n---\n",
    );

    let config_toml = r#"[features]
plugins = true

[[skills.config]]
name = "sample:missing-skill"
enabled = false

[plugins."sample@test"]
enabled = true
"#;
    let outcome =
        load_plugins_from_config(config_toml, codex_home.path(), /*auth_mode*/ None).await;

    assert!(outcome.plugins()[0].disabled_skill_paths.is_empty());
    assert!(outcome.plugins()[0].has_enabled_skills);
    assert_eq!(
        outcome.capability_summaries(),
        &[PluginCapabilitySummary {
            config_name: "sample@test".to_string(),
            display_name: "sample".to_string(),
            description: None,
            has_skills: true,
            mcp_server_names: Vec::new(),
            app_connector_ids: Vec::new(),
        }]
    );
}

#[tokio::test]
async fn plugin_telemetry_metadata_uses_default_mcp_config_path() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "sample"
}"#,
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "sample": {
      "type": "http",
      "url": "https://sample.example/mcp"
    }
  }
}"#,
    );

    let metadata = plugin_telemetry_metadata_from_root(
        &PluginId::parse("sample@test").expect("plugin id should parse"),
        &plugin_root.abs(),
    )
    .await;

    assert_eq!(
        metadata.capability_summary,
        Some(PluginCapabilitySummary {
            config_name: "sample@test".to_string(),
            display_name: "sample".to_string(),
            description: None,
            has_skills: false,
            mcp_server_names: vec!["sample".to_string()],
            app_connector_ids: Vec::new(),
        })
    );
}

#[tokio::test]
async fn plugin_telemetry_metadata_uses_manifest_mcp_server_objects() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/counter-sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "counter-sample",
  "version": "1.1.1",
  "mcpServers": {
    "counter": {
      "type": "http",
      "url": "https://sample.example/counter/mcp"
    }
  }
}"#,
    );

    let metadata = plugin_telemetry_metadata_from_root(
        &PluginId::parse("counter-sample@test").expect("plugin id should parse"),
        &plugin_root.abs(),
    )
    .await;

    assert_eq!(
        metadata.capability_summary,
        Some(PluginCapabilitySummary {
            config_name: "counter-sample@test".to_string(),
            display_name: "counter-sample".to_string(),
            description: None,
            has_skills: false,
            mcp_server_names: vec!["counter".to_string()],
            app_connector_ids: Vec::new(),
        })
    );
}

#[tokio::test]
async fn capability_summary_sanitizes_plugin_descriptions_to_one_line() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "sample",
  "description": "Plugin that\n includes   the sample\tserver"
}"#,
    );
    write_file(
        &plugin_root.join("skills/sample-search/SKILL.md"),
        "---\nname: sample-search\ndescription: search sample data\n---\n",
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
        codex_home.path(),
        /*auth_mode*/ None,
    )
    .await;

    assert_eq!(
        outcome.plugins()[0].manifest_description.as_deref(),
        Some("Plugin that\n includes   the sample\tserver")
    );
    assert_eq!(
        outcome.capability_summaries()[0].description.as_deref(),
        Some("Plugin that includes the sample server")
    );
}

#[tokio::test]
async fn capability_summary_truncates_overlong_plugin_descriptions() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");
    let too_long = "x".repeat(MAX_CAPABILITY_SUMMARY_DESCRIPTION_LEN + 1);

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        &format!(
            r#"{{
  "name": "sample",
  "description": "{too_long}"
}}"#
        ),
    );
    write_file(
        &plugin_root.join("skills/sample-search/SKILL.md"),
        "---\nname: sample-search\ndescription: search sample data\n---\n",
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
        codex_home.path(),
        /*auth_mode*/ None,
    )
    .await;

    assert_eq!(
        outcome.plugins()[0].manifest_description.as_deref(),
        Some(too_long.as_str())
    );
    assert_eq!(
        outcome.capability_summaries()[0].description,
        Some("x".repeat(MAX_CAPABILITY_SUMMARY_DESCRIPTION_LEN))
    );
}

#[tokio::test]
async fn load_plugins_uses_manifest_configured_component_paths() {
    for (skills_json, expected_skill_dirs) in [
        (r#""./custom-skills/""#, &["custom-skills"][..]),
        (
            r#"["./custom-skills/", "./extra-skills/"]"#,
            &["custom-skills", "extra-skills"][..],
        ),
        (
            r#"["./custom-skills/", "./custom-skills/"]"#,
            &["custom-skills"][..],
        ),
        (r#""./skills/""#, &["skills"][..]),
        (
            r#"["./skills/abc/", "./skills/edk/"]"#,
            &["skills/abc", "skills/edk"][..],
        ),
    ] {
        let codex_home = TempDir::new().unwrap();
        let plugin_root = codex_home
            .path()
            .join("plugins/cache")
            .join("test/sample/local");

        write_file(
            &plugin_root.join(".codex-plugin/plugin.json"),
            &format!(
                r#"{{
  "name": "sample",
  "skills": {skills_json},
  "mcpServers": "./config/custom.mcp.json",
  "apps": "./config/custom.app.json"
}}"#
            ),
        );
        write_file(
            &plugin_root.join("skills/default-skill/SKILL.md"),
            "---\nname: default-skill\ndescription: default skill\n---\n",
        );
        write_file(
            &plugin_root.join("skills/abc/SKILL.md"),
            "---\nname: abc\ndescription: abc skill\n---\n",
        );
        write_file(
            &plugin_root.join("skills/edk/SKILL.md"),
            "---\nname: edk\ndescription: edk skill\n---\n",
        );
        write_file(
            &plugin_root.join("custom-skills/custom-skill/SKILL.md"),
            "---\nname: custom-skill\ndescription: custom skill\n---\n",
        );
        write_file(
            &plugin_root.join("extra-skills/extra-skill/SKILL.md"),
            "---\nname: extra-skill\ndescription: extra skill\n---\n",
        );
        write_file(
            &plugin_root.join(".mcp.json"),
            r#"{
  "mcpServers": {
    "default": {
      "type": "http",
      "url": "https://default.example/mcp"
    }
  }
}"#,
        );
        write_file(
            &plugin_root.join("config/custom.mcp.json"),
            r#"{
  "mcpServers": {
    "custom": {
      "type": "http",
      "url": "https://custom.example/mcp"
    }
  }
}"#,
        );
        write_file(
            &plugin_root.join(".app.json"),
            r#"{
  "apps": {
    "default-app": {
      "id": "connector_default"
    }
  }
}"#,
        );
        write_file(
            &plugin_root.join("config/custom.app.json"),
            r#"{
  "apps": {
    "custom-app": {
      "id": "connector_custom"
    }
  }
}"#,
        );
        let outcome = load_plugins_from_config(
            &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
            codex_home.path(),
            Some(AuthMode::Chatgpt),
        )
        .await;
        let mut expected_skill_roots = expected_skill_dirs
            .iter()
            .map(|dir| plugin_root.join(dir).abs())
            .collect::<Vec<_>>();
        expected_skill_roots.sort_unstable();
        expected_skill_roots.dedup();

        assert_eq!(outcome.plugins()[0].skill_roots, expected_skill_roots);
        assert_eq!(
            outcome.plugins()[0].mcp_servers,
            HashMap::from([(
                "custom".to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: "https://custom.example/mcp".to_string(),
                        bearer_token_env_var: None,
                        http_headers: None,
                        env_http_headers: None,
                    },
                    environment_id: "local".to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    disabled_reason: None,
                    startup_timeout_sec: None,
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            )])
        );
        assert_eq!(
            outcome.plugins()[0].apps,
            vec![app_declaration("custom-app", "connector_custom")]
        );
    }
}

#[tokio::test]
async fn load_plugin_skills_dedupes_overlapping_manifest_roots() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local")
        .abs();
    write_file(
        &plugin_root.join("skills/abc/SKILL.md"),
        "---\nname: abc\ndescription: abc skill\n---\n",
    );
    write_file(
        &plugin_root.join("skills/edk/SKILL.md"),
        "---\nname: edk\ndescription: edk skill\n---\n",
    );
    let manifest = crate::manifest::PluginManifest {
        name: "sample".to_string(),
        version: None,
        description: None,
        keywords: Vec::new(),
        paths: crate::manifest::PluginManifestPaths {
            skills: vec![
                plugin_root.join("skills"),
                plugin_root.join("skills/abc"),
                plugin_root.join("skills/edk"),
                plugin_root.join("skills/abc"),
            ],
            mcp_servers: None,
            apps: None,
            hooks: None,
        },
        interface: None,
    };
    let plugin_id = PluginId::parse("sample@test").expect("plugin id should parse");

    let resolved = load_plugin_skills(
        &plugin_root,
        &plugin_id,
        &manifest,
        /*restriction_product*/ None,
        &SkillConfigRules::default(),
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    let skill_paths = resolved
        .skills
        .iter()
        .map(|skill| skill.path_to_skills_md.clone())
        .collect::<Vec<_>>();
    let canonical_skill_path = |path| {
        AbsolutePathBuf::from_absolute_path_checked(
            fs::canonicalize(plugin_root.join(path)).expect("canonical skill path"),
        )
        .expect("absolute skill path")
    };
    assert_eq!(
        skill_paths,
        vec![
            canonical_skill_path("skills/abc/SKILL.md"),
            canonical_skill_path("skills/edk/SKILL.md")
        ]
    );
}

#[tokio::test]
async fn load_plugins_ignores_manifest_component_paths_without_dot_slash() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "sample",
  "skills": "custom-skills",
  "mcpServers": "config/custom.mcp.json",
  "apps": "config/custom.app.json"
}"#,
    );
    write_file(
        &plugin_root.join("skills/default-skill/SKILL.md"),
        "---\nname: default-skill\ndescription: default skill\n---\n",
    );
    write_file(
        &plugin_root.join("custom-skills/custom-skill/SKILL.md"),
        "---\nname: custom-skill\ndescription: custom skill\n---\n",
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "default": {
      "type": "http",
      "url": "https://default.example/mcp"
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join("config/custom.mcp.json"),
        r#"{
  "mcpServers": {
    "custom": {
      "type": "http",
      "url": "https://custom.example/mcp"
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "default-app": {
      "id": "connector_default"
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join("config/custom.app.json"),
        r#"{
  "apps": {
    "custom-app": {
      "id": "connector_custom"
    }
  }
}"#,
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
        codex_home.path(),
        Some(AuthMode::Chatgpt),
    )
    .await;

    assert_eq!(
        outcome.plugins()[0].skill_roots,
        vec![plugin_root.join("skills").abs()]
    );
    assert_eq!(
        outcome.plugins()[0].mcp_servers,
        HashMap::from([(
            "default".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://default.example/mcp".to_string(),
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                environment_id: "local".to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            },
        )])
    );
    assert_eq!(
        outcome.plugins()[0].apps,
        vec![app_declaration("default-app", "connector_default")]
    );
}

#[tokio::test]
async fn load_plugins_ignores_invalid_manifest_skills_shape() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "sample",
  "skills": { "path": "./custom-skills/" }
}"#,
    );
    write_file(
        &plugin_root.join("skills/default-skill/SKILL.md"),
        "---\nname: default-skill\ndescription: default skill\n---\n",
    );
    write_file(
        &plugin_root.join("custom-skills/custom-skill/SKILL.md"),
        "---\nname: custom-skill\ndescription: custom skill\n---\n",
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
        codex_home.path(),
        /*auth_mode*/ None,
    )
    .await;

    assert_eq!(outcome.plugins()[0].error, None);
    assert_eq!(
        outcome.plugins()[0].skill_roots,
        vec![plugin_root.join("skills").abs()]
    );
}

#[tokio::test]
async fn load_plugins_preserves_disabled_plugins_without_effective_contributions() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "sample": {
      "type": "http",
      "url": "https://sample.example/mcp"
    }
  }
}"#,
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(
            /*enabled*/ false, /*plugins_feature_enabled*/ true,
        ),
        codex_home.path(),
        /*auth_mode*/ None,
    )
    .await;

    assert_eq!(
        outcome.plugins(),
        vec![LoadedPlugin {
            config_name: "sample@test".to_string(),
            manifest_name: None,
            plugin_namespace: None,
            manifest_description: None,
            root: AbsolutePathBuf::try_from(plugin_root).unwrap(),
            enabled: false,
            skill_roots: Vec::new(),
            disabled_skill_paths: HashSet::new(),
            has_enabled_skills: false,
            mcp_servers: HashMap::new(),
            apps: Vec::new(),
            hook_sources: Vec::new(),
            hook_load_warnings: Vec::new(),
            error: None,
        }]
    );
    assert!(outcome.effective_skill_roots().is_empty());
    assert!(outcome.effective_mcp_servers().is_empty());
}

#[tokio::test]
async fn effective_apps_dedupes_connector_ids_across_plugins() {
    let codex_home = TempDir::new().unwrap();
    let plugin_a_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/plugin-a/local");
    let plugin_b_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/plugin-b/local");

    write_file(
        &plugin_a_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"plugin-a"}"#,
    );
    write_file(
        &plugin_a_root.join(".app.json"),
        r#"{
  "apps": {
    "example": {
      "id": "connector_example"
    }
  }
}"#,
    );
    write_file(
        &plugin_b_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"plugin-b"}"#,
    );
    write_file(
        &plugin_b_root.join(".app.json"),
        r#"{
  "apps": {
    "chat": {
      "id": "connector_example"
    },
    "gmail": {
      "id": "connector_gmail"
    }
  }
}"#,
    );

    let mut root = toml::map::Map::new();
    let mut features = toml::map::Map::new();
    features.insert("plugins".to_string(), Value::Boolean(true));
    features.insert("apps".to_string(), Value::Boolean(true));
    root.insert("features".to_string(), Value::Table(features));

    let mut plugins = toml::map::Map::new();

    let mut plugin_a = toml::map::Map::new();
    plugin_a.insert("enabled".to_string(), Value::Boolean(true));
    plugins.insert("plugin-a@test".to_string(), Value::Table(plugin_a));

    let mut plugin_b = toml::map::Map::new();
    plugin_b.insert("enabled".to_string(), Value::Boolean(true));
    plugins.insert("plugin-b@test".to_string(), Value::Table(plugin_b));

    root.insert("plugins".to_string(), Value::Table(plugins));
    let config_toml =
        toml::to_string(&Value::Table(root)).expect("plugin test config should serialize");

    let outcome =
        load_plugins_from_config(&config_toml, codex_home.path(), Some(AuthMode::Chatgpt)).await;

    assert_eq!(
        outcome.effective_apps(),
        vec![
            AppConnectorId("connector_example".to_string()),
            AppConnectorId("connector_gmail".to_string()),
        ]
    );
}

#[tokio::test]
async fn effective_apps_preserves_app_config_order() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );
    write_file(
        &plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "slack": {
      "id": "connector_slack"
    },
    "github": {
      "id": "connector_github"
    },
    "slack-copy": {
      "id": "connector_slack"
    }
  }
}"#,
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
        codex_home.path(),
        Some(AuthMode::Chatgpt),
    )
    .await;

    assert_eq!(
        outcome.effective_apps(),
        vec![
            AppConnectorId("connector_slack".to_string()),
            AppConnectorId("connector_github".to_string()),
        ]
    );
}

#[test]
fn capability_index_filters_inactive_and_zero_capability_plugins() {
    let codex_home = TempDir::new().unwrap();
    let connector = |id: &str| AppConnectorId(id.to_string());
    let app = |name: &str, connector_id: &str| app_declaration(name, connector_id);
    let http_server = |url: &str| McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        environment_id: "local".to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    };
    let plugin = |config_name: &str, dir_name: &str, manifest_name: &str| LoadedPlugin {
        config_name: config_name.to_string(),
        manifest_name: Some(manifest_name.to_string()),
        plugin_namespace: Some(
            config_name
                .split_once('@')
                .map_or(config_name, |(name, _)| name)
                .to_string(),
        ),
        manifest_description: None,
        root: AbsolutePathBuf::try_from(codex_home.path().join(dir_name)).unwrap(),
        enabled: true,
        skill_roots: Vec::new(),
        disabled_skill_paths: HashSet::new(),
        has_enabled_skills: false,
        mcp_servers: HashMap::new(),
        apps: Vec::new(),
        hook_sources: Vec::new(),
        hook_load_warnings: Vec::new(),
        error: None,
    };
    let summary = |config_name: &str, display_name: &str| PluginCapabilitySummary {
        config_name: config_name.to_string(),
        display_name: display_name.to_string(),
        description: None,
        ..PluginCapabilitySummary::default()
    };
    let outcome = PluginLoadOutcome::from_plugins(vec![
        LoadedPlugin {
            skill_roots: vec![codex_home.path().join("skills-plugin/skills").abs()],
            has_enabled_skills: true,
            ..plugin("skills@test", "skills-plugin", "skills-plugin")
        },
        LoadedPlugin {
            mcp_servers: HashMap::from([("alpha".to_string(), http_server("https://alpha"))]),
            apps: vec![app("example", "connector_example")],
            ..plugin("alpha@test", "alpha-plugin", "alpha-plugin")
        },
        LoadedPlugin {
            mcp_servers: HashMap::from([("beta".to_string(), http_server("https://beta"))]),
            apps: vec![
                app("example", "connector_example"),
                app("gmail", "connector_gmail"),
            ],
            ..plugin("beta@test", "beta-plugin", "beta-plugin")
        },
        plugin("empty@test", "empty-plugin", "empty-plugin"),
        LoadedPlugin {
            enabled: false,
            skill_roots: vec![codex_home.path().join("disabled-plugin/skills").abs()],
            apps: vec![app("hidden", "connector_hidden")],
            ..plugin("disabled@test", "disabled-plugin", "disabled-plugin")
        },
        LoadedPlugin {
            apps: vec![app("broken", "connector_broken")],
            error: Some("failed to load".to_string()),
            ..plugin("broken@test", "broken-plugin", "broken-plugin")
        },
    ]);

    assert_eq!(
        outcome.capability_summaries(),
        &[
            PluginCapabilitySummary {
                has_skills: true,
                ..summary("skills@test", "skills-plugin")
            },
            PluginCapabilitySummary {
                mcp_server_names: vec!["alpha".to_string()],
                app_connector_ids: vec![connector("connector_example")],
                ..summary("alpha@test", "alpha-plugin")
            },
            PluginCapabilitySummary {
                mcp_server_names: vec!["beta".to_string()],
                app_connector_ids: vec![
                    connector("connector_example"),
                    connector("connector_gmail"),
                ],
                ..summary("beta@test", "beta-plugin")
            },
        ]
    );
}

#[tokio::test]
async fn load_plugins_returns_empty_when_feature_disabled() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );
    write_file(
        &plugin_root.join("skills/sample-search/SKILL.md"),
        "---\nname: sample-search\ndescription: search sample data\n---\n",
    );
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &plugin_config_toml(
            /*enabled*/ true, /*plugins_feature_enabled*/ false,
        ),
    );

    let config = load_config(codex_home.path(), codex_home.path()).await;
    let outcome = PluginsManager::new(codex_home.path().to_path_buf())
        .plugins_for_config(&config)
        .await;

    assert_eq!(outcome, PluginLoadOutcome::default());
}

#[tokio::test]
async fn plugin_cache_ignores_unrelated_session_overrides() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");
    write_plugin(
        codex_home.path().join("plugins/cache/test").as_path(),
        "sample/local",
        "sample",
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "sample": {
      "url": "https://sample.example/mcp"
    }
  }
}"#,
    );

    let user_file = codex_home.path().join(CONFIG_TOML_FILE).abs();
    let user_config: toml::Value = toml::from_str(&plugin_config_toml(
        /*enabled*/ true, /*plugins_feature_enabled*/ true,
    ))
    .expect("user config should parse");
    let stack = |session_config: &str| {
        ConfigLayerStack::new(
            vec![
                ConfigLayerEntry::new(
                    ConfigLayerSource::User {
                        file: user_file.clone(),
                        profile: None,
                    },
                    user_config.clone(),
                ),
                ConfigLayerEntry::new(
                    ConfigLayerSource::SessionFlags,
                    toml::from_str(session_config).expect("session config should parse"),
                ),
            ],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("config layer stack should build")
    };
    let config = |session_config| {
        PluginsConfigInput::new(
            stack(session_config),
            /*plugins_enabled*/ true,
            /*remote_plugin_enabled*/ false,
            "https://chatgpt.com".to_string(),
        )
    };
    let manager = PluginsManager::new(codex_home.path().to_path_buf());

    let first = manager
        .plugins_for_config(&config(r#"model = "first""#))
        .await;
    std::fs::remove_file(plugin_root.join(".mcp.json")).unwrap();
    let second = manager
        .plugins_for_config(&config(r#"model = "second""#))
        .await;

    assert_eq!(second, first);
    assert_eq!(second.plugins()[0].mcp_servers.len(), 1);
}

#[tokio::test]
async fn skills_service_reuses_skills_parsed_during_plugin_load() {
    let codex_home = TempDir::new().unwrap();
    let codex_home_abs = codex_home.path().to_path_buf().abs();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");
    write_plugin(
        codex_home.path().join("plugins/cache/test").as_path(),
        "sample/local",
        "sample",
    );
    let skill_path = plugin_root.join("skills/SKILL.md");
    write_file(&skill_path, "---\nname: search\ndescription: first\n---\n");
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
    );

    let config = load_config(codex_home.path(), codex_home.path()).await;
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    let plugin_outcome = manager.plugins_for_config(&config).await;
    let plugin_skill_snapshots = manager.plugin_skill_snapshots_for_config(&config);
    write_file(&skill_path, "---\nname: search\ndescription: second\n---\n");

    let skills_input = SkillsLoadInput::new(
        codex_home_abs.clone(),
        plugin_outcome.effective_plugin_skill_roots(),
        config.config_layer_stack.clone(),
        /*bundled_skills_enabled*/ false,
    )
    .with_plugin_skill_snapshots(plugin_skill_snapshots);
    let skills_service = SkillsService::new(codex_home_abs, /*bundled_skills_enabled*/ false);
    let cached = skills_service
        .snapshot_for_config(&skills_input, /*fs*/ None)
        .await;

    assert_eq!(
        cached
            .outcome()
            .skills
            .iter()
            .map(|skill| skill.description.as_str())
            .collect::<Vec<_>>(),
        vec!["first"]
    );
}

#[test]
fn loaded_plugins_cache_invalidation_rejects_stale_load_completion() {
    let codex_home = TempDir::new().unwrap();
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    let cache_key = PluginLoadCacheKey {
        configured_plugins: HashMap::new(),
        skill_config_rules: SkillConfigRules::default(),
        remote_plugin_enabled: false,
    };
    let stale_generation = manager.loaded_plugins_cache_generation();

    manager.clear_loaded_plugins_cache();
    manager.cache_loaded_plugins_if_current(
        stale_generation,
        cache_key.clone(),
        Vec::new(),
        PluginSkillSnapshots::for_plugin_load(),
    );

    assert_eq!(manager.cached_loaded_plugins(&cache_key), None);
}

#[tokio::test]
async fn load_plugins_rejects_invalid_plugin_keys() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );

    let mut root = toml::map::Map::new();
    let mut features = toml::map::Map::new();
    features.insert("plugins".to_string(), Value::Boolean(true));
    root.insert("features".to_string(), Value::Table(features));

    let mut plugin = toml::map::Map::new();
    plugin.insert("enabled".to_string(), Value::Boolean(true));

    let mut plugins = toml::map::Map::new();
    plugins.insert("sample".to_string(), Value::Table(plugin));
    root.insert("plugins".to_string(), Value::Table(plugins));

    let outcome = load_plugins_from_config(
        &toml::to_string(&Value::Table(root)).expect("plugin test config should serialize"),
        codex_home.path(),
        /*auth_mode*/ None,
    )
    .await;

    assert_eq!(outcome.plugins().len(), 1);
    assert_eq!(
        outcome.plugins()[0].error.as_deref(),
        Some("invalid plugin key `sample`; expected <plugin>@<marketplace>")
    );
    assert!(outcome.effective_skill_roots().is_empty());
    assert!(outcome.effective_mcp_servers().is_empty());
}

#[tokio::test]
async fn install_plugin_updates_config_with_relative_path_and_plugin_key() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin(&repo_root, "sample-plugin", "sample-plugin");
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      },
      "policy": {
        "authentication": "ON_USE"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let result = PluginsManager::new(tmp.path().to_path_buf())
        .install_plugin(PluginInstallRequest {
            plugin_name: "sample-plugin".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                repo_root.join(".agents/plugins/marketplace.json"),
            )
            .unwrap(),
        })
        .await
        .unwrap();

    let installed_path = tmp.path().join("plugins/cache/debug/sample-plugin/local");
    assert_eq!(
        result,
        PluginInstallOutcome {
            plugin_id: PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap(),
            plugin_version: "local".to_string(),
            installed_path: AbsolutePathBuf::try_from(installed_path).unwrap(),
            auth_policy: MarketplacePluginAuthPolicy::OnUse,
        }
    );

    let config = fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(config.contains(r#"[plugins."sample-plugin@debug"]"#));
    assert!(config.contains("enabled = true"));
}

#[tokio::test]
async fn install_openai_curated_plugin_uses_short_sha_cache_version() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);

    let result = PluginsManager::new(tmp.path().to_path_buf())
        .install_plugin(PluginInstallRequest {
            plugin_name: "slack".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                curated_root.join(".agents/plugins/marketplace.json"),
            )
            .unwrap(),
        })
        .await
        .unwrap();

    let installed_path = tmp.path().join(format!(
        "plugins/cache/openai-curated/slack/{TEST_CURATED_PLUGIN_CACHE_VERSION}"
    ));
    assert_eq!(
        result,
        PluginInstallOutcome {
            plugin_id: PluginId::new(
                "slack".to_string(),
                OPENAI_CURATED_MARKETPLACE_NAME.to_string()
            )
            .unwrap(),
            plugin_version: TEST_CURATED_PLUGIN_CACHE_VERSION.to_string(),
            installed_path: AbsolutePathBuf::try_from(installed_path).unwrap(),
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
}

#[tokio::test]
async fn install_plugin_uses_manifest_version_for_non_curated_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin_with_version(
        &repo_root,
        "sample-plugin",
        "sample-plugin",
        Some("1.2.3-beta+7"),
    );
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let result = PluginsManager::new(tmp.path().to_path_buf())
        .install_plugin(PluginInstallRequest {
            plugin_name: "sample-plugin".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                repo_root.join(".agents/plugins/marketplace.json"),
            )
            .unwrap(),
        })
        .await
        .unwrap();

    let installed_path = tmp
        .path()
        .join("plugins/cache/debug/sample-plugin/1.2.3-beta+7");
    assert_eq!(
        result,
        PluginInstallOutcome {
            plugin_id: PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap(),
            plugin_version: "1.2.3-beta+7".to_string(),
            installed_path: AbsolutePathBuf::try_from(installed_path).unwrap(),
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
}

#[tokio::test]
async fn install_plugin_writes_marketplace_manifest_fallback_when_missing_plugin_json() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let plugin_root = repo_root.join("plugins/quality-review");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(plugin_root.join("skills/thermo-nuclear-code-quality-review")).unwrap();
    fs::write(
        plugin_root.join("skills/thermo-nuclear-code-quality-review/SKILL.md"),
        "review skill",
    )
    .unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "quality-review",
      "description": "Strict code quality review focused on maintainability.",
      "source": "./plugins/quality-review",
      "author": {
        "name": "Byron Grogan"
      },
      "skills": [
        "./skills/thermo-nuclear-code-quality-review"
      ],
      "category": "code-review"
    }
  ]
}"#,
    )
    .unwrap();

    let result = PluginsManager::new(tmp.path().to_path_buf())
        .install_plugin(PluginInstallRequest {
            plugin_name: "quality-review".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                repo_root.join(".agents/plugins/marketplace.json"),
            )
            .unwrap(),
        })
        .await
        .unwrap();

    let installed_path = tmp.path().join("plugins/cache/debug/quality-review/local");
    assert_eq!(
        result,
        PluginInstallOutcome {
            plugin_id: PluginId::new("quality-review".to_string(), "debug".to_string()).unwrap(),
            plugin_version: "local".to_string(),
            installed_path: AbsolutePathBuf::try_from(installed_path.clone()).unwrap(),
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
    assert!(!plugin_root.join(".codex-plugin/plugin.json").exists());
    assert!(
        !tmp.path()
            .join("plugins/.marketplace-plugin-source-staging")
            .exists()
    );

    let manifest = crate::manifest::load_plugin_manifest(&installed_path).unwrap();
    assert_eq!(manifest.name, "quality-review");
    assert_eq!(
        manifest.description.as_deref(),
        Some("Strict code quality review focused on maintainability.")
    );
    assert_eq!(
        manifest.paths.skills,
        vec![
            AbsolutePathBuf::try_from(
                installed_path.join("skills/thermo-nuclear-code-quality-review")
            )
            .unwrap()
        ]
    );
    let interface = manifest.interface.expect("fallback interface");
    assert_eq!(interface.developer_name.as_deref(), Some("Byron Grogan"));
    assert_eq!(interface.category.as_deref(), Some("code-review"));
    let fallback_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(installed_path.join(".codex-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fallback_json["author"],
        serde_json::json!({ "name": "Byron Grogan" })
    );
    assert_eq!(fallback_json["category"], "code-review");
}

#[tokio::test]
async fn install_plugin_supports_git_subdir_marketplace_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("marketplace");
    let remote_repo = tmp.path().join("remote-plugin-repo");
    let remote_repo_url = url::Url::from_directory_path(&remote_repo)
        .unwrap()
        .to_string();
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin(&remote_repo, "plugins/toolkit", "toolkit");
    init_git_repo(&remote_repo);
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        format!(
            r#"{{
  "name": "debug",
  "plugins": [
    {{
      "name": "toolkit",
      "source": {{
        "source": "git-subdir",
        "url": "{remote_repo_url}",
        "path": "plugins/toolkit"
      }}
    }}
  ]
}}"#
        ),
    )
    .unwrap();

    let result = PluginsManager::new(tmp.path().to_path_buf())
        .install_plugin(PluginInstallRequest {
            plugin_name: "toolkit".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                repo_root.join(".agents/plugins/marketplace.json"),
            )
            .unwrap(),
        })
        .await
        .unwrap();

    let installed_path = tmp.path().join("plugins/cache/debug/toolkit/local");
    assert_eq!(
        result,
        PluginInstallOutcome {
            plugin_id: PluginId::new("toolkit".to_string(), "debug".to_string()).unwrap(),
            plugin_version: "local".to_string(),
            installed_path: AbsolutePathBuf::try_from(installed_path.clone()).unwrap(),
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
    assert!(installed_path.join(".codex-plugin/plugin.json").is_file());
}

#[tokio::test]
async fn install_plugin_supports_relative_git_subdir_marketplace_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("marketplace");
    let remote_repo = repo_root.join("remote-plugin-repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin(&remote_repo, "plugins/toolkit", "toolkit");
    init_git_repo(&remote_repo);
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "toolkit",
      "source": {
        "source": "git-subdir",
        "url": "./remote-plugin-repo",
        "path": "plugins/toolkit"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let result = PluginsManager::new(tmp.path().to_path_buf())
        .install_plugin(PluginInstallRequest {
            plugin_name: "toolkit".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                repo_root.join(".agents/plugins/marketplace.json"),
            )
            .unwrap(),
        })
        .await
        .unwrap();

    let installed_path = tmp.path().join("plugins/cache/debug/toolkit/local");
    assert_eq!(
        result,
        PluginInstallOutcome {
            plugin_id: PluginId::new("toolkit".to_string(), "debug".to_string()).unwrap(),
            plugin_version: "local".to_string(),
            installed_path: AbsolutePathBuf::try_from(installed_path.clone()).unwrap(),
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
    assert!(installed_path.join(".codex-plugin/plugin.json").is_file());
}

#[tokio::test]
async fn uninstall_plugin_removes_cache_and_config_entry() {
    let tmp = tempfile::tempdir().unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/local",
        "sample-plugin",
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample-plugin@debug"]
enabled = true
"#,
    );

    let manager = PluginsManager::new(tmp.path().to_path_buf());
    manager
        .uninstall_plugin("sample-plugin@debug".to_string())
        .await
        .unwrap();
    manager
        .uninstall_plugin("sample-plugin@debug".to_string())
        .await
        .unwrap();

    assert!(
        !tmp.path()
            .join("plugins/cache/debug/sample-plugin")
            .exists()
    );
    let config = fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).unwrap();
    assert!(!config.contains(r#"[plugins."sample-plugin@debug"]"#));
}

#[tokio::test]
async fn list_marketplaces_includes_enabled_state() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "enabled-plugin/local",
        "enabled-plugin",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "disabled-plugin/local",
        "disabled-plugin",
    );
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "enabled-plugin",
      "source": {
        "source": "local",
        "path": "./enabled-plugin"
      }
    },
    {
      "name": "disabled-plugin",
      "source": {
        "source": "local",
        "path": "./disabled-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."enabled-plugin@debug"]
enabled = true

[plugins."disabled-plugin@debug"]
enabled = false
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(
            &config,
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
            /*include_openai_curated*/ true,
        )
        .unwrap()
        .marketplaces;

    let marketplace = marketplaces
        .into_iter()
        .find(|marketplace| {
            marketplace.path
                == AbsolutePathBuf::try_from(
                    tmp.path().join("repo/.agents/plugins/marketplace.json"),
                )
                .unwrap()
        })
        .expect("expected repo marketplace entry");

    assert_eq!(
        marketplace,
        ConfiguredMarketplace {
            name: "debug".to_string(),
            path: AbsolutePathBuf::try_from(
                tmp.path().join("repo/.agents/plugins/marketplace.json"),
            )
            .unwrap(),
            interface: None,
            plugins: vec![
                ConfiguredMarketplacePlugin {
                    id: "enabled-plugin@debug".to_string(),
                    name: "enabled-plugin".to_string(),
                    local_version: None,
                    installed_version: Some("local".to_string()),
                    source: MarketplacePluginSource::Local {
                        path: AbsolutePathBuf::try_from(tmp.path().join("repo/enabled-plugin"))
                            .unwrap(),
                    },
                    policy: MarketplacePluginPolicy {
                        installation: MarketplacePluginInstallPolicy::Available,
                        authentication: MarketplacePluginAuthPolicy::OnInstall,
                        products: None,
                    },
                    interface: None,
                    keywords: Vec::new(),
                    manifest_fallback: None,
                    installed: true,
                    enabled: true,
                },
                ConfiguredMarketplacePlugin {
                    id: "disabled-plugin@debug".to_string(),
                    name: "disabled-plugin".to_string(),
                    local_version: None,
                    installed_version: Some("local".to_string()),
                    source: MarketplacePluginSource::Local {
                        path: AbsolutePathBuf::try_from(tmp.path().join("repo/disabled-plugin"),)
                            .unwrap(),
                    },
                    policy: MarketplacePluginPolicy {
                        installation: MarketplacePluginInstallPolicy::Available,
                        authentication: MarketplacePluginAuthPolicy::OnInstall,
                        products: None,
                    },
                    interface: None,
                    keywords: Vec::new(),
                    manifest_fallback: None,
                    installed: true,
                    enabled: false,
                },
            ],
        }
    );
}

#[tokio::test]
async fn list_marketplaces_returns_empty_when_feature_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "enabled-plugin",
      "source": {
        "source": "local",
        "path": "./enabled-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = false

[plugins."enabled-plugin@debug"]
enabled = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(
            &config,
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
            /*include_openai_curated*/ true,
        )
        .unwrap()
        .marketplaces;

    assert_eq!(marketplaces, Vec::new());
}

#[tokio::test]
async fn list_marketplaces_excludes_plugins_with_explicit_empty_products() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "disabled-plugin",
      "source": {
        "source": "local",
        "path": "./disabled-plugin"
      },
      "policy": {
        "products": []
      }
    },
    {
      "name": "default-plugin",
      "source": {
        "source": "local",
        "path": "./default-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(
            &config,
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
            /*include_openai_curated*/ true,
        )
        .unwrap()
        .marketplaces;

    let marketplace = marketplaces
        .into_iter()
        .find(|marketplace| {
            marketplace.path
                == AbsolutePathBuf::try_from(
                    tmp.path().join("repo/.agents/plugins/marketplace.json"),
                )
                .unwrap()
        })
        .expect("expected repo marketplace entry");
    assert_eq!(
        marketplace.plugins,
        vec![ConfiguredMarketplacePlugin {
            id: "default-plugin@debug".to_string(),
            name: "default-plugin".to_string(),
            local_version: None,
            installed_version: None,
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(tmp.path().join("repo/default-plugin")).unwrap(),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
            keywords: Vec::new(),
            manifest_fallback: None,
            installed: false,
            enabled: false,
        }]
    );
}

#[tokio::test]
async fn read_plugin_for_config_returns_plugins_disabled_when_feature_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap();
    fs::write(
        marketplace_path.as_path(),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "enabled-plugin",
      "source": {
        "source": "local",
        "path": "./enabled-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = false

[plugins."enabled-plugin@debug"]
enabled = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let err = PluginsManager::new(tmp.path().to_path_buf())
        .read_plugin_for_config(
            &config,
            &PluginReadRequest {
                plugin_name: "enabled-plugin".to_string(),
                marketplace_path,
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(err, MarketplaceError::PluginsDisabled));
}

#[tokio::test]
async fn read_plugin_for_config_filters_mcp_servers_for_codex_backend_auth() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    }
  ]
}"#,
    );
    write_file(
        &repo_root.join("sample-plugin/.codex-plugin/plugin.json"),
        r#"{"name":"sample-plugin"}"#,
    );
    write_file(
        &repo_root.join("sample-plugin/.app.json"),
        r#"{"apps":{"sample-mcp":{"id":"connector_sample"}}}"#,
    );
    write_file(
        &repo_root.join("sample-plugin/.mcp.json"),
        r#"{"mcpServers":{"other-mcp":{"command":"other-mcp"},"sample-mcp":{"command":"sample-mcp"}}}"#,
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let request = PluginReadRequest {
        plugin_name: "sample-plugin".to_string(),
        marketplace_path: AbsolutePathBuf::try_from(
            repo_root.join(".agents/plugins/marketplace.json"),
        )
        .unwrap(),
    };

    let chatgpt_outcome = PluginsManager::new_with_options(
        tmp.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::Chatgpt),
    )
    .read_plugin_for_config(&config, &request)
    .await
    .unwrap();
    assert_eq!(
        chatgpt_outcome.plugin.mcp_server_names,
        vec!["other-mcp".to_string()]
    );
    assert_eq!(
        chatgpt_outcome.plugin.apps,
        vec![AppConnectorId("connector_sample".to_string())]
    );

    let api_key_outcome = PluginsManager::new_with_options(
        tmp.path().to_path_buf(),
        Some(Product::Codex),
        Some(AuthMode::ApiKey),
    )
    .read_plugin_for_config(&config, &request)
    .await
    .unwrap();
    assert_eq!(
        api_key_outcome.plugin.mcp_server_names,
        vec!["other-mcp".to_string(), "sample-mcp".to_string()]
    );
    assert!(api_key_outcome.plugin.apps.is_empty());
}

#[tokio::test]
async fn read_plugin_for_config_uses_marketplace_manifest_fallback_paths_for_local_source() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let plugin_root = repo_root.join("sample-plugin");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": "./sample-plugin",
      "apps": "./config/custom.app.json",
      "mcpServers": {
        "sample-mcp": {
          "command": "sample-mcp"
        }
      }
    }
  ]
}"#,
    );
    write_file(
        &plugin_root.join("config/custom.app.json"),
        r#"{"apps":{"sample-app":{"id":"connector_sample"}}}"#,
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let outcome = manager
        .read_plugin_for_config(
            &config,
            &PluginReadRequest {
                plugin_name: "sample-plugin".to_string(),
                marketplace_path: AbsolutePathBuf::try_from(
                    repo_root.join(".agents/plugins/marketplace.json"),
                )
                .unwrap(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        outcome.plugin.apps,
        vec![AppConnectorId("connector_sample".to_string())]
    );
    assert_eq!(
        outcome.plugin.mcp_server_names,
        vec!["sample-mcp".to_string()]
    );

    let listed_plugin = manager
        .list_marketplaces_for_config(
            &config,
            &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
            /*include_openai_curated*/ false,
        )
        .unwrap()
        .marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == "debug")
        .unwrap()
        .plugins
        .into_iter()
        .find(|plugin| plugin.name == "sample-plugin")
        .unwrap();
    let listed_detail = manager
        .read_plugin_detail_for_marketplace_plugin(&config, "debug", listed_plugin)
        .await
        .unwrap();
    assert_eq!(
        listed_detail.apps,
        vec![AppConnectorId("connector_sample".to_string())]
    );
    assert_eq!(
        listed_detail.mcp_server_names,
        vec!["sample-mcp".to_string()]
    );
}

#[tokio::test]
async fn read_plugin_for_config_does_not_fallback_from_invalid_plugin_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let plugin_root = repo_root.join("sample-plugin");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": "./sample-plugin",
      "description": "Fallback metadata"
    }
  ]
}"#,
    );
    write_file(&plugin_root.join(".codex-plugin/plugin.json"), "{");
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let err = PluginsManager::new(tmp.path().to_path_buf())
        .read_plugin_for_config(
            &config,
            &PluginReadRequest {
                plugin_name: "sample-plugin".to_string(),
                marketplace_path: AbsolutePathBuf::try_from(
                    repo_root.join(".agents/plugins/marketplace.json"),
                )
                .unwrap(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(err.to_string(), "missing or invalid plugin.json");
}

#[tokio::test]
async fn read_plugin_for_config_uses_user_layer_skill_settings_only() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let plugin_root = repo_root.join("enabled-plugin");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "enabled-plugin",
      "source": {
        "source": "local",
        "path": "./enabled-plugin"
      }
    }
  ]
}"#,
    );
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"enabled-plugin"}"#,
    );
    write_file(
        &plugin_root.join("skills/sample-search/SKILL.md"),
        "---\nname: sample-search\ndescription: search sample data\n---\n",
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."enabled-plugin@debug"]
enabled = true
"#,
    );
    write_file(
        &repo_root.join(".codex/config.toml"),
        r#"[[skills.config]]
name = "enabled-plugin:sample-search"
enabled = false
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let outcome = PluginsManager::new(tmp.path().to_path_buf())
        .read_plugin_for_config(
            &config,
            &PluginReadRequest {
                plugin_name: "enabled-plugin".to_string(),
                marketplace_path: AbsolutePathBuf::try_from(
                    repo_root.join(".agents/plugins/marketplace.json"),
                )
                .unwrap(),
            },
        )
        .await
        .unwrap();

    assert!(outcome.plugin.disabled_skill_paths.is_empty());
}

#[tokio::test]
async fn read_plugin_for_config_uninstalled_git_source_requires_install_without_cloning() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let missing_remote_repo = tmp.path().join("missing-remote-plugin-repo");
    let missing_remote_repo_url = url::Url::from_directory_path(&missing_remote_repo)
        .unwrap()
        .to_string();
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "debug",
  "plugins": [
    {{
      "name": "toolkit",
      "source": {{
        "source": "git-subdir",
        "url": "{missing_remote_repo_url}",
        "path": "plugins/toolkit"
      }},
      "policy": {{
        "installation": "AVAILABLE",
        "authentication": "ON_INSTALL"
      }}
    }}
  ]
}}"#
        ),
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let outcome = PluginsManager::new(tmp.path().to_path_buf())
        .read_plugin_for_config(
            &config,
            &PluginReadRequest {
                plugin_name: "toolkit".to_string(),
                marketplace_path: AbsolutePathBuf::try_from(
                    repo_root.join(".agents/plugins/marketplace.json"),
                )
                .unwrap(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        outcome.plugin.details_unavailable_reason,
        Some(PluginDetailsUnavailableReason::InstallRequiredForRemoteSource)
    );
    assert!(!outcome.plugin.installed);
    let expected_description = format!(
        "This is a cross-repo plugin. Install it to view more detailed information. The source of the plugin is {missing_remote_repo_url}, path `plugins/toolkit`."
    );
    assert_eq!(
        outcome.plugin.description.as_deref(),
        Some(expected_description.as_str())
    );
    assert!(outcome.plugin.skills.is_empty());
    assert!(outcome.plugin.apps.is_empty());
    assert!(outcome.plugin.mcp_server_names.is_empty());
    assert!(
        !tmp.path()
            .join("plugins/.marketplace-plugin-source-staging")
            .exists()
    );
}

#[tokio::test]
async fn read_plugin_for_config_installed_git_source_reads_from_cache_without_cloning() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let missing_remote_repo = tmp.path().join("missing-remote-plugin-repo");
    let missing_remote_repo_url = url::Url::from_directory_path(&missing_remote_repo)
        .unwrap()
        .to_string();
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "debug",
  "plugins": [
    {{
      "name": "toolkit",
      "source": {{
        "source": "git-subdir",
        "url": "{missing_remote_repo_url}",
        "path": "plugins/toolkit"
      }},
      "category": "Developer Tools"
    }}
  ]
}}"#
        ),
    );
    let cached_plugin_root = tmp.path().join("plugins/cache/debug/toolkit/local");
    write_file(
        &cached_plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "toolkit",
  "description": "Cached toolkit plugin",
  "interface": {
    "displayName": "Toolkit"
  }
}"#,
    );
    write_file(
        &cached_plugin_root.join("skills/search/SKILL.md"),
        "---\nname: search\ndescription: search cached data\n---\n",
    );
    write_file(
        &cached_plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "calendar": {
      "id": "connector_calendar",
      "category": "First Category"
    },
    "calendar_duplicate": {
      "id": "connector_calendar",
      "category": "Second Category"
    }
  }
}"#,
    );
    write_file(
        &cached_plugin_root.join(".mcp.json"),
        r#"{"mcpServers":{"toolkit":{"command":"toolkit-mcp"}}}"#,
    );
    write_file(
        &cached_plugin_root.join("hooks/hooks.json"),
        r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "echo startup"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "echo first"
          },
          {
            "type": "command",
            "command": "echo second"
          }
        ]
      }
    ]
  }
}"#,
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."toolkit@debug"]
enabled = true

[hooks.state."toolkit@debug:hooks/hooks.json:pre_tool_use:0:0"]
enabled = false
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let outcome = PluginsManager::new(tmp.path().to_path_buf())
        .read_plugin_for_config(
            &config,
            &PluginReadRequest {
                plugin_name: "toolkit".to_string(),
                marketplace_path: AbsolutePathBuf::try_from(
                    repo_root.join(".agents/plugins/marketplace.json"),
                )
                .unwrap(),
            },
        )
        .await
        .unwrap();

    assert_eq!(outcome.plugin.details_unavailable_reason, None);
    assert_eq!(
        outcome.plugin.description.as_deref(),
        Some("Cached toolkit plugin")
    );
    assert_eq!(
        outcome.plugin.interface,
        Some(PluginManifestInterface {
            display_name: Some("Toolkit".to_string()),
            category: Some("Developer Tools".to_string()),
            ..Default::default()
        })
    );
    assert!(outcome.plugin.installed);
    assert_eq!(outcome.plugin.skills.len(), 1);
    assert_eq!(outcome.plugin.skills[0].name, "toolkit:search");
    assert_eq!(
        outcome.plugin.apps,
        vec![AppConnectorId("connector_calendar".to_string())]
    );
    assert_eq!(
        outcome.plugin.app_category_by_id,
        HashMap::from([(
            "connector_calendar".to_string(),
            "First Category".to_string()
        )])
    );
    assert_eq!(
        outcome.plugin.hooks,
        vec![
            PluginHookSummary {
                key: "toolkit@debug:hooks/hooks.json:pre_tool_use:0:0".to_string(),
                event_name: HookEventName::PreToolUse,
            },
            PluginHookSummary {
                key: "toolkit@debug:hooks/hooks.json:pre_tool_use:0:1".to_string(),
                event_name: HookEventName::PreToolUse,
            },
            PluginHookSummary {
                key: "toolkit@debug:hooks/hooks.json:session_start:0:0".to_string(),
                event_name: HookEventName::SessionStart,
            },
        ]
    );
    assert_eq!(outcome.plugin.mcp_server_names, vec!["toolkit".to_string()]);
    assert!(
        !tmp.path()
            .join("plugins/.marketplace-plugin-source-staging")
            .exists()
    );
}

#[tokio::test]
async fn list_marketplaces_installed_git_source_reads_metadata_from_cache_without_cloning() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let missing_remote_repo = tmp.path().join("missing-remote-plugin-repo");
    let missing_remote_repo_url = url::Url::from_directory_path(&missing_remote_repo)
        .unwrap()
        .to_string();
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "debug",
  "plugins": [
    {{
      "name": "toolkit",
      "source": {{
        "source": "git-subdir",
        "url": "{missing_remote_repo_url}",
        "path": "plugins/toolkit"
      }},
      "category": "Developer Tools"
    }}
  ]
}}"#
        ),
    );
    let cached_plugin_root = tmp.path().join("plugins/cache/debug/toolkit/local");
    write_file(
        &cached_plugin_root.join(".codex-plugin/plugin.json"),
        r##"{
  "name": "toolkit",
  "interface": {
    "displayName": "Toolkit",
    "shortDescription": "Search cached data",
    "category": "Cached Category",
    "brandColor": "#3B82F6",
    "composerIcon": "./assets/icon.png",
    "logo": "./assets/logo.png",
    "screenshots": ["./assets/screenshot.png"]
  }
}"##,
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."toolkit@debug"]
enabled = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(
            &config,
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
            /*include_openai_curated*/ true,
        )
        .unwrap()
        .marketplaces;

    let marketplace = marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == "debug")
        .expect("debug marketplace should be listed");

    let mut plugins = marketplace.plugins;
    assert!(plugins[0].manifest_fallback.is_some());
    plugins[0].manifest_fallback = None;
    assert_eq!(
        plugins,
        vec![ConfiguredMarketplacePlugin {
            id: "toolkit@debug".to_string(),
            name: "toolkit".to_string(),
            local_version: None,
            installed_version: Some("local".to_string()),
            source: MarketplacePluginSource::Git {
                url: missing_remote_repo_url,
                path: Some("plugins/toolkit".to_string()),
                ref_name: None,
                sha: None,
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: Some(PluginManifestInterface {
                display_name: Some("Toolkit".to_string()),
                short_description: Some("Search cached data".to_string()),
                category: Some("Developer Tools".to_string()),
                brand_color: Some("#3B82F6".to_string()),
                composer_icon: Some(
                    AbsolutePathBuf::try_from(cached_plugin_root.join("assets/icon.png")).unwrap(),
                ),
                logo: Some(
                    AbsolutePathBuf::try_from(cached_plugin_root.join("assets/logo.png")).unwrap(),
                ),
                screenshots: vec![
                    AbsolutePathBuf::try_from(cached_plugin_root.join("assets/screenshot.png"))
                        .unwrap(),
                ],
                ..Default::default()
            }),
            keywords: Vec::new(),
            manifest_fallback: None,
            installed: true,
            enabled: true,
        }]
    );
    assert!(
        !tmp.path()
            .join("plugins/.marketplace-plugin-source-staging")
            .exists()
    );
}

#[tokio::test]
async fn list_marketplaces_includes_curated_repo_marketplace() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    let plugin_root = curated_root.join("plugins/linear");

    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );
    fs::create_dir_all(curated_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        curated_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-curated",
  "plugins": [
    {
      "name": "linear",
      "source": {
        "source": "local",
        "path": "./plugins/linear"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"linear"}"#,
    )
    .unwrap();

    let config = load_config(tmp.path(), tmp.path()).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(&config, &[], /*include_openai_curated*/ true)
        .unwrap()
        .marketplaces;

    let curated_marketplace = marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == "openai-curated")
        .expect("curated marketplace should be listed");

    assert_eq!(
        curated_marketplace,
        ConfiguredMarketplace {
            name: "openai-curated".to_string(),
            path: AbsolutePathBuf::try_from(curated_root.join(".agents/plugins/marketplace.json"))
                .unwrap(),
            interface: None,
            plugins: vec![ConfiguredMarketplacePlugin {
                id: "linear@openai-curated".to_string(),
                name: "linear".to_string(),
                local_version: None,
                installed_version: None,
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(curated_root.join("plugins/linear")).unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
                manifest_fallback: None,
                installed: false,
                enabled: false,
            }],
        }
    );
}

#[tokio::test]
async fn list_marketplaces_can_skip_openai_curated_before_loading() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());

    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );
    write_file(
        &curated_root.join(".agents/plugins/marketplace.json"),
        "{not valid json",
    );

    let config = load_config(tmp.path(), tmp.path()).await;
    let outcome = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(&config, &[], /*include_openai_curated*/ false)
        .unwrap();

    assert_eq!(outcome.errors, Vec::new());
    assert_eq!(
        outcome
            .marketplaces
            .iter()
            .any(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME),
        false
    );
}

#[tokio::test]
async fn list_marketplaces_uses_api_curated_manifest_when_selected() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());

    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );
    write_file(
        &curated_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-curated",
  "plugins": [
    {
      "name": "siwc-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/siwc-plugin"
      }
    }
  ]
}"#,
    );
    write_file(
        &curated_root.join(".agents/plugins/api_marketplace.json"),
        r#"{
  "name": "openai-api-curated",
  "interface": {
    "displayName": "OpenAI Curated"
  },
  "plugins": [
    {
      "name": "api-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/api-plugin"
      }
    }
  ]
}"#,
    );

    let config = load_config(tmp.path(), tmp.path()).await;
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    manager.set_auth_mode(Some(AuthMode::ApiKey));
    let marketplaces = manager
        .list_marketplaces_for_config(&config, &[], /*include_openai_curated*/ true)
        .unwrap()
        .marketplaces;
    let curated_marketplace = marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == OPENAI_API_CURATED_MARKETPLACE_NAME)
        .expect("API curated marketplace should be listed");

    assert_eq!(
        curated_marketplace,
        ConfiguredMarketplace {
            name: "openai-api-curated".to_string(),
            path: AbsolutePathBuf::try_from(
                curated_root.join(".agents/plugins/api_marketplace.json")
            )
            .unwrap(),
            interface: Some(MarketplaceInterface {
                display_name: Some("OpenAI Curated".to_string()),
            }),
            plugins: vec![ConfiguredMarketplacePlugin {
                id: "api-plugin@openai-api-curated".to_string(),
                name: "api-plugin".to_string(),
                local_version: None,
                installed_version: None,
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(curated_root.join("plugins/api-plugin"))
                        .unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
                manifest_fallback: None,
                installed: false,
                enabled: false,
            }],
        }
    );
}

#[tokio::test]
async fn list_marketplaces_skips_missing_api_curated_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());

    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );
    write_file(
        &curated_root.join(".agents/plugins/marketplace.json"),
        "{not valid json",
    );

    let config = load_config(tmp.path(), tmp.path()).await;
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    manager.set_auth_mode(Some(AuthMode::BedrockApiKey));
    let outcome = manager
        .list_marketplaces_for_config(&config, &[], /*include_openai_curated*/ true)
        .unwrap();

    assert_eq!(outcome.errors, Vec::new());
    assert_eq!(
        outcome
            .marketplaces
            .iter()
            .any(|marketplace| marketplace.name == OPENAI_API_CURATED_MARKETPLACE_NAME),
        false
    );
}

#[tokio::test]
async fn list_marketplaces_includes_installed_marketplace_roots() {
    let tmp = tempfile::tempdir().unwrap();
    let marketplace_root = marketplace_install_root(tmp.path()).join("debug");
    let plugin_root = marketplace_root.join("plugins/sample");

    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[marketplaces.debug]
last_updated = "2026-04-10T12:34:56Z"
source_type = "git"
source = "/tmp/debug"
"#,
    );
    fs::create_dir_all(marketplace_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )
    .unwrap();
    let config = load_config(tmp.path(), tmp.path()).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(&config, &[], /*include_openai_curated*/ true)
        .unwrap()
        .marketplaces;

    let marketplace = marketplaces
        .into_iter()
        .find(|marketplace| {
            marketplace.path
                == AbsolutePathBuf::try_from(
                    marketplace_root.join(".agents/plugins/marketplace.json"),
                )
                .unwrap()
        })
        .expect("installed marketplace should be listed");

    assert_eq!(
        marketplace.path,
        AbsolutePathBuf::try_from(marketplace_root.join(".agents/plugins/marketplace.json"))
            .unwrap()
    );
    assert_eq!(marketplace.plugins.len(), 1);
    assert_eq!(marketplace.plugins[0].id, "sample@debug");
    assert_eq!(
        marketplace.plugins[0].source,
        MarketplacePluginSource::Local {
            path: AbsolutePathBuf::try_from(plugin_root).unwrap(),
        }
    );
}

#[tokio::test]
async fn configured_marketplace_upgrade_invalidates_cached_tool_suggest_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let remote_repo = tmp.path().join("remote-marketplace");
    let remote_repo_url = url::Url::from_directory_path(&remote_repo)
        .unwrap()
        .to_string();
    write_file(
        &remote_repo.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
    );
    write_curated_plugin(&remote_repo, "sample");
    write_file(
        &remote_repo.join("plugins/sample/.codex-plugin/plugin.json"),
        r#"{"name":"sample","description":"Before upgrade"}"#,
    );
    init_git_repo(&remote_repo);
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true

[marketplaces.debug]
source_type = "git"
source = "{remote_repo_url}"
"#
        ),
    );

    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let config = load_config(tmp.path(), tmp.path()).await;
    let initial_upgrade = manager
        .upgrade_configured_marketplaces_for_config(&config, /*marketplace_name*/ None)
        .expect("initial marketplace install should succeed");
    assert_eq!(initial_upgrade.errors, Vec::new());
    assert_eq!(initial_upgrade.upgraded_roots.len(), 1);

    let config = load_config(tmp.path(), tmp.path()).await;
    let input = ToolSuggestPluginDiscoveryInput {
        plugins: config.clone(),
        configured_plugin_ids: HashSet::from(["sample@debug".to_string()]),
        disabled_plugin_ids: HashSet::new(),
        loaded_plugin_app_connector_ids: HashSet::new(),
    };
    let expected = ToolSuggestDiscoverablePlugin {
        id: "sample@debug".to_string(),
        remote_plugin_id: None,
        name: "sample".to_string(),
        description: Some("Before upgrade".to_string()),
        has_skills: true,
        mcp_server_names: vec!["sample-docs".to_string()],
        app_connector_ids: vec!["connector_calendar".to_string()],
    };
    assert_eq!(
        manager
            .list_tool_suggest_discoverable_plugins(&input, /*auth*/ None)
            .await
            .expect("initial tool-suggest metadata should load"),
        vec![expected.clone()]
    );

    write_file(
        &remote_repo.join("plugins/sample/.codex-plugin/plugin.json"),
        r#"{"name":"sample","description":"After upgrade"}"#,
    );
    run_git(&remote_repo, &["add", "."]);
    run_git(&remote_repo, &["commit", "-m", "update plugin"]);
    let upgrade = manager
        .upgrade_configured_marketplaces_for_config(&config, /*marketplace_name*/ None)
        .expect("marketplace upgrade should succeed");
    assert_eq!(upgrade.errors, Vec::new());
    assert_eq!(upgrade.upgraded_roots.len(), 1);

    assert_eq!(
        manager
            .list_tool_suggest_discoverable_plugins(&input, /*auth*/ None)
            .await
            .expect("refreshed tool-suggest metadata should load"),
        vec![ToolSuggestDiscoverablePlugin {
            description: Some("After upgrade".to_string()),
            ..expected
        }]
    );
}

#[tokio::test]
async fn list_marketplaces_uses_config_when_known_registry_is_malformed() {
    let tmp = tempfile::tempdir().unwrap();
    let marketplace_root = marketplace_install_root(tmp.path()).join("debug");
    let plugin_root = marketplace_root.join("plugins/sample");
    let registry_path = tmp.path().join(".tmp/known_marketplaces.json");

    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[marketplaces.debug]
last_updated = "2026-04-10T12:34:56Z"
source_type = "git"
source = "/tmp/debug"
"#,
    );
    fs::create_dir_all(marketplace_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )
    .unwrap();
    fs::create_dir_all(registry_path.parent().unwrap()).unwrap();
    fs::write(registry_path, "{not valid json").unwrap();

    let config = load_config(tmp.path(), tmp.path()).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(&config, &[], /*include_openai_curated*/ true)
        .unwrap()
        .marketplaces;

    let marketplace = marketplaces
        .into_iter()
        .find(|marketplace| {
            marketplace.path
                == AbsolutePathBuf::try_from(
                    marketplace_root.join(".agents/plugins/marketplace.json"),
                )
                .unwrap()
        })
        .expect("configured marketplace should be discovered");

    assert_eq!(marketplace.plugins[0].id, "sample@debug");
}

#[tokio::test]
async fn list_marketplaces_ignores_installed_roots_missing_from_config() {
    let tmp = tempfile::tempdir().unwrap();
    let marketplace_root = marketplace_install_root(tmp.path()).join("debug");
    let plugin_root = marketplace_root.join("plugins/sample");

    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );
    fs::create_dir_all(marketplace_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )
    .unwrap();
    let config = load_config(tmp.path(), tmp.path()).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(&config, &[], /*include_openai_curated*/ true)
        .unwrap()
        .marketplaces;

    assert!(
        marketplaces.iter().all(|marketplace| {
            marketplace.path
                != AbsolutePathBuf::try_from(
                    marketplace_root.join(".agents/plugins/marketplace.json"),
                )
                .unwrap()
        }),
        "installed marketplace root missing from config should not be listed"
    );
}

#[tokio::test]
async fn list_marketplaces_uses_first_duplicate_plugin_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_a_root = tmp.path().join("repo-a");
    let repo_b_root = tmp.path().join("repo-b");
    fs::create_dir_all(repo_a_root.join(".git")).unwrap();
    fs::create_dir_all(repo_b_root.join(".git")).unwrap();
    fs::create_dir_all(repo_a_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(repo_b_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_a_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "dup-plugin",
      "source": {
        "source": "local",
        "path": "./from-a"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        repo_b_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "dup-plugin",
      "source": {
        "source": "local",
        "path": "./from-b"
      }
    },
    {
      "name": "b-only-plugin",
      "source": {
        "source": "local",
        "path": "./from-b-only"
      }
    }
  ]
}"#,
    )
    .unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."dup-plugin@debug"]
enabled = true

[plugins."b-only-plugin@debug"]
enabled = false
"#,
    );

    let config = load_config(tmp.path(), &repo_a_root).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(
            &config,
            &[
                AbsolutePathBuf::try_from(repo_a_root).unwrap(),
                AbsolutePathBuf::try_from(repo_b_root).unwrap(),
            ],
            /*include_openai_curated*/ true,
        )
        .unwrap()
        .marketplaces;

    let repo_a_marketplace = marketplaces
        .iter()
        .find(|marketplace| {
            marketplace.path
                == AbsolutePathBuf::try_from(
                    tmp.path().join("repo-a/.agents/plugins/marketplace.json"),
                )
                .unwrap()
        })
        .expect("repo-a marketplace should be listed");
    assert_eq!(
        repo_a_marketplace.plugins,
        vec![ConfiguredMarketplacePlugin {
            id: "dup-plugin@debug".to_string(),
            name: "dup-plugin".to_string(),
            local_version: None,
            installed_version: None,
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(tmp.path().join("repo-a/from-a")).unwrap(),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
            keywords: Vec::new(),
            manifest_fallback: None,
            installed: false,
            enabled: true,
        }]
    );

    let repo_b_marketplace = marketplaces
        .iter()
        .find(|marketplace| {
            marketplace.path
                == AbsolutePathBuf::try_from(
                    tmp.path().join("repo-b/.agents/plugins/marketplace.json"),
                )
                .unwrap()
        })
        .expect("repo-b marketplace should be listed");
    assert_eq!(
        repo_b_marketplace.plugins,
        vec![ConfiguredMarketplacePlugin {
            id: "b-only-plugin@debug".to_string(),
            name: "b-only-plugin".to_string(),
            local_version: None,
            installed_version: None,
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(tmp.path().join("repo-b/from-b-only")).unwrap(),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
            keywords: Vec::new(),
            manifest_fallback: None,
            installed: false,
            enabled: false,
        }]
    );

    let duplicate_plugin_count = marketplaces
        .iter()
        .flat_map(|marketplace| marketplace.plugins.iter())
        .filter(|plugin| plugin.name == "dup-plugin")
        .count();
    assert_eq!(duplicate_plugin_count, 1);
}

#[tokio::test]
async fn list_marketplaces_marks_configured_plugin_uninstalled_when_cache_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample-plugin@debug"]
enabled = true
"#,
    );

    let config = load_config(tmp.path(), &repo_root).await;
    let marketplaces = PluginsManager::new(tmp.path().to_path_buf())
        .list_marketplaces_for_config(
            &config,
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
            /*include_openai_curated*/ true,
        )
        .unwrap()
        .marketplaces;

    let marketplace = marketplaces
        .into_iter()
        .find(|marketplace| {
            marketplace.path
                == AbsolutePathBuf::try_from(
                    tmp.path().join("repo/.agents/plugins/marketplace.json"),
                )
                .unwrap()
        })
        .expect("expected repo marketplace entry");

    assert_eq!(
        marketplace,
        ConfiguredMarketplace {
            name: "debug".to_string(),
            path: AbsolutePathBuf::try_from(
                tmp.path().join("repo/.agents/plugins/marketplace.json"),
            )
            .unwrap(),
            interface: None,
            plugins: vec![ConfiguredMarketplacePlugin {
                id: "sample-plugin@debug".to_string(),
                name: "sample-plugin".to_string(),
                local_version: None,
                installed_version: None,
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(tmp.path().join("repo/sample-plugin")).unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
                manifest_fallback: None,
                installed: false,
                enabled: true,
            }],
        }
    );
}

#[tokio::test]
async fn featured_plugin_ids_for_config_uses_restriction_product_query_param() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/featured"))
        .and(query_param("platform", "chat"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"["chat-plugin"]"#))
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
    let manager = PluginsManager::new_with_options(
        tmp.path().to_path_buf(),
        Some(Product::Chatgpt),
        /*auth_mode*/ None,
    );

    let featured_plugin_ids = manager
        .featured_plugin_ids_for_config(
            &config,
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        )
        .await
        .unwrap();

    assert_eq!(featured_plugin_ids, vec!["chat-plugin".to_string()]);
}

#[tokio::test]
async fn featured_plugin_ids_for_config_defaults_query_param_to_codex() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/featured"))
        .and(query_param("platform", "codex"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"["codex-plugin"]"#))
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
    let manager = PluginsManager::new_with_options(
        tmp.path().to_path_buf(),
        /*restriction_product*/ None,
        /*auth_mode*/ None,
    );

    let featured_plugin_ids = manager
        .featured_plugin_ids_for_config(&config, /*auth*/ None)
        .await
        .unwrap();

    assert_eq!(featured_plugin_ids, vec!["codex-plugin".to_string()]);
}

#[tokio::test]
async fn remote_plugin_caches_refresh_warms_recommended_plugins_cache() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .and(query_param("scope", "GLOBAL"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "enabled": true,
            "plugins": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = server.uri();
    let manager = std::sync::Arc::new(PluginsManager::new(tmp.path().to_path_buf()));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let cache_key = recommended_plugins_cache_key(&config);

    manager.maybe_start_remote_plugin_caches_refresh(
        &config,
        Some(auth.clone()),
        /*on_effective_plugins_changed*/ None,
    );

    let mode = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(mode) = manager.cached_recommended_plugins_mode(&cache_key) {
                break mode;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("recommended plugins cache should be warmed");
    assert_eq!(
        mode,
        RecommendedPluginsMode::Endpoint {
            plugins: Vec::new()
        }
    );
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&config, Some(&auth))
            .await,
        mode
    );
    manager.clear_recommended_plugins_cache();
    assert_eq!(manager.cached_recommended_plugins_mode(&cache_key), None);
}

#[tokio::test]
async fn recommended_plugins_mode_deduplicates_concurrent_cache_misses() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .and(query_param("scope", "GLOBAL"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(header("OAI-Product-Sku", "codex"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "enabled": true,
                    "plugins": [
                        {
                            "id": "plugin_slack",
                            "name": "slack",
                            "release": {
                                "display_name": "Slack",
                                "app_ids": ["connector_slack"]
                            }
                        },
                        {
                            "id": "plugin_github",
                            "name": "github",
                            "release": {"display_name": "GitHub"}
                        }
                    ]
                }))
                .set_delay(Duration::from_millis(100)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = server.uri();
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let expected = RecommendedPluginsMode::Endpoint {
        plugins: vec![
            RecommendedPlugin {
                config_id: "github@openai-curated-remote".to_string(),
                remote_plugin_id: "plugin_github".to_string(),
                display_name: "GitHub".to_string(),
                app_connector_ids: Vec::new(),
            },
            RecommendedPlugin {
                config_id: "slack@openai-curated-remote".to_string(),
                remote_plugin_id: "plugin_slack".to_string(),
                display_name: "Slack".to_string(),
                app_connector_ids: vec!["connector_slack".to_string()],
            },
        ],
    };

    let (left, right) = tokio::join!(
        manager.recommended_plugins_mode_for_config(&config, Some(&auth)),
        manager.recommended_plugins_mode_for_config(&config, Some(&auth)),
    );
    assert_eq!((left, right), (expected.clone(), expected.clone()));
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&config, Some(&auth))
            .await,
        expected
    );
}

#[tokio::test]
async fn recommended_plugin_candidates_filter_installed_and_disabled_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "enabled": true,
            "plugins": [
                {
                    "id": "plugin_linear",
                    "name": "linear",
                    "release": {"display_name": "Linear"}
                },
                {
                    "id": "plugin_github",
                    "name": "github",
                    "release": {"display_name": "GitHub"}
                },
                {
                    "id": "plugin_slack",
                    "name": "slack",
                    "release": {"display_name": "Slack"}
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = server.uri();
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let mut installed_linear = remote_installed_plugin("linear");
    installed_linear.id = "plugin_linear".to_string();
    manager.write_remote_installed_plugins_cache(vec![installed_linear]);
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let disabled_tools = [ToolSuggestDisabledTool::plugin(
        "github@openai-curated-remote",
    )];
    let loaded_plugins = manager.plugins_for_config(&config).await;

    let candidates = manager
        .recommended_plugin_candidates_for_config(RecommendedPluginCandidatesInput {
            plugins_config: &config,
            loaded_plugins: &loaded_plugins,
            auth: Some(&auth),
            disabled_tools: &disabled_tools,
            app_server_client_name: None,
        })
        .await;

    assert_eq!(
        candidates,
        Some(vec![DiscoverableTool::from(DiscoverablePluginInfo {
            id: "slack@openai-curated-remote".to_string(),
            remote_plugin_id: Some("plugin_slack".to_string()),
            name: "Slack".to_string(),
            description: None,
            has_skills: false,
            mcp_server_names: Vec::new(),
            app_connector_ids: Vec::new(),
        })])
    );
}

#[tokio::test]
async fn recommended_plugins_mode_caches_explicit_false() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "enabled": false,
            "plugins": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = server.uri();
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&config, Some(&auth))
            .await,
        RecommendedPluginsMode::Legacy
    );
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&config, Some(&auth))
            .await,
        RecommendedPluginsMode::Legacy
    );
}

#[tokio::test]
async fn recommended_plugins_mode_retries_after_fetch_failure() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .respond_with(ResponseTemplate::new(500).set_body_string("unavailable"))
        .expect(1)
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = server.uri();
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&config, Some(&auth))
            .await,
        RecommendedPluginsMode::Legacy
    );

    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "enabled": true,
            "plugins": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    assert_eq!(
        manager
            .recommended_plugins_mode_for_config(&config, Some(&auth))
            .await,
        RecommendedPluginsMode::Endpoint {
            plugins: Vec::new()
        }
    );
}

#[test]
fn refresh_curated_plugin_cache_replaces_existing_local_version_with_short_sha_version() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);
    let plugin_id = PluginId::new(
        "slack".to_string(),
        OPENAI_CURATED_MARKETPLACE_NAME.to_string(),
    )
    .unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        "slack/local",
        "slack",
    );

    assert!(
        refresh_curated_plugin_cache(tmp.path(), TEST_CURATED_PLUGIN_SHA, &[plugin_id])
            .expect("cache refresh should succeed")
    );

    assert!(
        !tmp.path()
            .join("plugins/cache/openai-curated/slack/local")
            .exists()
    );
    assert!(
        tmp.path()
            .join(format!(
                "plugins/cache/openai-curated/slack/{TEST_CURATED_PLUGIN_CACHE_VERSION}"
            ))
            .is_dir()
    );
}

#[test]
fn refresh_curated_plugin_cache_reinstalls_missing_configured_plugin_with_current_short_version() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);
    let plugin_id = PluginId::new(
        "slack".to_string(),
        OPENAI_CURATED_MARKETPLACE_NAME.to_string(),
    )
    .unwrap();

    assert!(
        refresh_curated_plugin_cache(tmp.path(), TEST_CURATED_PLUGIN_SHA, &[plugin_id])
            .expect("cache refresh should recreate missing configured plugin")
    );

    assert!(
        tmp.path()
            .join(format!(
                "plugins/cache/openai-curated/slack/{TEST_CURATED_PLUGIN_CACHE_VERSION}"
            ))
            .is_dir()
    );
}

#[test]
fn refresh_curated_plugin_cache_reinstalls_missing_api_curated_plugin() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &[]);
    write_openai_api_curated_marketplace(&curated_root, &["api-only"]);
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);
    let plugin_id = PluginId::new(
        "api-only".to_string(),
        OPENAI_API_CURATED_MARKETPLACE_NAME.to_string(),
    )
    .unwrap();

    assert!(
        refresh_curated_plugin_cache(tmp.path(), TEST_CURATED_PLUGIN_SHA, &[plugin_id])
            .expect("cache refresh should recreate missing configured API curated plugin")
    );

    assert!(
        tmp.path()
            .join(format!(
                "plugins/cache/openai-api-curated/api-only/{TEST_CURATED_PLUGIN_CACHE_VERSION}"
            ))
            .is_dir()
    );
}

#[test]
fn refresh_curated_plugin_cache_leaves_api_curated_plugin_when_api_manifest_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &[]);
    write_cached_plugin(tmp.path(), OPENAI_API_CURATED_MARKETPLACE_NAME, "api-only");
    let plugin_id = PluginId::new(
        "api-only".to_string(),
        OPENAI_API_CURATED_MARKETPLACE_NAME.to_string(),
    )
    .unwrap();

    assert!(
        !refresh_curated_plugin_cache(tmp.path(), TEST_CURATED_PLUGIN_SHA, &[plugin_id])
            .expect("cache refresh should skip missing API curated manifest")
    );
    assert!(
        tmp.path()
            .join("plugins/cache/openai-api-curated/api-only/local")
            .is_dir()
    );
}

#[test]
fn refresh_curated_plugin_cache_removes_cache_for_plugin_removed_from_marketplace() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &[]);
    let plugin_id = PluginId::new(
        "google-sheets".to_string(),
        OPENAI_CURATED_MARKETPLACE_NAME.to_string(),
    )
    .unwrap();
    let plugin_cache_root = tmp
        .path()
        .join("plugins/cache/openai-curated/google-sheets");
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        &format!("google-sheets/{TEST_CURATED_PLUGIN_CACHE_VERSION}"),
        "google-sheets",
    );

    assert!(
        refresh_curated_plugin_cache(tmp.path(), TEST_CURATED_PLUGIN_SHA, &[plugin_id])
            .expect("cache refresh should remove stale configured plugin")
    );

    assert!(!plugin_cache_root.exists());
}

#[test]
fn curated_plugin_ids_from_config_keys_reads_latest_codex_home_user_config() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."slack@openai-curated"]
enabled = true

[plugins."api-only@openai-api-curated"]
enabled = true

[plugins."sample@debug"]
enabled = true
"#,
    );

    assert_eq!(
        configured_curated_plugin_ids_from_codex_home(tmp.path())
            .into_iter()
            .map(|plugin_id| plugin_id.as_key())
            .collect::<Vec<_>>(),
        vec![
            "api-only@openai-api-curated".to_string(),
            "slack@openai-curated".to_string(),
        ]
    );

    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    assert_eq!(
        configured_curated_plugin_ids_from_codex_home(tmp.path()),
        Vec::<PluginId>::new()
    );
}

#[test]
fn refresh_curated_plugin_cache_returns_false_when_configured_plugins_are_current() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    let plugin_id = PluginId::new(
        "slack".to_string(),
        OPENAI_CURATED_MARKETPLACE_NAME.to_string(),
    )
    .unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        &format!("slack/{TEST_CURATED_PLUGIN_CACHE_VERSION}"),
        "slack",
    );

    assert!(
        !refresh_curated_plugin_cache(tmp.path(), TEST_CURATED_PLUGIN_SHA, &[plugin_id])
            .expect("cache refresh should be a no-op when configured plugins are current")
    );
}

#[test]
fn refresh_curated_plugin_cache_migrates_full_sha_cache_version_to_short_version() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    let plugin_id = PluginId::new(
        "slack".to_string(),
        OPENAI_CURATED_MARKETPLACE_NAME.to_string(),
    )
    .unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        &format!("slack/{TEST_CURATED_PLUGIN_SHA}"),
        "slack",
    );

    assert!(
        refresh_curated_plugin_cache(tmp.path(), TEST_CURATED_PLUGIN_SHA, &[plugin_id])
            .expect("cache refresh should migrate the full sha cache version")
    );
    assert!(
        !tmp.path()
            .join(format!(
                "plugins/cache/openai-curated/slack/{TEST_CURATED_PLUGIN_SHA}"
            ))
            .exists()
    );
    assert!(
        tmp.path()
            .join(format!(
                "plugins/cache/openai-curated/slack/{TEST_CURATED_PLUGIN_CACHE_VERSION}"
            ))
            .is_dir()
    );
}

#[test]
fn refresh_non_curated_plugin_cache_replaces_existing_local_version_with_manifest_version() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin_with_version(&repo_root, "sample-plugin", "sample-plugin", Some("1.2.3"));
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    }
  ]
}"#,
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/local",
        "sample-plugin",
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample-plugin@debug"]
enabled = true
"#,
    );

    assert!(
        refresh_non_curated_plugin_cache(
            tmp.path(),
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        )
        .expect("cache refresh should succeed")
    );

    assert!(
        !tmp.path()
            .join("plugins/cache/debug/sample-plugin/local")
            .exists()
    );
    assert!(
        tmp.path()
            .join("plugins/cache/debug/sample-plugin/1.2.3")
            .is_dir()
    );
}

#[test]
fn refresh_non_curated_plugin_cache_reinstalls_missing_configured_plugin_with_manifest_version() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin_with_version(&repo_root, "sample-plugin", "sample-plugin", Some("1.2.3"));
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    }
  ]
}"#,
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample-plugin@debug"]
enabled = true
"#,
    );

    assert!(
        refresh_non_curated_plugin_cache(
            tmp.path(),
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        )
        .expect("cache refresh should reinstall missing configured plugin")
    );

    assert!(
        tmp.path()
            .join("plugins/cache/debug/sample-plugin/1.2.3")
            .is_dir()
    );
}

#[test]
fn refresh_non_curated_plugin_cache_refreshes_configured_git_source() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let remote_repo = tmp.path().join("remote-plugin-repo");
    let remote_repo_url = url::Url::from_directory_path(&remote_repo)
        .unwrap()
        .to_string();
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    write_plugin_with_version(
        &remote_repo,
        "plugins/sample-plugin",
        "sample-plugin",
        Some("1.2.3"),
    );
    init_git_repo(&remote_repo);
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "debug",
  "plugins": [
    {{
      "name": "sample-plugin",
      "source": {{
        "source": "git-subdir",
        "url": "{remote_repo_url}",
        "path": "plugins/sample-plugin"
      }}
    }}
  ]
}}"#
        ),
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample-plugin@debug"]
enabled = true
"#,
    );

    assert!(
        refresh_non_curated_plugin_cache(
            tmp.path(),
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        )
        .expect("cache refresh should materialize configured Git plugin")
    );

    assert!(
        tmp.path()
            .join("plugins/cache/debug/sample-plugin/1.2.3")
            .is_dir()
    );
}

#[test]
fn refresh_non_curated_plugin_cache_returns_false_when_configured_plugins_are_current() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin_with_version(&repo_root, "sample-plugin", "sample-plugin", Some("1.2.3"));
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    }
  ]
}"#,
    );
    write_plugin_with_version(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/1.2.3",
        "sample-plugin",
        Some("1.2.3"),
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample-plugin@debug"]
enabled = true
"#,
    );

    assert!(
        !refresh_non_curated_plugin_cache(
            tmp.path(),
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        )
        .expect("cache refresh should be a no-op when configured plugins are current")
    );
}

#[test]
fn refresh_non_curated_plugin_cache_force_reinstalls_current_local_version() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin(&repo_root, "sample-plugin", "sample-plugin");
    fs::write(repo_root.join("sample-plugin/skills/SKILL.md"), "new skill").unwrap();
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    }
  ]
}"#,
    );
    write_plugin(
        &tmp.path().join("plugins/cache/debug"),
        "sample-plugin/local",
        "sample-plugin",
    );
    fs::write(
        tmp.path()
            .join("plugins/cache/debug/sample-plugin/local/skills/SKILL.md"),
        "old skill",
    )
    .unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample-plugin@debug"]
enabled = true
"#,
    );

    assert!(
        refresh_non_curated_plugin_cache_force_reinstall(
            tmp.path(),
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        )
        .expect("cache refresh should reinstall unchanged local version")
    );

    assert_eq!(
        fs::read_to_string(
            tmp.path()
                .join("plugins/cache/debug/sample-plugin/local/skills/SKILL.md")
        )
        .unwrap(),
        "new skill"
    );
}

#[test]
fn refresh_non_curated_plugin_cache_ignores_invalid_unconfigured_plugin_versions() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    write_plugin_with_version(&repo_root, "sample-plugin", "sample-plugin", Some("1.2.3"));
    write_plugin_with_version(&repo_root, "broken-plugin", "broken-plugin", Some("   "));
    write_file(
        &repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    },
    {
      "name": "broken-plugin",
      "source": {
        "source": "local",
        "path": "./broken-plugin"
      }
    }
  ]
}"#,
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."sample-plugin@debug"]
enabled = true
"#,
    );

    assert!(
        refresh_non_curated_plugin_cache(
            tmp.path(),
            &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        )
        .expect("cache refresh should ignore unrelated invalid plugin manifests")
    );

    assert!(
        tmp.path()
            .join("plugins/cache/debug/sample-plugin/1.2.3")
            .is_dir()
    );
}

#[tokio::test]
async fn load_plugins_ignores_project_config_files() {
    let codex_home = TempDir::new().unwrap();
    let project_root = codex_home.path().join("project");
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    );
    write_file(
        &project_root.join(".codex/config.toml"),
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
    );

    let stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::Project {
                dot_codex_folder: AbsolutePathBuf::try_from(project_root.join(".codex")).unwrap(),
            },
            toml::from_str(&plugin_config_toml(
                /*enabled*/ true, /*plugins_feature_enabled*/ true,
            ))
            .expect("project config should parse"),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack should build");

    let plugins = load_plugins_from_layer_stack(
        &stack,
        std::collections::HashMap::new(),
        &PluginStore::new(codex_home.path().to_path_buf()),
        /*plugin_skill_snapshots*/ None,
        Some(Product::Codex),
        /*prefer_remote_curated_conflicts*/ false,
    )
    .await;

    assert_eq!(plugins, Vec::new());
}

#[tokio::test]
async fn plugin_hooks_for_layer_stack_loads_configured_plugin_hooks() {
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");
    write_plugin(
        codex_home.path().join("plugins/cache/test").as_path(),
        "sample/local",
        "sample",
    );
    write_file(
        &plugin_root.join("hooks/hooks.json"),
        r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "echo startup"
          }
        ]
      }
    ]
  }
}"#,
    );
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
    );
    let config = load_config(codex_home.path(), codex_home.path()).await;

    let outcome = PluginsManager::new(codex_home.path().to_path_buf())
        .plugin_hooks_for_layer_stack(&config.config_layer_stack, &config)
        .await;

    assert_eq!(outcome.hook_sources.len(), 1);
    assert_eq!(
        outcome.hook_sources[0].source_relative_path,
        "hooks/hooks.json"
    );
    assert_eq!(outcome.hook_load_warnings, Vec::<String>::new());
}
