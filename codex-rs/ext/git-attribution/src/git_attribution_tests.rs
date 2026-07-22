use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use tokio::sync::Notify;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::policy::resolve_attribution_policy;

fn enterprise_auth_manager() -> Arc<AuthManager> {
    AuthManager::from_auth_for_testing(enterprise_auth("workspace-123"))
}

fn http_client_factory() -> HttpClientFactory {
    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
}

fn enterprise_auth(account_id: &str) -> CodexAuth {
    CodexAuth::from_external_chatgpt_tokens("e30.e30.c2ln", account_id, Some("enterprise"))
        .expect("fake ChatGPT auth should parse")
}

struct StaticExternalAuth(CodexAuth);

impl ExternalAuth for StaticExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.clone()) })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

async fn set_auth(auth_manager: &AuthManager, account_id: &str) {
    auth_manager
        .set_external_auth(Arc::new(StaticExternalAuth(enterprise_auth(account_id))))
        .await
        .expect("auth refresh should succeed");
}

#[tokio::test]
async fn policy_resolution_recovers_after_unauthorized() {
    let server = MockServer::start().await;
    let request_count = Arc::new(AtomicUsize::new(0));
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .respond_with({
            let request_count = request_count.clone();
            move |_request: &wiremock::Request| {
                if request_count.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(401)
                } else {
                    ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"commit_attribution_enabled": true}))
                }
            }
        })
        .expect(2)
        .mount(&server)
        .await;
    let auth_manager = enterprise_auth_manager();
    set_auth(auth_manager.as_ref(), "workspace-123").await;

    let policy = resolve_attribution_policy(
        &auth_manager,
        &format!("{}/backend-api", server.uri()),
        &http_client_factory(),
    )
    .await
    .expect("policy resolution should not time out")
    .expect("policy should resolve after auth recovery");

    assert!(policy.enabled);
    assert_eq!(request_count.load(Ordering::SeqCst), 2);
    server.verify().await;
}

#[tokio::test]
async fn policy_resolution_retries_after_auth_refresh() {
    let server = MockServer::start().await;
    let request_started = Arc::new(Notify::new());
    let request_count = Arc::new(AtomicUsize::new(0));
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .respond_with({
            let request_started = request_started.clone();
            let request_count = request_count.clone();
            move |_request: &wiremock::Request| match request_count.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    request_started.notify_one();
                    ResponseTemplate::new(200)
                        .set_delay(Duration::from_millis(100))
                        .set_body_json(serde_json::json!({
                            "commit_attribution_enabled": true,
                        }))
                }
                1 => ResponseTemplate::new(401),
                _ => ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "commit_attribution_enabled": true,
                })),
            }
        })
        .expect(3)
        .mount(&server)
        .await;
    let auth_manager = enterprise_auth_manager();
    let resolve = tokio::spawn({
        let auth_manager = auth_manager.clone();
        let base_url = format!("{}/backend-api", server.uri());
        async move {
            resolve_attribution_policy(&auth_manager, &base_url, &http_client_factory())
                .await
                .ok()
                .flatten()
        }
    });

    tokio::time::timeout(Duration::from_secs(5), request_started.notified())
        .await
        .expect("first settings request should start");
    set_auth(auth_manager.as_ref(), "workspace-456").await;

    assert!(
        resolve
            .await
            .expect("policy task should complete")
            .expect("policy should resolve after refresh")
            .enabled
    );
    server.verify().await;
}
