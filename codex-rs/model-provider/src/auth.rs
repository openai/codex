use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::authorization_header_for_agent_task;
use codex_api::AgentIdentityTelemetry;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_login::AuthHeaders;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::auth::AgentIdentityAuth;
use codex_login::auth::AgentIdentityAuthError;
use codex_login::auth::AgentIdentityAuthPolicy;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use http::HeaderValue;

use crate::bearer_auth_provider::BearerAuthProvider;

const BEDROCK_API_KEY_UNSUPPORTED_MESSAGE: &str =
    "Bedrock API key auth is only supported by the Amazon Bedrock model provider";

#[derive(Clone, Debug)]
pub struct ProviderAuthScope {
    pub agent_identity_policy: AgentIdentityAuthPolicy,
    pub session_source: SessionSource,
    pub agent_identity_session_fallback: AgentIdentitySessionFallback,
}

#[derive(Clone, Debug, Default)]
pub struct AgentIdentitySessionFallback {
    engaged: Arc<AtomicBool>,
}

impl AgentIdentitySessionFallback {
    pub fn is_engaged(&self) -> bool {
        self.engaged.load(Ordering::Relaxed)
    }

    fn engage(&self) -> bool {
        !self.engaged.swap(true, Ordering::Relaxed)
    }
}

/// Provider auth resolved for a request, plus metadata describing the effective auth.
#[derive(Clone)]
pub struct ResolvedProviderAuth {
    pub auth: SharedAuthProvider,
    pub agent_identity_telemetry: Option<AgentIdentityTelemetry>,
}

impl ResolvedProviderAuth {
    pub(crate) fn new(auth: SharedAuthProvider) -> Self {
        Self {
            auth,
            agent_identity_telemetry: None,
        }
    }

    fn for_agent_identity(auth: AgentIdentityAuth) -> Self {
        let agent_identity_telemetry = agent_identity_telemetry(&auth);
        Self {
            auth: Arc::new(AgentIdentityAuthProvider { auth }),
            agent_identity_telemetry: Some(agent_identity_telemetry),
        }
    }
}

pub(crate) fn agent_identity_telemetry(auth: &AgentIdentityAuth) -> AgentIdentityTelemetry {
    AgentIdentityTelemetry {
        agent_id: auth.record().agent_runtime_id.clone(),
        task_id: auth.run_task_id().to_string(),
    }
}

#[derive(Clone, Debug)]
struct AgentIdentityAuthProvider {
    auth: AgentIdentityAuth,
}

impl AuthProvider for AgentIdentityAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let record = self.auth.record();
        let header_value = authorization_header_for_agent_task(
            AgentIdentityKey {
                agent_runtime_id: &record.agent_runtime_id,
                private_key_pkcs8_base64: &record.agent_private_key,
            },
            self.auth.run_task_id(),
        )
        .map_err(std::io::Error::other);

        if let Ok(header_value) = header_value
            && let Ok(header) = HeaderValue::from_str(&header_value)
        {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }

        if let Ok(header) = HeaderValue::from_str(self.auth.account_id()) {
            let _ = headers.insert("ChatGPT-Account-ID", header);
        }

        if self.auth.is_fedramp_account() {
            let _ = headers.insert("X-OpenAI-Fedramp", HeaderValue::from_static("true"));
        }
    }
}

#[derive(Clone, Debug)]
struct HeaderAuthProvider {
    auth: AuthHeaders,
}

impl AuthProvider for HeaderAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        headers.extend(self.auth.headers().clone());
    }
}

struct AuthManagerAuthProvider {
    auth_manager: Arc<AuthManager>,
    // Startup auth is only the account-scoped identity anchor. Request
    // headers always come from the current AuthManager snapshot below.
    expected_auth: CodexAuth,
}

impl AuthProvider for AuthManagerAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let Some(auth) = self
            .auth_manager
            .auth_cached()
            .filter(CodexAuth::uses_codex_backend)
        else {
            return;
        };
        // The caller's account-scoped state was built for the expected
        // identity. Follow token refreshes for that identity, but never cross
        // an account or workspace boundary without rebuilding that state.
        if auth.get_account_id() != self.expected_auth.get_account_id()
            || auth.get_chatgpt_user_id() != self.expected_auth.get_chatgpt_user_id()
            || auth.is_workspace_account() != self.expected_auth.is_workspace_account()
        {
            return;
        }
        auth_provider_from_auth(&auth).add_auth_headers(headers);
    }
}

