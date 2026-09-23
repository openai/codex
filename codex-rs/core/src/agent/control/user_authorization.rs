//! Projects bounded retained root evidence for worker reviewers.
//! Retained root instructions stay authoritative while old checkpoints use legacy review.
//! User evidence wins, then assistant context preceding ordinary replies, then other live evidence.
//! Recovery includes commentary and confirmed messaging context.
//! Projection limits do not change authorization completeness; unavailable source text does.
//! Retained-history reconciliation owns recovery order and missing-instruction provenance.
//! Known positions preserve host order, not delivery order or inferred question-answer pairs.
//! Unmatched legacy instructions supplement retained facts without claiming a known ordering.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::VecDeque;

use super::LocalAgentControl;
use crate::codex_thread::GuardianRootMessage;
use crate::codex_thread::GuardianRootSnapshot;
use crate::compact::is_summary_message;
use crate::context::GuardianReviewEvidence;
use crate::context::is_contextual_user_fragment;
use crate::event_mapping::parse_turn_item;
use crate::guardian::GUARDIAN_MAX_ROOT_MESSAGE_TOKENS;
use crate::guardian::guardian_truncate_text;
use codex_history::ReconciledRetainedContext;
use codex_history::RetainedContextEntry;
use codex_history::RetainedContextOrder;
use codex_history::RetainedUserMessage;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::MultiAgentVersion;

const MAX_ROOT_MESSAGES: usize = 8;

