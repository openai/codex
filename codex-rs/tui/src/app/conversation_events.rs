//! Conversation-origin routing policy for application events.
//!
//! Widget-owned senders attach a pane generation to their events. Presentation events from a
//! retired widget are discarded. Only events whose handlers are independent of widget-local
//! request state are allowed to outlive their originating generation.

use crate::app_event::AppEvent;
use crate::app_event::ConversationOrigin;

pub(super) fn normalize_conversation_event(
    event: AppEvent,
    is_live_origin: impl FnOnce(ConversationOrigin) -> bool,
) -> Option<(Option<ConversationOrigin>, AppEvent)> {
    match event {
        AppEvent::FromConversation { target, event } if is_live_origin(target) => {
            Some((Some(target), *event))
        }
        AppEvent::FromConversation { event, .. } if survives_conversation_retirement(&event) => {
            Some((None, *event))
        }
        AppEvent::FromConversation { .. } => None,
        event => Some((None, event)),
    }
}

pub(super) fn survives_conversation_retirement(event: &AppEvent) -> bool {
    match event {
        AppEvent::Exit(_)
        | AppEvent::Logout
        | AppEvent::FatalExitRequest(_)
        | AppEvent::ConversationOp { .. }
        | AppEvent::SubmitThreadOp { .. }
        | AppEvent::AppendMessageHistoryEntry { .. }
        | AppEvent::SyncThreadGitBranch { .. }
        | AppEvent::SetThreadGoalDraft { .. }
        | AppEvent::SetThreadGoalStatus { .. }
        | AppEvent::ClearThreadGoal { .. }
        | AppEvent::OpenUrlInBrowser { .. }
        | AppEvent::OpenDesktopThread { .. }
        | AppEvent::UpdateFeatureFlags { .. }
        | AppEvent::UpdateMemorySettings { .. }
        | AppEvent::ResetMemories
        | AppEvent::SetSkillEnabled { .. }
        | AppEvent::SetAppEnabled { .. }
        | AppEvent::PetDisabled
        | AppEvent::StatusLineSetup { .. }
        | AppEvent::TerminalTitleSetup { .. }
        | AppEvent::SyntaxThemeSelected { .. }
        | AppEvent::KeymapCaptured { .. }
        | AppEvent::KeymapCleared { .. } => true,
        AppEvent::FeedbackSubmitted {
            origin_thread_id: Some(_),
            ..
        } => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "conversation_events_tests.rs"]
mod tests;
