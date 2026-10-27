//! Projects bounded retained root evidence for worker reviewers.
//! Retained root instructions stay authoritative while old checkpoints use legacy review.
//! Selects recent user and assistant evidence together by source order.
//! Explicit assistant commentary is excluded; confirmed messaging survives compaction.
//! Projection omissions keep missing authorization and assistant context explicit.
//! Retained-history reconciliation owns recovery order and missing-instruction provenance.
//! Known positions preserve host order, not delivery order or inferred question-answer pairs.
//! Unmatched legacy instructions supplement retained facts without claiming a known ordering.
//! Optional handoff filtering narrows assistant context using surviving calls as relevance boundaries.
//! Handoff projection preserves saved heartbeat instructions and the active turn's skills.

use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use super::LocalAgentControl;
use crate::codex_thread::GuardianRootMessage;
use crate::codex_thread::GuardianRootSnapshot;
use crate::compact::is_summary_message;
use crate::context::ContextualUserFragment;
use crate::context::GuardianReviewEvidence;
use crate::context::UserGoalUpdate;
use crate::context::is_contextual_user_fragment;
use crate::event_mapping::parse_turn_item;
use codex_guardian_context::RootAssistantCandidate;
use codex_guardian_context::RootAssistantContent;
use codex_guardian_context::RootConversationCandidates;
use codex_guardian_context::RootDeliveredMessage;
use codex_guardian_context::RootMessageCandidate;
use codex_guardian_context::RootRetainedAssistant;
use codex_history::ReconciledRetainedContext;
use codex_history::RetainedContextEntry;
use codex_history::RetainedContextOrder;
use codex_history::RetainedUserMessage;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::MultiAgentVersion;

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

        let worker_path = root_thread
            .enabled(codex_features::Feature::GuardianRootHandoffContext)
            .then(|| {
                self.runtime
                    .registry
                    .agent_metadata_for_thread(thread_id)?
                    .agent_path
            })
            .flatten();
        let root_history = root_thread.session.clone_history().await;
        let history = root_history.conversation_history_snapshot();
        // Join calls to host-confirmed outputs in this snapshot. Older outputs without
        // captured text cannot establish what was sent, including after a hook rewrite.
        let delivered_candidates = root_history
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
                Some(RootDeliveredMessage { call_id, text })
            })
            .collect::<Vec<_>>();
        let root_evidence = root_thread
            .session
            .services
            .thread_extension_data
            .get_or_init(GuardianReviewEvidence::default);
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
                    if is_summary_message(&text)
                        || text.trim_start().starts_with("<user_action>")
                        || is_contextual_user_fragment(&ContentItem::InputText {
                            text: text.clone(),
                        })
                    {
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
                            phase: None,
                            origin: codex_history::UserInputOrigin::from_message(item),
                            turn_id: item.turn_id().unwrap_or_default().to_owned(),
                            message_id: item.id().map(|id| id.as_str().to_owned()),
                            text,
                            complete: false,
                        },
                    ))
                }),
        );
        let mut latest_user_turn_id = reconciled.latest_user_turn_id.clone();
        let mut missing_root_instructions = reconciled.missing_user_messages;
        let user_candidates = reconciled
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
                    let is_goal_update = UserGoalUpdate::matches_text(&text);
                    // A retained placeholder cannot recover an omitted goal restriction.
                    missing_root_instructions |= is_goal_update && !message.complete;
                    if is_contextual_user_fragment(&ContentItem::InputText {
                        text: text.to_string(),
                    }) && !is_goal_update
                    {
                        return None;
                    }
                    (!is_summary_message(&text) && !text.trim_start().starts_with("<user_action>"))
                        .then(|| RootMessageCandidate {
                            order: Some(order),
                            message: GuardianRootMessage::User(text.to_string()),
                        })
                }
                RetainedContextEntry::VerifiedAnswer(answer) => Some(RootMessageCandidate {
                    order: Some(order),
                    message: GuardianRootMessage::UserInput(
                        codex_guardian_context::render_original_verified_answer(answer),
                    ),
                }),
                RetainedContextEntry::AssistantMessage(_) => None,
            })
            .collect::<Vec<_>>();
        let original_missing_root_instructions = missing_root_instructions;
        let retained_user_sources = reconciled
            .ordered_entries()
            .filter_map(|(_, entry)| match entry {
                RetainedContextEntry::UserMessage(message) => Some(message.clone()),
                RetainedContextEntry::AssistantMessage(_)
                | RetainedContextEntry::VerifiedAnswer(_) => None,
            })
            .collect();
        let assistant_candidates = root_history
            .annotated_items()
            .iter()
            .filter(|envelope| {
                !envelope
                    .metadata
                    .as_ref()
                    .is_some_and(|metadata| metadata.compaction_output)
            })
            .filter_map(|envelope| {
                let content = match &envelope.item {
                    ResponseItem::FunctionCall { call_id, .. } => {
                        RootAssistantContent::DeliveredCall {
                            call_id: call_id.clone(),
                        }
                    }
                    ResponseItem::Message {
                        role,
                        content,
                        phase,
                        ..
                    } if role == "assistant" && *phase != Some(MessagePhase::Commentary) => {
                        RootAssistantContent::Text(content)
                    }
                    _ => return None,
                };
                Some(RootAssistantCandidate {
                    message_id: envelope.item.id().map(|id| id.as_str().to_owned()),
                    order: envelope
                        .metadata
                        .as_ref()
                        .filter(|metadata| !metadata.inherited_user_message)
                        .and_then(|metadata| metadata.user_input_order)
                        .map(RetainedContextOrder::Local),
                    content,
                })
            })
            .collect::<Vec<_>>();
        let retained_assistant_candidates = reconciled
            .ordered_entries()
            .filter_map(|(order, entry)| {
                let RetainedContextEntry::AssistantMessage(message) = entry else {
                    return None;
                };
                let standalone_eligible = !(message.phase == Some(MessagePhase::Commentary)
                    || message.message_id.as_deref().is_some_and(|id| {
                        root_history
                            .raw_items()
                            .chain(root_history.guardian_history_items().into_iter().flatten())
                            .any(|item| {
                                matches!(item, ResponseItem::Message {
                                id: Some(source_id), phase: Some(MessagePhase::Commentary), ..
                            } if source_id.as_str() == id)
                            })
                    }));
                Some(RootRetainedAssistant {
                    order,
                    message,
                    standalone_eligible,
                })
            })
            .collect::<Vec<_>>();
        let handoff_boundaries = if let Some(worker) = worker_path {
            super::root_handoff::handoff_boundaries(
                root_history.annotated_items(),
                &worker,
                root_thread
                    .config()
                    .await
                    .multi_agent_v2
                    .tool_namespace
                    .as_deref(),
            )
        } else {
            Vec::new()
        };
        let request_candidates = RootConversationCandidates {
            user_messages: user_candidates,
            assistant_messages: assistant_candidates,
            retained_assistants: retained_assistant_candidates,
            delivered_messages: delivered_candidates,
            retained_user_sources,
            handoff_boundaries,
            missing_root_instructions: original_missing_root_instructions,
            missing_assistant_context: retained_context.has_omitted_assistant_messages(),
            missing_verified_answers: !retained_context.verified_answers_complete(),
        };
        let prepared = codex_guardian_context::prepare_root_context(
            &request_candidates,
            root_history.original_legacy_user_messages(),
        );
        let mut authorization_version = root_evidence.authorization_version(history.as_ref());
        if prepared.missing_root_instructions {
            authorization_version.retained_context_complete = false;
        }
        let mut review_context_revision = history.guardian_review_context_revision();
        if let Some(selected) = &prepared.handoff_selection {
            let mut revision = DefaultHasher::new();
            review_context_revision.hash(&mut revision);
            selected.hash(&mut revision);
            review_context_revision = revision.finish();
        }
        if latest_user_turn_id.is_none() {
            latest_user_turn_id = prepared.latest_legacy_turn_id;
        }
        let trusted_skill_paths = latest_user_turn_id
            .as_deref()
            .map(|turn_id| root_evidence.trusted_skill_paths(turn_id))
            .unwrap_or_default();
        Some(GuardianRootSnapshot {
            root_thread_id,
            history_reset_version: root_history.reset_version,
            authorization_version,
            review_context_revision,
            messages: prepared.messages,
            trusted_skill_paths,
        })
    }
}
