use super::*;
use codex_protocol::protocol::Product;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::tempdir;

const ALTERNATE_MARKETPLACE_RELATIVE_PATH: &str = ".claude-plugin/marketplace.json";

fn write_alternate_marketplace(repo_root: &Path, contents: &str) -> AbsolutePathBuf {
    let marketplace_path = repo_root.join(ALTERNATE_MARKETPLACE_RELATIVE_PATH);
    fs::create_dir_all(marketplace_path.parent().unwrap()).unwrap();
    fs::write(&marketplace_path, contents).unwrap();
    AbsolutePathBuf::try_from(marketplace_path).unwrap()
}

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
            source: MarketplacePluginSource::Local {
                path: AbsolutePathBuf::try_from(repo_root.join("plugin-1")).unwrap(),
            },
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
}

#[test]
fn resolve_marketplace_plugin_supports_alternate_layout_and_string_local_source() {
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

    let resolved = resolve_marketplace_plugin(
        &marketplace_path,
        "string-source-plugin",
        Some(Product::Codex),
    )
    .unwrap();

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
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
}

#[test]
fn resolve_marketplace_plugin_supports_git_subdir_sources() {
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

    let resolved = resolve_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "remote-plugin",
        Some(Product::Codex),
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
            auth_policy: MarketplacePluginAuthPolicy::OnInstall,
        }
    );
}

#[test]
fn resolve_marketplace_plugin_normalizes_relative_git_source_urls_to_marketplace_root() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let remote_repo = repo_root.join("remotes/toolkit.git");
    fs::create_dir_all(repo_root.join(".git")).unwrap();
    fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
    fs::create_dir_all(&remote_repo).unwrap();
    fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "remote-plugin",
      "source": {
        "source": "git-subdir",
        "url": "./remotes/toolkit.git",
        "path": "plugins/toolkit"
      }
    }
  ]
}"#,
    )
    .unwrap();

    let resolved = resolve_marketplace_plugin(
        &AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap(),
        "remote-plugin",
        Some(Product::Codex),
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

#[test]
fn resolve_marketplace_plugin_supports_string_local_sources() {
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
      "source": "./plugin-2"
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
        resolved.source,
        MarketplacePluginSource::Local {
            path: AbsolutePathBuf::try_from(repo_root.join("plugin-2")).unwrap(),
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
fn list_marketplaces_supports_alternate_manifest_layout() {
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
            }],
        }]
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
            },
            MarketplacePlugin {
                name: "url-plugin".to_string(),
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
            },
            MarketplacePlugin {
                name: "git-subdir-plugin".to_string(),
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
fn resolve_marketplace_plugin_skips_non_relative_local_paths() {
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

    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.join(".agents/plugins/marketplace.json")).unwrap();
    let err = resolve_marketplace_plugin(&marketplace_path, "local-plugin", Some(Product::Codex))
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "plugin `local-plugin` was not found in marketplace `codex-curated`"
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
        resolved.source,
        MarketplacePluginSource::Local {
            path: AbsolutePathBuf::try_from(repo_root.join("first")).unwrap(),
        }
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
