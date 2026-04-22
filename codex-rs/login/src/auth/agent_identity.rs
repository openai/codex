use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::AgentRuntimeId;
use codex_agent_identity::AgentTaskExternalRef;
use codex_agent_identity::AgentTaskId;
use codex_agent_identity::AgentTaskKind;
use codex_agent_identity::RegisteredAgentTask;
use codex_agent_identity::normalize_chatgpt_base_url;
use codex_agent_identity::register_agent_task_with_external_ref;
use codex_protocol::account::PlanType as AccountPlanType;
use tokio::sync::OnceCell;

use crate::default_client::build_reqwest_client;

use super::storage::AgentIdentityAuthRecord;

const DEFAULT_CHATGPT_BACKEND_BASE_URL: &str = "https://chatgpt.com/backend-api";

#[derive(Debug)]
pub struct AgentIdentityAuth {
    record: AgentIdentityAuthRecord,
    task_ids: Arc<Mutex<HashMap<AgentTaskCacheKey, Arc<OnceCell<String>>>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum AgentTaskCacheKey {
    Process,
    Thread(AgentTaskExternalRef),
}

impl Clone for AgentIdentityAuth {
    fn clone(&self) -> Self {
        Self {
            record: self.record.clone(),
            task_ids: Arc::clone(&self.task_ids),
        }
    }
}

impl AgentIdentityAuth {
    pub fn new(record: AgentIdentityAuthRecord) -> Self {
        Self {
            record,
            task_ids: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn record(&self) -> &AgentIdentityAuthRecord {
        &self.record
    }

    pub fn process_task_id(&self) -> Option<String> {
        self.task_id_for_initialized_key(&AgentTaskCacheKey::Process)
    }

    pub async fn ensure_runtime(&self, chatgpt_base_url: Option<String>) -> std::io::Result<()> {
        self.task_id_for_key(
            AgentTaskCacheKey::Process,
            chatgpt_base_url,
            /*external_ref*/ None,
        )
        .await
        .map(|_| ())
    }

    pub async fn registered_thread_task(
        &self,
        external_ref: AgentTaskExternalRef,
        chatgpt_base_url: Option<String>,
    ) -> std::io::Result<RegisteredAgentTask> {
        let task_id = self
            .task_id_for_key(
                AgentTaskCacheKey::Thread(external_ref.clone()),
                chatgpt_base_url,
                Some(external_ref),
            )
            .await?;
        Ok(self.registered_task(task_id, AgentTaskKind::Thread))
    }

    pub async fn register_task(&self, chatgpt_base_url: Option<String>) -> std::io::Result<String> {
        self.register_task_with_external_ref(chatgpt_base_url, /*external_ref*/ None)
            .await
    }

    async fn register_task_with_external_ref(
        &self,
        chatgpt_base_url: Option<String>,
        external_ref: Option<&AgentTaskExternalRef>,
    ) -> std::io::Result<String> {
        let base_url = normalize_chatgpt_base_url(
            chatgpt_base_url
                .as_deref()
                .unwrap_or(DEFAULT_CHATGPT_BACKEND_BASE_URL),
        );
        register_agent_task_with_external_ref(
            &build_reqwest_client(),
            &base_url,
            self.key(),
            external_ref,
        )
        .await
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

    async fn task_id_for_key(
        &self,
        key: AgentTaskCacheKey,
        chatgpt_base_url: Option<String>,
        external_ref: Option<AgentTaskExternalRef>,
    ) -> std::io::Result<String> {
        let slot = self.task_slot(key)?;
        slot.get_or_try_init(|| async {
            self.register_task_with_external_ref(chatgpt_base_url, external_ref.as_ref())
                .await
        })
        .await
        .cloned()
    }

    fn task_slot(&self, key: AgentTaskCacheKey) -> std::io::Result<Arc<OnceCell<String>>> {
        let mut task_ids = self
            .task_ids
            .lock()
            .map_err(|_| std::io::Error::other("failed to lock agent task cache"))?;
        Ok(task_ids
            .entry(key)
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone())
    }

    fn task_id_for_initialized_key(&self, key: &AgentTaskCacheKey) -> Option<String> {
        let task_ids = self.task_ids.lock().ok()?;
        task_ids.get(key)?.get().cloned()
    }

    fn registered_task(&self, task_id: String, kind: AgentTaskKind) -> RegisteredAgentTask {
        RegisteredAgentTask::new(
            AgentRuntimeId::new(self.record.agent_runtime_id.clone()),
            AgentTaskId::new(task_id),
            kind,
        )
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
    use wiremock::matchers::body_partial_json;
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
            registered_at: None,
        }
    }

    fn agent_identity_auth() -> AgentIdentityAuth {
        let key_material = generate_agent_key_material().expect("generate key material");
        AgentIdentityAuth::new(agent_identity_record(key_material.private_key_pkcs8_base64))
    }

    #[tokio::test]
    async fn registered_thread_task_registers_once_per_external_ref() -> anyhow::Result<()> {
        let auth = agent_identity_auth();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-runtime-1/task/register"))
            .and(body_partial_json(json!({
                "external_task_ref": "thread-1",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "task_id": "task-thread-1",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let first = auth
            .registered_thread_task(AgentTaskExternalRef::new("thread-1"), Some(server.uri()))
            .await?;
        let second = auth
            .registered_thread_task(AgentTaskExternalRef::new("thread-1"), Some(server.uri()))
            .await?;

        assert_eq!(first, second);
        assert_eq!(
            first,
            RegisteredAgentTask::new(
                AgentRuntimeId::new("agent-runtime-1"),
                AgentTaskId::new("task-thread-1"),
                AgentTaskKind::Thread,
            )
        );
        Ok(())
    }

    #[tokio::test]
    async fn registered_thread_task_uses_distinct_external_refs() -> anyhow::Result<()> {
        let auth = agent_identity_auth();
        let server = MockServer::start().await;
        for (external_ref, task_id) in
            [("thread-1", "task-thread-1"), ("thread-2", "task-thread-2")]
        {
            Mock::given(method("POST"))
                .and(path("/v1/agent/agent-runtime-1/task/register"))
                .and(body_partial_json(json!({
                    "external_task_ref": external_ref,
                })))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "task_id": task_id,
                })))
                .expect(1)
                .mount(&server)
                .await;
        }

        let first = auth
            .registered_thread_task(AgentTaskExternalRef::new("thread-1"), Some(server.uri()))
            .await?;
        let second = auth
            .registered_thread_task(AgentTaskExternalRef::new("thread-2"), Some(server.uri()))
            .await?;

        assert_eq!(first.task_id.as_str(), "task-thread-1");
        assert_eq!(second.task_id.as_str(), "task-thread-2");
        Ok(())
    }

    #[tokio::test]
    async fn failed_thread_task_registration_can_retry() -> anyhow::Result<()> {
        let auth = agent_identity_auth();
        let server = MockServer::start().await;
        let request_count = Arc::new(AtomicUsize::new(0));
        let response_count = Arc::clone(&request_count);
        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-runtime-1/task/register"))
            .and(body_partial_json(json!({
                "external_task_ref": "thread-1",
            })))
            .respond_with(move |_request: &wiremock::Request| {
                if response_count.fetch_add(1, Ordering::SeqCst) == 0 {
                    ResponseTemplate::new(500)
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "task_id": "task-thread-1",
                    }))
                }
            })
            .expect(2)
            .mount(&server)
            .await;

        auth.registered_thread_task(AgentTaskExternalRef::new("thread-1"), Some(server.uri()))
            .await
            .expect_err("first registration should fail");
        let task = auth
            .registered_thread_task(AgentTaskExternalRef::new("thread-1"), Some(server.uri()))
            .await?;

        assert_eq!(request_count.load(Ordering::SeqCst), 2);
        assert_eq!(task.task_id.as_str(), "task-thread-1");
        Ok(())
    }

    #[test]
    fn task_slots_are_shared_across_clones() {
        let auth = agent_identity_auth();
        let cloned = auth.clone();
        let slot = auth
            .task_slot(AgentTaskCacheKey::Process)
            .expect("task slot should be available");
        slot.set("process-task-1".to_string())
            .expect("process task should be unset");

        assert_eq!(cloned.process_task_id(), Some("process-task-1".to_string()));
    }
}
