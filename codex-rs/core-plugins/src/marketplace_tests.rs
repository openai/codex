use super::*;
use codex_protocol::protocol::Product;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::tempdir;

const ALTERNATE_MARKETPLACE_RELATIVE_PATH: &str = ".claude-plugin/marketplace.json";
const ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH: &str = ".claude-plugin/plugin.json";

fn find_marketplace_plugin(
    marketplace_path: &AbsolutePathBuf,
    plugin_name: &str,
) -> Result<ResolvedMarketplacePlugin, MarketplaceError> {
    super::find_marketplace_plugin(
        marketplace_path,
        plugin_name,
        MarketplaceLoadContext::unfiltered(),
    )
}

fn find_installable_marketplace_plugin(
    marketplace_path: &AbsolutePathBuf,
    plugin_name: &str,
    restriction_product: Option<Product>,
) -> Result<ResolvedMarketplacePlugin, MarketplaceError> {
    super::find_installable_marketplace_plugin(
        marketplace_path,
        plugin_name,
        restriction_product,
        MarketplaceLoadContext::unfiltered(),
    )
}

fn list_marketplaces_with_home(
    additional_roots: &[AbsolutePathBuf],
    home_dir: Option<&Path>,
) -> Result<MarketplaceListOutcome, MarketplaceError> {
    super::list_marketplaces_with_home(
        additional_roots,
        home_dir,
        MarketplaceLoadContext::unfiltered(),
    )
}

fn write_alternate_marketplace(repo_root: &Path, contents: &str) -> AbsolutePathBuf {
    let marketplace_path = repo_root.join(ALTERNATE_MARKETPLACE_RELATIVE_PATH);
    fs::create_dir_all(marketplace_path.parent().unwrap()).unwrap();
    fs::write(&marketplace_path, contents).unwrap();
    AbsolutePathBuf::try_from(marketplace_path).unwrap()
}

fn write_alternate_plugin_manifest(plugin_root: &Path, contents: &str) {
    let manifest_path = plugin_root.join(ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH);
    fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
    fs::write(manifest_path, contents).unwrap();
}

fn write_default_marketplace(repo_root: &Path, contents: &str) -> AbsolutePathBuf {
    let marketplace_path = repo_root.join(".agents/plugins/marketplace.json");
    fs::create_dir_all(marketplace_path.parent().unwrap()).unwrap();
    fs::write(&marketplace_path, contents).unwrap();
    AbsolutePathBuf::try_from(marketplace_path).unwrap()
}

fn write_default_plugin_manifest(plugin_root: &Path, contents: &str) {
    let manifest_path = plugin_root.join(".codex-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
    fs::write(manifest_path, contents).unwrap();
}

#[test]
fn find_marketplace_plugin_finds_repo_marketplace_plugin() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(repo_root.join("nested")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./plugin-1"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let resolved = find_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "local-plugin",
    )
    .unwrap();

    assert_eq!(
        resolved,
        ResolvedMarketplacePlugin {
            plugin_id: PluginId::new("local-plugin".to_string(), "codex-curated".to_string())
                .unwrap(),
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(repo_root.join("plugin-1")).unwrap(),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
            manifest: None,
        }
    );
}

#[test]
fn find_marketplace_plugin_supports_alternate_layout_and_string_local_source() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    let marketplace_path = write_alternate_marketplace(
        &repo_root,
        r#"{
  "name": "alternate-marketplace",
  "plugins": [
    {
      "name": "string-source-plugin",
      "source": "./plugins/string-source-plugin"
    }
  ]
}"#,
    );

    let resolved = find_marketplace_plugin(&marketplace_path, "string-source-plugin").unwrap();

    assert_eq!(
        resolved,
        ResolvedMarketplacePlugin {
            plugin_id: PluginId::new(
                "string-source-plugin".to_string(),
                "alternate-marketplace".to_string()
            )
            .unwrap(),
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(repo_root.join("plugins/string-source-plugin"))
                    .unwrap(),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
            manifest: None,
        }
    );
}