// Some providers are meant to send no auth headers. Examples include local OSS
// providers and custom test providers with `requires_openai_auth = false`.
#[derive(Clone, Debug)]
struct UnauthenticatedAuthProvider;

impl AuthProvider for UnauthenticatedAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
}

pub fn unauthenticated_auth_provider() -> SharedAuthProvider {
    Arc::new(UnauthenticatedAuthProvider)
}

/// Returns the provider-scoped auth manager when this provider uses command-backed auth.
///
/// Providers without custom auth continue using the caller-supplied base manager, when present.
pub(crate) fn auth_manager_for_provider(
    auth_manager: Option<Arc<AuthManager>>,
    provider: &ModelProviderInfo,
) -> Option<Arc<AuthManager>> {
    match provider.auth.clone() {
        Some(config) => Some(AuthManager::external_bearer_only(config)),
        None => auth_manager,
    }
}

pub(crate) fn resolve_provider_auth(
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<SharedAuthProvider> {
    if matches!(auth, Some(CodexAuth::BedrockApiKey(_))) {
        return Err(CodexErr::UnsupportedOperation(
            BEDROCK_API_KEY_UNSUPPORTED_MESSAGE.to_string(),
        ));
    }

    if let Some(auth) = bearer_auth_for_provider(provider)? {
        return Ok(Arc::new(auth));
    }

    Ok(match auth {
        Some(auth) => auth_provider_from_auth(auth),
        None => unauthenticated_auth_provider(),
    })
}

pub(crate) async fn resolve_provider_auth_for_scope(
    auth_manager: Option<Arc<AuthManager>>,
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
    scope: ProviderAuthScope,
) -> codex_protocol::error::Result<ResolvedProviderAuth> {
    let ProviderAuthScope {
        agent_identity_policy,
        session_source,
        agent_identity_session_fallback,
    } = scope;
    if let Some(CodexAuth::AgentIdentity(agent_identity_auth)) = auth {
        return Ok(ResolvedProviderAuth::for_agent_identity(
            agent_identity_auth.clone(),
        ));
    }

    if !should_bootstrap_chatgpt_agent_identity(agent_identity_policy, auth)
        || agent_identity_session_fallback.is_engaged()
    {
        return resolve_provider_auth(auth, provider).map(ResolvedProviderAuth::new);
    }

    let Some(auth_manager) = auth_manager else {
        return resolve_provider_auth(auth, provider).map(ResolvedProviderAuth::new);
    };

    match auth_manager
        .agent_identity_auth(agent_identity_policy, session_source)
        .await
    {
        Ok(Some(agent_identity_auth)) => Ok(ResolvedProviderAuth::for_agent_identity(
            agent_identity_auth,
        )),
        Ok(None) => resolve_provider_auth(auth, provider).map(ResolvedProviderAuth::new),
        Err(err) => {
            if let Some(AgentIdentityAuthError::BootstrapUnavailable {
                operation,
                attempts,
                message,
            }) = err
                .get_ref()
                .and_then(|source| source.downcast_ref::<AgentIdentityAuthError>())
            {
                let newly_engaged = agent_identity_session_fallback.engage();
                tracing::warn!(
                    operation,
                    attempts = *attempts,
                    error = %message,
                    newly_engaged,
                    "agent identity bootstrap unavailable; using ChatGPT bearer auth for this session"
                );
                resolve_provider_auth(auth, provider).map(ResolvedProviderAuth::new)
            } else {
                Err(err.into())
            }
        }
    }
}

fn should_bootstrap_chatgpt_agent_identity(
    agent_identity_policy: AgentIdentityAuthPolicy,
    auth: Option<&CodexAuth>,
) -> bool {
    agent_identity_policy == AgentIdentityAuthPolicy::ChatGptAuth
        && matches!(auth, Some(CodexAuth::Chatgpt(_)))
}

fn bearer_auth_for_provider(
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<Option<BearerAuthProvider>> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(Some(BearerAuthProvider::new(api_key)));
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(Some(BearerAuthProvider::new(token)));
    }

    Ok(None)
}

/// Builds request-header auth for a first-party Codex auth snapshot.
pub fn auth_provider_from_auth(auth: &CodexAuth) -> SharedAuthProvider {
    match auth {
        CodexAuth::AgentIdentity(auth) => {
            Arc::new(AgentIdentityAuthProvider { auth: auth.clone() })
        }
        CodexAuth::Headers(auth) => Arc::new(HeaderAuthProvider { auth: auth.clone() }),
        CodexAuth::BedrockApiKey(_) => unreachable!("{BEDROCK_API_KEY_UNSUPPORTED_MESSAGE}"),
        CodexAuth::ApiKey(_)
        | CodexAuth::Chatgpt(_)
        | CodexAuth::ChatgptAuthTokens(_)
        | CodexAuth::PersonalAccessToken(_) => Arc::new(BearerAuthProvider {
            token: auth.get_token().ok(),
            account_id: auth.get_account_id(),
            is_fedramp_account: auth.is_fedramp_account(),
        }),
    }
}

