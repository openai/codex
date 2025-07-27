use crate::app_event_sender::AppEventSender;
use codex_core::protocol::AskForApproval;
use ratatui::layout::Rect;

struct ApprovalPolicySelectOption {
    label: AskForApproval,
    description: &'static str,
}

const APPROVAL_POLICY_OPTIONS: &[ApprovalPolicySelectOption] = &[
    ApprovalPolicySelectOption {
        label: AskForApproval::UnlessTrusted,
        description: "No approval required for messages.",
    },
    ApprovalPolicySelectOption {
        label: AskForApproval::OnFailure,
        description: "All messages require approval before sending.",
    },
    ApprovalPolicySelectOption {
        label: AskForApproval::Never,
        description: "Messages can be sent without approval, but approval is encouraged.",
    },
];

pub(crate) struct ChangeApprovalPolicyWidget {
    app_event_tx: AppEventSender,
}
impl ChangeApprovalPolicyWidget {
    pub(crate) fn get_height(&self, area: &Rect) -> u16 {
        todo!()
    }
}