#[test]
fn find_marketplace_plugin_supports_git_subdir_sources() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "remote-plugin",
      "source": {
        "source": "git-subdir",
        "url": "openai/joey_marketplace3",
        "path": "plugins/toolkit",
        "ref": "main",
        "sha": "abc123"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let resolved = find_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "remote-plugin",
    )
    .unwrap();

    assert_eq!(
        resolved,
        ResolvedMarketplacePlugin {
            plugin_id: PluginId::new("remote-plugin".to_string(), "codex-curated".to_string())
                .unwrap(),
            source: MarketplacePluginSource::Git {
                url: "https://github.com/openai/joey_marketplace3.git".to_string(),
                path: Some("plugins/toolkit".to_string()),
                ref_name: Some("main".to_string()),
                sha: Some("abc123".to_string()),
            },
            policy: MarketplacePluginPolicy {
                installation: MarketplacePluginInstallPolicy::Available,
                authentication: MarketplacePluginAuthPolicy::OnInstall,
                products: None,
            },
            interface: None,
            manifest: None,
        }
    );
}

#[test]
fn find_marketplace_plugin_normalizes_github_shorthand_with_dot_git_suffix() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "remote-plugin",
      "source": {
        "source": "git-subdir",
        "url": "openai/toolkit.git",
        "path": "plugins/toolkit"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let resolved = find_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "remote-plugin",
    )
    .unwrap();

    assert_eq!(
        resolved.source,
        MarketplacePluginSource::Git {
            url: "https://github.com/openai/toolkit.git".to_string(),
            path: Some("plugins/toolkit".to_string()),
            ref_name: None,
            sha: None,
        }
    );
}

#[test]
fn find_marketplace_plugin_normalizes_relative_git_source_urls_to_marketplace_root() {
    for source_url in ["./remotes/toolkit.git", ".\\remotes\\toolkit.git"] {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        let remote_repo = repo_root.join("remotes").join("toolkit.git");
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::create_dir_all(&remote_repo).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            format!(
                r#"{{
  "name": "codex-curated",
  "plugins": [
    {{
      "name": "remote-plugin",
      "source": {{
        "source": "git-subdir",
        "url": "{}",
        "path": "plugins/toolkit"
      }}
    }}
  ]
}}"#,
                source_url.replace('\\', "\\\\")
            ),
        )
        .unwrap();

        let resolved = find_marketplace_plugin(
            &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
            "remote-plugin",
        )
        .unwrap();

        assert_eq!(
            resolved.source,
            MarketplacePluginSource::Git {
                url: remote_repo.display().to_string(),
                path: Some("plugins/toolkit".to_string()),
                ref_name: None,
                sha: None,
            }
        );
    }
}

#[test]
fn normalize_relative_git_plugin_source_url_rejects_parent_traversal() {
    for source_url in [
        "../toolkit.git",
        "./../toolkit.git",
        "..\\toolkit.git",
        ".\\..\\toolkit.git",
    ] {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        let marketplace_path = repo_root.join(".agents/plugins/marketplace.json");
        let marketplace_path = AbsolutePathBuf::try_from(marketplace_path).unwrap();
        let err =
            normalize_relative_git_plugin_source_url(&marketplace_path, source_url).unwrap_err();

        assert_eq!(
            err.to_string(),
            format!(
                "invalid marketplace file `{}`: relative git plugin source url must stay within the marketplace root",
                marketplace_path.display()
            )
        );
    }
}

#[test]
fn find_marketplace_plugin_skips_root_equivalent_git_subdir_paths() {
    for path in [".", "./", "plugins/.."] {
        let tmp = tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            format!(
                r#"{{
  "name": "codex-curated",
  "plugins": [
    {{
      "name": "remote-plugin",
      "source": {{
        "source": "git-subdir",
        "url": "openai/toolkit",
        "path": "{path}"
      }}
    }}
  ]
}}"#
            ),
        )
        .unwrap();

        let err = find_marketplace_plugin(
            &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
            "remote-plugin",
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "plugin `remote-plugin` was not found in marketplace `codex-curated`"
        );
    }
}

#[test]
fn find_marketplace_plugin_reports_missing_plugin() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{"name":"codex-curated","plugins":[]}"#,
    )
    .unwrap();

    let err = find_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "missing",
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "plugin `missing` was not found in marketplace `codex-curated`"
    );
}

