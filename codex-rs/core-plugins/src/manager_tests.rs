use super::*;
use crate::LoadedPlugin;
use crate::PluginLoadOutcome;
use crate::installed_marketplaces::marketplace_install_root;
use crate::loader::load_plugins_from_layer_stack;
use crate::loader::refresh_non_curated_plugin_cache;
use crate::loader::refresh_non_curated_plugin_cache_force_reinstall;
use crate::marketplace::MarketplacePluginInstallPolicy;
use crate::remote::RemoteInstalledPlugin;
use crate::startup_sync::curated_plugins_repo_path;
use crate::test_support::TEST_CURATED_PLUGIN_CACHE_VERSION;
use crate::test_support::TEST_CURATED_PLUGIN_SHA;
use crate::test_support::load_plugins_config as load_plugins_config_input;
use crate::test_support::write_curated_plugin_sha_with as write_curated_plugin_sha;
use crate::test_support::write_file;
use crate::test_support::write_openai_curated_marketplace;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::AppToolApproval;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::McpServerConfig;
use codex_config::McpServerToolConfig;
use codex_config::types::McpServerTransportConfig;
use codex_login::CodexAuth;
use codex_protocol::protocol::Product;
use codex_utils_absolute_path::test_support::PathBufExt;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
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
    plugin_config_toml_with_plugin_hooks(
        enabled,
        plugins_feature_enabled,
        /*plugin_hooks_feature_enabled*/ false,
    )
}

fn plugin_config_toml_with_plugin_hooks(
    enabled: bool,
    plugins_feature_enabled: bool,
    plugin_hooks_feature_enabled: bool,
) -> String {
    let mut root = toml::map::Map::new();

    let mut features = toml::map::Map::new();
    features.insert(
        "plugins".to_string(),
        Value::Boolean(plugins_feature_enabled),
    );
    features.insert(
        "plugin_hooks".to_string(),
        Value::Boolean(plugin_hooks_feature_enabled),
    );
    root.insert("features".to_string(), Value::Table(features));

    let mut plugin = toml::map::Map::new();
    plugin.insert("enabled".to_string(), Value::Boolean(enabled));

    let mut plugins = toml::map::Map::new();
    plugins.insert("sample@test".to_string(), Value::Table(plugin));
    root.insert("plugins".to_string(), Value::Table(plugins));

    toml::to_string(&Value::Table(root)).expect("plugin test config should serialize")
}

async fn load_plugins_from_config(config_toml: &str, codex_home: &Path) -> PluginLoadOutcome {
    write_file(&codex_home.join(CONFIG_TOML_FILE), config_toml);
    let config = load_config(codex_home, codex_home).await;
    PluginsManager::new(codex_home.to_path_buf())
        .plugins_for_config(&config)
        .await
}

