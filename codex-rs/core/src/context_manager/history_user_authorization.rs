//! Captures original user/assistant exchanges and adopts copied context when a worker becomes a root.
//! Inherited history remains excluded for workers; roots also recover it from checkpoints.
//! Oversized originals retain bounded, incomplete excerpts for root review.
//! Checkpoint copies cannot establish original completeness, even when their text is short.

use std::sync::Arc;

use super::ContextManager;
use crate::compact::is_summary_message;
use crate::context::UserGoalUpdate;
use crate::context::is_contextual_user_fragment;
use crate::event_mapping::parse_turn_item;
use crate::guardian::GUARDIAN_MAX_ROOT_MESSAGE_TOKENS;
use crate::guardian::guardian_truncate_text;
use codex_history::CodexHarnessMetadata;
use codex_history::ReconciledRetainedContext;
use codex_history::RetainedContext;
use codex_history::RetainedInputSource;
use codex_history::RetainedUserMessage;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

/// Checkpoint items may have been shortened while preserving their original metadata.
pub(super) enum RetainedMessageSource {
    Original,
    Checkpoint,
}

impl ContextManager {
    pub(super) fn has_legacy_user_messages(&self) -> bool {
        ReconciledRetainedContext::new(Some(&self.retained_context), std::iter::empty())
            .unmatched_user_messages(self.legacy_user_messages())
            .next()
            .is_some()
    }

    /// Bounded instruction candidates from the legacy backup; their ordering is unknown.
    pub(crate) fn legacy_user_messages(&self) -> impl Iterator<Item = RetainedUserMessage> + '_ {
        self.guardian_history_items()
            .into_iter()
            .flatten()
            .filter_map(move |item| {
                if !crate::context::is_user_authorization_message(item) {
                    return None;
                }
                let Some(TurnItem::UserMessage(message)) = parse_turn_item(item) else {
                    return None;
                };
                let text = message.message();
                if is_summary_message(&text)
                    || text.trim_start().starts_with("<user_action>")
                    || is_contextual_user_fragment(&ContentItem::InputText { text: text.clone() })
                {
                    return None;
                }
                let text = guardian_truncate_text(&text, GUARDIAN_MAX_ROOT_MESSAGE_TOKENS).0;
                Some(RetainedUserMessage {
                    origin: codex_history::UserInputOrigin::from_message(item),
                    turn_id: item.turn_id().unwrap_or_default().to_owned(),
                    message_id: item.id().map(|id| id.as_str().to_owned()),
                    text,
                    complete: false,
                })
            })
    }

    pub(crate) fn restore_retained_context(&mut self, checkpoint: Option<&RetainedContext>) {
        Arc::make_mut(&mut self.retained_context).restore(checkpoint, &self.items);
        let items = Arc::clone(&self.items);
        for envelope in items.iter().filter(|envelope| {
            envelope
                .metadata
                .as_ref()
                .is_some_and(|metadata| metadata.delivered_assistant_message.is_some())
        }) {
            self.record_retained_message(
                &envelope.item,
                envelope.metadata.as_ref(),
                RetainedMessageSource::Checkpoint,
            );
        }
        if self.retain_inherited_user_messages
            && !self.retained_context.has_inherited_user_messages()
        {
            // Worker checkpoints intentionally omit copied instructions. A standalone
            // root adopts the surviving prefix without duplicating an adopted checkpoint.
            let items = Arc::clone(&self.items);
            for envelope in items.iter().filter(|envelope| {
                envelope
                    .metadata
                    .as_ref()
                    .is_some_and(|metadata| metadata.inherited_user_message)
            }) {
                self.record_retained_message(
                    &envelope.item,
                    envelope.metadata.as_ref(),
                    RetainedMessageSource::Checkpoint,
                );
            }
        }
    }

    pub(super) fn record_retained_message(
        &mut self,
        item: &ResponseItem,
        metadata: Option<&CodexHarnessMetadata>,
        source: RetainedMessageSource,
    ) {
        if let Some(text) =
            metadata.and_then(|metadata| metadata.delivered_assistant_message.as_ref())
            && let ResponseItem::FunctionCallOutput {
                call_id: Some(call_id),
                ..
            } = item
        {
            let Some(call) = self.items.iter().rev().find(|envelope| {
                matches!(&envelope.item, ResponseItem::FunctionCall { call_id: id, .. } if id == call_id)
            }) else {
                return;
            };
            let source = RetainedInputSource::from(call.metadata.as_ref());
            if source == RetainedInputSource::Inherited && !self.retain_inherited_user_messages {
                return;
            }
            // The host captured this bounded text before post-tool hooks. The output
            // may be rejected, aborted, or truncated; retain the confirmed text at
            // the original call's position, not at its later completion position.
            Arc::make_mut(&mut self.retained_context).record_assistant_message(
                RetainedUserMessage {
                    origin: codex_history::UserInputOrigin::User,
                    turn_id: call.item.turn_id().unwrap_or_default().to_owned(),
                    message_id: call.item.id().map(|id| id.as_str().to_owned()),
                    text: text.clone(),
                    complete: true,
                },
                source,
            );
            return;
        }
        let is_assistant =
            matches!(item, ResponseItem::Message { role, .. } if role == "assistant");
        if metadata.is_some_and(|metadata| metadata.compaction_output)
            || (!is_assistant && !crate::context::is_user_authorization_message(item))
        {
            return;
        }
        let inherited = metadata.is_some_and(|metadata| metadata.inherited_user_message);
        if (!inherited || self.retain_inherited_user_messages)
            && let ResponseItem::Message {
                content,
                internal_chat_message_metadata_passthrough,
                ..
            } = item
        {
            let mut complete = matches!(source, RetainedMessageSource::Original)
                && (is_assistant
                    || internal_chat_message_metadata_passthrough
                        .as_ref()
                        .and_then(|metadata| metadata.content_item_kinds.as_ref())
                        .is_some_and(|kinds| {
                            kinds.len() == content.len()
                                && kinds.iter().all(|kind| {
                                    kind.0.starts_with("user.")
                                        && kind.0 != UserGoalUpdate::OMITTED_OBJECTIVE_KIND
                                })
                        }));
            let text = content
                .iter()
                .filter_map(|content| match content {
                    ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                        Some(text.as_str())
                    }
                    _ => {
                        complete = false;
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            // Keep the same bounded text that child reviewers receive before
            // compaction, instead of letting storage discard a large source.
            // Local instruction sections still omit incomplete originals whole.
            let (text, truncated) = guardian_truncate_text(&text, GUARDIAN_MAX_ROOT_MESSAGE_TOKENS);
            complete &= !truncated;
            let message = RetainedUserMessage {
                origin: codex_history::UserInputOrigin::from_message(item),
                turn_id: item.turn_id().unwrap_or_default().to_owned(),
                message_id: item.id().map(|id| id.as_str().to_owned()),
                text,
                complete,
            };
            let retained = Arc::make_mut(&mut self.retained_context);
            if is_assistant {
                retained.record_assistant_message(message, metadata.into());
            } else {
                retained.record_user_message(message, metadata.into());
            }
        }
        if !is_assistant {
            self.user_message_revision = self.user_message_revision.saturating_add(1);
        }
    }
}
