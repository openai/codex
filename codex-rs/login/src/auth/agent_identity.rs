use std::sync::Arc;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::normalize_chatgpt_base_url;
use codex_agent_identity::register_agent_task;
use codex_protocol::account::PlanType as AccountPlanType;
use tokio::sync::OnceCell;

use crate::default_client::build_reqwest_client;

use super::storage::AgentIdentityAuthRecord;

const DEFAULT_CHATGPT_BACKEND_BASE_URL: &str = "https://chatgpt.com/backend-api";
const CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL_ENV: &str = "CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL";
const PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL: &str =
    "https://authapi-login-provider.gateway.unified-7.internal.api.openai.org/api/accounts";
const STAGING_AGENT_IDENTITY_AUTHAPI_BASE_URL: &str =
    "https://authapi-login-provider.gateway.unified-0s.internal.api.openai.org/api/accounts";

#[derive(Debug)]
pub struct AgentIdentityAuth {
    record: AgentIdentityAuthRecord,
    process_task_id: Arc<OnceCell<String>>,
}

impl Clone for AgentIdentityAuth {
    fn clone(&self) -> Self {
        Self {
            record: self.record.clone(),
            process_task_id: Arc::clone(&self.process_task_id),
        }
    }
}

impl AgentIdentityAuth {
    pub fn new(record: AgentIdentityAuthRecord) -> Self {
        Self {
            record,
            process_task_id: Arc::new(OnceCell::new()),
        }
    }

    pub fn record(&self) -> &AgentIdentityAuthRecord {
        &self.record
    }

    pub fn process_task_id(&self) -> Option<&str> {
        self.process_task_id.get().map(String::as_str)
    }

    pub async fn ensure_runtime(&self, chatgpt_base_url: Option<String>) -> std::io::Result<()> {
        self.process_task_id
            .get_or_try_init(|| async {
                let base_url = agent_identity_authapi_base_url(chatgpt_base_url.as_deref());
                register_agent_task(&build_reqwest_client(), &base_url, self.key())
                    .await
                    .map_err(std::io::Error::other)
            })
            .await
            .map(|_| ())
    }

    pub fn account_id(&self) -> &str {
        &self.record.account_id
    }

    pub fn chatgpt_user_id(&self) -> &str {
        &self.record.chatgpt_user_id
    }

    pub fn email(&self) -> &str {
        &self.record.email
    }

    pub fn plan_type(&self) -> AccountPlanType {
        self.record.plan_type
    }

    pub fn is_fedramp_account(&self) -> bool {
        self.record.chatgpt_account_is_fedramp
    }

    fn key(&self) -> AgentIdentityKey<'_> {
        AgentIdentityKey {
            agent_runtime_id: &self.record.agent_runtime_id,
            private_key_pkcs8_base64: &self.record.agent_private_key,
        }
    }
}

fn agent_identity_authapi_base_url(chatgpt_base_url: Option<&str>) -> String {
    if let Ok(base_url) = std::env::var(CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL_ENV) {
        let trimmed = base_url.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let chatgpt_base_url = chatgpt_base_url.unwrap_or(DEFAULT_CHATGPT_BACKEND_BASE_URL);
    if chatgpt_base_url.contains("chatgpt-staging.com")
        || chatgpt_base_url.contains("openai-staging.com")
    {
        return STAGING_AGENT_IDENTITY_AUTHAPI_BASE_URL.to_string();
    }

    if chatgpt_base_url.starts_with("http://localhost")
        || chatgpt_base_url.starts_with("http://127.0.0.1")
        || chatgpt_base_url.starts_with("http://[::1]")
    {
        return normalize_chatgpt_base_url(chatgpt_base_url);
    }

    PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL.to_string()
}
