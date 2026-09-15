//! Authentication recovery preserves actionable status without changing the account scope.

use super::*;
use codex_http_client::OutboundProxyPolicy;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_login::RefreshTokenError;
use codex_protocol::auth::RefreshTokenFailedError;
use codex_protocol::auth::RefreshTokenFailedReason;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

enum RefreshOutcome {
    Accepted,
    Rejected,
}

struct ExternalCredentials {
    auth: CodexAuth,
    outcome: RefreshOutcome,
    refreshes: AtomicUsize,
}

impl ExternalAuth for ExternalCredentials {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.auth.clone()) })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            self.refreshes.fetch_add(/*val*/ 1, Ordering::SeqCst);
            match self.outcome {
                RefreshOutcome::Accepted => Ok(self.auth.clone()),
                RefreshOutcome::Rejected => Err(std::io::Error::other("refresh rejected")),
            }
        })
    }

    fn classify_error(&self, error: std::io::Error) -> RefreshTokenError {
        RefreshTokenError::Permanent(RefreshTokenFailedError::new(
            RefreshTokenFailedReason::Other,
            error.to_string(),
        ))
    }
}

#[tokio::test]
async fn failed_recovery_preserves_unauthorized_status() {
    assert_unauthorized_after_recovery(RefreshOutcome::Rejected, /*expected_requests*/ 1).await;
}

#[tokio::test]
async fn persistent_unauthorized_stops_after_successful_recovery() {
    assert_unauthorized_after_recovery(RefreshOutcome::Accepted, /*expected_requests*/ 2).await;
}

async fn assert_unauthorized_after_recovery(outcome: RefreshOutcome, expected_requests: u64) {
    let auth = CodexAuth::from_external_chatgpt_tokens(
        "e30.eyJleHAiOjQxMDI0NDQ4MDAsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X3VzZXJfaWQiOiJ1c2VyLWEifX0.test", "account-a", Some("plus"),
    ).unwrap();
    let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
    let credentials = Arc::new(ExternalCredentials {
        auth: auth.clone(),
        outcome,
        refreshes: AtomicUsize::new(/*v*/ 0),
    });
    auth_manager
        .set_external_auth(credentials.clone())
        .await
        .unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(/*s*/ 401))
        .expect(expected_requests)
        .mount(&server)
        .await;
    let client = Client::new_without_redirects(
        server.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .with_auth_provider(codex_model_provider::auth_provider_from_auth_manager(
        Arc::clone(&auth_manager),
        &auth,
    ));
    let session = AnalyticsSession {
        account: AnalyticsAccount {
            id: "account-a".into(),
            email: None,
            plan_type: auth.account_plan_type(),
        },
        client,
        auth_manager,
        auth,
    };
    let error = session
        .request(|client| async move {
            client
                .get_account_analytics(crate::AnalyticsReport::Usage, "2026-09-01", "2026-09-07")
                .await
        })
        .await
        .unwrap_err();
    assert_eq!(error.status().map(|status| status.as_u16()), Some(401));
    session.ensure_identity().await.unwrap();
    assert_eq!(credentials.refreshes.load(Ordering::SeqCst), 1);
    server.verify().await;
}
