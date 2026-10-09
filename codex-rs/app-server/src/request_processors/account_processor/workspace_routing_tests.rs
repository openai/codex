//! Origin resolution, strict discovery validation, and background read eligibility.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::Value;
use test_case::test_case;

#[test_case(None; "signed_out")]
#[test_case(Some("dummy"); "api_key")]
#[tokio::test]
async fn background_routing_skips_config_until_chatgpt_sign_in(
    api_key: Option<&str>,
) -> anyhow::Result<()> {
    use app_test_support::ChatGptAuthFixture;
    use app_test_support::write_chatgpt_auth;
    use codex_config::CloudConfigBundleLoader;
    use codex_config::LoaderOverrides;
    use codex_config::types::AuthCredentialsStoreMode;
    use codex_exec_server::EnvironmentManager;
    use codex_login::AuthKeyringBackendKind;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    let home = tempfile::tempdir()?;
    let backend = wiremock::MockServer::start().await;
    app_test_support::mount_workspace_routing(&backend).await;
    let loads = Arc::new(AtomicUsize::new(/*v*/ 0));
    let calls = Arc::clone(&loads);
    let loader = CloudConfigBundleLoader::from_getter(move || {
        calls.fetch_add(/*val*/ 1, Ordering::SeqCst);
        async { Ok(None) }
    });
    let manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        vec![(
            "chatgpt_base_url".into(),
            format!("{}/backend-api", backend.uri()).into(),
        )],
        LoaderOverrides::without_managed_config_for_tests(),
        loader,
    );
    let config = Arc::new(
        manager
            .load_latest_config(Some(home.path().to_path_buf()))
            .await?,
    );
    if let Some(api_key) = api_key {
        codex_login::login_with_api_key(
            home.path(),
            api_key,
            AuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )?;
    }
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
    let threads = Arc::new(
        codex_core::test_support::thread_manager_with_models_provider_and_home(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.to_path_buf(),
            Arc::new(EnvironmentManager::default_for_tests()),
        ),
    );
    let (sender, _receiver) = tokio::sync::mpsc::channel(16);
    let outgoing = Arc::new(OutgoingMessageSender::new(
        sender,
        codex_analytics::AnalyticsEventsClient::disabled(),
    ));
    let processor = AccountRequestProcessor::new(
        Arc::clone(&auth_manager),
        threads,
        outgoing,
        config,
        manager,
    );
    let initial_loads = loads.load(Ordering::SeqCst);
    for _ in 0..3 {
        assert!(
            processor
                .read_account_for_background_workspace_routing()
                .await?
                .is_none()
        );
    }
    assert_eq!(loads.load(Ordering::SeqCst), initial_loads);

    processor.read_account(/*request*/ None).await?;
    assert!(loads.load(Ordering::SeqCst) > initial_loads);

    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("access-chatgpt").account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    assert!(auth_manager.reload().await);
    let initial_loads = loads.load(Ordering::SeqCst);
    let read = processor
        .read_account_for_background_workspace_routing()
        .await?
        .unwrap();
    assert_eq!(
        read.workspace_routing,
        Some(WorkspaceRouting {
            chatgpt_account_id: "account-123".into(),
            backend_origin: "https://chatgpt.com".into(),
            account_routing_override: "NO_CONSTRAINT".into(),
        })
    );
    assert!(loads.load(Ordering::SeqCst) > initial_loads);

    std::fs::write(home.path().join("config.toml"), "invalid = [")?;
    assert!(matches!(
        processor
            .read_account_for_background_workspace_routing()
            .await,
        Err(AccountReadError::Routing(
            WorkspaceRoutingError::RequirementsLoad
        ))
    ));
    Ok(())
}

