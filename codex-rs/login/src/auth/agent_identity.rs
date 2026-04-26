use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::normalize_chatgpt_base_url;
use codex_agent_identity::register_agent_task;
use codex_protocol::account::PlanType as AccountPlanType;

use crate::default_client::build_reqwest_client;

use super::storage::AgentIdentityAuthRecord;

const DEFAULT_CHATGPT_BACKEND_BASE_URL: &str = "https://chatgpt.com/backend-api";

#[derive(Clone, Debug)]
pub struct AgentIdentityAuth {
    record: AgentIdentityAuthRecord,
    process_task_id: String,
}

impl AgentIdentityAuth {
    pub async fn load(
        record: AgentIdentityAuthRecord,
        chatgpt_base_url: Option<&str>,
    ) -> std::io::Result<Self> {
        let base_url = normalize_chatgpt_base_url(
            chatgpt_base_url.unwrap_or(DEFAULT_CHATGPT_BACKEND_BASE_URL),
        );
        let process_task_id = register_agent_task(&build_reqwest_client(), &base_url, key(&record))
            .await
            .map_err(std::io::Error::other)?;
        Ok(Self {
            record,
            process_task_id,
        })
    }

    pub fn record(&self) -> &AgentIdentityAuthRecord {
        &self.record
    }

    pub fn process_task_id(&self) -> &str {
        &self.process_task_id
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
}

fn key(record: &AgentIdentityAuthRecord) -> AgentIdentityKey<'_> {
    AgentIdentityKey {
        agent_runtime_id: &record.agent_runtime_id,
        private_key_pkcs8_base64: &record.agent_private_key,
    }
}
