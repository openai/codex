//! Captures genuine sender instructions and assistant context for host-delivered task messages.
//! Only turn-input admission calls this, before queueing; ordinary tool results and
//! quoted delegation text cannot establish sender provenance. Unloaded senders are read
//! through the host's authenticated thread store, without resuming their runtime.

use crate::context::GuardianSenderExchange;
use crate::context::GuardianSenderMessages;
use crate::context_manager::ContextManager;
use codex_guardian_context::GuardianRootMessage;
use codex_history::RetainedContextEntry;
use codex_history::RetainedContextOrder;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_thread_store::LoadThreadHistoryParams;
use codex_utils_output_truncation::TruncationPolicy;
use std::time::Duration;

use super::LocalAgentRuntime;

impl LocalAgentRuntime {
    pub(crate) async fn capture_sender_user_messages(
        &self,
        item: &ResponseItem,
        receiver_thread_id: ThreadId,
    ) -> Option<GuardianSenderMessages> {
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
            from_storage: false,
            messages: Vec::new(),
        };
        if let Some(source_thread_id) = source_thread_id
            && let Ok(manager) = self.upgrade()
        {
            let context = match manager.get_thread(source_thread_id).await {
                Ok(sender) => sender
                    .conversation_history_snapshot()
                    .await
                    .retained_context()
                    .cloned(),
                Err(_) => {
                    // The store owns authorization, including cross-worker tenant boundaries.
                    // Bound the read so unavailable storage cannot stall turn admission.
                    let stored = tokio::time::timeout(
                        Duration::from_secs(/*secs*/ 5),
                        manager.load_latest_model_context(LoadThreadHistoryParams {
                            thread_id: source_thread_id,
                            include_archived: true,
                        }),
                    )
                    .await;
                    match stored {
                        Ok(Ok(stored)) if stored.thread_id == source_thread_id => {
                            let meta = stored.items.iter().find_map(|item| match item {
                                RolloutItem::SessionMeta(meta) => Some(&meta.meta),
                                _ => None,
                            });
                            meta.filter(|meta| {
                                meta.id == source_thread_id && !meta.source.is_internal()
                            })
                            .map(|meta| {
                                fragment.from_storage = true;
                                ContextManager::reconstruct_rollout(
                                    &stored.items,
                                    meta.history_mode,
                                    ContextManager::new(),
                                    // Tool bodies are not sender authorization evidence.
                                    TruncationPolicy::Bytes(0),
                                )
                                .retained_context
                            })
                        }
                        Ok(Ok(_)) | Ok(Err(_)) | Err(_) => None,
                    }
                }
            };
            if let Some(context) = context {
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
        Some(fragment)
    }
}
