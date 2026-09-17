//! Approval routing coverage at the policy boundary, without host session setup.

use super::routes_approval_policy_to_guardian;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use pretty_assertions::assert_eq;

#[test]
fn approval_routing_depends_on_policy_and_reviewer() {
    for (policy, expected) in [
        (AskForApproval::UnlessTrusted, [false, false]),
        (AskForApproval::OnRequest, [false, true]),
        (
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            [false, true],
        ),
        (AskForApproval::Never, [false, false]),
    ] {
        let actual = [ApprovalsReviewer::User, ApprovalsReviewer::AutoReview]
            .map(|reviewer| routes_approval_policy_to_guardian(policy, reviewer));
        assert_eq!(actual, expected, "approval policy: {policy:?}");
    }
}
