use crate::AuthManager;
#[cfg(any(test, feature = "test-support"))]
use crate::CodexAuth;
#[cfg(any(test, feature = "test-support"))]
use crate::ModelProviderInfo;
use crate::codex::Codex;
use crate::codex::CodexSpawnOk;
use crate::codex::INITIAL_SUBMIT_ID;
use crate::codex_conversation::CodexConversation;
use crate::config::Config;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::history_truncation;
use crate::models_manager::manager::ModelsManager;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::SessionConfiguredEvent;
use crate::rollout::RolloutRecorder;
use crate::skills::SkillsManager;
use codex_protocol::ConversationId;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::protocol::InitialHistory;
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
    conversations: Arc<RwLock<HashMap<ConversationId, Arc<CodexConversation>>>>,
    auth_manager: Arc<AuthManager>,
    models_manager: Arc<ModelsManager>,
    skills_manager: Arc<SkillsManager>,
    session_source: SessionSource,
    #[cfg(any(test, feature = "test-support"))]
    _test_codex_home_guard: Option<TempDir>,
}

impl ConversationManager {
    pub fn new(auth_manager: Arc<AuthManager>, session_source: SessionSource) -> Self {
        let skills_manager = Arc::new(SkillsManager::new(auth_manager.codex_home().to_path_buf()));
        Self {
            conversations: Arc::new(RwLock::new(HashMap::new())),
            auth_manager: auth_manager.clone(),
            session_source,
            models_manager: Arc::new(ModelsManager::new(auth_manager)),
            skills_manager,
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
        let auth_manager = crate::AuthManager::from_auth_for_testing_with_home(auth, codex_home);
        let skills_manager = Arc::new(SkillsManager::new(auth_manager.codex_home().to_path_buf()));
        Self {
            conversations: Arc::new(RwLock::new(HashMap::new())),
            auth_manager: auth_manager.clone(),
            session_source: SessionSource::Exec,
            models_manager: Arc::new(ModelsManager::with_provider(auth_manager, provider)),
            skills_manager,
            _test_codex_home_guard: None,
        }
    }

    pub fn session_source(&self) -> SessionSource {
        self.session_source.clone()
    }

    pub fn skills_manager(&self) -> Arc<SkillsManager> {
        self.skills_manager.clone()
    }

    pub async fn new_conversation(&self, config: Config) -> CodexResult<NewConversation> {
        self.spawn_conversation(
            config,
            self.auth_manager.clone(),
            self.models_manager.clone(),
        )
        .await
    }

    async fn spawn_conversation(
        &self,
        config: Config,
        auth_manager: Arc<AuthManager>,
        models_manager: Arc<ModelsManager>,
    ) -> CodexResult<NewConversation> {
        let CodexSpawnOk {
            codex,
            conversation_id,
        } = Codex::spawn(
            config,
            auth_manager,
            models_manager,
            self.skills_manager.clone(),
            InitialHistory::New,
            self.session_source.clone(),
        )
        .await?;
        self.finalize_spawn(codex, conversation_id).await
    }

    async fn finalize_spawn(
        &self,
        codex: Codex,
        conversation_id: ConversationId,
    ) -> CodexResult<NewConversation> {
        // The first event must be `SessionInitialized`. Validate and forward it
        // to the caller so that they can display it in the conversation
        // history.
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

    pub async fn get_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> CodexResult<Arc<CodexConversation>> {
        let conversations = self.conversations.read().await;
        conversations
            .get(&conversation_id)
            .cloned()
            .ok_or_else(|| CodexErr::ConversationNotFound(conversation_id))
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
        let CodexSpawnOk {
            codex,
            conversation_id,
        } = Codex::spawn(
            config,
            auth_manager,
            self.models_manager.clone(),
            self.skills_manager.clone(),
            initial_history,
            self.session_source.clone(),
        )
        .await?;
        self.finalize_spawn(codex, conversation_id).await
    }

    /// Removes the conversation from the manager's internal map, though the
    /// conversation is stored as `Arc<CodexConversation>`, it is possible that
    /// other references to it exist elsewhere. Returns the conversation if the
    /// conversation was found and removed.
    pub async fn remove_conversation(
        &self,
        conversation_id: &ConversationId,
    ) -> Option<Arc<CodexConversation>> {
        self.conversations.write().await.remove(conversation_id)
    }

    /// Fork an existing conversation by taking messages up to the given position
    /// (not including the message at the given position) and starting a new
    /// conversation with identical configuration (unless overridden by the
    /// caller's `config`). The new conversation will have a fresh id.
    pub async fn fork_conversation(
        &self,
        nth_user_message: usize,
        config: Config,
        path: PathBuf,
    ) -> CodexResult<NewConversation> {
        // Compute the prefix up to the cut point.
        let history = RolloutRecorder::get_rollout_history(&path).await?;
        let rollout_items = history.get_rollout_items();
        let truncated = history_truncation::truncate_rollout_before_nth_user_message_from_start(
            &rollout_items,
            nth_user_message,
        );
        let history = if truncated.is_empty() {
            InitialHistory::New
        } else {
            InitialHistory::Forked(truncated)
        };

        // Spawn a new conversation with the computed initial history.
        let auth_manager = self.auth_manager.clone();
        let CodexSpawnOk {
            codex,
            conversation_id,
        } = Codex::spawn(
            config,
            auth_manager,
            self.models_manager.clone(),
            self.skills_manager.clone(),
            history,
            self.session_source.clone(),
        )
        .await?;

        self.finalize_spawn(codex, conversation_id).await
    }

    pub async fn list_models(&self, config: &Config) -> Vec<ModelPreset> {
        self.models_manager.list_models(config).await
    }

    pub fn get_models_manager(&self) -> Arc<ModelsManager> {
        self.models_manager.clone()
    }
}
