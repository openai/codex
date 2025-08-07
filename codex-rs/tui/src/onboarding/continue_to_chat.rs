use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use crate::app::ChatWidgetArgs;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::onboarding::onboarding_screen::StepStateProvider;

use super::onboarding_screen::StepState;

/// This doesn't render anything explicitly but serves as a signal that we made it to the end and
/// we should continue to the chat.
pub(crate) struct ContinueToChatWidget {
    pub event_tx: AppEventSender,
    pub chat_widget_args: ChatWidgetArgs,
}

impl StepStateProvider for ContinueToChatWidget {
    fn get_step_state(&self) -> StepState {
        StepState::Complete
    }
}

impl WidgetRef for &ContinueToChatWidget {
    fn render_ref(&self, _area: Rect, _buf: &mut Buffer) {
        self.event_tx
            .send(AppEvent::OnboardingComplete(self.chat_widget_args.clone()));
    }
}