#[test]
fn list_marketplaces_supports_alternate_manifest_layout() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let plugin_root = repo_root.join("plugins/string-source-plugin");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    write_alternate_plugin_manifest(
        &plugin_root,
        r#"{
  "name":"string-source-plugin",
  "interface": {
    "displayName": "String Source Plugin"
  }
}"#,
    );
    let marketplace_path = write_alternate_marketplace(
        &repo_root,
        r#"{
  "name": "alternate-marketplace",
  "plugins": [
    {
      "name": "string-source-plugin",
      "source": "./plugins/string-source-plugin"
    }
  ]
}"#,
    );

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces,
        vec![Marketplace {
            name: "alternate-marketplace".to_string(),
            path: marketplace_path,
            interface: None,
            plugins: vec![MarketplacePlugin {
                name: "string-source-plugin".to_string(),
                local_version: None,
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(repo_root.join("plugins/string-source-plugin"))
                        .unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: Some(PluginManifestInterface {
                    display_name: Some("String Source Plugin".to_string()),
                    short_description: None,
                    long_description: None,
                    developer_name: None,
                    category: None,
                    capabilities: Vec::new(),
                    website_url: None,
                    privacy_policy_url: None,
                    terms_of_service_url: None,
                    default_prompt: None,
                    brand_color: None,
                    composer_icon: None,
                    logo: None,
                    screenshots: Vec::new(),
                }),
                keywords: Vec::new(),
            }],
        }]
    );
}

#[test]
fn list_marketplaces_includes_plugins_without_discoverable_manifest() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    let marketplace_path = write_alternate_marketplace(
        &repo_root,
        r#"{
  "name": "alternate-marketplace",
  "plugins": [
    {
      "name": "missing-plugin",
      "source": "./plugins/missing-plugin"
    }
  ]
}"#,
    );

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces,
        vec![Marketplace {
            name: "alternate-marketplace".to_string(),
            path: marketplace_path,
            interface: None,
            plugins: vec![MarketplacePlugin {
                name: "missing-plugin".to_string(),
                local_version: None,
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(repo_root.join("plugins/missing-plugin"),)
                        .unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
            }],
        }]
    );
}

