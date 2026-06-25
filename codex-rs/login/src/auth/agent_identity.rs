use std::future::Future;
use std::sync::Arc;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::ChatGptEnvironment;
use codex_agent_identity::agent_identity_jwks_url;
use codex_agent_identity::agent_registration_url;
use codex_agent_identity::agent_task_registration_url;
use codex_agent_identity::build_abom;
use codex_agent_identity::decode_agent_identity_jwt;
use codex_agent_identity::fetch_agent_identity_jwks;
use codex_agent_identity::generate_agent_key_material;
use codex_agent_identity::is_retryable_registration_error;
use codex_agent_identity::public_key_ssh_from_private_key_pkcs8_base64;
use codex_agent_identity::register_agent_identity;
use codex_agent_identity::register_agent_task;
use codex_protocol::account::PlanType as AccountPlanType;
use codex_protocol::protocol::SessionSource;
use thiserror::Error;

use crate::default_client::build_default_auth_reqwest_client;
use crate::outbound_proxy::AuthRouteConfig;

use super::storage::AgentIdentityAuthRecord;

pub(super) const MAX_AGENT_IDENTITY_BOOTSTRAP_ATTEMPTS: usize = 3;

pub(super) fn agent_identity_authapi_base_url(
    chatgpt_base_url: Option<&str>,
) -> std::io::Result<String> {
    let environment = match chatgpt_base_url {
        Some(chatgpt_base_url) => ChatGptEnvironment::from_chatgpt_base_url(chatgpt_base_url)
            .map_err(std::io::Error::other)?,
        None => ChatGptEnvironment::default(),
    };
    Ok(environment.agent_identity_authapi_base_url().to_string())
}

pub(super) fn require_agent_identity_authapi_base_url(
    agent_identity_authapi_base_url: Option<&str>,
) -> std::io::Result<&str> {
    agent_identity_authapi_base_url.ok_or_else(|| {
        std::io::Error::other(
            "Agent Identity only supports production and staging ChatGPT environments",
        )
    })
}

#[derive(Clone, Debug, Error)]
pub enum AgentIdentityAuthError {
    #[error(
        "agent identity bootstrap unavailable after {attempts} attempts during {operation}: {message}"
    )]
    BootstrapUnavailable {
        operation: &'static str,
        attempts: usize,
        message: String,
    },
}

impl AgentIdentityAuthError {
    pub(super) fn bootstrap_unavailable(error: &std::io::Error) -> Option<&Self> {
        match error
            .get_ref()
            .and_then(|source| source.downcast_ref::<Self>())
        {
            Some(error @ Self::BootstrapUnavailable { .. }) => Some(error),
            None => None,
        }
    }
}

#[derive(Debug, Error)]
#[error("retryable agent identity registration failure: {message}")]
pub(super) struct RetryableAgentIdentityRegistrationError {
    message: String,
}

impl RetryableAgentIdentityRegistrationError {
    pub(super) fn new(message: String) -> Self {
        Self { message }
    }
}

#[derive(Clone, Debug)]
pub struct AgentIdentityAuth {
    record: Arc<AgentIdentityAuthRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ManagedChatGptAgentIdentityBinding {
    pub(super) account_id: String,
    pub(super) chatgpt_user_id: String,
    pub(super) email: Option<String>,
    pub(super) plan_type: AccountPlanType,
    pub(super) chatgpt_account_is_fedramp: bool,
    pub(super) access_token: String,
}

impl AgentIdentityAuth {
    pub async fn from_record(
        mut record: AgentIdentityAuthRecord,
        agent_identity_authapi_base_url: &str,
        auth_route_config: Option<&AuthRouteConfig>,
    ) -> std::io::Result<Self> {
        public_key_ssh_from_private_key_pkcs8_base64(&record.agent_private_key)
            .map_err(std::io::Error::other)?;
        if record_needs_task_registration(&record) {
            record.task_id = Some(
                register_task_for_record_with_retries(
                    &record,
                    agent_identity_authapi_base_url,
                    auth_route_config,
                )
                .await?,
            );
        }
        Ok(Self {
            record: Arc::new(record),
        })
    }