async fn load_config(codex_home: &Path, cwd: &Path) -> PluginsConfigInput {
    load_plugins_config_input(codex_home, cwd).await
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
    )
    .await;

    assert_eq!(
        outcome.plugins(),
        vec![LoadedPlugin {
            config_name: "sample@test".to_string(),
            manifest_name: Some("sample".to_string()),
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
                    experimental_environment: None,
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
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            )]),
            apps: vec![AppConnectorId("connector_example".to_string())],
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

    let outcome = load_plugins_from_config(config_toml, codex_home.path()).await;
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
async fn remote_installed_cache_adds_plugin_skill_roots_without_marketplace_config() {
    let codex_home = TempDir::new().unwrap();
    let plugin_base = codex_home
        .path()
        .join("plugins/cache/chatgpt-global/linear");
    write_plugin(&plugin_base, "local", "linear");
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );

    let config = load_config(codex_home.path(), codex_home.path()).await;
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    manager.write_remote_installed_plugins_cache(vec![RemoteInstalledPlugin {
        marketplace_name: "chatgpt-global".to_string(),
        id: "plugins~Plugin_linear".to_string(),
        name: "linear".to_string(),
        enabled: true,
    }]);

    let outcome = manager.plugins_for_config(&config).await;
    assert_eq!(
        outcome.effective_skill_roots(),
        vec![AbsolutePathBuf::try_from(plugin_base.join("local/skills")).unwrap()]
    );
    assert_eq!(outcome.plugins().len(), 1);
    assert_eq!(outcome.plugins()[0].config_name, "linear@chatgpt-global");
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
    manager.write_remote_installed_plugins_cache(vec![RemoteInstalledPlugin {
        marketplace_name: "chatgpt-global".to_string(),
        id: "plugins~Plugin_linear".to_string(),
        name: "linear".to_string(),
        enabled: true,
    }]);

    let outcome = manager.plugins_for_config(&config).await;
    assert_eq!(outcome, PluginLoadOutcome::default());
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
    let outcome = load_plugins_from_config(config_toml, codex_home.path()).await;
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
    let outcome = load_plugins_from_config(config_toml, codex_home.path()).await;

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
    let codex_home = TempDir::new().unwrap();
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join("test/sample/local");

    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "sample",
  "skills": "./custom-skills/",
  "mcpServers": "./config/custom.mcp.json",
  "apps": "./config/custom.app.json"
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
    "default": {
      "id": "connector_default"
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join("config/custom.app.json"),
        r#"{
  "apps": {
    "custom": {
      "id": "connector_custom"
    }
  }
}"#,
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
        codex_home.path(),
    )
    .await;

    assert_eq!(
        outcome.plugins()[0].skill_roots,
        vec![
            plugin_root.join("custom-skills").abs(),
            plugin_root.join("skills").abs()
        ]
    );
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
                experimental_environment: None,
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
                oauth_resource: None,
                tools: HashMap::new(),
            },
        )])
    );
    assert_eq!(
        outcome.plugins()[0].apps,
        vec![AppConnectorId("connector_custom".to_string())]
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
    "default": {
      "id": "connector_default"
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join("config/custom.app.json"),
        r#"{
  "apps": {
    "custom": {
      "id": "connector_custom"
    }
  }
}"#,
    );

    let outcome = load_plugins_from_config(
        &plugin_config_toml(/*enabled*/ true, /*plugins_feature_enabled*/ true),
        codex_home.path(),
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
                experimental_environment: None,
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
                oauth_resource: None,
                tools: HashMap::new(),
            },
        )])
    );
    assert_eq!(
        outcome.plugins()[0].apps,
        vec![AppConnectorId("connector_default".to_string())]
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
    )
    .await;

    assert_eq!(
        outcome.plugins(),
        vec![LoadedPlugin {
            config_name: "sample@test".to_string(),
            manifest_name: None,
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

    let outcome = load_plugins_from_config(&config_toml, codex_home.path()).await;

    assert_eq!(
        outcome.effective_apps(),
        vec![
            AppConnectorId("connector_example".to_string()),
            AppConnectorId("connector_gmail".to_string()),
        ]
    );
}

#[test]
fn capability_index_filters_inactive_and_zero_capability_plugins() {
    let codex_home = TempDir::new().unwrap();
    let connector = |id: &str| AppConnectorId(id.to_string());
    let http_server = |url: &str| McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        experimental_environment: None,
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
        oauth_resource: None,
        tools: HashMap::new(),
    };
    let plugin = |config_name: &str, dir_name: &str, manifest_name: &str| LoadedPlugin {
        config_name: config_name.to_string(),
        manifest_name: Some(manifest_name.to_string()),
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
            apps: vec![connector("connector_example")],
            ..plugin("alpha@test", "alpha-plugin", "alpha-plugin")
        },
        LoadedPlugin {
            mcp_servers: HashMap::from([("beta".to_string(), http_server("https://beta"))]),
            apps: vec![connector("connector_example"), connector("connector_gmail")],
            ..plugin("beta@test", "beta-plugin", "beta-plugin")
        },
        plugin("empty@test", "empty-plugin", "empty-plugin"),
        LoadedPlugin {
            enabled: false,
            skill_roots: vec![codex_home.path().join("disabled-plugin/skills").abs()],
            apps: vec![connector("connector_hidden")],
            ..plugin("disabled@test", "disabled-plugin", "disabled-plugin")
        },
        LoadedPlugin {
            apps: vec![connector("connector_broken")],
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
async fn plugins_for_config_reloads_when_plugin_hooks_enablement_changes() {
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
        &plugin_root.join("hooks/hooks.json"),
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [{ "type": "command", "command": "echo plugin hook" }]
      }
    ]
  }
}"#,
    );

    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &plugin_config_toml_with_plugin_hooks(
            /*enabled*/ true, /*plugins_feature_enabled*/ true,
            /*plugin_hooks_feature_enabled*/ false,
        ),
    );
    let config_without_plugin_hooks = load_config(codex_home.path(), codex_home.path()).await;
    let without_plugin_hooks = manager
        .plugins_for_config(&config_without_plugin_hooks)
        .await;
    assert!(
        without_plugin_hooks
            .effective_plugin_hook_sources()
            .is_empty()
    );

    write_file(
        &codex_home.path().join(CONFIG_TOML_FILE),
        &plugin_config_toml_with_plugin_hooks(
            /*enabled*/ true, /*plugins_feature_enabled*/ true,
            /*plugin_hooks_feature_enabled*/ true,
        ),
    );
    let config_with_plugin_hooks = load_config(codex_home.path(), codex_home.path()).await;
    let with_plugin_hooks = manager.plugins_for_config(&config_with_plugin_hooks).await;
    assert_eq!(with_plugin_hooks.effective_plugin_hook_sources().len(), 1);
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
        .list_marketplaces_for_config(&config, &[AbsolutePathBuf::try_from(repo_root).unwrap()])
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
                    installed: true,
                    enabled: true,
                },
                ConfiguredMarketplacePlugin {
                    id: "disabled-plugin@debug".to_string(),
                    name: "disabled-plugin".to_string(),
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
        .list_marketplaces_for_config(&config, &[AbsolutePathBuf::try_from(repo_root).unwrap()])
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
        .list_marketplaces_for_config(&config, &[AbsolutePathBuf::try_from(repo_root).unwrap()])
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
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(tmp.path().join("repo/default-plugin")).unwrap(),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
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
        r#"{"apps":{"calendar":{"id":"connector_calendar"}}}"#,
    );
    write_file(
        &cached_plugin_root.join(".mcp.json"),
        r#"{"mcpServers":{"toolkit":{"command":"toolkit-mcp"}}}"#,
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
    assert_eq!(outcome.plugin.mcp_server_names, vec!["toolkit".to_string()]);
    assert!(
        !tmp.path()
            .join("plugins/.marketplace-plugin-source-staging")
            .exists()
    );
}

