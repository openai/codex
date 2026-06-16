use std::sync::Arc;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::agent_task_registration_url;
use codex_agent_identity::register_agent_task;
use codex_protocol::account::PlanType as AccountPlanType;

use crate::default_client::build_default_auth_reqwest_client;
use crate::outbound_proxy::AuthRouteConfig;

use super::storage::AgentIdentityAuthRecord;

#[derive(Clone, Debug)]
pub struct AgentIdentityAuth {
    inner: Arc<AgentIdentityAuthInner>,
}

#[derive(Debug)]
struct AgentIdentityAuthInner {
    record: AgentIdentityAuthRecord,
    run_task_id: String,
}

impl AgentIdentityAuth {
    pub(crate) async fn load(
        record: AgentIdentityAuthRecord,
        agent_identity_authapi_base_url: &str,
        auth_route_config: Option<&AuthRouteConfig>,
    ) -> std::io::Result<Self> {
        let task_registration_url =
            agent_task_registration_url(agent_identity_authapi_base_url, &record.agent_runtime_id);
        let client = build_default_auth_reqwest_client(&task_registration_url, auth_route_config)?;
        let run_task_id = register_agent_task(
            &client,
            agent_identity_authapi_base_url,
            key_for_record(&record),
        )
        .await
        .map_err(std::io::Error::other)?;
        Ok(Self {
            inner: Arc::new(AgentIdentityAuthInner {
                record,
                run_task_id,
            }),
        })
    }

    #[cfg(test)]
    fn from_initialized_record(record: AgentIdentityAuthRecord, run_task_id: String) -> Self {
        Self {
            inner: Arc::new(AgentIdentityAuthInner {
                record,
                run_task_id,
            }),
        }
    }

    pub fn record(&self) -> &AgentIdentityAuthRecord {
        &self.inner.record
    }

    pub fn run_task_id(&self) -> &str {
        &self.inner.run_task_id
    }

    pub fn account_id(&self) -> &str {
        &self.inner.record.account_id
    }

    pub fn chatgpt_user_id(&self) -> &str {
        &self.inner.record.chatgpt_user_id
    }

    pub fn email(&self) -> &str {
        &self.inner.record.email
    }

    pub fn plan_type(&self) -> AccountPlanType {
        self.inner.record.plan_type
    }

    pub fn is_fedramp_account(&self) -> bool {
        self.inner.record.chatgpt_account_is_fedramp
    }
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
            email: "agent@example.com".to_string(),
            plan_type: AccountPlanType::Plus,
            chatgpt_account_is_fedramp: false,
        }
    }

    fn agent_identity_record_with_generated_key() -> AgentIdentityAuthRecord {
        let key_material = generate_agent_key_material().expect("generate key material");
        agent_identity_record(key_material.private_key_pkcs8_base64)
    }

    #[tokio::test]
    async fn load_registers_run_task() -> anyhow::Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-runtime-1/task/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "task_id": "task-run-1",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = AgentIdentityAuth::load(
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

    #[test]
    fn run_task_is_shared_across_clones() {
        let auth = AgentIdentityAuth::from_initialized_record(
            agent_identity_record_with_generated_key(),
            "task-run-1".to_string(),
        );
        let cloned = auth.clone();

        assert!(Arc::ptr_eq(&auth.inner, &cloned.inner));
        assert_eq!(cloned.run_task_id(), "task-run-1");
    }

    #[tokio::test]
    async fn failed_run_task_registration_can_be_retried_on_next_call() -> anyhow::Result<()> {
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
        let record = agent_identity_record_with_generated_key();
        AgentIdentityAuth::load(record.clone(), &server.uri(), /*auth_route_config*/ None)
            .await
            .expect_err("first registration should fail");
        let auth = AgentIdentityAuth::load(record, &server.uri(), /*auth_route_config*/ None)
            .await?;

        assert_eq!(request_count.load(Ordering::SeqCst), 2);
        assert_eq!(auth.run_task_id(), "task-run-1");
        Ok(())
    }
}