#[test]
fn load_marketplace_for_api_key_filters_plugins_without_surviving_capabilities() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let app_only_root = repo_root.join("plugins/app-only");
    let app_and_skills_root = repo_root.join("plugins/app-and-skills");
    let app_and_mcp_root = repo_root.join("plugins/app-and-mcp");
    let app_and_other_mcp_root = repo_root.join("plugins/app-and-other-mcp");
    let mcp_only_root = repo_root.join("plugins/mcp-only");
    let skills_only_root = repo_root.join("plugins/skills-only");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    let marketplace_path = write_default_marketplace(
        &repo_root,
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "app-only",
      "source": {
        "source": "local",
        "path": "./plugins/app-only"
      }
    },
    {
      "name": "app-and-skills",
      "source": {
        "source": "local",
        "path": "./plugins/app-and-skills"
      }
    },
    {
      "name": "app-and-mcp",
      "source": {
        "source": "local",
        "path": "./plugins/app-and-mcp"
      }
    },
    {
      "name": "app-and-other-mcp",
      "source": {
        "source": "local",
        "path": "./plugins/app-and-other-mcp"
      }
    },
    {
      "name": "mcp-only",
      "source": {
        "source": "local",
        "path": "./plugins/mcp-only"
      }
    },
    {
      "name": "skills-only",
      "source": {
        "source": "local",
        "path": "./plugins/skills-only"
      }
    },
    {
      "name": "remote-plugin",
      "source": {
        "source": "git-subdir",
        "url": "openai/toolkit",
        "path": "plugins/toolkit"
      }
    }
  ]
}"#,
    );

    write_default_plugin_manifest(&app_only_root, r#"{"name":"app-only"}"#);
    fs::write(
        app_only_root.join(".app.json"),
        r#"{"apps":{"linear":{"id":"connector_linear"}}}"#,
    )
    .unwrap();

    write_default_plugin_manifest(
        &app_and_skills_root,
        r#"{"name":"app-and-skills","skills":"./skills/"}"#,
    );
    fs::write(
        app_and_skills_root.join(".app.json"),
        r#"{"apps":{"linear":{"id":"connector_linear"}}}"#,
    )
    .unwrap();
    fs::create_dir_all(app_and_skills_root.join("skills/example")).unwrap();
    fs::write(
        app_and_skills_root.join("skills/example/SKILL.md"),
        "---\nname: example\ndescription: example skill\n---\n",
    )
    .unwrap();

    write_default_plugin_manifest(&app_and_mcp_root, r#"{"name":"app-and-mcp"}"#);
    fs::write(
        app_and_mcp_root.join(".app.json"),
        r#"{"apps":{"linear":{"id":"connector_linear"}}}"#,
    )
    .unwrap();
    fs::write(
        app_and_mcp_root.join(".mcp.json"),
        r#"{"mcpServers":{"linear":{"command":"linear-mcp"}}}"#,
    )
    .unwrap();

    write_default_plugin_manifest(&app_and_other_mcp_root, r#"{"name":"app-and-other-mcp"}"#);
    fs::write(
        app_and_other_mcp_root.join(".app.json"),
        r#"{"apps":{"linear":{"id":"connector_linear"}}}"#,
    )
    .unwrap();
    fs::write(
        app_and_other_mcp_root.join(".mcp.json"),
        r#"{"mcpServers":{"other":{"command":"other-mcp"}}}"#,
    )
    .unwrap();

    write_default_plugin_manifest(&mcp_only_root, r#"{"name":"mcp-only"}"#);
    fs::write(
        mcp_only_root.join(".mcp.json"),
        r#"{"mcpServers":{"toolkit":{"command":"toolkit-mcp"}}}"#,
    )
    .unwrap();

    write_default_plugin_manifest(&skills_only_root, r#"{"name":"skills-only"}"#);
    fs::create_dir_all(skills_only_root.join("skills/example")).unwrap();
    fs::write(
        skills_only_root.join("skills/example/SKILL.md"),
        "---\nname: example\ndescription: example skill\n---\n",
    )
    .unwrap();

    let marketplace = super::load_marketplace(
        &marketplace_path,
        MarketplaceLoadContext::for_auth(Some(codex_app_server_protocol::AuthMode::ApiKey)),
    )
    .unwrap();

    let plugin_names = marketplace
        .plugins
        .iter()
        .map(|plugin| plugin.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        plugin_names,
        vec!["app-and-mcp", "mcp-only", "skills-only", "remote-plugin"]
    );
}

#[test]
fn find_marketplace_plugin_for_api_key_treats_app_only_local_plugins_as_not_found() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let plugin_root = repo_root.join("plugins/app-only");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    let marketplace_path = write_default_marketplace(
        &repo_root,
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "app-only",
      "source": {
        "source": "local",
        "path": "./plugins/app-only"
      }
    }
  ]
}"#,
    );
    write_default_plugin_manifest(&plugin_root, r#"{"name":"app-only"}"#);
    fs::write(
        plugin_root.join(".app.json"),
        r#"{"apps":{"linear":{"id":"connector_linear"}}}"#,
    )
    .unwrap();

    assert!(find_marketplace_plugin(&marketplace_path, "app-only").is_ok());

    let err = super::find_marketplace_plugin(
        &marketplace_path,
        "app-only",
        MarketplaceLoadContext::for_auth(Some(codex_app_server_protocol::AuthMode::ApiKey)),
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "plugin `app-only` was not found in marketplace `codex-curated`"
    );
}

#[test]
fn list_marketplaces_prefers_first_supported_manifest_layout() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "agents-marketplace",
  "plugins": [
    {
      "name": "agents-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/agents-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    write_alternate_marketplace(
        &repo_root,
        r#"{
  "name": "alternate-marketplace",
  "plugins": [
    {
      "name": "string-source-plugin",
      "source": "./plugins/string-source-plugin"
    }
  ]
}"#,
    );

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(marketplaces.len(), 1);
    assert_eq!(marketplaces[0].name, "agents-marketplace");
    assert_eq!(
        marketplaces[0].path,
        AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap()
    );
}

