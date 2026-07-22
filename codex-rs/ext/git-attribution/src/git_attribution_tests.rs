use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tokio::sync::Notify;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::install;
use super::policy::resolve_attribution_policy;
use super::world_state::DISABLED_INSTRUCTIONS;
use super::world_state::ENABLED_INSTRUCTIONS;
use super::world_state::LEGACY_COMMIT_ATTRIBUTION_INSTRUCTIONS;

async fn contribute(
    registry: &ExtensionRegistry<String>,
    thread_store: &ExtensionData,
) -> WorldStateSectionContribution {
    let session_store = ExtensionData::new("session");
    let turn_store = ExtensionData::new("turn");
    let step_store = ExtensionData::new("step");
    registry.context_contributors()[0]
        .contribute_world_state(WorldStateContributionInput {
            thread_id: ThreadId::new(),
            turn_id: "turn",
            environments: &[],
            ready_selected_capability_roots: &[],
            executor_capability_discovery: None,
            session_store: &session_store,
            thread_store,
            turn_store: &turn_store,
            step_store: &step_store,
        })
        .await
        .remove(0)
}

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

async fn mount_settings(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
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
async fn installed_contributor_composes_policy_changes() {
    let server = MockServer::start().await;
    let base_url = format!("{}/backend-api", server.uri());
    let api_key = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test"));
    assert!(
        !resolve_attribution_policy(&api_key, &base_url, &http_client_factory())
            .await
            .expect("API key policy should not time out")
            .expect("API key policy should resolve")
            .enabled
    );
    let auth_manager = enterprise_auth_manager();
    let mut builder = ExtensionRegistryBuilder::<String>::new();
    install(
        &mut builder,
        auth_manager.clone(),
        base_url.clone(),
        http_client_factory(),
    );
    let registry = builder.build();
    let thread_store = ExtensionData::new("thread");
    mount_settings(
        &server,
        ResponseTemplate::new(200)
            .set_delay(Duration::from_secs(1))
            .set_body_json(serde_json::json!({"commit_attribution_enabled": true})),
    )
    .await;
    let unavailable = contribute(&registry, &thread_store).await;
    assert_eq!(unavailable.snapshot(), &Value::Bool(false));
    assert!(
        unavailable
            .render_diff(PreviousWorldStateSection::Absent)
            .is_none()
    );
    assert!(!unavailable.has_retained_fragment_matcher());
    assert!(
        unavailable.matches_legacy_fragment("developer", LEGACY_COMMIT_ATTRIBUTION_INSTRUCTIONS)
    );
    assert_eq!(
        unavailable
            .render_diff(PreviousWorldStateSection::Unknown)
            .map(|fragment| fragment.body().to_string()),
        Some(DISABLED_INSTRUCTIONS.to_string())
    );
    assert_eq!(
        contribute(&registry, &thread_store).await.snapshot(),
        &Value::Bool(false)
    );
    server.verify().await;
    server.reset().await;
    mount_settings(&server, ResponseTemplate::new(503)).await;
    set_auth(auth_manager.as_ref(), "workspace-456").await;
    let failed = contribute(&registry, &thread_store).await;
    assert_eq!(failed.snapshot(), &Value::Bool(false));
    assert!(
        failed
            .render_diff(PreviousWorldStateSection::Absent)
            .is_none()
    );
    server.verify().await;
    server.reset().await;
    mount_settings(
        &server,
        ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({"commit_attribution_enabled": true})),
    )
    .await;
    set_auth(auth_manager.as_ref(), "workspace-789").await;
    let enabled = contribute(&registry, &thread_store).await;
    let enabled_fragment = enabled
        .render_diff(PreviousWorldStateSection::Known(&Value::Bool(false)))
        .expect("enabled policy should replace disabled policy");
    assert_eq!(enabled_fragment.body(), ENABLED_INSTRUCTIONS);
    let (start, end) = enabled_fragment.markers();
    let rendered = format!("{start}{}{end}", enabled_fragment.body());
    assert!(enabled.matches_legacy_fragment(enabled_fragment.role(), &rendered));
    assert!(enabled.matches_retained_fragment(enabled_fragment.role(), &rendered));
    assert!(!enabled.matches_legacy_fragment("developer", LEGACY_COMMIT_ATTRIBUTION_INSTRUCTIONS));
    assert!(
        contribute(&registry, &thread_store)
            .await
            .render_diff(PreviousWorldStateSection::Known(&Value::Bool(true)))
            .is_none()
    );
    server.verify().await;
    server.reset().await;
    mount_settings(
        &server,
        ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({"commit_attribution_enabled": false})),
    )
    .await;
    set_auth(auth_manager.as_ref(), "workspace-disabled").await;
    let disabled = contribute(&registry, &thread_store).await;
    assert_eq!(
        disabled
            .render_diff(PreviousWorldStateSection::Known(&Value::Bool(true)))
            .map(|fragment| fragment.body().to_string()),
        Some(DISABLED_INSTRUCTIONS.to_string())
    );
    assert!(
        disabled
            .render_diff(PreviousWorldStateSection::Absent)
            .is_none()
    );
    server.verify().await;
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
