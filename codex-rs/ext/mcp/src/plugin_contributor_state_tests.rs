//! Tests thread-owned discovery caching, installation ordering, and contribution projection.

use super::*;
use crate::PluginProviders;
use crate::plugin_contributor::PluginContributor;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::ExecutorPluginProvider;
use codex_core_plugins::PluginCatalog;
use codex_core_plugins::PluginCatalogEntry;
use codex_core_plugins::PluginIdentity;
use codex_core_plugins::PluginProvider;
use codex_core_plugins::PluginSourceLocation;
use codex_exec_server::CapabilityTextFile;
use codex_exec_server::DiscoverV2CapabilitiesResponse;
use codex_exec_server::DiscoveredPluginFiles;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorCapabilityDiscoveryCache;
use codex_exec_server::ExecutorPlugin;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_features::Feature;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;

async fn contribution_labels(
    contributor: &PluginContributor,
    context: McpServerContributionContext<'_, codex_core::config::Config>,
) -> Vec<String> {
    let mut labels = Vec::new();
    let policy = codex_config::types::PluginsConfigToml::default();
    for plugin in contributor.selected_plugins(context, &policy).await {
        if !plugin.enabled {
            labels.push(format!("disabled:{}", plugin.plugin_id));
            continue;
        }
        let contribution = plugin.mcp.await;
        labels.extend(
            contribution
                .servers
                .into_iter()
                .map(|(name, _)| format!("mcp:{name}")),
        );
        labels.push(plugin.selected_root_id);
    }
    for contribution in contributor.contribute(context).await {
        if let McpServerContribution::HostedPluginConnectors { plugin_id, .. } = contribution {
            labels.push(format!("cloud:{plugin_id}"));
        } else {
            panic!("unexpected contribution: {contribution:?}");
        }
    }
    labels
}

fn cloud_plugin(remote_id: &str) -> PluginCatalogEntry {
    PluginCatalogEntry {
        enabled: true,
        id: PluginIdentity::Remote {
            remote_plugin_id: remote_id.to_string(),
        },
        display_name: "Example".to_string(),
        version: "1.0.0".to_string(),
        mcp_servers: BTreeMap::from([(
            "example".to_string(),
            serde_json::json!({"command":"node","args":["server.js"]}),
        )]),
        connector_ids: vec!["connector_example".to_string()],
        locations: vec![PluginSourceLocation::Cloud {
            resource_uri: format!("plugin://{remote_id}"),
            bundle_uri: None,
        }],
    }
}

fn executor_snapshot(
    environment_id: &str,
) -> anyhow::Result<ExecutorCapabilityDiscoveryV2Snapshot> {
    let root = PathUri::parse("file:///plugins/example/1.0.0")?;
    Ok(ExecutorCapabilityDiscoveryV2Snapshot {
        environment_id: environment_id.to_string(),
        result: Ok(Arc::new(DiscoverV2CapabilitiesResponse {
            plugins: vec![ExecutorPlugin {
                id: "example@marketplace".to_string(),
                display_name: "Example".to_string(),
                remote_plugin_id: Some("plugin_a".to_string()),
                version: "1.0.0".to_string(),
                files: DiscoveredPluginFiles {
                    manifest: CapabilityTextFile {
                        path: root.join(".codex-plugin/plugin.json")?,
                        contents: r#"{"name":"example"}"#.to_string(),
                    },
                    mcp_config: None,
                    apps_config: None,
                },
                root,
                skills: Vec::new(),
            }],
            ..Default::default()
        })),
    })
}

#[test]
fn executor_refresh_preserves_order_and_removes_stale_entries() -> anyhow::Result<()> {
    let mut state = PluginContributorState::default();
    // Reuse equal snapshots, including equal responses allocated by a later discovery.
    let b = executor_snapshot("executor-b")?;
    let c = executor_snapshot("executor-c")?;
    state.update_executor_catalog(&[b.clone(), c.clone()]);
    state.update_executor_catalog(&[executor_snapshot("executor-b")?, c.clone()]);
    assert!(Arc::ptr_eq(
        state.executor_snapshots[0].result.as_ref().unwrap(),
        b.result.as_ref().unwrap(),
    ));

    // Keep B ahead of C even when discovery reverses them and adds an earlier-named A.
    let a = executor_snapshot("executor-a")?;
    let mut updated_b = b;
    Arc::make_mut(updated_b.result.as_mut().unwrap()).plugins[0].display_name =
        "Updated".to_string();
    state.update_executor_catalog(&[a.clone(), c.clone(), updated_b.clone()]);
    assert_eq!(
        state.executor_catalog.as_ref().unwrap(),
        &catalog_from_snapshots(&[updated_b.clone(), c.clone(), a.clone()]),
    );
    // Removing B promotes C; B returning later appends it rather than displacing C.
    state.update_executor_catalog(&[a.clone(), c.clone()]);
    state.update_executor_catalog(&[updated_b.clone(), a.clone(), c.clone()]);
    assert_eq!(
        state.executor_catalog.as_ref().unwrap(),
        &catalog_from_snapshots(&[c.clone(), a.clone(), updated_b]),
    );
    // A failed scan removes that environment's entries, and recovery rebuilds them.
    let failed = ExecutorCapabilityDiscoveryV2Snapshot {
        environment_id: c.environment_id.clone(),
        result: Err("discovery failed".to_string()),
    };
    state.update_executor_catalog(&[failed.clone(), a.clone()]);
    assert_eq!(
        state.executor_catalog.as_ref().unwrap(),
        &catalog_from_snapshots(&[failed, a.clone()]),
    );
    state.update_executor_catalog(&[c.clone(), a.clone()]);
    assert_eq!(
        state.executor_catalog.as_ref().unwrap(),
        &catalog_from_snapshots(&[a, c]),
    );
    Ok(())
}

