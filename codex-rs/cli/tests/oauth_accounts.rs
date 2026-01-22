use std::path::Path;

use anyhow::Result;
use assert_cmd::Command;
use base64::Engine;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::add_oauth_account;
use codex_core::auth::list_oauth_accounts;
use codex_core::auth::load_auth_dot_json;
use codex_core::auth::set_openai_api_key;
use codex_core::token_data::TokenData;
use codex_core::token_data::parse_id_token;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn codex_command(codex_home: &Path) -> Result<Command> {
    let mut cmd = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    cmd.env("CODEX_HOME", codex_home);
    Ok(cmd)
}

fn minimal_jwt_with_email(email: &str) -> String {
    let header = serde_json::json!({"alg": "none", "typ": "JWT"});
    let payload = serde_json::json!({"email": email});
    let encode = |value: &serde_json::Value| {
        let bytes =
            serde_json::to_vec(value).unwrap_or_else(|err| panic!("serialize jwt segment: {err}"));
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    };
    let header_b64 = encode(&header);
    let payload_b64 = encode(&payload);
    let signature_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"sig");
    format!("{header_b64}.{payload_b64}.{signature_b64}")
}

fn token_data_with_email(access: &str, refresh: &str, email: &str) -> TokenData {
    let jwt = minimal_jwt_with_email(email);
    let id_token = parse_id_token(&jwt).unwrap_or_else(|err| panic!("parse jwt: {err}"));
    TokenData {
        id_token,
        access_token: access.to_string(),
        refresh_token: refresh.to_string(),
        account_id: None,
    }
}

#[test]
fn login_accounts_lists_accounts() -> Result<()> {
    let codex_home = TempDir::new()?;

    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data_with_email("access-1", "refresh-1", "user1@example.com"),
        None,
        Some("Work".to_string()),
    )?;
    let record2 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data_with_email("access-2", "refresh-2", "user2@example.com"),
        None,
        None,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd.args(["login", "accounts"]).output()?;
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("ChatGPT accounts:"));
    assert!(stderr.contains(&record1));
    assert!(stderr.contains(&record2));
    assert!(stderr.contains("Work"));
    assert!(stderr.contains("user2@example.com"));
    assert!(stderr.contains(&format!("* user2@example.com  id={record2}")));

    Ok(())
}

#[test]
fn logout_account_removes_single_account_and_keeps_api_key() -> Result<()> {
    let codex_home = TempDir::new()?;
    set_openai_api_key(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        Some("sk-test".to_string()),
    )?;

    let record1 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data_with_email("access-1", "refresh-1", "user1@example.com"),
        None,
        None,
    )?;
    let record2 = add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data_with_email("access-2", "refresh-2", "user2@example.com"),
        None,
        None,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd
        .args(["logout", "--account", "user1@example.com"])
        .output()?;
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains(&format!("Logged out ChatGPT account {record1}")));

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)?;
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].record_id, record2);

    let auth = load_auth_dot_json(codex_home.path(), AuthCredentialsStoreMode::File)?;
    assert_eq!(
        auth.and_then(|value| value.openai_api_key),
        Some("sk-test".to_string())
    );

    Ok(())
}

#[test]
fn logout_all_accounts_requires_tty() -> Result<()> {
    let codex_home = TempDir::new()?;
    add_oauth_account(
        codex_home.path(),
        AuthCredentialsStoreMode::File,
        token_data_with_email("access-1", "refresh-1", "user1@example.com"),
        None,
        None,
    )?;

    let mut cmd = codex_command(codex_home.path())?;
    let output = cmd.args(["logout", "--all-accounts"]).output()?;
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("Refusing to log out all ChatGPT accounts"));

    let accounts = list_oauth_accounts(codex_home.path(), AuthCredentialsStoreMode::File)?;
    assert_eq!(accounts.len(), 1);

    Ok(())
}