/// Builds request-header auth that reads the current managed auth snapshot on
/// every request while remaining scoped to the expected auth identity.
///
/// Callers with account-scoped state should pass the same snapshot that keyed
/// that state so a later account switch cannot reuse it.
pub fn auth_provider_from_auth_manager(
    auth_manager: Arc<AuthManager>,
    expected_auth: &CodexAuth,
) -> SharedAuthProvider {
    Arc::new(AuthManagerAuthProvider {
        auth_manager,
        expected_auth: expected_auth.clone(),
    })
}

#[cfg(test)]
mod tests {
    use codex_agent_identity::generate_agent_key_material;
    use codex_login::AuthCredentialsStoreMode;
    use codex_login::AuthKeyringBackendKind;
    use codex_login::auth::AgentIdentityAuthRecord;
    use codex_login::auth::BedrockApiKeyAuth;
    use codex_login::auth::login_with_chatgpt_auth_tokens;
    use codex_model_provider_info::WireApi;
    use codex_model_provider_info::create_oss_provider_with_base_url;
    use codex_protocol::account::PlanType;
    use http::header::AUTHORIZATION;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use super::*;

    static NEXT_CODEX_HOME_ID: AtomicUsize = AtomicUsize::new(0);
    const TEST_CHATGPT_ID_TOKEN: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJlbWFpbCI6InVzZXJAZXhhbXBsZS5jb20iLCJlbWFpbF92ZXJpZmllZCI6dHJ1ZSwiaHR0cHM6Ly9hcGkub3BlbmFpLmNvbS9hdXRoIjp7ImNoYXRncHRfdXNlcl9pZCI6InVzZXItMTIzNDUiLCJ1c2VyX2lkIjoidXNlci0xMjM0NSIsImNoYXRncHRfcGxhbl90eXBlIjoicHJvIiwiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudC0xMjMifX0.c2ln";

    async fn agent_identity_auth(chatgpt_account_is_fedramp: bool) -> AgentIdentityAuth {
        let key_material = generate_agent_key_material().expect("generate key material");
        AgentIdentityAuth::from_record(
            AgentIdentityAuthRecord {
                agent_runtime_id: "agent-runtime-1".to_string(),
                agent_private_key: key_material.private_key_pkcs8_base64,
                account_id: "account-1".to_string(),
                chatgpt_user_id: "user-1".to_string(),
                email: Some("agent@example.com".to_string()),
                plan_type: PlanType::Plus,
                chatgpt_account_is_fedramp,
                task_id: Some("task-run-1".to_string()),
            },
            "https://auth.openai.com/api/accounts",
            &codex_login::test_support::transport_default_auth_route_config(),
        )
        .await
        .expect("agent identity auth record should include task id")
    }

    fn provider_auth_scope(
        policy: AgentIdentityAuthPolicy,
        fallback: AgentIdentitySessionFallback,
    ) -> ProviderAuthScope {
        ProviderAuthScope {
            agent_identity_policy: policy,
            session_source: SessionSource::Cli,
            agent_identity_session_fallback: fallback,
        }
    }

    fn test_codex_home() -> PathBuf {
        let id = NEXT_CODEX_HOME_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "codex-model-provider-agent-identity-{pid}-{id}",
            pid = std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp codex home");
        path
    }

    fn write_chatgpt_auth_json(codex_home: &Path) {
        let auth_json = json!({
            "tokens": {
                "id_token": TEST_CHATGPT_ID_TOKEN,
                "access_token": "test-access-token",
                "refresh_token": "test-refresh-token",
                "account_id": "account-123"
            },
            "last_refresh": "2099-01-01T00:00:00Z"
        });
        std::fs::write(
            codex_home.join("auth.json"),
            serde_json::to_string_pretty(&auth_json).expect("serialize auth.json"),
        )
        .expect("write auth.json");
    }