    pub async fn from_jwt(
        jwt: &str,
        chatgpt_base_url: &str,
        agent_identity_authapi_base_url: &str,
        auth_route_config: Option<&AuthRouteConfig>,
    ) -> std::io::Result<Self> {
        let record = verified_record_from_jwt(jwt, chatgpt_base_url, auth_route_config).await?;
        Self::from_record(record, agent_identity_authapi_base_url, auth_route_config).await
    }

    #[cfg(test)]
    fn from_initialized_record(mut record: AgentIdentityAuthRecord, run_task_id: String) -> Self {
        record.task_id = Some(run_task_id);
        Self {
            record: Arc::new(record),
        }
    }

    pub fn record(&self) -> &AgentIdentityAuthRecord {
        self.record.as_ref()
    }

    pub fn run_task_id(&self) -> &str {
        match self.record.task_id.as_deref() {
            Some(task_id) => task_id,
            None => unreachable!("AgentIdentityAuth should only be constructed with a task_id"),
        }
    }

    pub fn account_id(&self) -> &str {
        &self.record.account_id
    }

    pub fn chatgpt_user_id(&self) -> &str {
        &self.record.chatgpt_user_id
    }

    pub fn email(&self) -> Option<&str> {
        self.record.email.as_deref()
    }

    pub fn plan_type(&self) -> AccountPlanType {
        self.record.plan_type
    }

