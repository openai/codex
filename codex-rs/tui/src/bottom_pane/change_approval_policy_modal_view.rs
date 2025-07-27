use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::change_approval_policy_widget::ChangeApprovalPolicyWidget;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Modal view for choosing an approval policy.

pub(crate) struct ChangeApprovalPolicyModelView {
    change_approval_policy_widget: ChangeApprovalPolicyWidget,
}

impl BottomPaneView<'_> for ChangeApprovalPolicyModelView {


    fn render(&self, area: Rect, buf: &mut Buffer) {
        todo!()
    }
}
