use codex_protocol::protocol::AgentStatus;
use codex_utils_output_truncation::approx_token_count;

use super::ContextualUserFragment;
use super::NOTIFICATION_MAX_TOKENS;
use super::SubagentNotification;

#[test]
fn error_notification_stays_below_manual_review_threshold() {
    let notification = SubagentNotification::new(
        "/root/worker",
        AgentStatus::Errored("stream disconnected ".repeat(1_000)),
    )
    .render();

    assert!(approx_token_count(&notification) < NOTIFICATION_MAX_TOKENS);
}