#[tokio::test]
async fn sync_plugins_from_remote_returns_default_when_feature_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = false
"#,
    );

    let config = load_config(tmp.path(), tmp.path()).await;
    let outcome = PluginsManager::new(tmp.path().to_path_buf())
        .sync_plugins_from_remote(&config, /*auth*/ None, /*additive_only*/ false)
        .await
        .unwrap();

    assert_eq!(outcome, RemotePluginSyncResult::default());
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
        .list_marketplaces_for_config(&config, &[])
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
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(curated_root.join("plugins/linear")).unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                installed: false,
                enabled: false,
            }],
        }
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
        .list_marketplaces_for_config(&config, &[])
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
        .list_marketplaces_for_config(&config, &[])
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
        .list_marketplaces_for_config(&config, &[])
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
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(tmp.path().join("repo-a/from-a")).unwrap(),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
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
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(tmp.path().join("repo-b/from-b-only")).unwrap(),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
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
        .list_marketplaces_for_config(&config, &[AbsolutePathBuf::try_from(repo_root).unwrap()])
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
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(tmp.path().join("repo/sample-plugin")).unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                installed: false,
                enabled: true,
            }],
        }
    );
}

#[tokio::test]
async fn sync_plugins_from_remote_reconciles_cache_and_config() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["linear", "gmail", "calendar"]);
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        "linear/local",
        "linear",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        "gmail/local",
        "gmail",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        "calendar/local",
        "calendar",
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."linear@openai-curated"]
enabled = false

[plugins."gmail@openai-curated"]
enabled = false

