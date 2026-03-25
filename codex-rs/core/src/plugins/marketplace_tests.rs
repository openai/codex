use super::*;
use codex_protocol::protocol::Product;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn resolve_marketplace_plugin_finds_repo_marketplace_plugin() {
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

    let resolved = resolve_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "local-plugin",
        Some(Product::Codex),
    )
    .unwrap();

    assert_eq!(
        resolved,
        ResolvedMarketplacePlugin {
            plugin_id: PluginId::new("local-plugin".to_string(), "codex-curated".to_string())
                .unwrap(),
            source_path: AbsolutePathBuf::try_from(repo_root.join("plugin-1")).unwrap(),
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
}

#[test]
fn resolve_marketplace_plugin_reports_missing_plugin() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{"name":"codex-curated","plugins":[]}"#,
    )
    .unwrap();

    let err = resolve_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "missing",
        Some(Product::Codex),
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "plugin `missing` was not found in marketplace `codex-curated`"
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
                        source: MarketplacePluginSource::Local {
                            path: AbsolutePathBuf::try_from(home_root.join("home-shared")).unwrap(),
                        },
                        policy: MarketplacePluginPolicy {
                            installation: MarketplacePluginInstallPolicy::Available,
                            authentication: MarketplacePluginAuthPolicy::OnInstall,
                            products: None,
                        },
                        interface: None,
                    },
                    MarketplacePlugin {
                        name: "home-only".to_string(),
                        source: MarketplacePluginSource::Local {
                            path: AbsolutePathBuf::try_from(home_root.join("home-only")).unwrap(),
                        },
                        policy: MarketplacePluginPolicy {
                            installation: MarketplacePluginInstallPolicy::Available,
                            authentication: MarketplacePluginAuthPolicy::OnInstall,
                            products: None,
                        },
                        interface: None,
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
                        source: MarketplacePluginSource::Local {
                            path: AbsolutePathBuf::try_from(repo_root.join("repo-shared")).unwrap(),
                        },
                        policy: MarketplacePluginPolicy {
                            installation: MarketplacePluginInstallPolicy::Available,
                            authentication: MarketplacePluginAuthPolicy::OnInstall,
                            products: None,
                        },
                        interface: None,
                    },
                    MarketplacePlugin {
                        name: "repo-only".to_string(),
                        source: MarketplacePluginSource::Local {
                            path: AbsolutePathBuf::try_from(repo_root.join("repo-only")).unwrap(),
                        },
                        policy: MarketplacePluginPolicy {
                            installation: MarketplacePluginInstallPolicy::Available,
                            authentication: MarketplacePluginAuthPolicy::OnInstall,
                            products: None,
                        },
                        interface: None,
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
                    source: MarketplacePluginSource::Local {
                        path: AbsolutePathBuf::try_from(home_root.join("home-plugin")).unwrap(),
                    },
                    policy: MarketplacePluginPolicy {
                        installation: MarketplacePluginInstallPolicy::Available,
                        authentication: MarketplacePluginAuthPolicy::OnInstall,
                        products: None,
                    },
                    interface: None,
                }],
            },
            Marketplace {
                name: "codex-curated".to_string(),
                path: AbsolutePathBuf::try_from(repo_marketplace.clone()).unwrap(),
                interface: None,
                plugins: vec![MarketplacePlugin {
                    name: "local-plugin".to_string(),
                    source: MarketplacePluginSource::Local {
                        path: AbsolutePathBuf::try_from(repo_root.join("repo-plugin")).unwrap(),
                    },
                    policy: MarketplacePluginPolicy {
                        installation: MarketplacePluginInstallPolicy::Available,
                        authentication: MarketplacePluginAuthPolicy::OnInstall,
                        products: None,
                    },
                    interface: None,
                }],
            },
        ]
    );

    let resolved = resolve_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_marketplace).unwrap(),
        "local-plugin",
        Some(Product::Codex),
    )
    .unwrap();

    assert_eq!(
        resolved.source_path,
        AbsolutePathBuf::try_from(repo_root.join("repo-plugin")).unwrap()
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
        None,
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
                source: MarketplacePluginSource::Local {
                    path: AbsolutePathBuf::try_from(repo_root.join("plugin")).unwrap(),
                },
                policy: MarketplacePluginPolicy {
                    installation: MarketplacePluginInstallPolicy::Available,
                    authentication: MarketplacePluginAuthPolicy::OnInstall,
                    products: None,
                },
                interface: None,
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

    let marketplaces =
        list_marketplaces_with_home(&[AbsolutePathBuf::try_from(repo_root).unwrap()], None)
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
fn list_marketplaces_skips_marketplaces_that_fail_to_load() {
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
        None,
    )
    .unwrap()
    .marketplaces;

    assert_eq!(marketplaces.len(), 1);
    assert_eq!(marketplaces[0].name, "valid-marketplace");
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
        None,
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

    let marketplaces =
        list_marketplaces_with_home(&[AbsolutePathBuf::try_from(repo_root).unwrap()], None)
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

    let marketplaces =
        list_marketplaces_with_home(&[AbsolutePathBuf::try_from(repo_root).unwrap()], None)
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

    let marketplaces =
        list_marketplaces_with_home(&[AbsolutePathBuf::try_from(repo_root).unwrap()], None)
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
fn resolve_marketplace_plugin_rejects_non_relative_local_paths() {
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

    let err = resolve_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "local-plugin",
        Some(Product::Codex),
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        format!(
            "invalid marketplace file `{}`: local plugin source path must start with `./`",
            repo_root.join(".agents/plugins/marketplace.json").display()
        )
    );
}

#[test]
fn resolve_marketplace_plugin_uses_first_duplicate_entry() {
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

    let resolved = resolve_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "local-plugin",
        Some(Product::Codex),
    )
    .unwrap();

    assert_eq!(
        resolved.source_path,
        AbsolutePathBuf::try_from(repo_root.join("first")).unwrap()
    );
}

#[test]
fn resolve_marketplace_plugin_rejects_disallowed_product() {
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

    let err = resolve_marketplace_plugin(
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
fn resolve_marketplace_plugin_allows_missing_products_field() {
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

    let resolved = resolve_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "default-plugin",
        Some(Product::Codex),
    )
    .unwrap();

    assert_eq!(resolved.plugin_id.as_key(), "default-plugin@codex-curated");
}

#[test]
fn resolve_marketplace_plugin_rejects_explicit_empty_products() {
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

    let err = resolve_marketplace_plugin(
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
