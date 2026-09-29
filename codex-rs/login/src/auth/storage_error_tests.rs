//! Checks that storage diagnostics preserve useful causes without exposing credential values.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn secure_storage_errors_display_the_underlying_cause() -> anyhow::Result<()> {
    for kind in [
        AuthKeyringBackendKind::Direct,
        AuthKeyringBackendKind::Secrets,
    ] {
        for (error_kind, cause) in [
            (
                std::io::ErrorKind::PermissionDenied,
                "permission denied by keyring",
            ),
            (std::io::ErrorKind::NotConnected, "keyring unavailable"),
        ] {
            let home = tempdir()?;
            let keyring = MockKeyringStore::default();
            let storage = create_auth_storage_with_store(
                home.path().to_path_buf(),
                AuthCredentialsStoreMode::Keyring,
                Arc::new(keyring.clone()),
                kind,
            );
            let auth = auth_with_prefix("private credential value");
            storage.save(&auth)?;
            let key = match kind {
                AuthKeyringBackendKind::Direct => compute_store_key(home.path())?,
                AuthKeyringBackendKind::Secrets => {
                    compute_keyring_account(home.path(), LocalSecretsNamespace::CodexAuth)
                }
            };
            for operation in [Operation::Load, Operation::Save, Operation::Delete] {
                keyring.set_error(
                    &key,
                    KeyringError::PlatformFailure(Box::new(std::io::Error::new(error_kind, cause))),
                );
                let error = match operation {
                    Operation::Load => storage.load().unwrap_err(),
                    Operation::Save => storage.save(&auth).unwrap_err(),
                    Operation::Delete => storage.delete().unwrap_err(),
                    Operation::Cleanup | Operation::RefreshPersist => unreachable!(),
                };
                let message = error.to_string();
                assert!(message.starts_with("failed to "), "{message}");
                assert!(message.contains(cause), "{message}");
                assert!(!message.contains("private credential value"));
                let debug = format!("{error:?}");
                assert!(debug.contains(cause), "{debug}");
                assert!(!debug.contains("private credential value"));
            }
        }
    }
    Ok(())
}

#[test]
fn json_syntax_and_eof_errors_preserve_diagnostics() {
    for (input, category) in [
        (
            r#"{"OPENAI_API_KEY" "private-credential-value"}"#,
            serde_json::error::Category::Syntax,
        ),
        (
            r#"{"OPENAI_API_KEY":"private-credential-value""#,
            serde_json::error::Category::Eof,
        ),
    ] {
        let source = serde_json::from_str::<AuthDotJson>(input).unwrap_err();
        assert_eq!(source.classify(), category);
        let expected = format!("failed to load auth: decrypted auth JSON: {source}");
        let error = storage_error::with_context(
            "failed to load auth",
            anyhow::Error::new(source).context("decrypted auth JSON"),
        );
        let inner = error.get_ref().unwrap();
        assert_eq!(
            (error.to_string(), format!("{inner:?}")),
            (expected.clone(), expected)
        );
        assert!(!format!("{error:?}").contains("private-credential-value"));
    }
}

#[test]
fn json_data_errors_redact_credential_values() {
    enum Wrapper {
        Direct,
        AnyhowContext,
        Io,
        Nested,
    }

    for wrapper in [
        Wrapper::Direct,
        Wrapper::AnyhowContext,
        Wrapper::Io,
        Wrapper::Nested,
    ] {
        let source =
            serde_json::from_str::<AuthDotJson>(r#"{"auth_mode":"private-credential-value"}"#)
                .unwrap_err();
        assert!(source.to_string().contains("private-credential-value"));
        let expected = format!(
            "failed to load auth: invalid credential JSON (Data at line {} column {})",
            source.line(),
            source.column(),
        );
        let source = match wrapper {
            Wrapper::Direct => anyhow::Error::new(source),
            Wrapper::AnyhowContext => anyhow::Error::new(source).context("decrypted auth JSON"),
            Wrapper::Io => anyhow::Error::new(std::io::Error::other(source)),
            Wrapper::Nested => anyhow::Error::new(std::io::Error::other(
                anyhow::Error::new(std::io::Error::other(source))
                    .context("private-credential-value"),
            ))
            .context("decrypted auth JSON"),
        };
        let error = storage_error::with_context("failed to load auth", source);
        let inner = error.get_ref().unwrap();
        assert_eq!(
            (error.to_string(), format!("{inner:?}")),
            (expected.clone(), expected)
        );
        assert!(!format!("{error:?}").contains("private-credential-value"));

        let error = anyhow::Error::new(error).context("credential storage failed");
        for formatted in [format!("{error:#}"), format!("{error:?}")] {
            assert!(
                !formatted.contains("private-credential-value"),
                "{formatted}"
            );
        }
        for cause in error.chain() {
            assert!(cause.downcast_ref::<serde_json::Error>().is_none());
            for formatted in [cause.to_string(), format!("{cause:?}")] {
                assert!(
                    !formatted.contains("private-credential-value"),
                    "{formatted}"
                );
            }
        }
    }
}
