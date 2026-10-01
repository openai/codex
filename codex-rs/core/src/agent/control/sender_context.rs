//! Captures genuine sender instructions and assistant context for host-delivered task messages.
//! Only turn-input admission calls this, before queueing; ordinary tool results and
//! quoted delegation text cannot establish sender provenance. Lookup stays in this host.

use crate::context::ContextualUserFragment;
use crate::context::GuardianSenderExchange;
use crate::context::GuardianSenderMessages;
use codex_guardian_context::GuardianRootMessage;
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
        if !matches!(
            (namespace.as_str(), name.as_str()),
            ("codex_app" | "codex_tui", "send_message_to_thread")
                | ("cloud_threads", "send_message")
        ) {
            return None;
        }
        // Recognized deliveries always get their own snapshot, even without usable provenance.
        let source_thread_id = output.body.to_text().and_then(|text| {
            let (source, input) = text
                .strip_prefix("<codex_delegation>")?
                .strip_suffix("</codex_delegation>")?
                .trim()
                .strip_prefix("<source_thread_id>")?
                .split_once("</source_thread_id>")?;
            input
                .trim()
                .strip_prefix("<input>")?
                .strip_suffix("</input>")?;
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
                let mut exchanges = Vec::new();
                let mut assistant = None;
                for (order, entry) in context.ordered_entries() {
                    match (order, entry) {
                        (
                            RetainedContextOrder::Local(_),
                            RetainedContextEntry::UserMessage(message),
                        ) => exchanges.push((message, assistant.take())),
                        (
                            RetainedContextOrder::Local(_),
                            RetainedContextEntry::AssistantMessage(message),
                        ) => assistant = Some(message),
                        (RetainedContextOrder::Inherited(_), _)
                        | (_, RetainedContextEntry::VerifiedAnswer(_)) => {}
                    }
                }
                fragment.messages = exchanges
                    .into_iter()
                    .rev()
                    .take(/*n*/ 3)
                    .map(|(user, assistant)| GuardianSenderExchange {
                        user: user.complete.then(|| user.text.clone()),
                        assistant: assistant.map(|message| {
                            if message.complete {
                                GuardianRootMessage::Assistant(message.text.clone())
                            } else {
                                GuardianRootMessage::IncompleteAssistantContext
                            }
                        }),
                    })
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