impl LocalAgentControl {
    /// Returns bounded root conversation and authorization state for a MultiAgent V2 worker.
    pub(crate) async fn root_user_authorization(
        &self,
        thread_id: ThreadId,
    ) -> Option<GuardianRootSnapshot> {
        let root_thread_id = self
            .runtime
            .registry
            .agent_id_for_path(&AgentPath::root())?;
        if root_thread_id == thread_id {
            return None;
        }
        let manager = self.runtime.upgrade().ok()?;
        let root_thread = manager.get_thread(root_thread_id).await.ok()?;
        if root_thread.multi_agent_version() != Some(MultiAgentVersion::V2) {
            return None;
        }

        let root_history = root_thread.session.clone_history().await;
        let history = root_history.conversation_history_snapshot();
        // Join calls to host-confirmed outputs in this snapshot. Older outputs without
        // captured text cannot establish what was sent, including after a hook rewrite.
        let delivered_messages = root_history
            .annotated_items()
            .iter()
            .rev()
            .filter_map(|envelope| {
                let ResponseItem::FunctionCallOutput {
                    call_id: Some(call_id),
                    ..
                } = &envelope.item
                else {
                    return None;
                };
                let text = envelope
                    .metadata
                    .as_ref()?
                    .delivered_assistant_message
                    .as_deref()?;
                Some((call_id.as_str(), text))
            })
            .take(MAX_ROOT_MESSAGES)
            .collect::<HashMap<_, _>>();
        let root_evidence = root_thread
            .session
            .services
            .thread_extension_data
            .get_or_init(GuardianReviewEvidence::default);
        let mut latest_user_turn_id = None;
        let retained_context = root_history.retained_context();
        let reconciled = ReconciledRetainedContext::new(
            Some(retained_context),
            root_history
                .annotated_items()
                .iter()
                .filter_map(|envelope| {
                    let item = &envelope.item;
                    let Some(TurnItem::UserMessage(message)) = parse_turn_item(item) else {
                        return None;
                    };
                    let text = message.message();
                    if is_summary_message(&text) || text.trim_start().starts_with("<user_action>") {
                        return None;
                    }
                    let order = envelope
                        .metadata
                        .as_ref()
                        .filter(|metadata| !metadata.inherited_user_message)
                        .and_then(|metadata| metadata.user_input_order);
                    Some((
                        order,
                        RetainedUserMessage {
                            turn_id: item.turn_id().unwrap_or_default().to_owned(),
                            message_id: item.id().map(|id| id.as_str().to_owned()),
                            text,
                            complete: false,
                        },
                    ))
                }),
        );
        let mut missing_root_instructions = reconciled.missing_user_messages;
        let mut messages = reconciled
            .ordered_entries()
            .filter_map(|(order, entry)| match entry {
                RetainedContextEntry::UserMessage(message) => {
                    let text = if message.text.is_empty() && !message.complete {
                        // Older records may omit a large instruction. Recover that exact
                        // source while it remains available in the parent context.
                        let original = message.message_id.as_deref().and_then(|id| {
                            root_history
                                .raw_items()
                                .chain(root_history.guardian_history_items().into_iter().flatten())
                                .find(|item| {
                                    item.id().is_some_and(|item_id| item_id.as_str() == id)
                                })
                        });
                        let Some(TurnItem::UserMessage(original)) =
                            original.and_then(parse_turn_item)
                        else {
                            missing_root_instructions = true;
                            return None;
                        };
                        Cow::Owned(original.message())
                    } else {
                        Cow::Borrowed(message.text.as_str())
                    };
                    if is_contextual_user_fragment(&ContentItem::InputText {
                        text: text.to_string(),
                    }) {
                        return None;
                    }
                    (!is_summary_message(&text) && !text.trim_start().starts_with("<user_action>"))
                        .then(|| {
                            latest_user_turn_id = Some(message.turn_id.clone());
                            (
                                Some(order),
                                GuardianRootMessage::User(
                                    guardian_truncate_text(&text, GUARDIAN_MAX_ROOT_MESSAGE_TOKENS)
                                        .0,
                                ),
                            )
                        })
                }
                RetainedContextEntry::VerifiedAnswer(answer) => {
                    codex_guardian_context::render_verified_answer(answer)
                        .map(|text| (Some(order), GuardianRootMessage::UserInput(text)))
                }
                RetainedContextEntry::AssistantMessage(_) => None,
            })
            .collect::<Vec<_>>();
        messages.drain(..messages.len().saturating_sub(MAX_ROOT_MESSAGES));
        // Old opt-out checkpoints have genuine instructions but no retained source order.
        // Keep them separately bounded and explicitly unordered; never replace newer facts.
        let mut legacy_messages = VecDeque::new();
        let legacy_capacity = MAX_ROOT_MESSAGES.saturating_sub(messages.len());
        if missing_root_instructions && legacy_capacity > 0 {
            let mut latest_legacy_turn_id = None;
            for message in reconciled.unmatched_user_messages(root_history.legacy_user_messages()) {
                latest_legacy_turn_id = (!message.turn_id.is_empty()).then_some(message.turn_id);
                legacy_messages.push_back(GuardianRootMessage::User(message.text));
                if legacy_messages.len() > legacy_capacity {
                    legacy_messages.pop_front();
                }
            }
            if latest_user_turn_id.is_none() {
                latest_user_turn_id = latest_legacy_turn_id;
            }
        }
        let mut missing_assistant_context = retained_context.has_omitted_assistant_messages();
        // Prefer a renderable retained original over a shortened checkpoint copy.
        // Otherwise preserve the bounded live evidence, including confirmed messaging sends.
        let live_assistant_messages = root_history
            .annotated_items()
            .iter()
            .filter(|envelope| {
                !envelope
                    .metadata
                    .as_ref()
                    .is_some_and(|metadata| metadata.compaction_output)
            })
            .filter_map(|envelope| {
                let text = root_assistant_text(&envelope.item, &delivered_messages)?;
                let id = envelope
                    .item
                    .id()
                    .map(codex_protocol::ResponseItemId::as_str);
                let retained = reconciled.ordered_entries().find_map(|(order, entry)| {
                    let RetainedContextEntry::AssistantMessage(message) = entry else {
                        return None;
                    };
                    id.filter(|id| message.message_id.as_deref() == Some(*id))
                        .map(|_| (order, message))
                });
                if let Some((order, message)) = retained
                    && let Some(message) =
                        codex_guardian_context::retained_assistant_message(message)
                {
                    return Some((id, Some(order), message));
                }
                let order = envelope
                    .metadata
                    .as_ref()
                    .filter(|metadata| !metadata.inherited_user_message)
                    .and_then(|metadata| metadata.user_input_order)
                    .map(RetainedContextOrder::Local);
                let order = retained.map(|(order, _)| order).or(order);
                let message = if order.is_some() {
                    GuardianRootMessage::Assistant(text)
                } else {
                    GuardianRootMessage::UnorderedAssistant(text)
                };
                Some((id, order, message))
            })
            .collect::<Vec<_>>();
        let mut assistant_messages = reconciled
            .ordered_entries()
            .filter_map(|(order, entry)| {
                let RetainedContextEntry::AssistantMessage(message) = entry else {
                    return None;
                };
                if message.message_id.as_deref().is_some_and(|id| {
                    live_assistant_messages
                        .iter()
                        .any(|(live_id, _, _)| *live_id == Some(id))
                }) {
                    return None;
                }
                let rendered = codex_guardian_context::retained_assistant_message(message);
                missing_assistant_context |= rendered.is_none();
                rendered.map(|message| (Some(order), message))
            })
            .collect::<Vec<_>>();
        assistant_messages.extend(
            live_assistant_messages
                .into_iter()
                .map(|(_, order, message)| (order, message)),
        );
        // Keep the nearest known assistant context for each ordinary reply. Verified
        // answers already include their questions; unsequenced sources cannot be paired.
        let reply_context_orders = messages
            .iter()
            .filter_map(|(user_order, message)| {
                let (Some(user_order), GuardianRootMessage::User(_)) = (user_order, message) else {
                    return None;
                };
                assistant_messages
                    .iter()
                    .filter_map(|(order, _)| *order)
                    .filter(|order| order < user_order)
                    .max()
            })
            .collect::<Vec<_>>();
        // Stable sorting keeps other live evidence ahead of retained-only extras.
        assistant_messages
            .sort_by_key(|(order, _)| order.filter(|order| reply_context_orders.contains(order)));
        let available = MAX_ROOT_MESSAGES.saturating_sub(messages.len() + legacy_messages.len());
        missing_assistant_context |= assistant_messages.len() > available;
        assistant_messages.drain(..assistant_messages.len().saturating_sub(available));
        messages.extend(assistant_messages);
        messages.sort_by_key(|(order, _)| (order.is_none(), *order));
        let mut messages = messages
            .into_iter()
            .map(|(_, message)| message)
            .collect::<Vec<_>>();
        if missing_assistant_context {
            messages.insert(
                /*index*/ 0,
                GuardianRootMessage::IncompleteAssistantContext,
            );
        }
        let mut authorization_version = root_evidence.authorization_version(history.as_ref());
        if !authorization_version.retained_context_complete {
            messages.insert(
                /*index*/ 0,
                GuardianRootMessage::IncompleteVerifiedAnswers,
            );
        }
        if missing_root_instructions {
            authorization_version.retained_context_complete = false;
            messages.insert(
                /*index*/ 0,
                GuardianRootMessage::IncompleteRootInstructions,
            );
        }
        messages.insert(/*index*/ 0, GuardianRootMessage::RetainedContextScope);
        if !legacy_messages.is_empty() {
            legacy_messages.push_front(GuardianRootMessage::LegacyContextScope);
            legacy_messages.extend(messages);
            messages = legacy_messages.into_iter().collect();
        }
        let trusted_skill_paths = latest_user_turn_id
            .as_deref()
            .map(|turn_id| root_evidence.trusted_skill_paths(turn_id))
            .unwrap_or_default();
        Some(GuardianRootSnapshot {
            root_thread_id,
            authorization_version,
            messages,
            trusted_skill_paths,
        })
    }
}

fn root_assistant_text(
    item: &ResponseItem,
    delivered_messages: &HashMap<&str, &str>,
) -> Option<String> {
    let text = match item {
        ResponseItem::FunctionCall { call_id, .. } => {
            (*delivered_messages.get(call_id.as_str())?).to_owned()
        }
        _ => {
            let Some(TurnItem::AgentMessage(message)) = parse_turn_item(item) else {
                return None;
            };
            message
                .content
                .iter()
                .map(|content| match content {
                    AgentMessageContent::Text { text } => text.as_str(),
                })
                .collect::<String>()
        }
    };
    Some(guardian_truncate_text(&text, GUARDIAN_MAX_ROOT_MESSAGE_TOKENS).0)
}
