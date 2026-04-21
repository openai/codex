use std::sync::Arc;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::AgentTaskAuthorizationTarget;
use codex_agent_identity::authorization_header_for_agent_task;
use codex_agent_identity::normalize_chatgpt_base_url;
use codex_agent_identity::register_agent_task;
use codex_protocol::account::PlanType as AccountPlanType;
use tokio::sync::OnceCell;

use crate::default_client::build_reqwest_client;

use super::storage::AgentIdentityAuthRecord;

const DEFAULT_CHATGPT_BACKEND_BASE_URL: &str = "https://chatgpt.com/backend-api";

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

    pub async fn ensure_runtime(&self, chatgpt_base_url: Option<String>) -> std::io::Result<()> {
        self.process_task_id
            .get_or_try_init(|| async {
                let base_url = normalize_chatgpt_base_url(
                    chatgpt_base_url
                        .as_deref()
                        .unwrap_or(DEFAULT_CHATGPT_BACKEND_BASE_URL),
                );
                register_agent_task(&build_reqwest_client(), &base_url, self.key())
                    .await
                    .map_err(std::io::Error::other)
            })
            .await
            .map(|_| ())
    }

    pub fn authorization_header_value(&self) -> std::io::Result<String> {
        let process_task_id = self.process_task_id.get().ok_or_else(|| {
            std::io::Error::other("agent identity process task is not initialized")
        })?;
        authorization_header_for_agent_task(
            self.key(),
            AgentTaskAuthorizationTarget {
                agent_runtime_id: &self.record.agent_runtime_id,
                task_id: process_task_id,
            },
        )
        .map_err(std::io::Error::other)
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