    pub fn is_fedramp_account(&self) -> bool {
        self.record.chatgpt_account_is_fedramp
    }
}

pub(super) async fn register_managed_chatgpt_agent_identity(
    binding: ManagedChatGptAgentIdentityBinding,
    agent_identity_authapi_base_url: &str,
    session_source: SessionSource,
    auth_route_config: Option<&AuthRouteConfig>,
) -> std::io::Result<AgentIdentityAuth> {
    let key_material = generate_agent_key_material().map_err(std::io::Error::other)?;
    let registration_url = agent_registration_url(agent_identity_authapi_base_url);
    let client = build_default_auth_reqwest_client(&registration_url, auth_route_config)?;
    let runtime_id = retry_registration(|| async {
        register_agent_identity(
            &client,
            agent_identity_authapi_base_url,
            &binding.access_token,
            binding.chatgpt_account_is_fedramp,
            &key_material,
            build_abom(session_source.clone()),
            vec!["responsesapi".to_string()],
        )
        .await
        .map_err(|err| {
            if is_retryable_registration_error(&err) {
                std::io::Error::other(RetryableAgentIdentityRegistrationError::new(
                    err.to_string(),
                ))
            } else {
                std::io::Error::other(err)
            }
        })
    })
    .await
    .map_err(|err| classify_bootstrap_error("agent identity registration", err))?;

    let record = AgentIdentityAuthRecord {
        agent_runtime_id: runtime_id,
        agent_private_key: key_material.private_key_pkcs8_base64,
        account_id: binding.account_id,
        chatgpt_user_id: binding.chatgpt_user_id,
        email: binding.email,
        plan_type: binding.plan_type,
        chatgpt_account_is_fedramp: binding.chatgpt_account_is_fedramp,
        task_id: None,
    };
    AgentIdentityAuth::from_record(record, agent_identity_authapi_base_url, auth_route_config)
        .await
        .map_err(|err| classify_bootstrap_error("agent task registration", err))
}

pub(super) async fn verified_record_from_jwt(
    jwt: &str,
    chatgpt_base_url: &str,
    auth_route_config: Option<&AuthRouteConfig>,
) -> std::io::Result<AgentIdentityAuthRecord> {
    AgentIdentityAuthRecord::from_agent_identity_jwt(jwt)?;
    let jwks_url = agent_identity_jwks_url(chatgpt_base_url);
    let client = build_default_auth_reqwest_client(&jwks_url, auth_route_config)?;
    let jwks = fetch_agent_identity_jwks(&client, chatgpt_base_url)
        .await
        .map_err(std::io::Error::other)?;
    let claims = decode_agent_identity_jwt(jwt, Some(&jwks)).map_err(std::io::Error::other)?;
    Ok(claims.into())
}

pub(super) fn record_needs_task_registration(record: &AgentIdentityAuthRecord) -> bool {
    record
        .task_id
        .as_deref()
        .is_none_or(|task_id| task_id.trim().is_empty())
}

pub(super) fn record_matches_managed_chatgpt_binding(
    record: &AgentIdentityAuthRecord,
    binding: &ManagedChatGptAgentIdentityBinding,
) -> bool {
    record.account_id == binding.account_id
        && record.chatgpt_user_id == binding.chatgpt_user_id
        && public_key_ssh_from_private_key_pkcs8_base64(&record.agent_private_key).is_ok()
}

pub(super) fn classify_bootstrap_error(
    operation: &'static str,
    err: std::io::Error,
) -> std::io::Error {
    if is_retryable_io_registration_error(&err) {
        std::io::Error::other(AgentIdentityAuthError::BootstrapUnavailable {
            operation,
            attempts: MAX_AGENT_IDENTITY_BOOTSTRAP_ATTEMPTS,
            message: err.to_string(),
        })
    } else {
        err
    }
}

pub(super) fn is_retryable_io_registration_error(err: &std::io::Error) -> bool {
    err.get_ref().is_some_and(
        <dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static>::is::<
            RetryableAgentIdentityRegistrationError,
        >,
    )
}

pub(super) async fn retry_registration<T, F, Fut>(mut operation: F) -> std::io::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::io::Result<T>>,
{
    let mut attempt = 1;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err)
                if attempt < MAX_AGENT_IDENTITY_BOOTSTRAP_ATTEMPTS
                    && is_retryable_io_registration_error(&err) =>
            {
                tracing::warn!(
                    attempt,
                    max_attempts = MAX_AGENT_IDENTITY_BOOTSTRAP_ATTEMPTS,
                    error = %err,
                    "agent identity registration attempt failed; retrying"
                );
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

async fn register_task_for_record_with_retries(
    record: &AgentIdentityAuthRecord,
    agent_identity_authapi_base_url: &str,
    auth_route_config: Option<&AuthRouteConfig>,
) -> std::io::Result<String> {
    let task_registration_url =
        agent_task_registration_url(agent_identity_authapi_base_url, &record.agent_runtime_id);
    let client = build_default_auth_reqwest_client(&task_registration_url, auth_route_config)?;
    retry_registration(|| async {
        register_task_for_record(&client, record, agent_identity_authapi_base_url).await
    })
    .await
}

async fn register_task_for_record(
    client: &reqwest::Client,
    record: &AgentIdentityAuthRecord,
    agent_identity_authapi_base_url: &str,
) -> std::io::Result<String> {
    register_agent_task(
        client,
        agent_identity_authapi_base_url,
        key_for_record(record),
    )
    .await
    .map_err(|err| {
        if is_retryable_registration_error(&err) {
            std::io::Error::other(RetryableAgentIdentityRegistrationError::new(
                err.to_string(),
            ))
        } else {
            std::io::Error::other(err)
        }
    })
}

fn key_for_record(record: &AgentIdentityAuthRecord) -> AgentIdentityKey<'_> {
    AgentIdentityKey {
        agent_runtime_id: &record.agent_runtime_id,
        private_key_pkcs8_base64: &record.agent_private_key,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use codex_agent_identity::generate_agent_key_material;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use super::*;

    fn agent_identity_record(private_key: String) -> AgentIdentityAuthRecord {
        AgentIdentityAuthRecord {
            agent_runtime_id: "agent-runtime-1".to_string(),
            agent_private_key: private_key,
            account_id: "account-1".to_string(),
            chatgpt_user_id: "user-1".to_string(),
            email: Some("agent@example.com".to_string()),
            plan_type: AccountPlanType::Plus,
            chatgpt_account_is_fedramp: false,
            task_id: None,
        }
    }

    fn agent_identity_record_with_generated_key() -> AgentIdentityAuthRecord {
        let key_material = generate_agent_key_material().expect("generate key material");
        agent_identity_record(key_material.private_key_pkcs8_base64)
    }

    #[tokio::test]
    async fn from_record_registers_task() -> anyhow::Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-runtime-1/task/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "task_id": "task-run-1",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = AgentIdentityAuth::from_record(
            agent_identity_record_with_generated_key(),
            &server.uri(),
            /*auth_route_config*/ None,
        )
        .await?;

        assert_eq!(auth.run_task_id(), "task-run-1");
        let requests = server
            .received_requests()
            .await
            .expect("failed to fetch task registration request");
        let request_body = requests[0]
            .body_json::<serde_json::Value>()
            .expect("task registration request should be JSON");
        let request_body = request_body
            .as_object()
            .expect("request body should be object");
        assert!(request_body.get("timestamp").is_some());
        assert!(request_body.get("signature").is_some());
        assert_eq!(request_body.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn from_jwt_registers_task() -> anyhow::Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/agent-identities/jwks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(test_jwks_body()))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-runtime-1/task/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "task_id": "task-run-1",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let record = agent_identity_record_with_generated_key();
        let jwt = signed_agent_identity_jwt(&record)?;
        let auth = AgentIdentityAuth::from_jwt(
            &jwt,
            &format!("{}/backend-api", server.uri()),
            &server.uri(),
            /*auth_route_config*/ None,
        )
        .await?;

        assert_eq!(auth.record().agent_runtime_id, "agent-runtime-1");
        assert_eq!(auth.run_task_id(), "task-run-1");
        Ok(())
    }

    #[test]
    fn run_task_is_shared_across_clones() {
        let auth = AgentIdentityAuth::from_initialized_record(
            agent_identity_record_with_generated_key(),
            "task-run-1".to_string(),
        );
        let cloned = auth.clone();

        assert!(Arc::ptr_eq(&auth.record, &cloned.record));
        assert_eq!(cloned.run_task_id(), "task-run-1");
    }

    #[tokio::test]
    async fn from_record_retries_transient_registration() -> anyhow::Result<()> {
        let server = MockServer::start().await;
        let request_count = Arc::new(AtomicUsize::new(0));
        let response_count = Arc::clone(&request_count);
        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-runtime-1/task/register"))
            .respond_with(move |_request: &wiremock::Request| {
                if response_count.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(500)
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "task_id": "task-run-1",
                    }))
                }
            })
            .expect(2)
            .mount(&server)
            .await;
        let auth = AgentIdentityAuth::from_record(
            agent_identity_record_with_generated_key(),
            &server.uri(),
            /*auth_route_config*/ None,
        )
        .await?;

        assert_eq!(request_count.load(Ordering::SeqCst), 2);
        assert_eq!(auth.run_task_id(), "task-run-1");
        Ok(())
    }

    fn signed_agent_identity_jwt(
        record: &AgentIdentityAuthRecord,
    ) -> jsonwebtoken::errors::Result<String> {
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("test-key".to_string());
        jsonwebtoken::encode(
            &header,
            &json!({
                "iss": "https://chatgpt.com/codex-backend/agent-identity",
                "aud": "codex-app-server",
                "iat": 1_700_000_000usize,
                "exp": 4_000_000_000usize,
                "agent_runtime_id": record.agent_runtime_id,
                "agent_private_key": record.agent_private_key,
                "account_id": record.account_id,
                "chatgpt_user_id": record.chatgpt_user_id,
                "email": record.email,
                "plan_type": record.plan_type,
                "chatgpt_account_is_fedramp": record.chatgpt_account_is_fedramp,
            }),
            &jsonwebtoken::EncodingKey::from_rsa_pem(TEST_AGENT_IDENTITY_RSA_PRIVATE_KEY_PEM)?,
        )
    }

    fn test_jwks_body() -> serde_json::Value {
        json!({
            "keys": [{
                "kty": "RSA",
                "kid": "test-key",
                "use": "sig",
                "alg": "RS256",
                "n": "1qQF2MqTrGAMDm7wXbjJP5sWqGA83tAGUs2ksy7iJXLJdhCg4AtwGm4SFl4f6kxhCSzlN1QdXuZjvRT2wZZiGUi9xUE28rf4WLrTxSnwqLuTy5knMP08yC0t_0YU_FGPZMcWb14hG05IvZr8UbmRaVagxSR8H4rSIymRoVwwmFSrqz068XrWGSYNIfLEASyo5GdAaqmk1JALINHgYGQJVxMxtwcvDxoVKmC7eltUNymMNBZhsv4E8sx9YNLpBoEibznfEpDU_DGzrM5eZCsQzaqbhBOlGd427ifud_Nnd9cPqzgCUc23-0FXSPfpbgksCXAwAmD0OFjQWrgqVdKL6Q",
                "e": "AQAB",
            }]
        })
    }

    const TEST_AGENT_IDENTITY_RSA_PRIVATE_KEY_PEM: &[u8] = br#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDWpAXYypOsYAwO
