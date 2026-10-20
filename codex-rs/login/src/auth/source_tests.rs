//! Runtime reuse follows the credential source, not just its owner or token bytes.

use super::*;
use base64::Engine;
use pretty_assertions::assert_eq;
use serde_json::json;

fn oauth(session: &str, token: &str) -> CodexAuth {
    let claims = json!({
        "iss": "https://auth.openai.com/",
        "session_id": session,
        "sub": "user-a",
        "jti": token,
        "https://api.openai.com/auth": {
            "user_id": "user-a", "chatgpt_account_id": "workspace-a"
        },
    });
    from_claims(claims, "workspace-a")
}

fn from_claims(claims: serde_json::Value, workspace: &str) -> CodexAuth {
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    CodexAuth::from_external_chatgpt_tokens(
        &format!("header.{payload}.signature"),
        workspace,
        /*chatgpt_plan_type*/ None,
    )
    .unwrap()
}

#[test]
fn refreshed_credentials_reuse_the_source_but_a_new_login_does_not() {
    let original = oauth("login-a", "token-1");
    let manager = AuthManager::from_auth_for_testing(original.clone());
    let initial = manager.auth_change_state_for(&original).unwrap();
    manager.set_cached_auth(Some(oauth("login-a", "token-2")));
    let refreshed = manager.auth_change_state_for(&original).unwrap();
    assert_ne!(initial.generation, refreshed.generation);
    assert_eq!(initial.source_generation, refreshed.source_generation);

    manager.set_cached_auth(Some(oauth("login-b", "token-3")));
    assert!(manager.auth_change_state_for(&original).is_none());
    // Returning to an earlier credential does not revive its retired runtime.
    manager.set_cached_auth(Some(original.clone()));
    assert_ne!(
        initial.source_generation,
        manager
            .auth_change_state_for(&original)
            .unwrap()
            .source_generation
    );
}

#[test]
fn token_rotation_does_not_hide_a_resource_or_workload_change() {
    let original = oauth("login-a", "token-1");
    let token = original.get_token().unwrap();
    let payload = token.split('.').nth(1).unwrap();
    let claims: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .unwrap(),
    )
    .unwrap();
    for path in [
        vec!["iss"],
        vec!["sub"],
        vec!["client_id"],
        vec!["https://api.openai.com/auth", "user_id"],
        vec![
            "https://api.openai.com/auth",
            "chatgpt_backing_organization_id",
        ],
        vec!["https://api.openai.com/auth", "wif_federation_rule_id"],
    ] {
        let manager = AuthManager::from_auth_for_testing(original.clone());
        let mut changed = claims.clone();
        let mut value = &mut changed;
        for component in path {
            value = &mut value[component];
        }
        *value = json!("changed");
        manager.set_cached_auth(Some(from_claims(changed, "workspace-a")));
        assert!(manager.auth_change_state_for(&original).is_none());
    }
    let manager = AuthManager::from_auth_for_testing(original.clone());
    manager.set_cached_auth(Some(from_claims(claims, "workspace-b")));
    assert!(manager.auth_change_state_for(&original).is_none());
}

#[test]
fn a_sessionless_token_cannot_claim_refresh_continuity() {
    let claims = json!({"https://api.openai.com/auth": {"user_id": "user-a"}, "jti": "one"});
    let original = from_claims(claims.clone(), "workspace-a");
    let manager = AuthManager::from_auth_for_testing(original.clone());
    assert!(manager.auth_change_state_for(&original).is_some());
    let mut changed = claims;
    changed["jti"] = json!("two");
    manager.set_cached_auth(Some(from_claims(changed, "workspace-a")));
    assert!(manager.auth_change_state_for(&original).is_none());
}

#[test]
fn api_key_replacement_and_logout_retire_the_previous_source() {
    let key_a = CodexAuth::from_api_key("key-a");
    let key_b = CodexAuth::from_api_key("key-b");
    let manager = AuthManager::from_auth_for_testing(key_a.clone());
    let first = manager.auth_change_state_for(&key_a).unwrap();
    manager.set_cached_auth(Some(key_a.clone()));
    assert_eq!(first, manager.auth_change_state_for(&key_a).unwrap());
    manager.set_cached_auth(Some(key_b.clone()));
    assert!(manager.auth_change_state_for(&key_a).is_none());
    let second = manager.auth_change_state_for(&key_b).unwrap();
    manager.set_cached_auth(/*new_auth*/ None);
    assert!(manager.auth_change_state_for(&key_b).is_none());
    manager.set_cached_auth(Some(key_b.clone()));
    assert_ne!(
        second.source_generation,
        manager
            .auth_change_state_for(&key_b)
            .unwrap()
            .source_generation
    );
}

#[tokio::test]
#[serial_test::serial(codex_auth_env)]
async fn legacy_identity_persistence_does_not_reload_credentials() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let storage = create_auth_storage(
        home.path().to_path_buf(),
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    );
    let original = oauth("login-a", "token-1");
    let replacement = oauth("login-b", "token-2");
    let mut stored = original.get_current_auth_json().unwrap();
    stored.auth_mode = Some(AuthMode::Chatgpt);
    stored.last_refresh = Some(chrono::Utc::now());
    storage.save(&stored)?;
    let manager = AuthManager::shared(
        home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        AuthKeyringBackendKind::Direct,
        crate::test_support::transport_default_auth_route_config(),
    )
    .await;
    let captured = manager.auth_cached().unwrap();
    let initial = manager.auth_change_state_for(&captured).unwrap();
    stored.tokens = replacement.get_current_auth_json().unwrap().tokens;
    storage.save(&stored)?;
    let stored = storage.load()?.unwrap();
    let identity = serde_json::from_value(json!({
        "agent_runtime_id": "legacy-runtime",
        "agent_private_key": codex_agent_identity::generate_agent_key_material()?.private_key_pkcs8_base64,
        "account_id": "workspace-a", "chatgpt_user_id": "user-a", "plan_type": "pro",
        "chatgpt_account_is_fedramp": false, "task_id": "legacy-task",
    }))?;
    captured.persist_managed_chatgpt_agent_identity_record(identity)?;
    assert_eq!(captured.get_token()?, original.get_token()?);
    assert_eq!(manager.auth_change_state_for(&captured), Some(initial));
    assert_eq!(storage.load()?.unwrap().tokens, stored.tokens);
    manager.reload().await;
    assert_eq!(
        manager.auth_cached().unwrap().get_token()?,
        replacement.get_token()?
    );
    assert_ne!(
        initial.source_generation,
        manager
            .auth_change_state_receiver()
            .borrow()
            .source_generation
    );
    assert!(manager.auth_change_state_for(&captured).is_none());
    assert_eq!(captured.get_token()?, original.get_token()?);
    Ok(())
}
