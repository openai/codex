use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::change_approval_policy_widget::ChangeApprovalPolicyWidget;
use codex_core::protocol::AskForApproval;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

/// Modal view for choosing an approval policy.
/// The view will propagate logic to the `ChangeApprovalPolicyWidget`
pub(crate) struct ChangeApprovalPolicyModelView<'a> {
    change_approval_policy_widget: ChangeApprovalPolicyWidget<'a>,
}

impl ChangeApprovalPolicyModelView<'_> {
    pub(crate) fn new(
        app_event_tx: AppEventSender,
        current_approval_policy: AskForApproval,
    ) -> Self {
        Self {
            change_approval_policy_widget: ChangeApprovalPolicyWidget::new(
                app_event_tx,
                current_approval_policy,
            ),
        }
    }
}

impl<'a> BottomPaneView<'a> for ChangeApprovalPolicyModelView<'a> {
    fn handle_key_event(&mut self, pane: &mut BottomPane<'_>, key_event: KeyEvent) {
        self.change_approval_policy_widget
            .handle_key_event(pane, key_event);
    }

    fn is_complete(&self) -> bool {
        self.change_approval_policy_widget.is_complete()
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.change_approval_policy_widget.render_ref(area, buf);
    }
}