bvBduMk/mxaoYDze0AZSzaSzLuIlcsl2EKDgC3AabhIWXh/qTGEJLOU3VB1e5mO9
FPbBlmIZSL3FQTbyt/hYutPFKfCou5PLmScw/TzILS3/RhT8UY9kxxZvXiEbTki9
mvxRuZFpVqDFJHwfitIjKZGhXDCYVKurPTrxetYZJg0h8sQBLKjkZ0BqqaTUkAsg
0eBgZAlXEzG3By8PGhUqYLt6W1Q3KYw0FmGy/gTyzH1g0ukGgSJvOd8SkNT8MbOs
zl5kKxDNqpuEE6UZ3jbuJ+5382d31w+rOAJRzbf7QVdI9+luCSwJcDACYPQ4WNBa
uCpV0ovpAgMBAAECggEAVu84LwZdqYN9XpswX8VoPYrjMm9IODapWQBRpQFoNyK2
1ksF3bjEPvA2Azk8U/l7k+vLKw22l6lY3EyRZPcz5GnB8xLm3ogE3mtNOp4yCyVu
RxhQ91aaN7mU17/a4BdorLi2LYVCg3zBmYociD1Q2AluNGsCmwPu+K7tfR2J0Sg8
NjqiTbDG1XDpR/icwgC9t6vh8lZpCHDhF4tbQfLLVLeA/OdcuzXDyMCXbmdVIdBQ
rm4aIFmr2e1/2ctTbCg85S6AGFTH+pSLjrwTzyvf+F6NW5uNjLQAQLFj+EznBDxj
Xdx90cySrjsKK6PVWQF4RiTvkSW8eWL7R6B2FZbGwQKBgQDuVQRj72hWloR7mbEL
aUEEv3pIXTMXWEsoMBNczos/1L1RnAN1AI44TurznasPZAWvQj+kVbLDR+TAeZrL
iA8HIWswQUI18hFmgKzSkwIXGtubcKVrgsKeS4lMDKCM/Ef6WAYdeq6ronoY5lCN
YrJFmGp81W5zcV7lyiycgbSiGwKBgQDmjWYf6pZjrK7Z+OJ3X1AZfi2vss15SCvL
3fPgzIDbViztpGyQhc3DQZIsBNIu0xZp/veGce9TEeTds2ro9NfdJFeou8+fC7Pq
sOsM3amGFFi+ZW/9BWyjZEM88bgWWAjqLHbpfHDxjAf5CSxddqxgHlbP0Ytyb1Vg
gmPDn9YKSwKBgQDbTi3hC35WFuDHn0/zcSHcDZmnFuOZeqyFyV83yfMGhGrEuqvP
sPgtRikajJ3IZsB4WZyYSidZXEFY/0z6NjOl2xF38MTNQPbT/FmK1q1Yt2UWrlv5
BvSwlk87RG9D7C0LZo4R+D7cPoDdgqjiwMvMEIkEX5zn641oI1ZTmWKuuwKBgQCD
KF+3unnRvHRAVoFnTZbA2fJdqMeRvogD04GhGlYX8V9f1hFY6nXTJaNlXVzA/J8c
r8ra9kgjJuPfZ+ljG58OFFW2DRohLcQtuHYPfK6rMzoFHqnl9EcIcMp7ijuionR3
29HOJFgQYgxLFXfit9d6WugiE+BTupiEbckZif13HwKBgE/lAlkVHP6YahOO2Ljc
J1bwkqKZTB5dHolX9A58e/xXnfZ5P8f3Z83+Izap3FwqQulk7b1WO1MQcHuVg2NN
5da0D4h2rYOXnbYIg0BVu4spQbaM6ewsp66b8+MzLOBvj8SzWdt1Oyw0q/MRyQAR
8U4M2TSWCKUY/A6sT4W8+mT9
-----END PRIVATE KEY-----"#;
}
