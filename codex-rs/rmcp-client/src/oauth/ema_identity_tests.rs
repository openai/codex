use codex_config::types::AuthKeyringBackendKind;
use codex_keyring_store::tests::MockKeyringStore;
use serde_json::json;

use super::*;
use crate::oauth::RefreshCredentialLock;
use crate::oauth::compute_store_key;
use crate::oauth::test_support::TempCodexHome;

#[tokio::test]
async fn keyring_failure_does_not_reuse_the_pinned_refresh_token() -> Result<()> {
    let _home = TempCodexHome::new();
    let tokens: StoredOAuthTokens = serde_json::from_value(json!({
        "server_name": "ema-idp:keyring-failure",
        "url": "https://idp.example",
        "issuer": "https://idp.example",
        "client_id": "client",
        "token_response": {
            "access_token": "unused",
            "token_type": "Bearer",
            "refresh_token": "stale-refresh",
        },
    }))?;
    let key = compute_store_key(&tokens.server_name, &tokens.url)?;
    let _lock = RefreshCredentialLock::acquire_for_server(&tokens.server_name, &tokens.url).await?;
    let snapshot = StoredOAuthCredentialSnapshot::new(
        tokens,
        ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct),
    );
    let keyring = MockKeyringStore::default();
    keyring.set_error(
        &key,
        keyring::Error::Invalid("backend".into(), "unavailable".into()),
    );
    let error = snapshot
        .load_ema_credentials(&keyring)
        .expect_err("keyring failure must be terminal");
    assert!(error.to_string().contains("refusing file fallback"));
    Ok(())
}

#[test]
fn ordinary_oauth_names_cannot_alias_enterprise_credential_keys() -> Result<()> {
    let _home = TempCodexHome::new();
    let issuer = "https://idp.example";
    let enterprise_name = "ema-idp:synthetic-identity";
    let ordinary: codex_config::McpServerConfig = serde_json::from_value(json!({
        "url": issuer,
        "oauth": {"client_id": "idp-client"},
    }))?;
    let ordinary_name = ordinary.oauth_credential_name(enterprise_name);

    pretty_assertions::assert_ne!(
        compute_store_key(&ordinary_name, issuer)?,
        compute_store_key(enterprise_name, issuer)?,
        "an ordinary server name must not select the enterprise credential namespace"
    );
    let legacy_key = compute_store_key("ordinary-server", issuer)?;
    let legacy_hash = legacy_key.split_once('|').expect("legacy key separator").1;
    pretty_assertions::assert_eq!(
        compute_store_key(&ordinary_name, issuer)?,
        format!("{enterprise_name}|{legacy_hash}"),
        "escaping the reserved prefix preserves the pre-EMA ordinary credential key"
    );

    let keyring = MockKeyringStore::default();
    let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
    let enterprise_tokens: StoredOAuthTokens = serde_json::from_value(json!({
        "server_name": enterprise_name, "url": issuer, "issuer": issuer,
        "client_id": "idp-client", "token_response": {
            "access_token": "unused", "token_type": "Bearer", "refresh_token": "enterprise-refresh"
        }
    }))?;
    store.save(&keyring, enterprise_name, &enterprise_tokens)?;
    let mut ordinary_tokens = enterprise_tokens.clone();
    ordinary_tokens.server_name = ordinary_name.to_string();
    store.save(&keyring, &ordinary_name, &ordinary_tokens)?;
    assert!(store.delete(&keyring, &ordinary_name, issuer)?);
    pretty_assertions::assert_eq!(
        store.load(&keyring, enterprise_name, issuer)?,
        Some(enterprise_tokens),
        "ordinary OAuth save/logout must not overwrite or remove the enterprise entry"
    );
    Ok(())
}
