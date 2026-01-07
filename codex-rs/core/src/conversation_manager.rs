use crate::AuthManager;
#[cfg(any(test, feature = "test-support"))]
use crate::CodexAuth;
#[cfg(any(test, feature = "test-support"))]
use crate::ModelProviderInfo;
use crate::agent::AgentControl;
use crate::codex::Codex;
use crate::codex::CodexSpawnOk;
use crate::codex::INITIAL_SUBMIT_ID;
use crate::codex_conversation::CodexConversation;
use crate::config::Config;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::models_manager::manager::ModelsManager;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::SessionConfiguredEvent;
use crate::rollout::RolloutRecorder;
use crate::rollout::truncation;
use crate::skills::SkillsManager;
use codex_protocol::ConversationId;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(any(test, feature = "test-support"))]
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Represents a newly created Codex conversation, including the first event
/// (which is [`EventMsg::SessionConfigured`]).
pub struct NewConversation {
    pub conversation_id: ConversationId,
    pub conversation: Arc<CodexConversation>,
    pub session_configured: SessionConfiguredEvent,
}

/// [`ConversationManager`] is responsible for creating conversations and
/// maintaining them in memory.
pub struct ConversationManager {
    state: Arc<ConversationManagerState>,
    #[cfg(any(test, feature = "test-support"))]
    _test_codex_home_guard: Option<TempDir>,
}

/// Shared, `Arc`-owned state for [`ConversationManager`]. This `Arc` is required to have a single
/// `Arc` reference that can be downgraded to by `AgentControl` while preventing every single
/// function to require an `Arc<&Self>`.
pub(crate) struct ConversationManagerState {
    conversations: Arc<RwLock<HashMap<ConversationId, Arc<CodexConversation>>>>,
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    skills_manager: Arc<SkillsManager>,
    session_source: SessionSource,
}

