//! Exercises background routing call sites and explicit account reads through JSON-RPC.

use super::message_processor_tracing_tests::TEST_CONNECTION_ID;
use super::message_processor_tracing_tests::read_response;
use super::*;
use codex_app_server_protocol::GetAccountResponse;
use codex_app_server_protocol::InitializeResponse;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::mpsc;

#[test_case(None; "signed_out")]
#[test_case(Some("dummy"); "api_key")]
#[tokio::test]
async fn background_routing_call_sites_skip_config(api_key: Option<&str>) -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    if let Some(api_key) = api_key {
        codex_login::login_with_api_key(
            home.path(),
            api_key,
            AuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )?;
    }
    let config = Arc::new(
        codex_core::config::ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?,
    );
    let auth_manager = Arc::new(
        AuthManager::new(
            home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*forced_chatgpt_workspace_id*/ None,
            Some(config.chatgpt_base_url.clone()),
            AuthKeyringBackendKind::default(),
            config.auth_config().auth_route_config,
        )
        .await,
    );
    let (loads_tx, mut loads_rx) = mpsc::unbounded_channel();
    let loader = CloudConfigBundleLoader::from_getter(move || {
        loads_tx.send(()).expect("config-load observer is alive");
        async { Ok(None) }
    });
    let config_manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        loader,
    );
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*buffer*/ 16);
    let analytics_events_client = AnalyticsEventsClient::disabled();
    let outgoing = Arc::new(OutgoingMessageSender::new(
        outgoing_tx,
        analytics_events_client.clone(),
    ));
    let processor = Arc::new(MessageProcessor::new(MessageProcessorArgs {
        outgoing,
        analytics_events_client,
        arg0_paths: Arg0DispatchPaths::default(),
        config,
        config_manager,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db: None,
        config_warnings: Vec::new(),
        session_source: SessionSource::VSCode,
        user_verification: Arc::new(crate::user_verification::Service::new(Arc::clone(
            &auth_manager,
        ))),
        auth_manager,
        installation_id: "11111111-1111-4111-8111-111111111111".to_string(),
        code_mode_session_provider: None,
        rpc_transport: AppServerRpcTransport::Stdio,
        remote_control_handle: None,
        // Isolate routing from unrelated plugin startup configuration loads.
        plugin_startup_tasks: None,
    }));
    // The model catalog has its own legitimate config refresh, outside routing.
    processor.models_refresh_worker.shutdown();

    // Observe the task scheduled by real processor construction, not the helper.
    assert!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), loads_rx.recv())
            .await
            .is_err(),
        "startup routing entered configuration loading"
    );
    let session = Arc::new(ConnectionSessionState::new(
        crate::transport::ConnectionOrigin::Stdio,
    ));
    processor
        .process_request(
            TEST_CONNECTION_ID,
            serde_json::from_value(json!({
                "id": 1, "method": "initialize", "params": {
                    "clientInfo": {"name": "workspace-routing-test", "version": "1"}
                }
            }))?,
            &crate::transport::AppServerTransport::Stdio,
            Arc::clone(&session),
        )
        .await;
    let _: InitializeResponse = read_response(&mut outgoing_rx, /*request_id*/ 1).await;
    // Complete the same post-initialize hook used by the real transports.
    processor
        .connection_initialized(TEST_CONNECTION_ID, /*request_attestation*/ false)
        .await;
    assert!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), loads_rx.recv())
            .await
            .is_err(),
        "connection initialization routing entered configuration loading"
    );

    processor
        .process_request(
            TEST_CONNECTION_ID,
            serde_json::from_value(json!({
                "id": 2, "method": "account/read", "params": {"refreshToken": false}
            }))?,
            &crate::transport::AppServerTransport::Stdio,
            session,
        )
        .await;
    let response: GetAccountResponse = read_response(&mut outgoing_rx, /*request_id*/ 2).await;
    assert_eq!(
        response,
        GetAccountResponse {
            account: api_key.map(|_| codex_app_server_protocol::Account::ApiKey {}),
            requires_openai_auth: true,
            workspace_routing: None,
        }
    );
    assert_eq!(
        loads_rx.try_recv(),
        Ok(()),
        "explicit account/read must reload config"
    );
    processor.shutdown_threads().await;
    Ok(())
}
