use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::register_agent_task;
use codex_protocol::account::PlanType as AccountPlanType;
use std::env;

use crate::default_client::build_reqwest_client;

use super::storage::AgentIdentityAuthRecord;

const PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL: &str = "https://auth.openai.com/api/accounts";
const CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL_ENV_VAR: &str = "CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL";

#[derive(Clone, Debug)]
pub struct AgentIdentityAuth {
    record: AgentIdentityAuthRecord,
    process_task_id: String,
}

impl AgentIdentityAuth {
    pub async fn load(record: AgentIdentityAuthRecord) -> std::io::Result<Self> {
        let agent_identity_authapi_base_url = agent_identity_authapi_base_url();
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

fn agent_identity_authapi_base_url() -> String {
    env::var(CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL_ENV_VAR)
        .ok()
        .map(|base_url| base_url.trim().trim_end_matches('/').to_string())
        .filter(|base_url| !base_url.is_empty())
        .unwrap_or_else(|| PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL.to_string())
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
    use serial_test::serial;

    #[test]
    #[serial(codex_auth_env)]
    fn agent_identity_authapi_base_url_prefers_env_value() {
        let _guard = EnvVarGuard::set(
            CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL_ENV_VAR,
            "https://authapi.example.test/api/accounts/",
        );
        assert_eq!(
            agent_identity_authapi_base_url(),
            "https://authapi.example.test/api/accounts"
        );
    }

    #[test]
    #[serial(codex_auth_env)]
    fn agent_identity_authapi_base_url_uses_prod_authapi_by_default() {
        let _guard = EnvVarGuard::remove(CODEX_AGENT_IDENTITY_AUTHAPI_BASE_URL_ENV_VAR);
        assert_eq!(
            agent_identity_authapi_base_url(),
            PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL
        );
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var_os(key);
            unsafe {
                env::set_var(key, value);
            }
            Self { key, original }
        }

        fn remove(key: &'static str) -> Self {
            let original = env::var_os(key);
            unsafe {
                env::remove_var(key);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => env::set_var(self.key, value),
                    None => env::remove_var(self.key),
                }
            }
        }
    }
}
