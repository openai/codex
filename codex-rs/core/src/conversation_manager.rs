use crate::AuthManager;
use crate::CodexAuth;
use crate::codex::Codex;
use crate::codex::CodexSpawnOk;
use crate::codex::INITIAL_SUBMIT_ID;
use crate::codex::compact::content_items_to_text;
use crate::codex::compact::is_session_prefix_message;
use crate::codex_conversation::CodexConversation;
use crate::config::Config;
use crate::cross_session::CrossSessionError;
use crate::cross_session::CrossSessionHub;
use crate::cross_session::RegisteredSession;
use crate::cross_session::SessionDefaults;
use crate::cross_session::SessionRegistration;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::Op;
use crate::protocol::SessionConfiguredEvent;
use crate::rollout::RolloutRecorder;
use codex_protocol::ConversationId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

/// Represents a newly created Codex conversation, including the first event
/// (which is [`EventMsg::SessionConfigured`]).
pub struct NewConversation {
    pub conversation_id: ConversationId,
    pub conversation: Arc<CodexConversation>,
    pub session_configured: SessionConfiguredEvent,
}

pub struct CrossSessionSpawnParams {
    pub hub: Arc<CrossSessionHub>,
    pub run_id: Option<String>,
    pub role: Option<String>,
}

/// [`ConversationManager`] is responsible for creating conversations and
/// maintaining them in memory.
pub struct ConversationManager {
    conversations: Arc<RwLock<HashMap<ConversationId, Arc<CodexConversation>>>>,
    cross_session_registrations: Arc<RwLock<HashMap<ConversationId, RegisteredSession>>>,
    auth_manager: Arc<AuthManager>,
    session_source: SessionSource,
}

impl ConversationManager {
    pub fn new(auth_manager: Arc<AuthManager>, session_source: SessionSource) -> Self {
        Self {
            conversations: Arc::new(RwLock::new(HashMap::new())),
            cross_session_registrations: Arc::new(RwLock::new(HashMap::new())),
            auth_manager,
            session_source,
        }
    }

    /// Construct with a dummy AuthManager containing the provided CodexAuth.
    /// Used for integration tests: should not be used by ordinary business logic.
    pub fn with_auth(auth: CodexAuth) -> Self {
        Self::new(
            crate::AuthManager::from_auth_for_testing(auth),
            SessionSource::Exec,
        )
    }

    pub async fn new_conversation(&self, config: Config) -> CodexResult<NewConversation> {
        self.spawn_conversation_with_history(
            config,
            self.auth_manager.clone(),
            InitialHistory::New,
            None,
        )
        .await
    }

    pub async fn new_conversation_with_cross_session(
        &self,
        config: Config,
        params: CrossSessionSpawnParams,
    ) -> CodexResult<NewConversation> {
        self.spawn_conversation_with_history(
            config,
            self.auth_manager.clone(),
            InitialHistory::New,
            Some(params),
        )
        .await
    }

    async fn spawn_conversation_with_history(
        &self,
        config: Config,
        auth_manager: Arc<AuthManager>,
        initial_history: InitialHistory,
        cross_session: Option<CrossSessionSpawnParams>,
    ) -> CodexResult<NewConversation> {
        let cross_session =
            cross_session.map(|params| (SessionDefaults::from_config(&config), params));

        let CodexSpawnOk {
            codex,
            conversation_id,
        } = Codex::spawn(config, auth_manager, initial_history, self.session_source).await?;

        let new_conversation = self.finalize_spawn(codex, conversation_id).await?;

        if let Some((defaults, params)) = cross_session
            && let Err(err) = self
                .register_cross_session(
                    conversation_id,
                    defaults,
                    params,
                    Arc::clone(&new_conversation.conversation),
                )
                .await
        {
            self.abort_conversation(conversation_id, Arc::clone(&new_conversation.conversation))
                .await;
            return Err(CodexErr::Fatal(format!(
                "failed to register cross-session for conversation {conversation_id}: {err}"
            )));
        }

        Ok(new_conversation)
    }

