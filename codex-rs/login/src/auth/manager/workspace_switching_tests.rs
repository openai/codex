#![cfg(test)]

//! Verifies workspace exchange persistence and failed-switch isolation.
//!
//! Auth environment readers share `codex_auth_env` with tests that override tokens.

use super::*;
use base64::Engine;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn token(workspace: &str, user: &str, plan: &str) -> String {
    let claims = json!({
        "exp": (Utc::now() + chrono::Duration::hours(/*hours*/ 1)).timestamp(),
        "email": "person@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": workspace,
            "chatgpt_user_id": user,
            "chatgpt_plan_type": plan,
        },
    });
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    format!("e30.{encoded}.signature")
}

async fn session(server: &MockServer) -> anyhow::Result<(TempDir, AuthManager, AuthDotJson)> {
    let home = tempfile::tempdir()?;
    let jwt = token("personal", "user-1", "plus");
    let snapshot = AuthDotJson {
        auth_mode: Some(AuthMode::Chatgpt),
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token: parse_chatgpt_jwt_claims(&jwt)?,
            access_token: jwt,
            refresh_token: "retained-refresh-token".into(),
            account_id: Some("personal".into()),
            login_owner_id: Some("login-1".into()),
            oauth_provenance: None,
        }),
        last_refresh: Some(Utc::now()),
        agent_identity: None,
        personal_access_token: None,
        bedrock_api_key: None,
        bedrock_access_keys: None,
    };
    create_auth_storage(
        home.path().to_path_buf(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )
    .save(&snapshot)?;
    let manager = AuthManager::new(
        home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        Some(server.uri()),
        AuthKeyringBackendKind::default(),
        crate::test_support::transport_default_auth_route_config(),
    )
    .await;
    Ok((home, manager, snapshot))
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn workspace_switch_preserves_login_and_survives_reload() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let (home, manager, snapshot) = session(&server).await?;
    let access_token = token("business", "user-1", "business");
    Mock::given(method("POST"))
        .and(path("/accounts/switch-workspace-token"))
        .and(header(
            "authorization",
            format!("Bearer {}", snapshot.tokens.as_ref().unwrap().access_token),
        ))
        .and(body_json(json!({"workspace_id": "business"})))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({"access_token": access_token})),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let before = *manager.auth_change_state_receiver().borrow();
    manager.switch_workspace("login-1", "business").await?;
    enforce_login_restrictions(&AuthConfig {
        codex_home: home.path().to_path_buf(),
        auth_credentials_store_mode: AuthCredentialsStoreMode::File,
        keyring_backend_kind: AuthKeyringBackendKind::default(),
        forced_login_method: None,
        forced_chatgpt_workspace_id: Some(vec!["business".into()]),
        managed_auth_policy: ManagedAuthPolicy::default(),
        chatgpt_base_url: Some(server.uri()),
        auth_route_config: crate::test_support::transport_default_auth_route_config(),
    })
    .await?;
    manager.reload().await;
    let auth = manager.auth_cached().unwrap();
    let mut expected = snapshot.clone();
    let tokens = expected.tokens.as_mut().unwrap();
    tokens.access_token = access_token;
    tokens.account_id = Some("business".into());
    assert_eq!(auth.get_current_auth_json(), Some(expected));
    assert_eq!(
        (
            auth.get_account_id(),
            auth.get_chatgpt_user_id(),
            auth.account_plan_type()
        ),
        (
            Some("business".into()),
            Some("user-1".into()),
            Some(AccountPlanType::Business)
        ),
    );
    assert!(
        manager
            .auth_change_state_receiver()
            .borrow()
            .owner_generation
            > before.owner_generation
    );
    manager.switch_workspace("login-1", "business").await?;
    Ok(())
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn workspace_switch_rejects_stale_agent_identity_and_rolls_back_conditionally()
-> anyhow::Result<()> {
    let server = MockServer::start().await;
    let (_home, manager, snapshot) = session(&server).await?;
    let original_auth = manager.auth_cached().unwrap();
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200)
                .set_body_json(json!({"access_token": token("business", "user-1", "business")})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_json(json!({"workspace_id": "personal"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token": token("personal", "user-1", "plus")})),
        )
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    let switched = manager.switch_workspace("login-1", "business").await?;
    let selected = manager.auth_cached().unwrap().get_current_auth_json();
    let error = original_auth
        .persist_managed_chatgpt_agent_identity_record(AgentIdentityAuthRecord {
            agent_runtime_id: "runtime-personal".into(),
            agent_private_key: "private-key-personal".into(),
            account_id: "personal".into(),
            chatgpt_user_id: "user-1".into(),
            email: None,
            plan_type: AccountPlanType::Plus,
            chatgpt_account_is_fedramp: false,
            task_id: None,
        })
        .unwrap_err();
    assert!(error.to_string().contains("Account changed"));
    manager.reload().await;
    assert_eq!(
        manager.auth_cached().unwrap().get_current_auth_json(),
        selected
    );
    manager.rollback_workspace_switch(switched).await?;
    assert_eq!(
        manager.auth_cached().unwrap().get_current_auth_json(),
        Some(snapshot)
    );
    let switched = manager.switch_workspace("login-1", "business").await?;
    manager.logout().await?;
    assert_eq!(
        manager
            .rollback_workspace_switch(switched)
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
    assert!(manager.auth_cached().is_none());
    Ok(())
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn workspace_switch_rejection_preserves_current_login() -> anyhow::Result<()> {
    for (status, response) in [
        (
            403,
            json!({"error": {"code": "invalid_workspace_selected"}}),
        ),
        (
            200,
            json!({"access_token": token("business", "another-user", "business")}),
        ),
        (
            200,
            json!({"access_token": token("wrong-workspace", "user-1", "business")}),
        ),
    ] {
        let server = MockServer::start().await;
        let (_home, manager, snapshot) = session(&server).await?;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(status).set_body_json(response))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        assert!(
            manager
                .switch_workspace("login-1", "business")
                .await
                .is_err()
        );
        manager.reload().await;
        assert_eq!(
            manager.auth_cached().unwrap().get_current_auth_json(),
            Some(snapshot)
        );
    }
    Ok(())
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn workspace_switch_obeys_workspace_policy() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let (_home, manager, snapshot) = session(&server).await?;
    manager.set_forced_chatgpt_workspace_id(Some(vec!["personal".into()]));
    assert_eq!(
        manager
            .switch_workspace("login-1", "business")
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(server.received_requests().await.unwrap().is_empty());
    assert_eq!(
        manager.auth_cached().unwrap().get_current_auth_json(),
        Some(snapshot)
    );
    Ok(())
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn workspace_switch_does_not_restore_a_logged_out_session() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let (home, manager, _) = session(&server).await?;
    let received = Arc::new(tokio::sync::Notify::new());
    let notify = received.clone();
    Mock::given(method("POST"))
        .respond_with(move |_: &wiremock::Request| {
            notify.notify_one();
            ResponseTemplate::new(/*s*/ 200)
                .set_body_json(json!({"access_token": token("business", "user-1", "business")}))
                .set_delay(Duration::from_millis(/*millis*/ 100))
        })
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let (switched, logged_out) =
        tokio::join!(manager.switch_workspace("login-1", "business"), async {
            received.notified().await;
            manager.logout().await
        },);
    logged_out?;
    assert_eq!(switched.unwrap_err().kind(), io::ErrorKind::Interrupted);
    assert!(manager.auth_cached().is_none());
    assert!(
        create_auth_storage(
            home.path().to_path_buf(),
            AuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default()
        )
        .load()?
        .is_none()
    );
    Ok(())
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn workspace_switch_rejects_a_session_from_a_previous_login() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let (home, manager, mut snapshot) = session(&server).await?;
    let session_id = manager
        .auth_cached()
        .unwrap()
        .managed_account_session_id()
        .unwrap();
    let jwt = token("personal", "user-1", "plus");
    let tokens = snapshot.tokens.as_mut().unwrap();
    tokens.login_owner_id = Some("new-login".into());
    tokens.id_token = parse_chatgpt_jwt_claims(&jwt)?;
    tokens.access_token = jwt;
    save_auth(
        home.path(),
        &snapshot,
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    manager.reload().await;
    assert_eq!(
        manager
            .switch_workspace(&session_id, "business")
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(server.received_requests().await.unwrap().is_empty());
    assert_eq!(
        manager.auth_cached().unwrap().get_current_auth_json(),
        Some(snapshot)
    );
    Ok(())
}

#[tokio::test]
#[serial(codex_auth_env)]
async fn workspace_sso_only_commits_for_the_requesting_login_and_workspace() -> anyhow::Result<()> {
    let backend = MockServer::start().await;
    let (_home, manager, original) = session(&backend).await?;
    let login = manager.auth_cached().unwrap().workspace_login(
        "login-1",
        "team".into(),
        /*connection*/ None,
    )?;
    let result = |workspace: &str, user: &str| -> anyhow::Result<AuthDotJson> {
        let mut updated = original.clone();
        let tokens = updated.tokens.as_mut().unwrap();
        tokens.access_token = token(workspace, user, "business");
        tokens.id_token = parse_chatgpt_jwt_claims(&tokens.access_token)?;
        tokens.account_id = Some(workspace.into());
        tokens.login_owner_id = Some("new-login".into());
        Ok(updated)
    };
    assert!(login.save(result("team", "different-user")?).is_err());
    assert!(login.save(result("other-workspace", "user-1")?).is_err());
    assert_eq!(login.storage.load_account_grant()?, Some(original.clone()));
    login.save(result("team", "user-1")?)?;
    let selected = login.storage.load_account_grant()?.unwrap().tokens.unwrap();
    assert_eq!(selected.account_id.as_deref(), Some("team"));
    assert_eq!(selected.login_owner_id.as_deref(), Some("login-1"));
    manager.logout().await?;
    assert!(login.save(result("team", "user-1")?).is_err());
    assert!(login.storage.load_account_grant()?.is_none());
    Ok(())
}