impl ConversationManager {
    pub fn new(auth_manager: Arc<AuthManager>, session_source: SessionSource) -> Self {
        Self {
            state: Arc::new(ConversationManagerState {
                conversations: Arc::new(RwLock::new(HashMap::new())),
                models_manager: Arc::new(ModelsManager::new(auth_manager.clone())),
                skills_manager: Arc::new(SkillsManager::new(
                    auth_manager.codex_home().to_path_buf(),
                )),
                auth_manager,
                session_source,
            }),
            #[cfg(any(test, feature = "test-support"))]
            _test_codex_home_guard: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Construct with a dummy AuthManager containing the provided CodexAuth.
    /// Used for integration tests: should not be used by ordinary business logic.
    pub fn with_models_provider(auth: CodexAuth, provider: ModelProviderInfo) -> Self {
        let temp_dir = tempfile::tempdir().unwrap_or_else(|err| panic!("temp codex home: {err}"));
        let codex_home = temp_dir.path().to_path_buf();
        let mut manager = Self::with_models_provider_and_home(auth, provider, codex_home);
        manager._test_codex_home_guard = Some(temp_dir);
        manager
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Construct with a dummy AuthManager containing the provided CodexAuth and codex home.
    /// Used for integration tests: should not be used by ordinary business logic.
    pub fn with_models_provider_and_home(
        auth: CodexAuth,
        provider: ModelProviderInfo,
        codex_home: PathBuf,
    ) -> Self {
        let auth_manager = AuthManager::from_auth_for_testing_with_home(auth, codex_home);
        Self {
            state: Arc::new(ConversationManagerState {
                conversations: Arc::new(RwLock::new(HashMap::new())),
                models_manager: Arc::new(ModelsManager::with_provider(
                    auth_manager.clone(),
                    provider,
                )),
                skills_manager: Arc::new(SkillsManager::new(
                    auth_manager.codex_home().to_path_buf(),
                )),
                auth_manager,
                session_source: SessionSource::Exec,
            }),
            _test_codex_home_guard: None,
        }
    }

    pub fn session_source(&self) -> SessionSource {
        self.state.session_source.clone()
    }

    pub fn skills_manager(&self) -> Arc<SkillsManager> {
        self.state.skills_manager.clone()
    }

    pub fn get_models_manager(&self) -> Arc<ModelsManager> {
        self.state.models_manager.clone()
    }

    pub async fn list_models(&self, config: &Config) -> Vec<ModelPreset> {
        self.state.models_manager.list_models(config).await
    }

    pub async fn get_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> CodexResult<Arc<CodexConversation>> {
        self.state.get_conversation(conversation_id).await
    }

    pub async fn new_conversation(&self, config: Config) -> CodexResult<NewConversation> {
        self.state
            .spawn_conversation(
                config,
                InitialHistory::New,
                Arc::clone(&self.state.auth_manager),
                self.agent_control(),
            )
            .await
    }

    pub async fn resume_conversation_from_rollout(
        &self,
        config: Config,
        rollout_path: PathBuf,
        auth_manager: Arc<AuthManager>,
    ) -> CodexResult<NewConversation> {
        let initial_history = RolloutRecorder::get_rollout_history(&rollout_path).await?;
        self.resume_conversation_with_history(config, initial_history, auth_manager)
            .await
    }

    pub async fn resume_conversation_with_history(
        &self,
        config: Config,
        initial_history: InitialHistory,
        auth_manager: Arc<AuthManager>,
    ) -> CodexResult<NewConversation> {
        self.state
            .spawn_conversation(config, initial_history, auth_manager, self.agent_control())
            .await
    }

    /// Removes the conversation from the manager's internal map, though the conversation is stored
    /// as `Arc<CodexConversation>`, it is possible that other references to it exist elsewhere.
    /// Returns the conversation if the conversation was found and removed.
    pub async fn remove_conversation(
        &self,
        conversation_id: &ConversationId,
    ) -> Option<Arc<CodexConversation>> {
        self.state
            .conversations
            .write()
            .await
            .remove(conversation_id)
    }

    /// Fork an existing conversation by taking messages up to the given position (not including
    /// the message at the given position) and starting a new conversation with identical
    /// configuration (unless overridden by the caller's `config`). The new conversation will have
    /// a fresh id.
    pub async fn fork_conversation(
        &self,
        nth_user_message: usize,
        config: Config,
        path: PathBuf,
    ) -> CodexResult<NewConversation> {
        let history = RolloutRecorder::get_rollout_history(&path).await?;
        let history = truncate_before_nth_user_message(history, nth_user_message);
        self.state
            .spawn_conversation(
                config,
                history,
                Arc::clone(&self.state.auth_manager),
                self.agent_control(),
            )
            .await
    }

    fn agent_control(&self) -> AgentControl {
        AgentControl::new(Arc::downgrade(&self.state))
    }
}

impl ConversationManagerState {
    pub(crate) async fn get_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> CodexResult<Arc<CodexConversation>> {
        let conversations = self.conversations.read().await;
        conversations
            .get(&conversation_id)
            .cloned()
            .ok_or_else(|| CodexErr::ConversationNotFound(conversation_id))
    }

    pub(crate) async fn send_op(
        &self,
        conversation_id: ConversationId,
        op: Op,
    ) -> CodexResult<String> {
        self.get_conversation(conversation_id)
            .await?
            .submit(op)
            .await
    }

    #[allow(dead_code)] // Used by upcoming multi-agent tooling.
    pub(crate) async fn spawn_new_conversation(
        &self,
        config: Config,
        agent_control: AgentControl,
    ) -> CodexResult<NewConversation> {
        self.spawn_conversation(
            config,
            InitialHistory::New,
            Arc::clone(&self.auth_manager),
            agent_control,
        )
        .await
    }

    pub(crate) async fn spawn_conversation(
        &self,
        config: Config,
        initial_history: InitialHistory,
        auth_manager: Arc<AuthManager>,
        agent_control: AgentControl,
    ) -> CodexResult<NewConversation> {
        let CodexSpawnOk {
            codex,
            conversation_id,
        } = Codex::spawn(
            config,
            auth_manager,
            Arc::clone(&self.models_manager),
            Arc::clone(&self.skills_manager),
            initial_history,
            self.session_source.clone(),
            agent_control,
        )
        .await?;
        self.finalize_spawn(codex, conversation_id).await
    }

    async fn finalize_spawn(
        &self,
        codex: Codex,
        conversation_id: ConversationId,
    ) -> CodexResult<NewConversation> {
        let event = codex.next_event().await?;
        let session_configured = match event {
            Event {
                id,
                msg: EventMsg::SessionConfigured(session_configured),
            } if id == INITIAL_SUBMIT_ID => session_configured,
            _ => {
                return Err(CodexErr::SessionConfiguredNotFirstEvent);
            }
        };

        let conversation = Arc::new(CodexConversation::new(
            codex,
            session_configured.rollout_path.clone(),
        ));
        self.conversations
            .write()
            .await
            .insert(conversation_id, conversation.clone());

        Ok(NewConversation {
            conversation_id,
            conversation,
            session_configured,
        })
    }
}

/// Return a prefix of `items` obtained by cutting strictly before the nth user message
/// (0-based) and all items that follow it.
fn truncate_before_nth_user_message(history: InitialHistory, n: usize) -> InitialHistory {
    let items: Vec<RolloutItem> = history.get_rollout_items();
    let rolled = truncation::truncate_rollout_before_nth_user_message_from_start(&items, n);

    if rolled.is_empty() {
        InitialHistory::New
    } else {
        InitialHistory::Forked(rolled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use assert_matches::assert_matches;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;

    fn user_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
        }
    }
    fn assistant_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
        }
    }

    #[test]
    fn drops_from_last_user_only() {
        let items = [
            user_msg("u1"),
            assistant_msg("a1"),
            assistant_msg("a2"),
            user_msg("u2"),
            assistant_msg("a3"),
            ResponseItem::Reasoning {
                id: "r1".to_string(),
                summary: vec![ReasoningItemReasoningSummary::SummaryText {
                    text: "s".to_string(),
                }],
                content: None,
                encrypted_content: None,
            },
            ResponseItem::FunctionCall {
                id: None,
                call_id: "c1".to_string(),
                name: "tool".to_string(),
                arguments: "{}".to_string(),
            },
            assistant_msg("a4"),
        ];

        let initial: Vec<RolloutItem> = items
            .iter()
            .cloned()
            .map(RolloutItem::ResponseItem)
            .collect();
        let truncated = truncate_before_nth_user_message(InitialHistory::Forked(initial), 1);
        let got_items = truncated.get_rollout_items();
        let expected_items = vec![
            RolloutItem::ResponseItem(items[0].clone()),
            RolloutItem::ResponseItem(items[1].clone()),
            RolloutItem::ResponseItem(items[2].clone()),
        ];
        assert_eq!(
            serde_json::to_value(&got_items).unwrap(),
            serde_json::to_value(&expected_items).unwrap()
        );

        let initial2: Vec<RolloutItem> = items
            .iter()
            .cloned()
            .map(RolloutItem::ResponseItem)
            .collect();
        let truncated2 = truncate_before_nth_user_message(InitialHistory::Forked(initial2), 2);
        assert_matches!(truncated2, InitialHistory::New);
    }

    #[tokio::test]
    async fn ignores_session_prefix_messages_when_truncating() {
        let (session, turn_context) = make_session_and_context().await;
        let mut items = session.build_initial_context(&turn_context);
        items.push(user_msg("feature request"));
        items.push(assistant_msg("ack"));
        items.push(user_msg("second question"));
        items.push(assistant_msg("answer"));

        let rollout_items: Vec<RolloutItem> = items
            .iter()
            .cloned()
            .map(RolloutItem::ResponseItem)
            .collect();

        let truncated = truncate_before_nth_user_message(InitialHistory::Forked(rollout_items), 1);
        let got_items = truncated.get_rollout_items();

        let expected: Vec<RolloutItem> = vec![
            RolloutItem::ResponseItem(items[0].clone()),
            RolloutItem::ResponseItem(items[1].clone()),
            RolloutItem::ResponseItem(items[2].clone()),
        ];

        assert_eq!(
            serde_json::to_value(&got_items).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }
}