#[test]
fn list_marketplaces_returns_home_and_repo_marketplaces() {
    let tmp = tempdir().unwrap();
    let home_root = tmp.path().join("home");
    let repo_root = tmp.path().join("repo");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(home_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        home_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "shared-plugin",
      "source": {
        "source": "local",
        "path": "./home-shared"
      }
    },
    {
      "name": "home-only",
      "source": {
        "source": "local",
        "path": "./home-only"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "shared-plugin",
      "source": {
        "source": "local",
        "path": "./repo-shared"
      }
    },
    {
      "name": "repo-only",
      "source": {
        "source": "local",
        "path": "./repo-only"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
        Some(&home_root),
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces,
        vec![
            Marketplace {
                name: "codex-curated".to_string(),
                path:
                    AbsolutePathBuf::try_from(home_root.join(".agents/plugins/marketplace.json"),)
                        .unwrap(),
                interface: None,
                plugins: vec![
                    MarketplacePlugin {
                        name: "shared-plugin".to_string(),
                        local_version: None,
                        source: MarketplacePluginSource::Local {
                            path: AbsolutePathBuf::try_from(home_root.join("home-shared")).unwrap(),
                        },
                        policy: MarketplacePluginPolicy {
                            installation: MarketplacePluginInstallPolicy::Available,
                            authentication: MarketplacePluginAuthPolicy::OnInstall,
                            products: None,
                        },
                        interface: None,
                        keywords: Vec::new(),
                    },
                    MarketplacePlugin {
                        name: "home-only".to_string(),
                        local_version: None,
                        source: MarketplacePluginSource::Local {
                            path: AbsolutePathBuf::try_from(home_root.join("home-only")).unwrap(),
                        },
                        policy: MarketplacePluginPolicy {
                            installation: MarketplacePluginInstallPolicy::Available,
                            authentication: MarketplacePluginAuthPolicy::OnInstall,
                            products: None,
                        },
                        interface: None,
                        keywords: Vec::new(),
                    },
                ],
            },
            Marketplace {
                name: "codex-curated".to_string(),
                path:
                    AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json"),)
                        .unwrap(),
                interface: None,
                plugins: vec![
                    MarketplacePlugin {
                        name: "shared-plugin".to_string(),
                        local_version: None,
                        source: MarketplacePluginSource::Local {
                            path: AbsolutePathBuf::try_from(repo_root.join("repo-shared")).unwrap(),
                        },
                        policy: MarketplacePluginPolicy {
                            installation: MarketplacePluginInstallPolicy::Available,
                            authentication: MarketplacePluginAuthPolicy::OnInstall,
                            products: None,
                        },
                        interface: None,
                        keywords: Vec::new(),
                    },
                    MarketplacePlugin {
                        name: "repo-only".to_string(),
                        local_version: None,
                        source: MarketplacePluginSource::Local {
                            path: AbsolutePathBuf::try_from(repo_root.join("repo-only")).unwrap(),
                        },
                        policy: MarketplacePluginPolicy {
                            installation: MarketplacePluginInstallPolicy::Available,
                            authentication: MarketplacePluginAuthPolicy::OnInstall,
                            products: None,
                        },
                        interface: None,
                        keywords: Vec::new(),
                    },
                ],
            },
        ]
    );
}

#[test]
fn list_marketplaces_keeps_distinct_entries_for_same_name() {
    let tmp = tempdir().unwrap();
    let home_root = tmp.path().join("home");
    let repo_root = tmp.path().join("repo");
    let home_marketplace = home_root.join(".agents/plugins/marketplace.json");
    let repo_marketplace = repo_root.join(".agents/plugins/marketplace.json");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(home_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();

    fs::write(
        home_marketplace.clone(),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./home-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        repo_marketplace.clone(),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./repo-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
        Some(&home_root),
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces,
        vec![
            Marketplace {
                name: "codex-curated".to_string(),
                path: AbsolutePathBuf::try_from(home_marketplace).unwrap(),
                interface: None,
                plugins: vec![MarketplacePlugin {
                    name: "local-plugin".to_string(),
                    local_version: None,
                    source: MarketplacePluginSource::Local {
                        path: AbsolutePathBuf::try_from(home_root.join("home-plugin")).unwrap(),
                    },
                    policy: MarketplacePluginPolicy {
                        installation: MarketplacePluginInstallPolicy::Available,
                        authentication: MarketplacePluginAuthPolicy::OnInstall,
                        products: None,
                    },
                    interface: None,
                    keywords: Vec::new(),
                }],
            },
            Marketplace {
                name: "codex-curated".to_string(),
                path: AbsolutePathBuf::try_from(repo_marketplace.clone()).unwrap(),
                interface: None,
                plugins: vec![MarketplacePlugin {
                    name: "local-plugin".to_string(),
                    local_version: None,
                    source: MarketplacePluginSource::Local {
                        path: AbsolutePathBuf::try_from(repo_root.join("repo-plugin")).unwrap(),
                    },
                    policy: MarketplacePluginPolicy {
                        installation: MarketplacePluginInstallPolicy::Available,
                        authentication: MarketplacePluginAuthPolicy::OnInstall,
                        products: None,
                    },
                    interface: None,
                    keywords: Vec::new(),
                }],
            },
        ]
    );

    let resolved = find_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_marketplace).unwrap(),
        "local-plugin",
    )
    .unwrap();

    assert_eq!(
        resolved.source,
        MarketplacePluginSource::Local {
            path: AbsolutePathBuf::try_from(repo_root.join("repo-plugin")).unwrap(),
        }
    );
}

#[test]
fn list_marketplaces_dedupes_multiple_roots_in_same_repo() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let nested_root = repo_root.join("nested/project");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(&nested_root).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[
            AbsolutePathBuf::try_from(repo_root.clone()).unwrap(),
            AbsolutePathBuf::try_from(nested_root).unwrap(),
        ],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces,
        vec![Marketplace {
            name: "codex-curated".to_string(),
            path: AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json"))
                .unwrap(),
            interface: None,
            plugins: vec![MarketplacePlugin {
                name: "local-plugin".to_string(),
                local_version: None,
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(repo_root.join("plugin")).unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
            }],
        }]
    );
}

