use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::register_agent_task;
use codex_protocol::account::PlanType as AccountPlanType;

use crate::default_client::build_reqwest_client;

use super::storage::AgentIdentityAuthRecord;

const PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL: &str = "https://auth.openai.com/api/accounts";

#[derive(Clone, Debug)]
pub struct AgentIdentityAuth {
    record: AgentIdentityAuthRecord,
    process_task_id: String,
}

impl AgentIdentityAuth {
    pub async fn load(
        record: AgentIdentityAuthRecord,
        configured_agent_identity_authapi_base_url: Option<&str>,
    ) -> std::io::Result<Self> {
        let agent_identity_authapi_base_url =
            agent_identity_authapi_base_url(configured_agent_identity_authapi_base_url);
        let process_task_id = register_agent_task(
            &build_reqwest_client(),
            &agent_identity_authapi_base_url,
            key(&record),
        )
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

fn agent_identity_authapi_base_url(
    configured_agent_identity_authapi_base_url: Option<&str>,
) -> String {
    if let Some(base_url) = configured_agent_identity_authapi_base_url {
        return base_url.trim_end_matches('/').to_string();
    }

    PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL.to_string()
}

fn key(record: &AgentIdentityAuthRecord) -> AgentIdentityKey<'_> {
    AgentIdentityKey {
        agent_runtime_id: &record.agent_runtime_id,
        private_key_pkcs8_base64: &record.agent_private_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_identity_authapi_base_url_prefers_configured_value() {
        assert_eq!(
            agent_identity_authapi_base_url(Some("https://authapi.example.test/api/accounts/")),
            "https://authapi.example.test/api/accounts"
        );
    }

    #[test]
    fn agent_identity_authapi_base_url_uses_prod_authapi_by_default() {
        assert_eq!(
            agent_identity_authapi_base_url(
                /*configured_agent_identity_authapi_base_url*/ None,
            ),
            PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL
        );
    }
}
