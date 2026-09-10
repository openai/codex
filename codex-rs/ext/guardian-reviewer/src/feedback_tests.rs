use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn oversized_context_keeps_the_action_and_decision() -> anyhow::Result<()> {
    let thread_id = ThreadId::new();
    let reviewer_thread_id = ThreadId::new();
    let decision = r#"{"outcome":"deny","rationale":"Missing approval."}"#;
    let action = r#"{"command":"git push"}"#;
    let outcome = GuardianReviewSessionOutcome::Completed(Ok(Some(decision.to_owned())));
    let feedback = FailedReviewFeedback::for_outcome(
        &outcome,
        ReviewFeedbackSettings {
            enabled: true,
            ephemeral: false,
        },
    )
    .expect("denials retain feedback");
    feedback.store(ReviewFeedbackContext {
        reviewed_thread_id: thread_id,
        reviewed_turn_id: "parent-turn",
        target_item_id: Some("push-call"),
        reviewer_thread_id,
        model: "review-model",
        action,
        action_truncated: false,
        instructions: Some(&"x".repeat(MAX_RECORD_BYTES)),
        history: Vec::new(),
    });
    let contents = codex_feedback::guardian_review_failures(&[thread_id])
        .attachment
        .expect("failed-review record")
        .buffer;
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&contents)?,
        json!({
            "reviewed_thread_id": thread_id,
            "reviewed_turn_id": "parent-turn",
            "target_item_id": "push-call",
            "reviewer_thread_id": reviewer_thread_id,
            "model": "review-model",
            "status": "denied",
            "decision": decision,
            "action": action,
            "action_truncated": false,
            "instructions": null,
            "history": [],
            "context_omitted": true,
        })
    );
    Ok(())
}
