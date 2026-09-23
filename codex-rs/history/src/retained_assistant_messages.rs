//! Retains ordinary assistant context separately from user authorization and verified answers.
//! Original delivery order survives checkpoints; assistant omissions never evict user evidence.
//! Omission markers describe observed loss, not unknown legacy capture coverage.

use super::Ordered;
use super::RetainedContext;
use super::RetainedInputSource;
use super::RetainedUserMessage;
use super::bound_family;

impl RetainedContext {
    /// Reports observed storage loss or a retained message that cannot be delivered whole.
    /// False does not establish complete capture coverage for legacy history.
    pub fn has_omitted_assistant_messages(&self) -> bool {
        self.assistant_messages_incomplete
            || self
                .assistant_messages
                .iter()
                .any(|entry| !entry.value.complete)
    }

    pub(super) fn next_inherited_order(&self) -> u64 {
        self.user_messages
            .iter()
            .chain(&self.assistant_messages)
            .filter(|entry| entry.inherited)
            .map(|entry| entry.order.saturating_add(1))
            .max()
            .unwrap_or_default()
    }

    /// Records original assistant text without interpreting it as a question or grant.
    /// Older unsequenced sources cannot establish order relative to queued user replies.
    pub fn record_assistant_message(
        &mut self,
        mut message: RetainedUserMessage,
        source: RetainedInputSource,
    ) {
        if source == RetainedInputSource::Local(None) {
            // Leave unsequenced sources to legacy transcript selection. Missing
            // ordering alone does not establish an omission from reviewer context.
            return;
        }
        message.bound();
        let inherited = source == RetainedInputSource::Inherited;
        if let Some(index) = self.assistant_messages.iter().position(|entry| {
            message.message_id.is_some() && entry.value.message_id == message.message_id
        }) {
            if self.assistant_messages[index].value == message
                && self.assistant_messages[index].inherited == inherited
            {
                return;
            }
            self.assistant_messages.remove(index);
        }
        let order = if inherited {
            self.next_inherited_order()
        } else {
            self.record_order(source.acceptance_order())
        };
        self.assistant_messages.push_back(Ordered {
            inherited,
            order,
            value: message,
        });
        bound_family(
            &mut self.assistant_messages,
            &mut self.assistant_messages_incomplete,
        );
    }
}
