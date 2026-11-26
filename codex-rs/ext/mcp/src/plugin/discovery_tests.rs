//! Verifies merged MCPs follow the enabled environment scope while preserving all package identities.

use super::*;
use codex_config::McpServerTransportConfig;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginIdentity;
use codex_extension_api::SelectedPluginContribution;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn merged_mcp_declarations_require_an_executor_location() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cli_overrides(vec![("features.plugins".to_string(), true.into())])
        .build()
        .await?;
    let mut entry = PluginCatalogEntry {
        enabled: true,
        id: PluginIdentity::Remote {
            remote_plugin_id: "plugin_example".to_string(),
        },
        display_name: "Example".to_string(),
        version: "1".to_string(),
        mcp_servers: std::collections::BTreeMap::from([(
            "example".to_string(),
            serde_json::json!({"command":"node","args":["server.js"],"cwd":"mcp"}),
        )]),
        connector_ids: vec!["connector_example".to_string()],
        locations: vec![PluginSourceLocation::Cloud {
            resource_uri: "plugin://plugin_example".to_string(),
            bundle_uri: None,
            release_id: None,
        }],
    };
    let projections = v2_metadata_from_catalog(&entry, &config.features);
    let [(root_id, metadata)] = projections.as_slice() else {
        panic!("one cloud projection expected");
    };
    assert_eq!(root_id, "cloud:plugin_example");
    assert!(metadata.servers.is_empty());
    assert_eq!(metadata.connector_ids, vec!["connector_example"]);

    // Attribution keeps the installation key; server configs distinguish the executors.
    let installations = [
        ("executor-b", "example@acme", "/other/example/1"),
        ("executor-a", "example@acme", "/plugins/example/1"),
    ];
    // Retained catalog order, not lexical environment order, chooses the executor.
    for (environment_id, plugin_id, root) in installations {
        entry.locations.push(PluginSourceLocation::Executor {
            environment_id: environment_id.to_string(),
            plugin_id: plugin_id.to_string(),
            root: PathUri::parse(&format!("file://{root}"))?,
        });
    }
    let projections = v2_metadata_from_catalog(&entry, &config.features);
    assert_eq!(
        projections
            .iter()
            .map(|(_, metadata)| metadata.servers.len())
            .collect::<Vec<_>>(),
        vec![1, 0, 0]
    );

    // Opting out the first copy must leave the next eligible copy usable.
    let mut executor = TurnEnvironmentSelection {
        environment_id: "executor-b".to_string(),
        cwd: PathUri::parse("file:///workspace")?,
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
        selected_capability_roots: Default::default(),
    };
    let mut attachment =
        core_test_support::test_codex::environment_config_for_selection(&config, &executor);
    attachment.plugins = serde_json::from_value(serde_json::json!({
        "example@acme": { "enabled": false }
    }))?;
    executor.config = EnvironmentConfigState::Ready(attachment);
    let mut selected = entry.clone();
    filter_plugin_locations_for_mcp(
        &mut selected,
        &config.plugins,
        &[executor],
        /*allowed_by_catalog*/ true,
    );
    let projections = v2_metadata_from_catalog(&selected, &config.features);
    assert_eq!(projections[0].0, "executor-a:example@acme");
    assert_eq!(projections[0].1.servers.len(), 1);

    // Qualified server names let both installations contribute their own executor paths.
    config.features.enable(Feature::MultiEnvironmentMcp)?;
    let projections = v2_metadata_from_catalog(&entry, &config.features);
    assert_eq!(projections.len(), installations.len() + 1);
    for ((root_id, metadata), (environment_id, plugin_id, root)) in
        projections.iter().zip(installations)
    {
        assert_eq!(root_id, &format!("{environment_id}:{plugin_id}"));
        let contribution = super::super::project_metadata(
            &config,
            &PluginsConfigToml::default(),
            plugin_id,
            SelectedPluginContribution {
                plugin_display_name: metadata.plugin_display_name.clone(),
                source_environment_id: environment_id.to_string(),
                servers: metadata.servers.clone(),
                connector_ids: metadata.connector_ids.clone(),
            },
        );
        assert_eq!(contribution.connector_ids, entry.connector_ids);
        let [(name, mcp_config)] = contribution.servers.as_slice() else {
            panic!("one MCP contribution expected");
        };
        assert_eq!(mcp_config.environment_id, environment_id);
        assert_eq!(name, "example");
        let McpServerTransportConfig::Stdio { command, cwd, .. } = &mcp_config.transport else {
            panic!("stdio expected");
        };
        assert_eq!(command, "node");
        assert_eq!(
            cwd.as_ref()
                .map(codex_utils_path_uri::LegacyAppPathString::as_str),
            Some(format!("{root}/mcp").as_str())
        );
    }

    let (root_id, metadata) = &projections[installations.len()];
    assert_eq!(root_id, "cloud:plugin_example");
    assert!(metadata.servers.is_empty());
    assert_eq!(metadata.connector_ids, vec!["connector_example"]);

    // Bound projected metadata without mutating the source catalog or losing installations.
    entry.display_name = "界".repeat(MAX_PLUGIN_DISPLAY_NAME_CHARS + 1);
    entry.connector_ids = (0..=MAX_CONNECTOR_IDS_PER_PLUGIN)
        .flat_map(|index| vec![format!("connector_{index}"); 2])
        .collect();
    let expected_connectors = (0..MAX_CONNECTOR_IDS_PER_PLUGIN)
        .map(|index| format!("connector_{index}"))
        .collect::<Vec<_>>();
    entry.mcp_servers.clear();
    let projections = v2_metadata_from_catalog(&entry, &config.features);
    assert_eq!(projections.len(), installations.len() + 1);
    assert!(
        projections
            .iter()
            .all(|(_, metadata)| metadata.servers.is_empty())
    );
    for (_, metadata) in projections {
        assert_eq!(
            metadata.plugin_display_name,
            "界".repeat(MAX_PLUGIN_DISPLAY_NAME_CHARS)
        );
        assert_eq!(metadata.connector_ids, expected_connectors);
    }
    assert_eq!(
        entry.display_name.chars().count(),
        MAX_PLUGIN_DISPLAY_NAME_CHARS + 1
    );
    assert_eq!(
        entry.connector_ids.len(),
        (MAX_CONNECTOR_IDS_PER_PLUGIN + 1) * 2
    );
    Ok(())
}

/// An explicit thread disable must also cover installations whose source supplies membership.
#[test]
fn explicit_thread_disable_without_default_denies_executor_plugin() -> anyhow::Result<()> {
    let thread = serde_json::from_value(serde_json::json!({
        "notes@market": { "enabled": false }
    }))?;
    for allowed_by_catalog in [true, false] {
        assert_eq!(
            plugin_enabled(&thread, &[], "local", "notes@market", allowed_by_catalog),
            PluginEnablement::DisabledForThread,
        );
    }
    Ok(())
}
