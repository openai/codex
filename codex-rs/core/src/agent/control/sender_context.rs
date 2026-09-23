//! Captures genuine sender instructions for host-delivered task messages.
//! Only turn-input admission calls this, before queueing; ordinary tool results and
//! quoted delegation text cannot establish sender provenance. Lookup stays in this host.

use crate::context::ContextualUserFragment;
use crate::context::GuardianSenderMessages;
use codex_history::RetainedContextEntry;
use codex_history::RetainedContextOrder;
use codex_history::SenderUserMessages;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;

use super::LocalAgentRuntime;

impl LocalAgentRuntime {
    pub(crate) async fn capture_sender_user_messages(
        &self,
        item: &ResponseItem,
        receiver_thread_id: ThreadId,
        receiver_turn_id: &str,
    ) -> Option<SenderUserMessages> {
        let ResponseItem::FunctionCallOutput {
            id: Some(id),
            call_id: None,
            name: Some(name),
            namespace: Some(namespace),
            output,
            ..
        } = item
        else {
            return None;
        };
        if !matches!(namespace.as_str(), "codex_app" | "codex_tui")
            || name != "send_message_to_thread"
        {
            return None;
        }
        // Recognized deliveries always get their own snapshot, even without usable provenance.
        let source_thread_id = output.body.to_text().and_then(|text| {
            let (source, input) = text
                .strip_prefix("<codex_delegation>\n  <source_thread_id>")?
                .split_once("</source_thread_id>\n  <input>")?;
            input.strip_suffix("</input>\n</codex_delegation>")?;
            ThreadId::from_string(source)
                .ok()
                .filter(|source| *source != receiver_thread_id)
        });
        let mut fragment = GuardianSenderMessages {
            source: source_thread_id,
            delivery: id.to_string(),
            messages: Vec::new(),
        };
        if let Some(source_thread_id) = source_thread_id
            && let Ok(manager) = self.upgrade()
            && let Ok(sender) = manager.get_thread(source_thread_id).await
        {
            let history = sender.conversation_history_snapshot().await;
            if let Some(context) = history.retained_context() {
                fragment.messages = context
                    .ordered_entries()
                    .filter_map(|(order, entry)| match (order, entry) {
                        (
                            RetainedContextOrder::Local(_),
                            RetainedContextEntry::UserMessage(message),
                        ) => Some(message.complete.then(|| message.text.clone())),
                        (RetainedContextOrder::Inherited(_), _)
                        | (_, RetainedContextEntry::AssistantMessage(_))
                        | (_, RetainedContextEntry::VerifiedAnswer(_)) => None,
                    })
                    .rev()
                    .take(/*n*/ 3)
                    .collect();
                fragment.messages.reverse();
            }
        }
        let mut snapshot = SenderUserMessages {
            receiver_turn_id: receiver_turn_id.to_owned(),
            receiver_message_id: id.to_string(),
            text: fragment.render(),
        };
        snapshot.bound();
        Some(snapshot)
    }
}