#[test]
fn list_marketplaces_reads_marketplace_display_name() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-curated",
  "interface": {
    "displayName": "ChatGPT Official"
  },
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces[0].interface,
        Some(MarketplaceInterface {
            display_name: Some("ChatGPT Official".to_string()),
        })
    );
}

#[test]
fn list_marketplaces_skips_invalid_plugins_but_keeps_marketplace() {
    let tmp = tempdir().unwrap();
    let valid_repo_root = tmp.path().join("valid-repo");
    let invalid_repo_root = tmp.path().join("invalid-repo");

    fs::create_dir_all(valid_repo_root.join(".git")).unwrap();
    fs::create_dir_all(valid_repo_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(invalid_repo_root.join(".git")).unwrap();
    fs::create_dir_all(invalid_repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        valid_repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "valid-marketplace",
  "plugins": [
    {
      "name": "valid-plugin",
      "source": {
        "source": "local",
        "path": "./plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        invalid_repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "invalid-marketplace",
  "plugins": [
    {
      "name": "broken-plugin",
      "source": {
        "source": "local",
        "path": "plugin-without-dot-slash"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[
            AbsolutePathBuf::try_from(valid_repo_root).unwrap(),
            AbsolutePathBuf::try_from(invalid_repo_root).unwrap(),
        ],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(marketplaces.len(), 2);
    assert_eq!(marketplaces[0].name, "valid-marketplace");
    assert_eq!(marketplaces[1].name, "invalid-marketplace");
    assert!(marketplaces[1].plugins.is_empty());
}

#[test]
fn list_marketplaces_skips_plugins_with_invalid_names_but_keeps_marketplace() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "invalid-name-marketplace",
  "plugins": [
    {
      "name": "valid-plugin",
      "source": {
        "source": "local",
        "path": "./valid-plugin"
      }
    },
    {
      "name": "invalid.plugin",
      "source": {
        "source": "local",
        "path": "./invalid-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces,
        vec![Marketplace {
            name: "invalid-name-marketplace".to_string(),
            path: AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json"))
                .unwrap(),
            interface: None,
            plugins: vec![MarketplacePlugin {
                name: "valid-plugin".to_string(),
                local_version: None,
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(repo_root.join("valid-plugin")).unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
            }],
        }]
    );
}

#[test]
fn list_marketplaces_reports_marketplace_load_errors() {
    let tmp = tempdir().unwrap();
    let valid_repo_root = tmp.path().join("valid-repo");
    let invalid_repo_root = tmp.path().join("invalid-repo");

    fs::create_dir_all(valid_repo_root.join(".git")).unwrap();
    fs::create_dir_all(valid_repo_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(invalid_repo_root.join(".git")).unwrap();
    fs::create_dir_all(invalid_repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        valid_repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "valid-marketplace",
  "plugins": [
    {
      "name": "valid-plugin",
      "source": {
        "source": "local",
        "path": "./plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    let invalid_marketplace_path =
        AbsolutePathBuf::try_from(invalid_repo_root.join(".agents/plugins/marketplace.json"))
            .unwrap();
    fs::write(invalid_marketplace_path.as_path(), "{not json").unwrap();

    let outcome = list_marketplaces_with_home(
        &[
            AbsolutePathBuf::try_from(valid_repo_root).unwrap(),
            AbsolutePathBuf::try_from(invalid_repo_root).unwrap(),
        ],
        /*home_dir*/ None,
    )
    .unwrap();

    assert_eq!(outcome.marketplaces.len(), 1);
    assert_eq!(outcome.marketplaces[0].name, "valid-marketplace");
    assert_eq!(outcome.errors.len(), 1);
    assert_eq!(outcome.errors[0].path, invalid_marketplace_path);
    assert!(
        outcome.errors[0]
            .message
            .contains("invalid marketplace file"),
        "unexpected errors: {:?}",
        outcome.errors
    );
}

#[test]
fn list_marketplaces_keeps_remote_and_local_plugin_sources() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    write_alternate_marketplace(
        &repo_root,
        r#"{
  "name": "mixed-source-marketplace",
  "plugins": [
    {
      "name": "local-plugin",
      "source": "./plugins/local-plugin"
    },
    {
      "name": "url-plugin",
      "source": {
        "source": "url",
        "url": "https://github.com/example/plugin"
      }
    },
    {
      "name": "git-subdir-plugin",
      "source": {
        "source": "git-subdir",
        "url": "owner/repo",
        "path": "plugins/example",
        "ref": "main",
        "sha": "abc123"
      }
    }
  ]
}"#,
    );

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root.clone()).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(marketplaces.len(), 1);
    assert_eq!(
        marketplaces[0].plugins,
        vec![
            MarketplacePlugin {
                name: "local-plugin".to_string(),
                local_version: None,
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(repo_root.join("plugins/local-plugin"))
                        .unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
            },
            MarketplacePlugin {
                name: "url-plugin".to_string(),
                local_version: None,
                source: MarketplacePluginSource::Git {
                    url: "https://github.com/example/plugin.git".to_string(),
                    path: None,
                    ref_name: None,
                    sha: None,
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
            },
            MarketplacePlugin {
                name: "git-subdir-plugin".to_string(),
                local_version: None,
                source: MarketplacePluginSource::Git {
                    url: "https://github.com/owner/repo.git".to_string(),
                    path: Some("plugins/example".to_string()),
                    ref_name: Some("main".to_string()),
                    sha: Some("abc123".to_string()),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
                keywords: Vec::new(),
            },
        ]
    );
}

#[test]
fn list_marketplaces_resolves_plugin_interface_paths_to_absolute() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let plugin_root = repo_root.join("plugins/demo-plugin");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "demo-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/demo-plugin"
      },
      "policy": {
        "installation": "AVAILABLE",
        "authentication": "ON_INSTALL",
        "products": ["CODEX", "CHATGPT", "ATLAS"]
      },
      "category": "Design"
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "demo-plugin",
  "interface": {
    "displayName": "Demo",
    "category": "Productivity",
    "capabilities": ["Interactive", "Write"],
    "composerIcon": "./assets/icon.png",
    "logo": "./assets/logo.png",
    "screenshots": ["./assets/shot1.png"]
  }
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces[0].plugins[0].policy.installation,
        MarketplacePluginInstallPolicy::Available
    );
    assert_eq!(
        marketplaces[0].plugins[0].policy.authentication,
        MarketplacePluginAuthPolicy::OnInstall
    );
    assert_eq!(
        marketplaces[0].plugins[0].policy.products,
        Some(vec![Product::Codex, Product::Chatgpt, Product::Atlas])
    );
    assert_eq!(
        marketplaces[0].plugins[0].interface,
        Some(PluginManifestInterface {
            display_name: Some("Demo".to_string()),
            short_description: None,
            long_description: None,
            developer_name: None,
            category: Some("Design".to_string()),
            capabilities: vec!["Interactive".to_string(), "Write".to_string()],
            website_url: None,
            privacy_policy_url: None,
            terms_of_service_url: None,
            default_prompt: None,
            brand_color: None,
            composer_icon: Some(
                AbsolutePathBuf::try_from(plugin_root.join("assets/icon.png")).unwrap(),
            ),
            logo: Some(AbsolutePathBuf::try_from(plugin_root.join("assets/logo.png")).unwrap()),
            screenshots: vec![
                AbsolutePathBuf::try_from(plugin_root.join("assets/shot1.png")).unwrap(),
            ],
        })
    );
}

#[test]
fn list_marketplaces_ignores_legacy_top_level_policy_fields() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "demo-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/demo-plugin"
      },
      "installPolicy": "NOT_AVAILABLE",
      "authPolicy": "ON_USE"
    }
  ]
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces[0].plugins[0].policy.installation,
        MarketplacePluginInstallPolicy::Available
    );
    assert_eq!(
        marketplaces[0].plugins[0].policy.authentication,
        MarketplacePluginAuthPolicy::OnInstall
    );
    assert_eq!(marketplaces[0].plugins[0].policy.products, None);
}

#[test]
fn list_marketplaces_ignores_plugin_interface_assets_without_dot_slash() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let plugin_root = repo_root.join("plugins/demo-plugin");

    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "demo-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/demo-plugin"
      }
    }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{
  "name": "demo-plugin",
  "interface": {
    "displayName": "Demo",
    "capabilities": ["Interactive"],
    "composerIcon": "assets/icon.png",
    "logo": "/tmp/logo.png",
    "screenshots": ["assets/shot1.png"]
  }
}"#,
    )
    .unwrap();

    let marketplaces = list_marketplaces_with_home(
        &[AbsolutePathBuf::try_from(repo_root).unwrap()],
        /*home_dir*/ None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(
        marketplaces[0].plugins[0].interface,
        Some(PluginManifestInterface {
            display_name: Some("Demo".to_string()),
            short_description: None,
            long_description: None,
            developer_name: None,
            category: None,
            capabilities: vec!["Interactive".to_string()],
            website_url: None,
            privacy_policy_url: None,
            terms_of_service_url: None,
            default_prompt: None,
            brand_color: None,
            composer_icon: None,
            logo: None,
            screenshots: Vec::new(),
        })
    );
    assert_eq!(
        marketplaces[0].plugins[0].policy.installation,
        MarketplacePluginInstallPolicy::Available
    );
    assert_eq!(
        marketplaces[0].plugins[0].policy.authentication,
        MarketplacePluginAuthPolicy::OnInstall
    );
    assert_eq!(marketplaces[0].plugins[0].policy.products, None);
}

#[test]
fn find_marketplace_plugin_skips_invalid_local_paths() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "../plugin-1"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let err = find_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "local-plugin",
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "plugin `local-plugin` was not found in marketplace `codex-curated`"
    );
}