#[test_case(Some("https://gov.chatgpt.com/backend-api/"), "NO_CONSTRAINT", "https://gov.chatgpt.com"; "configured_only")]
#[test_case(None, "https://gov.chatgpt.com", "https://gov.chatgpt.com"; "discovered_only")]
#[test_case(Some("https://GOV.chatgpt.com:443/backend-api/"), "https://gov.chatgpt.com", "https://gov.chatgpt.com"; "matching_effective_port")]
#[test_case(Some("https://example.com:8443/backend-api/"), "https://example.com:8443", "https://example.com:8443"; "custom_port")]
#[test_case(None, "NO_CONSTRAINT", "https://chatgpt.com"; "existing_default")]
fn resolves_origins(required: Option<&str>, discovered: &str, expected: &str) {
    let entry = serde_json::from_value(serde_json::json!({
        "id": "workspace", "workspace_backend_origin": discovered,
        "account_routing_override": "NO_CONSTRAINT",
    }))
    .unwrap();
    assert_eq!(
        resolve_routing(entry, required, "https://chatgpt.com/backend-api/").unwrap(),
        WorkspaceRouting {
            chatgpt_account_id: "workspace".into(),
            backend_origin: expected.into(),
            account_routing_override: "NO_CONSTRAINT".into(),
        }
    );
}

#[test_case("https://other.example/backend-api/", "https://gov.chatgpt.com"; "host_conflict")]
#[test_case("http://gov.chatgpt.com/backend-api/", "https://gov.chatgpt.com"; "scheme_conflict")]
#[test_case("https://gov.chatgpt.com:444/backend-api/", "https://gov.chatgpt.com"; "port_conflict")]
fn rejects_conflicting_origins(required: &str, discovered: &str) {
    let entry = serde_json::from_value(serde_json::json!({
        "id": "workspace", "workspace_backend_origin": discovered,
        "account_routing_override": "us_cr",
    }))
    .unwrap();
    assert!(resolve_routing(entry, Some(required), "https://chatgpt.com").is_err());
}

#[test_case(serde_json::json!(null), serde_json::json!("us_cr"), WorkspaceRoutingError::MissingBackendOrigin; "null_origin")]
#[test_case(serde_json::json!("NO_CONSTRAINT"), serde_json::json!(null), WorkspaceRoutingError::InvalidRoutingOverride; "null_routing")]
#[test_case(serde_json::json!(""), serde_json::json!("us_cr"), WorkspaceRoutingError::InvalidBackendUrl; "empty_origin")]
#[test_case(serde_json::json!("https://example.com/backend-api/"), serde_json::json!("us_cr"), WorkspaceRoutingError::BackendIsNotOrigin; "origin_with_path")]
#[test_case(serde_json::json!("https://example.com?query"), serde_json::json!("us_cr"), WorkspaceRoutingError::BackendIsNotOrigin; "origin_with_query")]
#[test_case(serde_json::json!("https://example.com#fragment"), serde_json::json!("us_cr"), WorkspaceRoutingError::BackendIsNotOrigin; "origin_with_fragment")]
#[test_case(serde_json::json!("https://user:pass@example.com"), serde_json::json!("us_cr"), WorkspaceRoutingError::InvalidBackendOrigin; "credentials")]
#[test_case(serde_json::json!("http://example.com"), serde_json::json!("us_cr"), WorkspaceRoutingError::InvalidBackendOrigin; "insecure_origin")]
#[test_case(serde_json::json!("NO_CONSTRAINT"), serde_json::json!("unknown"), WorkspaceRoutingError::InvalidRoutingOverride; "unknown_routing")]
#[test_case(serde_json::json!("NO_CONSTRAINT"), serde_json::json!(""), WorkspaceRoutingError::InvalidRoutingOverride; "empty_routing")]
fn rejects_invalid_discovery(backend: Value, routing: Value, expected: WorkspaceRoutingError) {
    let entry = serde_json::from_value(serde_json::json!({
        "id": "workspace", "workspace_backend_origin": backend, "account_routing_override": routing,
    }))
    .unwrap();
    assert_eq!(
        resolve_routing(
            entry,
            /*required_chatgpt_base_url*/ None,
            "https://chatgpt.com"
        ),
        Err(AccountReadError::Routing(expected))
    );
}

#[test]
fn unrestricted_discovery_uses_effective_custom_backend() {
    let entry = serde_json::from_value(serde_json::json!({
        "id": "workspace", "workspace_backend_origin": "NO_CONSTRAINT", "account_routing_override": "us",
    })).unwrap();
    assert_eq!(
        resolve_routing(
            entry,
            /*required_chatgpt_base_url*/ None,
            "https://custom.example:8443/backend-api/"
        )
        .unwrap(),
        WorkspaceRouting {
            chatgpt_account_id: "workspace".into(),
            backend_origin: "https://custom.example:8443".into(),
            account_routing_override: "us".into(),
        }
    );
}