[plugins."calendar@openai-curated"]
enabled = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/list"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[
  {"id":"1","name":"linear","marketplace_name":"openai-curated","version":"1.0.0","enabled":true},
  {"id":"2","name":"gmail","marketplace_name":"openai-curated","version":"1.0.0","enabled":false}
]"#,
        ))
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let result = manager
        .sync_plugins_from_remote(
            &config,
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            /*additive_only*/ false,
        )
        .await
        .unwrap();

    assert_eq!(
        result,
        RemotePluginSyncResult {
            installed_plugin_ids: Vec::new(),
            enabled_plugin_ids: vec!["linear@openai-curated".to_string()],
            disabled_plugin_ids: Vec::new(),
            uninstalled_plugin_ids: vec![
                "gmail@openai-curated".to_string(),
                "calendar@openai-curated".to_string(),
            ],
        }
    );

    assert!(
        tmp.path()
            .join("plugins/cache/openai-curated/linear/local")
            .is_dir()
    );
    assert!(
        !tmp.path()
            .join("plugins/cache/openai-curated/gmail")
            .exists()
    );
    assert!(
        !tmp.path()
            .join("plugins/cache/openai-curated/calendar")
            .exists()
    );

    let config = fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).unwrap();
    assert!(config.contains(r#"[plugins."linear@openai-curated"]"#));
    assert!(config.contains("enabled = true"));
    assert!(!config.contains(r#"[plugins."gmail@openai-curated"]"#));
    assert!(!config.contains(r#"[plugins."calendar@openai-curated"]"#));

    let synced_config = load_config(tmp.path(), tmp.path()).await;
    let curated_marketplace = manager
        .list_marketplaces_for_config(&synced_config, &[])
        .unwrap()
        .marketplaces
        .into_iter()
        .find(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
        .unwrap();
    assert_eq!(
        curated_marketplace
            .plugins
            .into_iter()
            .map(|plugin| (plugin.id, plugin.installed, plugin.enabled))
            .collect::<Vec<_>>(),
        vec![
            ("linear@openai-curated".to_string(), true, true),
            ("gmail@openai-curated".to_string(), false, false),
            ("calendar@openai-curated".to_string(), false, false),
        ]
    );
}

#[tokio::test]
async fn sync_plugins_from_remote_additive_only_keeps_existing_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["linear", "gmail", "calendar"]);
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        "linear/local",
        "linear",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        "gmail/local",
        "gmail",
    );
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        "calendar/local",
        "calendar",
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."linear@openai-curated"]
enabled = false

[plugins."gmail@openai-curated"]
enabled = false

[plugins."calendar@openai-curated"]
enabled = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/list"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[
  {"id":"1","name":"linear","marketplace_name":"openai-curated","version":"1.0.0","enabled":true},
  {"id":"2","name":"gmail","marketplace_name":"openai-curated","version":"1.0.0","enabled":false}
]"#,
        ))
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let result = manager
        .sync_plugins_from_remote(
            &config,
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            /*additive_only*/ true,
        )
        .await
        .unwrap();

    assert_eq!(
        result,
        RemotePluginSyncResult {
            installed_plugin_ids: Vec::new(),
            enabled_plugin_ids: vec!["linear@openai-curated".to_string()],
            disabled_plugin_ids: Vec::new(),
            uninstalled_plugin_ids: Vec::new(),
        }
    );

    assert!(
        tmp.path()
            .join("plugins/cache/openai-curated/linear/local")
            .is_dir()
    );
    assert!(
        tmp.path()
            .join("plugins/cache/openai-curated/gmail/local")
            .is_dir()
    );
    assert!(
        tmp.path()
            .join("plugins/cache/openai-curated/calendar/local")
            .is_dir()
    );

    let config = fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).unwrap();
    assert!(config.contains(r#"[plugins."linear@openai-curated"]"#));
    assert!(config.contains(r#"[plugins."gmail@openai-curated"]"#));
    assert!(config.contains(r#"[plugins."calendar@openai-curated"]"#));
    assert!(config.contains("enabled = true"));
}

#[tokio::test]
async fn sync_plugins_from_remote_ignores_unknown_remote_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["linear"]);
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."linear@openai-curated"]
enabled = false
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
            .and(path("/backend-api/plugins/list"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[
  {"id":"1","name":"plugin-one","marketplace_name":"openai-curated","version":"1.0.0","enabled":true}
]"#,
            ))
            .mount(&server)
            .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let result = manager
        .sync_plugins_from_remote(
            &config,
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            /*additive_only*/ false,
        )
        .await
        .unwrap();

    assert_eq!(
        result,
        RemotePluginSyncResult {
            installed_plugin_ids: Vec::new(),
            enabled_plugin_ids: Vec::new(),
            disabled_plugin_ids: Vec::new(),
            uninstalled_plugin_ids: vec!["linear@openai-curated".to_string()],
        }
    );
    let config = fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).unwrap();
    assert!(!config.contains(r#"[plugins."linear@openai-curated"]"#));
    assert!(
        !tmp.path()
            .join("plugins/cache/openai-curated/linear")
            .exists()
    );
}

#[tokio::test]
async fn sync_plugins_from_remote_keeps_existing_plugins_when_install_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["linear", "gmail"]);
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);
    fs::remove_dir_all(curated_root.join("plugins/gmail")).unwrap();
    write_plugin(
        &tmp.path().join("plugins/cache/openai-curated"),
        "linear/local",
        "linear",
    );
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."linear@openai-curated"]
enabled = false
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/list"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[
  {"id":"1","name":"gmail","marketplace_name":"openai-curated","version":"1.0.0","enabled":true}
]"#,
        ))
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let err = manager
        .sync_plugins_from_remote(
            &config,
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            /*additive_only*/ false,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        PluginRemoteSyncError::Store(PluginStoreError::Invalid(ref message))
            if message.contains("plugin source path is not a directory")
    ));
    assert!(
        tmp.path()
            .join("plugins/cache/openai-curated/linear/local")
            .is_dir()
    );
    assert!(
        !tmp.path()
            .join("plugins/cache/openai-curated/gmail")
            .exists()
    );

    let config = fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).unwrap();
    assert!(config.contains(r#"[plugins."linear@openai-curated"]"#));
    assert!(!config.contains(r#"[plugins."gmail@openai-curated"]"#));
    assert!(config.contains("enabled = false"));
}

#[tokio::test]
async fn sync_plugins_from_remote_uses_first_duplicate_local_plugin_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_curated_plugin_sha(tmp.path(), TEST_CURATED_PLUGIN_SHA);
    fs::create_dir_all(curated_root.join(".agents/plugins")).unwrap();
    fs::write(
        curated_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-curated",
  "plugins": [
    {
      "name": "gmail",
      "source": {
        "source": "local",
        "path": "./plugins/gmail-first"
      }
    },
    {
      "name": "gmail",
      "source": {
        "source": "local",
        "path": "./plugins/gmail-second"
      }
    }
  ]
}"#,
    )
    .unwrap();
    write_plugin(&curated_root, "plugins/gmail-first", "gmail");
    write_plugin(&curated_root, "plugins/gmail-second", "gmail");
    fs::write(curated_root.join("plugins/gmail-first/marker.txt"), "first").unwrap();
    fs::write(
        curated_root.join("plugins/gmail-second/marker.txt"),
        "second",
    )
    .unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/list"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[
  {"id":"1","name":"gmail","marketplace_name":"openai-curated","version":"1.0.0","enabled":true}
]"#,
        ))
        .mount(&server)
        .await;

    let mut config = load_config(tmp.path(), tmp.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
    let manager = PluginsManager::new(tmp.path().to_path_buf());
    let result = manager
        .sync_plugins_from_remote(
            &config,
            Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            /*additive_only*/ false,
        )
        .await
        .unwrap();

    assert_eq!(
        result,
        RemotePluginSyncResult {
            installed_plugin_ids: vec!["gmail@openai-curated".to_string()],
            enabled_plugin_ids: vec!["gmail@openai-curated".to_string()],
            disabled_plugin_ids: Vec::new(),
            uninstalled_plugin_ids: Vec::new(),
        }
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join(format!(
            "plugins/cache/openai-curated/gmail/{TEST_CURATED_PLUGIN_CACHE_VERSION}/marker.txt"
        )))
        .unwrap(),
        "first"
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
    let manager = PluginsManager::new_with_restriction_product(
        tmp.path().to_path_buf(),
        Some(Product::Chatgpt),
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
    let manager = PluginsManager::new_with_restriction_product(
        tmp.path().to_path_buf(),
        /*restriction_product*/ None,
    );

    let featured_plugin_ids = manager
        .featured_plugin_ids_for_config(&config, /*auth*/ None)
        .await
        .unwrap();

    assert_eq!(featured_plugin_ids, vec!["codex-plugin".to_string()]);
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
fn curated_plugin_ids_from_config_keys_reads_latest_codex_home_user_config() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."slack@openai-curated"]
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
        vec!["slack@openai-curated".to_string()]
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

    let outcome = load_plugins_from_layer_stack(
        &stack,
        std::collections::HashMap::new(),
        &PluginStore::new(codex_home.path().to_path_buf()),
        Some(Product::Codex),
        /*plugin_hooks_enabled*/ false,
    )
    .await;

    assert_eq!(outcome, PluginLoadOutcome::default());
}
