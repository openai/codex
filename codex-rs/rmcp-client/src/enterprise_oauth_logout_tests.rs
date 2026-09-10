//! Cross-process logout exercises staged login and the production keyring adapter.

use std::fs;
use std::path::PathBuf;

use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;

use super::*;

const TEST: &str =
    "enterprise_oauth_login::tests::logout::logout_invalidates_pending_login_across_processes";
const LOGOUT_ISSUER: &str = "CODEX_ENTERPRISE_LOGOUT_TEST_ISSUER";

#[tokio::test]
async fn logout_invalidates_pending_login_across_processes() -> Result<()> {
    if isolated_process(TEST).await? {
        return Ok(());
    }
    let home = PathBuf::from(std::env::var("CODEX_HOME")?);
    let keyring = FileKeyring(home.join("test-keyring"));
    fs::create_dir_all(&keyring.0)?;
    keyring::set_default_credential_builder(Box::new(keyring));
    if let Ok(issuer) = std::env::var(LOGOUT_ISSUER) {
        delete_enterprise_oauth_tokens(CREDENTIAL_NAME, &issuer, AuthKeyringBackendKind::Direct)
            .await?;
        return Ok(());
    }
    let server = MockServer::start().await;
    let issuer = format!("{}/idp", server.uri());
    metadata(&server, &issuer).await;
    let assertion = format!(
        "{}.{}.signature",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({"iss":issuer,"sub":"user","aud":"enterprise-client","exp":4102444800_u64})
                .to_string()
        )
    );
    Mock::given(method("POST"))
        .and(path("/idp/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token":SECRET,"refresh_token":SECRET,"id_token":assertion,"token_type":"Bearer",
        })))
        .mount(&server)
        .await;

    // The first logout has no grant to delete. The second deletes the fresh grant
    // from the previous iteration. Both must invalidate pending callbacks and staged grants.
    for _ in 0..2 {
        let pending = login(&issuer, /*callback_url*/ None).await?;
        let staged = complete_login(&issuer).await?;
        let output = tokio::process::Command::new(std::env::current_exe()?)
            .args(["--exact", TEST, "--nocapture"])
            .env(LOGOUT_ISSUER, &issuer)
            .output()
            .await?;
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stored(&issuer)?.is_none());
        // The logout process has exited; its invalidation must survive that exit.
        assert!(staged.commit_if(|| async { Some(()) }).await.is_err());
        assert!(stored(&issuer)?.is_none());

        // A fresh login after logout remains usable, including for the same account.
        complete_login(&issuer)
            .await?
            .commit_if(|| async { Some(()) })
            .await?;
        let fresh = stored(&issuer)?.expect("fresh grant");
        callback(
            &pending.authorization_url(),
            &issuer,
            /*provider_error*/ false,
        )
        .await?;
        assert!(
            pending
                .wait()
                .await?
                .commit_if(|| async { Some(()) })
                .await
                .is_err()
        );
        assert_eq!(stored(&issuer)?, Some(fresh));
    }

    let stale = complete_login(&issuer).await?;
    let generation_path = fs::read_dir(home.join("mcp-oauth-locks"))?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|ext| ext == "enterprise-generation")
        })
        .expect("persistent generation");
    fs::write(&generation_path, b"incomplete write")?;
    assert!(stale.commit_if(|| async { Some(()) }).await.is_err());
    assert!(login(&issuer, /*callback_url*/ None).await.is_err());

    // A lost marker never revives a pre-logout attempt or a pre-reset generation.
    fs::remove_file(&generation_path)?;
    let stale = complete_login(&issuer).await?;
    fs::remove_file(&generation_path)?;
    complete_login(&issuer)
        .await?
        .commit_if(|| async { Some(()) })
        .await?;
    assert!(stale.commit_if(|| async { Some(()) }).await.is_err());

    // Metadata failures are logout errors and must leave the stored grant intact.
    let before = stored(&issuer)?;
    fs::remove_file(&generation_path)?;
    fs::create_dir(&generation_path)?;
    assert!(
        delete_enterprise_oauth_tokens(CREDENTIAL_NAME, &issuer, AuthKeyringBackendKind::Direct)
            .await
            .is_err()
    );
    assert_eq!(stored(&issuer)?, before);
    assert!(!home.join(".credentials.json").exists());
    Ok(())
}

fn stored(issuer: &str) -> Result<Option<StoredOAuthTokens>> {
    crate::stored_oauth_credentials(
        CREDENTIAL_NAME,
        issuer,
        OAuthCredentialsStoreMode::Keyring,
        AuthKeyringBackendKind::Direct,
    )
}

// Only synthetic fixture credentials are persisted here. Every process still uses
// DefaultKeyringStore and the production credential lock, serializer and logout API.
struct FileKeyring(PathBuf);
struct FileCredential(PathBuf);

impl CredentialBuilderApi for FileKeyring {
    fn build(
        &self,
        _target: Option<&str>,
        _service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        let name = format!("{:x}", Sha256::digest(user.as_bytes()));
        Ok(Box::new(FileCredential(self.0.join(name))))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CredentialApi for FileCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        fs::write(&self.0, secret).map_err(keyring_error)
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        fs::read(&self.0).map_err(keyring_error)
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        fs::remove_file(&self.0).map_err(keyring_error)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn keyring_error(error: std::io::Error) -> keyring::Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        keyring::Error::NoEntry
    } else {
        keyring::Error::PlatformFailure(Box::new(error))
    }
}
