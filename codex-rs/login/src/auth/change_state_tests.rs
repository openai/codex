use super::*;
use crate::auth::storage::AuthStorageBackend;
use crate::auth::storage::FileAuthStorage;
use base64::Engine;
use codex_http_client::DestinationPolicy;
use codex_http_client::NetworkPolicyController;
use pretty_assertions::assert_eq;
use serde_json::json;

fn chatgpt_auth(user: &str, workspace: &str, token: &str) -> CodexAuth {
    let claims = json!({
        "jti": token,
        "https://api.openai.com/auth": {"chatgpt_user_id": user},
    });
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    CodexAuth::from_external_chatgpt_tokens(
        &format!("header.{payload}.signature"),
        workspace,
        /*chatgpt_plan_type*/ None,
    )
    .unwrap()
}

#[test]
fn auth_owner_generation_distinguishes_refreshes_from_identity_changes() {
    let mut manager = AuthManager::from_optional_auth_for_testing(/*auth*/ None);
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    Arc::get_mut(&mut manager).unwrap().auth_route_config =
        AuthRouteConfig::from_http_client_factory(
            manager
                .http_client_factory()
                .with_network_policy(policy.clone()),
        );
    let endpoint = "https://example.com/".parse().unwrap();
    let mut changes = manager.auth_change_state_receiver();
    let mut legacy_changes = manager.auth_change_receiver();
    let initial = chatgpt_auth("user-a", "workspace-a", "token-1");
    for (auth, generation, owner_generation) in [
        (Some(initial.clone()), 1, 1),
        (Some(initial), 1, 1),
        (Some(chatgpt_auth("user-a", "workspace-a", "token-2")), 2, 1),
        (Some(chatgpt_auth("user-b", "workspace-a", "token-3")), 3, 2),
        (Some(chatgpt_auth("user-b", "workspace-b", "token-4")), 4, 3),
        (Some(chatgpt_auth("", "workspace-b", "token-5")), 5, 4),
        (Some(chatgpt_auth("", "workspace-b", "token-6")), 6, 5),
        (Some(CodexAuth::from_api_key("key-1")), 7, 6),
        (Some(CodexAuth::from_api_key("key-2")), 8, 7),
    ] {
        let previous_owner = changes.borrow().owner_generation;
        let factory = manager.http_client_factory();
        manager.set_cached_auth(auth);
        assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
        assert_eq!(
            factory.network_policy().acquire(&endpoint).is_ok(),
            owner_generation == previous_owner,
        );
        assert_eq!(
            *changes.borrow_and_update(),
            AuthChangeState {
                generation,
                owner_generation,
                source_generation: generation,
                login_generation: owner_generation,
            },
        );
        assert_eq!(*legacy_changes.borrow_and_update(), generation);
    }
}

#[tokio::test]
#[serial_test::serial(codex_auth_env)]
async fn auth_refresh_read_failure_is_transient_and_preserves_network_permit() {
    let home = tempfile::tempdir().unwrap();
    let storage = FileAuthStorage::new(home.path().to_path_buf());
    let initial = chatgpt_auth("user-a", "workspace-a", "token-1");
    let stored = crate::AuthDotJson {
        auth_mode: Some(codex_protocol::auth::AuthMode::Chatgpt),
        openai_api_key: None,
        tokens: Some(initial.get_token_data().unwrap()),
        last_refresh: Some(chrono::Utc::now()),
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: None,
        bedrock_access_keys: None,
    };
    storage.save(&stored).unwrap();
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    let manager = AuthManager::new(
        home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        crate::AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ Some(vec!["workspace-a".to_string()]),
        /*chatgpt_base_url*/ None,
        crate::AuthKeyringBackendKind::default(),
        AuthRouteConfig::from_http_client_factory(
            crate::test_support::transport_default_auth_route_config()
                .http_client_factory()
                .clone()
                .with_network_policy(policy.clone()),
        ),
    )
    .await;
    let initial_tokens = manager.auth_cached().unwrap().get_token_data().unwrap();
    let changes = manager.auth_change_state_receiver();
    let initial_changes = *changes.borrow();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let permit = policy
        .acquire(&"https://example.com/".parse().unwrap())
        .unwrap();

    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(json!({"iat": "private-credential-value"}).to_string());
    let jwt_auth = json!({"agent_identity": format!("header.{payload}.signature")}).to_string();
    // Cover interrupted writes and credential values rejected by outer JSON or nested JWT parsing.
    for contents in [
        "",
        r#"{"auth_mode":"private-credential-value"}"#,
        jwt_auth.as_str(),
    ] {
        std::fs::write(home.path().join("auth.json"), contents).unwrap();
        let error = manager.refresh_token().await.unwrap_err();
        assert!(matches!(error, RefreshTokenError::Transient(_)));
        for formatted in [error.to_string(), format!("{error:?}")] {
            assert!(
                !formatted.contains("private-credential-value"),
                "{formatted}"
            );
        }
        assert_eq!(
            manager.auth_cached().unwrap().get_token_data().unwrap(),
            initial_tokens
        );
        assert_eq!(*changes.borrow(), initial_changes);
        assert_eq!(permit.check(), Ok(()));
    }
}