#[tokio::test]
async fn projection_respects_discovery_mode_and_preserves_enabled_plugins() -> anyhow::Result<()> {
    struct CloudProvider;
    impl PluginProvider for CloudProvider {}

    let home = tempfile::tempdir()?;
    let root = home.path().join("legacy");
    std::fs::create_dir_all(root.join(".codex-plugin"))?;
    std::fs::write(
        root.join(".codex-plugin/plugin.json"),
        r#"{"name":"legacy","mcpServers":{"legacy":{"command":"legacy-command"}}}"#,
    )?;
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await?;
    config.features.enable(Feature::Plugins)?;
    let environments = Arc::new(EnvironmentManager::default_for_tests());
    let roots = vec![SelectedCapabilityRoot {
        id: "legacy-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            path: PathUri::from_host_native_path(&root)?,
        },
    }];
    let v1 = ExecutorCapabilityDiscoveryCache::new(Arc::clone(&environments))
        .snapshot(&roots, &Default::default())
        .await;
    let mut contributor = PluginContributor {
        providers: PluginProviders::new(Arc::new(ExecutorPluginProvider::new(environments)))
            .with_cloud_provider(Arc::new(CloudProvider)),
    };
    let thread = ExtensionData::new("projection-test");
    let init = ExtensionDataInit::default();
    let state = thread.get_or_init(PluginsThreadState::default);
    {
        let mut state = state.contributor_state();
        let key = state.prepare_cloud_generation(/*mcp_resources*/ None);
        assert!(state.publish_cloud_catalog(
            key.as_ref(),
            PluginCatalog {
                entries: vec![cloud_plugin("plugin_a")],
                ..Default::default()
            }
        ));
    }
    // A different environment ensures per-root filtering cannot hide a legacy fallback.
    let mut valid = [executor_snapshot("executor-a")?];
    Arc::make_mut(valid[0].result.as_mut().unwrap()).plugins[0]
        .files
        .mcp_config = Some(CapabilityTextFile {
        path: PathUri::parse("file:///plugins/example/1.0.0/.mcp.json")?,
        contents: r#"{"executor-only":{"command":"node"}}"#.to_string(),
    });
    let mut local = [
        executor_snapshot("executor-a")?,
        executor_snapshot("executor-b")?,
    ];
    for snapshot in &mut local {
        Arc::make_mut(snapshot.result.as_mut().unwrap()).plugins[0].remote_plugin_id = None;
    }
    let empty = [ExecutorCapabilityDiscoveryV2Snapshot {
        environment_id: "executor-a".to_string(),
        result: Ok(Arc::new(DiscoverV2CapabilitiesResponse::default())),
    }];
    let failed = [ExecutorCapabilityDiscoveryV2Snapshot {
        environment_id: "executor-a".to_string(),
        result: Err("discovery failed".to_string()),
    }];
    for (case, v2_enabled, snapshots, expected) in [
        (
            "snapshot selects V2 independently of the acquisition flag",
            false,
            Some(valid.as_slice()),
            vec![
                "mcp:example",
                "executor-a:example@marketplace",
                "cloud:plugin_a",
            ],
        ),
        (
            "legacy",
            false,
            None,
            vec!["mcp:legacy", "legacy-root", "cloud:plugin_a"],
        ),
        (
            "no V2 snapshot",
            true,
            None,
            vec!["mcp:legacy", "legacy-root", "cloud:plugin_a"],
        ),
        (
            "empty snapshots",
            true,
            Some([].as_slice()),
            vec!["cloud:plugin_a"],
        ),
        (
            "empty inventory",
            true,
            Some(empty.as_slice()),
            vec!["cloud:plugin_a"],
        ),
        (
            "failed",
            true,
            Some(failed.as_slice()),
            vec!["cloud:plugin_a"],
        ),
        (
            "valid",
            true,
            Some(valid.as_slice()),
            vec![
                "mcp:example",
                "executor-a:example@marketplace",
                "cloud:plugin_a",
            ],
        ),
        (
            "unconfigured local",
            true,
            Some(local.as_slice()),
            vec![
                "disabled:example@marketplace",
                "disabled:example@marketplace",
                "cloud:plugin_a",
            ],
        ),
    ] {
        config
            .features
            .set_enabled(Feature::ExecutorCapabilityDiscoveryV2, v2_enabled)?;
        for legacy_snapshot in [None, Some(&v1)] {
            state.contributor_state().update_executor_catalog(&valid);
            let contributions = contribution_labels(
                &contributor,
                McpServerContributionContext::for_step(
                    &config,
                    &init,
                    &thread,
                    "test",
                    &roots,
                    legacy_snapshot,
                    snapshots,
                ),
            )
            .await;
            assert_eq!(
                contributions,
                expected,
                "{case}, V2={v2_enabled}, V1={}",
                legacy_snapshot.is_some()
            );
            let expected_catalog = catalog_from_snapshots(snapshots.unwrap_or_default());
            assert_eq!(
                state.contributor_state().executor_catalog.as_ref(),
                Some(&expected_catalog),
                "{case}"
            );
        }
    }
    std::fs::write(
        home.path().join("config.toml"),
        "[plugins.\"example@marketplace\"]\nenabled = true\n",
    )?;
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await?;
    config.features.enable(Feature::Plugins)?;
    config
        .features
        .enable(Feature::ExecutorCapabilityDiscoveryV2)?;

    // Cloud counterparts control enablement without deleting the executor installation.
    let mut disabled_cloud = cloud_plugin("plugin_a");
    disabled_cloud.enabled = false;
    for (case, cloud_enabled, catalog, enabled) in [
        ("no cloud provider", false, None, true),
        ("no cloud catalog", true, None, true),
        ("absent", true, Some(PluginCatalog::default()), false),
        (
            "partial absent",
            true,
            Some(PluginCatalog {
                is_partial: true,
                warnings: vec!["partial cloud listing".to_string()],
                ..Default::default()
            }),
            false,
        ),
        (
            "disabled",
            true,
            Some(PluginCatalog {
                entries: vec![disabled_cloud],
                ..Default::default()
            }),
            false,
        ),
        (
            "re-enabled",
            true,
            Some(PluginCatalog {
                entries: vec![cloud_plugin("plugin_a")],
                ..Default::default()
            }),
            true,
        ),
        (
            "partial counterpart",
            true,
            Some(PluginCatalog {
                entries: vec![cloud_plugin("plugin_a")],
                is_partial: true,
                warnings: vec!["partial cloud listing".to_string()],
            }),
            true,
        ),
    ] {
        let cloud_available = catalog.is_some();
        contributor.providers.cloud = if cloud_enabled {
            Some(Arc::new(CloudProvider))
        } else {
            None
        };
        {
            let mut cache = state.contributor_state();
            cache.cloud_generation = None;
            let key = cache.prepare_cloud_generation(/*mcp_resources*/ None);
            if let Some(catalog) = catalog {
                assert!(cache.publish_cloud_catalog(key.as_ref(), catalog));
            }
        }
        let contributions = contribution_labels(
            &contributor,
            McpServerContributionContext::for_step(
                &config,
                &init,
                &thread,
                case,
                &roots,
                /*executor_capability_discovery*/ None,
                Some(&valid),
            ),
        )
        .await;
        let expected = if !cloud_enabled || !cloud_available {
            vec!["mcp:executor-only", "executor-a:example@marketplace"]
        } else if enabled {
            vec![
                "mcp:example",
                "executor-a:example@marketplace",
                "cloud:plugin_a",
            ]
        } else {
            vec!["disabled:example@marketplace"]
        };
        assert_eq!(contributions, expected, "{case}");
        assert_eq!(
            state.contributor_state().executor_catalog,
            Some(catalog_from_snapshots(&valid)),
            "{case}"
        );
    }

    // A full Cloud catalog must not crowd out enabled executor-local plugins.
    let cloud_plugin_count = 100;
    {
        let mut state = state.contributor_state();
        let key = state.prepare_cloud_generation(/*mcp_resources*/ None);
        let entries = (0..cloud_plugin_count)
            .map(|index| cloud_plugin(&format!("plugin_{index}")))
            .collect::<Vec<_>>();
        assert!(state.publish_cloud_catalog(
            key.as_ref(),
            PluginCatalog {
                entries,
                ..Default::default()
            }
        ));
    }
    let contributions = contribution_labels(
        &contributor,
        McpServerContributionContext::for_step(
            &config,
            &init,
            &thread,
            "merged",
            &roots,
            /*executor_capability_discovery*/ None,
            Some(&local[..1]),
        ),
    )
    .await;
    assert_eq!(contributions.len(), cloud_plugin_count + 1);
    assert_eq!(contributions[0], "executor-a:example@marketplace");
    for (index, contribution) in contributions[1..].iter().enumerate() {
        assert_eq!(contribution, &format!("cloud:plugin_{index}"));
    }
    Ok(())
}