    async fn chatgpt_auth_manager(
        agent_identity_authapi_base_url: String,
    ) -> (PathBuf, Arc<AuthManager>, CodexAuth) {
        let codex_home = test_codex_home();
        write_chatgpt_auth_json(&codex_home);
        let auth_manager = AuthManager::shared(
            codex_home.clone(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            codex_login::test_support::transport_default_auth_route_config(),
        )
        .await;
        let auth = auth_manager.auth().await.expect("auth should load");
        let auth_manager = AuthManager::from_auth_for_testing_with_agent_identity_authapi_base_url(
            auth.clone(),
            agent_identity_authapi_base_url,
        );
        (codex_home, auth_manager, auth)
    }

    async fn mount_transient_agent_registration(
        server: &MockServer,
        status: u16,
        registration_count: Arc<AtomicUsize>,
    ) {
        Mock::given(method("POST"))
            .and(path("/v1/agent/register"))
            .respond_with(move |_request: &wiremock::Request| {
                registration_count.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(status)
            })
            .mount(server)
            .await;
    }

    #[test]
    fn unauthenticated_auth_provider_adds_no_headers() {
        let provider =
            create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
        let auth = resolve_provider_auth(/*auth*/ None, &provider).expect("auth should resolve");

        assert!(auth.to_auth_headers().is_empty());
    }

    #[test]
    fn header_auth_adds_predefined_headers() {
        let mut expected = HeaderMap::new();
        expected.insert(
            http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer external"),
        );
        expected.insert("x-external-auth", HeaderValue::from_static("enabled"));
        let auth = CodexAuth::Headers(AuthHeaders::new(expected.clone()));

        let actual = auth_provider_from_auth(&auth).to_auth_headers();

        assert_eq!(actual, expected);
    }

    #[test]
    fn openai_provider_rejects_bedrock_api_key_auth() {
        let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
        let auth = CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "bedrock-api-key-test".to_string(),
            region: "us-east-1".to_string(),
        });

