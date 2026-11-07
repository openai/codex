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
        /*runtime_auth*/ None,
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
            resolve_attribution_policy(
                &auth_manager,
                &base_url,
                &http_client_factory(),
                /*runtime_auth*/ None,
            )
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

struct RotatingExternalAuth(AtomicUsize);

impl ExternalAuth for RotatingExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            let generation = self.0.load(Ordering::SeqCst);
            CodexAuth::from_external_chatgpt_tokens(
                &format!("e30.e30.signature-{generation}"),
                "workspace-123",
                Some("enterprise"),
            )
        })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.0.fetch_add(/*val*/ 1, Ordering::SeqCst);
        self.resolve()
    }
}

#[tokio::test]
async fn world_state_stops_after_rejected_refresh() {
    use codex_extension_api::ContextContributor;
    use codex_extension_api::ExtensionData;
    use codex_extension_api::PreviousWorldStateSection;
    use codex_extension_api::WorldStateContributionInput;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .respond_with(ResponseTemplate::new(401))
        .expect(2)
        .mount(&server)
        .await;
    let source = Arc::new(RotatingExternalAuth(AtomicUsize::new(/*v*/ 0)));
    let auth_manager = enterprise_auth_manager();
    auth_manager
        .set_external_auth(source.clone())
        .await
        .expect("external credentials should load");
    let extension = super::GitAttributionExtension {
        auth_manager,
        base_url: format!("{}/backend-api", server.uri()),
        http_client_factory: http_client_factory(),
    };
    let model_info = serde_json::from_value(serde_json::json!({
        "slug": "test-model",
        "display_name": "Test model",
        "supported_reasoning_levels": [],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": 0,
        "support_verbosity": false,
        "truncation_policy": {"mode": "bytes", "limit": 10000},
        "experimental_supported_tools": []
    }))
    .expect("model metadata should parse");
    let store = ExtensionData::new("test");
    let sections = tokio::time::timeout(
        Duration::from_secs(2),
        extension.contribute_world_state(WorldStateContributionInput {
            thread_id: Default::default(),
            turn_id: "turn",
            model_info: &model_info,
            environments: &[],
            extension_metrics: None,
            session_store: &store,
            thread_store: &store,
            turn_store: &store,
            step_store: &store,
            previous_world_state: None,
        }),
    )
    .await
    .expect("an optional settings failure must not keep refreshing credentials");
    assert_eq!(source.0.load(Ordering::SeqCst), 1);
    assert!(sections.iter().all(|section| {
        section
            .render_diff(PreviousWorldStateSection::Absent)
            .1
            .is_none()
    }));
    server.verify().await;
}
