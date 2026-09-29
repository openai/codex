//! Verifies storage failures remain visible in direct login diagnostics.

use super::DEFAULT_LOGIN_LOG_FILTER;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::login_with_api_key;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tracing_subscriber::EnvFilter;

#[test]
fn login_default_filter_records_sanitized_storage_fallback() -> anyhow::Result<()> {
    let home = tempdir()?;
    // Fail before accessing the real keyring, then allow the plaintext fallback to succeed.
    std::fs::write(home.path().join("secrets"), "not a directory")?;
    let log_path = home.path().join("login.log");
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_env_filter(EnvFilter::new(DEFAULT_LOGIN_LOG_FILTER))
        .with_writer(std::fs::File::create(&log_path)?)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        login_with_api_key(
            home.path(),
            "private-api-key",
            AuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Secrets,
        )
    })?;

    let logs = std::fs::read_to_string(log_path)?;
    assert_eq!(logs.lines().count(), 2);
    let failure = logs.lines().next().unwrap();
    assert!(
        failure.contains("failed to save auth to keyring, falling back to file storage"),
        "{logs}"
    );
    assert!(failure.contains("failed to create secrets dir"), "{logs}");
    assert!(
        logs.contains("credential storage file fallback completed"),
        "{logs}"
    );
    assert!(logs.contains("fallback_reason=\"secure_error\""), "{logs}");
    assert!(logs.contains("outcome=\"success\""), "{logs}");
    assert!(!logs.contains("private-api-key"));
    let completion = logs.lines().last().unwrap();
    assert!(!completion.contains(home.path().to_str().unwrap()));
    let cause = std::fs::create_dir_all(home.path().join("secrets"))
        .unwrap_err()
        .to_string();
    insta::assert_snapshot!(
        logs.replace(
            home.path().join("secrets").to_str().unwrap(),
            "[SECRETS_PATH]"
        )
        .replace(&cause, "[OS_ERROR]")
    );
    Ok(())
}

#[test]
fn login_storage_error_displays_context_and_cause() -> anyhow::Result<()> {
    let home = tempdir()?;
    let secrets_path = home.path().join("secrets");
    std::fs::write(&secrets_path, "not a directory")?;
    let cause = std::fs::create_dir_all(&secrets_path)
        .unwrap_err()
        .to_string();
    let error = login_with_api_key(
        home.path(),
        "private-api-key",
        AuthCredentialsStoreMode::Keyring,
        AuthKeyringBackendKind::Secrets,
    )
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains(&cause), "{message}");
    assert!(!message.contains("private-api-key"));
    // The temporary path and native filesystem error differ across platforms.
    insta::assert_snapshot!(
        message
            .replace(secrets_path.to_str().unwrap(), "[SECRETS_PATH]")
            .replace(&cause, "[OS_ERROR]")
    );
    Ok(())
}
