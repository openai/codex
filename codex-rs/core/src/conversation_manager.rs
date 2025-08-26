use std::collections::HashMap;
use std::sync::Arc;

use codex_login::AuthManager;
use codex_login::CodexAuth;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::codex::Codex;
use crate::codex::CodexSpawnOk;
use crate::codex::INITIAL_SUBMIT_ID;
use crate::codex_conversation::CodexConversation;
use crate::config::Config;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::SessionConfiguredEvent;
use codex_protocol::models::ResponseItem;

/// Represents a newly created Codex conversation, including the first event
/// (which is [`EventMsg::SessionConfigured`]).
pub struct NewConversation {
    pub conversation_id: Uuid,
    pub conversation: Arc<CodexConversation>,
    pub session_configured: SessionConfiguredEvent,
}

/// [`ConversationManager`] is responsible for creating conversations and
/// maintaining them in memory.
pub struct ConversationManager {
    conversations: Arc<RwLock<HashMap<Uuid, Arc<CodexConversation>>>>,
    auth_manager: Arc<AuthManager>,
}

impl ConversationManager {
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self {
            conversations: Arc::new(RwLock::new(HashMap::new())),
            auth_manager,
        }
    }

    /// Construct with a dummy AuthManager containing the provided CodexAuth.
    /// Used for integration tests: should not be used by ordinary business logic.
    pub fn with_auth(auth: CodexAuth) -> Self {
        Self::new(codex_login::AuthManager::from_auth_for_testing(auth))
    }

    pub async fn new_conversation(&self, config: Config) -> CodexResult<NewConversation> {
        self.spawn_conversation(config, self.auth_manager.clone())
            .await
    }

    async fn spawn_conversation(
        &self,
        config: Config,
        auth_manager: Arc<AuthManager>,
    ) -> CodexResult<NewConversation> {
        let CodexSpawnOk {
            codex,
            session_id: conversation_id,
        } = {
            let initial_history = None;
            Codex::spawn(config, auth_manager, initial_history).await?
        };
        self.finalize_spawn(codex, conversation_id).await
    }

    async fn finalize_spawn(
        &self,
        codex: Codex,
        conversation_id: Uuid,
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
        conversation_id: Uuid,
    ) -> CodexResult<Arc<CodexConversation>> {
        let conversations = self.conversations.read().await;
        conversations
            .get(&conversation_id)
            .cloned()
            .ok_or_else(|| CodexErr::ConversationNotFound(conversation_id))
    }

    /// Fork an existing conversation by dropping the last `drop_last_messages`
    /// user/assistant messages from its transcript and starting a new
    /// conversation with identical configuration (unless overridden by the
    /// caller's `config`). The new conversation will have a fresh id.
    pub async fn fork_conversation(
        &self,
        conversation_history: Vec<ResponseItem>,
        num_messages_to_drop: usize,
        config: Config,
    ) -> CodexResult<NewConversation> {
        // Compute the prefix up to the cut point.
        let truncated_history =
            truncate_after_dropping_last_messages(conversation_history, num_messages_to_drop);

        // Spawn a new conversation with the computed initial history.
        let auth_manager = self.auth_manager.clone();
        let CodexSpawnOk {
            codex,
            session_id: conversation_id,
        } = Codex::spawn(config, auth_manager, Some(truncated_history)).await?;

        self.finalize_spawn(codex, conversation_id).await
    }
}

/// Return a prefix of `items` obtained by dropping the last `n` user messages
/// and all items that follow them.
fn truncate_after_dropping_last_messages(items: Vec<ResponseItem>, n: usize) -> Vec<ResponseItem> {
    if n == 0 || items.is_empty() {
        return items;
    }

    // Walk backwards counting only `user` Message items, find cut index.
    let mut count = 0usize;
    let mut cut_index = 0usize;
    for (idx, item) in items.iter().enumerate().rev() {
        if let ResponseItem::Message { role, .. } = item
            && role == "user"
        {
            count += 1;
            if count == n {
                // Cut everything from this user message to the end.
                cut_index = idx;
                break;
            }
        }
    }
    if count < n {
        // If fewer than n messages exist, drop everything.
        Vec::new()
    } else {
        items.into_iter().take(cut_index).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ReasoningItemReasoningSummary;
    use codex_protocol::models::ResponseItem;

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
        let items = vec![
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

        let truncated = truncate_after_dropping_last_messages(items.clone(), 1);
        assert_eq!(
            truncated,
            vec![items[0].clone(), items[1].clone(), items[2].clone()]
        );

        let truncated2 = truncate_after_dropping_last_messages(items, 2);
        assert!(truncated2.is_empty());
    }
}
