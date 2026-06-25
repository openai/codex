use codex_connectors::ConnectorSnapshot;
use codex_connectors::ConnectorSnapshotState;
use codex_connectors::PluginConnectorSource;
use codex_core_plugins::SelectedCapabilityBindings;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_plugin::AppConnectorId;
use codex_plugin::AppDeclaration;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn bindings_only_initialization_preserves_legacy_snapshot_output() -> TestResult {
    let plugin_root = tempfile::tempdir()?;
    std::fs::create_dir_all(plugin_root.path().join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"calendar","apps":"./.app.json","interface":{"displayName":"Calendar"}}"#,
    )?;
    std::fs::write(
        plugin_root.path().join(".app.json"),
        r#"{"apps":{"calendar":{"id":"calendar"}}}"#,
    )?;
    let bindings = SelectedCapabilityBindings::new(
        vec![SelectedCapabilityRoot {
            id: "calendar@1".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                path: PathUri::from_host_native_path(plugin_root.path())?,
            },
        }],
        Arc::new(EnvironmentManager::default_for_tests()),
    );
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    codex_connectors_extension::install_selected_executor_connectors(&mut builder);
    let registry = builder.build();
    let mut thread_init = ExtensionDataInit::new();
    thread_init.insert(bindings);

    registry.initialize_thread_data(&mut thread_init).await;

    let expected = ConnectorSnapshot::from_plugin_sources([PluginConnectorSource::new(
        "calendar@1",
        "Calendar",
        [AppDeclaration {
            name: "calendar".to_string(),
            connector_id: AppConnectorId("calendar".to_string()),
            category: None,
        }],
    )]);
    assert_eq!(
        thread_init
            .get::<ConnectorSnapshot>()
            .expect("legacy snapshot")
            .as_ref(),
        &expected
    );
    let state = thread_init
        .get::<ConnectorSnapshotState>()
        .expect("connector state");
    assert_eq!(state.snapshot(), expected);

    let mut candidate = thread_init.clone();
    registry.prepare_runtime_snapshot(&mut candidate).await;
    registry.commit_runtime_snapshot(&candidate, &thread_init);
    assert_eq!(state.snapshot(), expected);

    Ok(())
}

#[tokio::test]
async fn explicit_snapshot_initialization_does_not_wait_for_pending_bindings() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let environment_manager = Arc::new(
        EnvironmentManager::create_for_tests(
            Some(format!("ws://{}", listener.local_addr()?)),
            /*local_runtime_paths*/ None,
        )
        .await,
    );
    let bindings = SelectedCapabilityBindings::new(
        vec![SelectedCapabilityRoot {
            id: "pending@1".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                path: PathUri::parse("file:///plugins/pending")?,
            },
        }],
        environment_manager,
    );
    assert!(!bindings.snapshot().is_terminal());
    let expected =
        ConnectorSnapshot::from_plugin_sources([PluginConnectorSource::from_connector_ids(
            "explicit@1",
            "Explicit",
            [AppConnectorId("explicit".to_string())],
        )]);
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    codex_connectors_extension::install_selected_executor_connectors(&mut builder);
    let registry = builder.build();
    let mut thread_init = ExtensionDataInit::new();
    thread_init.insert(bindings);
    thread_init.insert(expected.clone());

    timeout(
        Duration::from_millis(100),
        registry.initialize_thread_data(&mut thread_init),
    )
    .await
    .expect("explicit snapshot should bypass pending bindings");

    assert_eq!(
        thread_init
            .get::<ConnectorSnapshot>()
            .expect("explicit snapshot")
            .as_ref(),
        &expected
    );
    assert_eq!(
        thread_init
            .get::<ConnectorSnapshotState>()
            .expect("connector state")
            .snapshot(),
        expected
    );

    Ok(())
}
