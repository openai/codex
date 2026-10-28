//! Projects recent sender exchanges from host-authenticated retained context.
//! Only local user messages and preceding assistant context are selected.

use codex_guardian_context::GuardianRootMessage;
use codex_history::RetainedContext;
use codex_history::RetainedContextEntry;
use codex_history::RetainedContextOrder;

/// Recorded adjacency provides context, not a verified question/answer association.
pub struct GuardianSenderExchange {
    pub user: Option<String>,
    pub assistant: Option<GuardianRootMessage>,
}

pub fn guardian_sender_exchanges(context: &RetainedContext) -> Vec<GuardianSenderExchange> {
    let mut exchanges = Vec::new();
    let mut assistant = None;
    for (order, entry) in context.ordered_entries() {
        match (order, entry) {
            (RetainedContextOrder::Local(_), RetainedContextEntry::UserMessage(message)) => {
                exchanges.push((message, assistant.take()))
            }
            (RetainedContextOrder::Local(_), RetainedContextEntry::AssistantMessage(message)) => {
                assistant = Some(message)
            }
            (RetainedContextOrder::Inherited(_), _)
            | (_, RetainedContextEntry::VerifiedAnswer(_)) => {}
        }
    }
    let mut messages = exchanges
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
        .collect::<Vec<_>>();
    messages.reverse();
    messages
}