    async fn register_cross_session(
        &self,
        conversation_id: ConversationId,
        defaults: SessionDefaults,
        params: CrossSessionSpawnParams,
        conversation: Arc<CodexConversation>,
    ) -> Result<(), CrossSessionError> {
        let CrossSessionSpawnParams { hub, run_id, role } = params;

        let registration = SessionRegistration {
            conversation_id,
            conversation,
            defaults,
            run_id,
            role,
        };

        let guard = hub.register_session(registration)?;
        self.cross_session_registrations
            .write()
            .await
            .insert(conversation_id, guard);
        Ok(())
    }

    async fn abort_conversation(
        &self,
        conversation_id: ConversationId,
        conversation: Arc<CodexConversation>,
    ) {
        let _ = self.remove_conversation(&conversation_id).await;
        if let Err(err) = conversation.submit(Op::Shutdown).await {
            warn!(
                %conversation_id,
                ?err,
                "failed to shutdown conversation after cross-session registration error"
            );
        }
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

        let conversation = Arc::new(CodexConversation::new(codex));
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
        self.spawn_conversation_with_history(config, auth_manager, initial_history, None)
            .await
    }

    pub async fn resume_conversation_from_rollout_with_cross_session(
        &self,
        config: Config,
        rollout_path: PathBuf,
        auth_manager: Arc<AuthManager>,
        params: CrossSessionSpawnParams,
    ) -> CodexResult<NewConversation> {
        let initial_history = RolloutRecorder::get_rollout_history(&rollout_path).await?;
        self.spawn_conversation_with_history(config, auth_manager, initial_history, Some(params))
            .await
    }

    pub async fn resume_conversation_with_cross_session(
        &self,
        config: Config,
        rollout_path: PathBuf,
        params: CrossSessionSpawnParams,
    ) -> CodexResult<NewConversation> {
        self.resume_conversation_from_rollout_with_cross_session(
            config,
            rollout_path,
            self.auth_manager.clone(),
            params,
        )
        .await
    }

    /// Removes the conversation from the manager's internal map, though the
    /// conversation is stored as `Arc<CodexConversation>`, it is possible that
    /// other references to it exist elsewhere. Returns the conversation if the
    /// conversation was found and removed.
    pub async fn remove_conversation(
        &self,
        conversation_id: &ConversationId,
    ) -> Option<Arc<CodexConversation>> {
        self.cross_session_registrations
            .write()
            .await
            .remove(conversation_id);
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
        let history = truncate_before_nth_user_message(history, nth_user_message);

        // Spawn a new conversation with the computed initial history.
        let auth_manager = self.auth_manager.clone();
        self.spawn_conversation_with_history(config, auth_manager, history, None)
            .await
    }

    pub async fn fork_conversation_with_cross_session(
        &self,
        nth_user_message: usize,
        config: Config,
        path: PathBuf,
        params: CrossSessionSpawnParams,
    ) -> CodexResult<NewConversation> {
        let history = RolloutRecorder::get_rollout_history(&path).await?;
        let history = truncate_before_nth_user_message(history, nth_user_message);

        let auth_manager = self.auth_manager.clone();
        self.spawn_conversation_with_history(config, auth_manager, history, Some(params))
            .await
    }
}

/// Return a prefix of `items` obtained by cutting strictly before the nth user message
/// (0-based) and all items that follow it.
fn truncate_before_nth_user_message(history: InitialHistory, n: usize) -> InitialHistory {
    // Work directly on rollout items, and cut the vector at the nth user message input.
    let items: Vec<RolloutItem> = history.get_rollout_items();

    // Find indices of user message inputs in rollout order.
    let mut user_positions: Vec<usize> = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) = item
            && role == "user"
            && content_items_to_text(content).is_some_and(|text| !is_session_prefix_message(&text))
        {
            user_positions.push(idx);
        }
    }

    // If fewer than or equal to n user messages exist, treat as empty (out of range).
    if user_positions.len() <= n {
        return InitialHistory::New;
    }

    // Cut strictly before the nth user message (do not keep the nth itself).
    let cut_idx = user_positions[n];
    let rolled: Vec<RolloutItem> = items.into_iter().take(cut_idx).collect();

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
                name: "tool".to_string(),
                arguments: "{}".to_string(),
                call_id: "c1".to_string(),
            },
            assistant_msg("a4"),
        ];

        // Wrap as InitialHistory::Forked with response items only.
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

    #[test]
    fn ignores_session_prefix_messages_when_truncating() {
        let (session, turn_context) = make_session_and_context();
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
