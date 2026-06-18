use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn load_without_codex_backend_auth_returns_no_routes() {
    assert_eq!(
        load_for_session("https://example.invalid", /*auth*/ None).await,
        CredentialedRoutesConfig::default()
    );
}
