//! Explain how exec handles approvals when the effective settings call for a human.

use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;

pub(super) fn warn_if_human_approval(
    policy: AskForApproval,
    reviewer: ApprovalsReviewer,
    flags_supported: bool,
) {
    // Even with every granular category disabled, a dangerous command can
    // require approval to run within the existing sandbox.
    if reviewer != ApprovalsReviewer::User || policy == AskForApproval::Never {
        return;
    }
    eprintln!(
        "WARNING: This thread is configured to ask a human for approval. \
         `codex exec` runs non-interactively and immediately rejects approval requests it receives.\n\
         - To review requests yourself, run this turn from the TUI."
    );
    // Switching an untrusted project to Never would also allow ordinary
    // sandboxed commands that currently require approval.
    if flags_supported && policy != AskForApproval::UnlessTrusted {
        eprintln!(
            "- To change the thread's saved reviewer, pass `--config 'approvals_reviewer=\"auto_review\"'` to `codex exec resume`.\n\
             - To disable approval prompts for this and future turns, pass `--config 'approval_policy=\"never\"'` to `codex exec resume`."
        );
    }
}