#[tokio::test]
async fn auth_owner_generation_preserves_logout_and_switches_when_coalesced() {
    let home = tempfile::tempdir().unwrap();
    let initial = chatgpt_auth("user-a", "workspace-a", "token-1");
    let manager =
        AuthManager::from_auth_for_testing_with_home(initial.clone(), home.path().to_path_buf());
    let mut changes = manager.auth_change_state_receiver();

    let generation = changes.borrow().generation;
    manager.logout().await.unwrap();
    assert!(
        manager
            .publish_reloaded_auth(generation, Some(initial.clone()))
            .is_none()
    );
    assert!(manager.auth_cached().is_none());
    // Deletion also fences a read when the cache was already empty.
    let generation = changes.borrow().generation;
    manager.logout().await.unwrap();
    assert!(
        manager
            .publish_reloaded_auth(generation, Some(initial.clone()))
            .is_none()
    );
    manager.set_cached_auth(Some(initial.clone()));
    assert_eq!(
        *changes.borrow_and_update(),
        AuthChangeState {
            generation: 4,
            owner_generation: 2,
            source_generation: 2,
            login_generation: 2,
        },
    );

    manager.set_cached_auth(Some(chatgpt_auth("user-b", "workspace-a", "token-2")));
    manager.set_cached_auth(Some(initial));
    assert_eq!(
        *changes.borrow_and_update(),
        AuthChangeState {
            generation: 6,
            owner_generation: 4,
            source_generation: 4,
            login_generation: 4,
        },
    );
}

#[tokio::test]
async fn captured_credentials_are_revoked_if_account_changes_before_client_construction() {
    let mut manager = AuthManager::from_optional_auth_for_testing(/*auth*/ None);
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    Arc::get_mut(&mut manager).unwrap().auth_route_config =
        AuthRouteConfig::from_http_client_factory(
            manager
                .http_client_factory()
                .with_network_policy(policy.clone()),
        );
    let initial = chatgpt_auth("user-a", "workspace-a", "token-a");
    manager.set_cached_auth(Some(initial.clone()));
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let (auth, factory) = manager.auth_with_http_client_factory().await.unwrap();

    manager.set_cached_auth(Some(chatgpt_auth("user-b", "workspace-b", "token-b")));
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let client = crate::default_client::create_client_for_route_async(
        factory,
        "https://example.com/".to_string(),
        codex_http_client::ClientRouteClass::Api,
        crate::default_client::ClientRedirectPolicy::Default,
    )
    .await
    .unwrap();
    assert_eq!(auth.get_account_id(), initial.get_account_id());
    assert!(matches!(
        client
            .get("https://example.com/")
            .bearer_auth(auth.get_token().unwrap())
            .send()
            .await,
        Err(codex_http_client::RouteAwareRequestError::Policy(
            codex_http_client::NetworkPolicyDenied::Revoked
        ))
    ));
}