        match resolve_provider_auth(Some(&auth), &provider) {
            Err(CodexErr::UnsupportedOperation(message)) => {
                assert_eq!(message, BEDROCK_API_KEY_UNSUPPORTED_MESSAGE);
            }
            Err(err) => panic!("unexpected auth error: {err:?}"),
            Ok(_) => panic!("Bedrock API key auth should be rejected"),
        }
    }

    #[tokio::test]
    async fn auth_manager_provider_follows_refreshes_but_not_account_switches() {
        let codex_home = test_codex_home();
        login_with_chatgpt_auth_tokens(
            &codex_home,
            "header.e30.first",
            "test-account",
            /*chatgpt_plan_type*/ None,
        )
        .expect("save initial auth");
        let auth_manager = Arc::new(
            AuthManager::new(
                codex_home.clone(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::Ephemeral,
                /*forced_chatgpt_workspace_id*/ None,
                /*chatgpt_base_url*/ None,
                AuthKeyringBackendKind::default(),
                codex_login::test_support::transport_default_auth_route_config(),
            )
            .await,
        );
        let expected_auth = auth_manager
            .auth_cached()
            .expect("initial auth should be cached");
        let provider = auth_provider_from_auth_manager(Arc::clone(&auth_manager), &expected_auth);

        assert_eq!(
            provider.to_auth_headers().get(AUTHORIZATION),
            Some(&HeaderValue::from_static("Bearer header.e30.first"))
        );

        login_with_chatgpt_auth_tokens(
            &codex_home,
            "header.e30.reloaded",
            "test-account",
            /*chatgpt_plan_type*/ None,
        )
        .expect("save reloaded auth");
        auth_manager.reload().await;

        assert_eq!(
            provider.to_auth_headers().get(AUTHORIZATION),
            Some(&HeaderValue::from_static("Bearer header.e30.reloaded"))
        );

        login_with_chatgpt_auth_tokens(
            &codex_home,
            "header.e30.other-account",
            "other-account",
            /*chatgpt_plan_type*/ None,
        )
        .expect("save switched-account auth");
        auth_manager.reload().await;

        assert!(provider.to_auth_headers().is_empty());
    }

    #[tokio::test]
    async fn first_party_run_scope_uses_agent_assertion_and_exposes_telemetry() {
        let auth = CodexAuth::AgentIdentity(
            agent_identity_auth(/*chatgpt_account_is_fedramp*/ false).await,
        );
        let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);

        let auth = resolve_provider_auth_for_scope(
            /*auth_manager*/ None,
            Some(&auth),
            &provider,
            provider_auth_scope(
                AgentIdentityAuthPolicy::JwtOnly,
                AgentIdentitySessionFallback::default(),
            ),
        )
        .await
        .expect("auth should resolve");

        assert_eq!(
            auth.agent_identity_telemetry,
            Some(AgentIdentityTelemetry {
                agent_id: "agent-runtime-1".to_string(),
                task_id: "task-run-1".to_string(),
            })
        );
        let headers = auth.auth.to_auth_headers();
        assert!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("AgentAssertion "))
        );
    }

    #[tokio::test]
    async fn agent_identity_auth_provider_preserves_account_routing_headers() {
        let auth = agent_identity_auth(/*chatgpt_account_is_fedramp*/ true).await;
        let provider = auth_provider_from_auth(&CodexAuth::AgentIdentity(auth));

        let headers = provider.to_auth_headers();

        assert!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("AgentAssertion "))
        );
        assert_eq!(
            headers
                .get("ChatGPT-Account-ID")
                .and_then(|value| value.to_str().ok()),
            Some("account-1")
        );
        assert_eq!(
            headers
                .get("X-OpenAI-Fedramp")
                .and_then(|value| value.to_str().ok()),
            Some("true")
        );
    }

    #[tokio::test]
    async fn chatgpt_bootstrap_unavailable_uses_session_bearer_fallback() {
        let server = MockServer::start().await;
        let registration_count = Arc::new(AtomicUsize::new(0));
        mount_transient_agent_registration(
            &server,
            /*status*/ 503,
            Arc::clone(&registration_count),
        )
        .await;
        let (_codex_home, auth_manager, auth) = chatgpt_auth_manager(server.uri()).await;
        let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
        let fallback = AgentIdentitySessionFallback::default();

        let provider_auth = resolve_provider_auth_for_scope(
            Some(auth_manager),
            Some(&auth),
            &provider,
            provider_auth_scope(AgentIdentityAuthPolicy::ChatGptAuth, fallback.clone()),
        )
        .await
        .expect("fallback should resolve bearer auth");

        let headers = provider_auth.auth.to_auth_headers();
        assert_eq!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer test-access-token")
        );
        assert_eq!(
            headers
                .get("ChatGPT-Account-ID")
                .and_then(|value| value.to_str().ok()),
            Some("account-123")
        );
        assert!(fallback.is_engaged());
        assert_eq!(registration_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn chatgpt_session_fallback_skips_later_agent_identity_bootstrap() {
        let server = MockServer::start().await;
        let registration_count = Arc::new(AtomicUsize::new(0));
        mount_transient_agent_registration(
            &server,
            /*status*/ 503,
            Arc::clone(&registration_count),
        )
        .await;
        let (_codex_home, auth_manager, auth) = chatgpt_auth_manager(server.uri()).await;
        let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
        let fallback = AgentIdentitySessionFallback::default();

        resolve_provider_auth_for_scope(
            Some(Arc::clone(&auth_manager)),
            Some(&auth),
            &provider,
            provider_auth_scope(AgentIdentityAuthPolicy::ChatGptAuth, fallback.clone()),
        )
        .await
        .expect("first fallback should resolve bearer auth");
        resolve_provider_auth_for_scope(
            Some(auth_manager),
            Some(&auth),
            &provider,
            provider_auth_scope(AgentIdentityAuthPolicy::ChatGptAuth, fallback),
        )
        .await
        .expect("second fallback should resolve bearer auth");

        assert_eq!(registration_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn chatgpt_sessions_share_bootstrap_failure_cooldown() {
        let server = MockServer::start().await;
        let registration_count = Arc::new(AtomicUsize::new(0));
        mount_transient_agent_registration(
            &server,
            /*status*/ 503,
            Arc::clone(&registration_count),
        )
        .await;
        let (_codex_home, auth_manager, auth) = chatgpt_auth_manager(server.uri()).await;
        let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
        let first_fallback = AgentIdentitySessionFallback::default();
        let second_fallback = AgentIdentitySessionFallback::default();

        resolve_provider_auth_for_scope(
            Some(Arc::clone(&auth_manager)),
            Some(&auth),
            &provider,
            provider_auth_scope(AgentIdentityAuthPolicy::ChatGptAuth, first_fallback.clone()),
        )
        .await
        .expect("first session fallback should resolve bearer auth");
        resolve_provider_auth_for_scope(
            Some(auth_manager),
            Some(&auth),
            &provider,
            provider_auth_scope(
                AgentIdentityAuthPolicy::ChatGptAuth,
                second_fallback.clone(),
            ),
        )
        .await
        .expect("second session fallback should resolve bearer auth");

        assert!(first_fallback.is_engaged());
        assert!(second_fallback.is_engaged());
        assert_eq!(registration_count.load(Ordering::SeqCst), 3);
    }
}