#[test]
fn find_marketplace_plugin_uses_first_duplicate_entry() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./first"
      }
    },
    {
      "name": "local-plugin",
      "source": {
        "source": "local",
        "path": "./second"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let resolved = find_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "local-plugin",
    )
    .unwrap();

    assert_eq!(
        resolved.source,
        MarketplacePluginSource::Local {
            path: AbsolutePathBuf::try_from(repo_root.join("first")).unwrap(),
        }
    );
}

#[test]
fn find_installable_marketplace_plugin_rejects_disallowed_product() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "chatgpt-plugin",
      "source": {
        "source": "local",
        "path": "./plugin"
      },
      "policy": {
        "products": ["CHATGPT"]
      }
    }
  ]
}"#,
    )
    .unwrap();

    let err = find_installable_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "chatgpt-plugin",
        Some(Product::Atlas),
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "plugin `chatgpt-plugin` is not available for install in marketplace `codex-curated`"
    );
}

#[test]
fn find_marketplace_plugin_allows_missing_products_field() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "default-plugin",
      "source": {
        "source": "local",
        "path": "./plugin"
      },
      "policy": {}
    }
  ]
}"#,
    )
    .unwrap();

    let resolved = find_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "default-plugin",
    )
    .unwrap();

    assert_eq!(resolved.plugin_id.as_key(), "default-plugin@codex-curated");
}

#[test]
fn find_installable_marketplace_plugin_rejects_explicit_empty_products() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "disabled-plugin",
      "source": {
        "source": "local",
        "path": "./plugin"
      },
      "policy": {
        "products": []
      }
    }
  ]
}"#,
    )
    .unwrap();

    let err = find_installable_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "disabled-plugin",
        Some(Product::Codex),
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "plugin `disabled-plugin` is not available for install in marketplace `codex-curated`"
    );
}
