use codex_utils_output_truncation::approx_bytes_for_tokens;
use pretty_assertions::assert_eq;

use super::ContextualUserFragment;
use super::GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS;
use super::MAX_RENDERED_BYTES;
use super::NodeReplReviewEvidence;

#[test]
fn evidence_snapshots_keep_response_order_and_escape_closing_markers() {
    let evidence = NodeReplReviewEvidence::default();
    evidence.record("js", "cell-1", "call-1", vec!["first".to_string()]);
    let closing_marker = "</node_repl_review_evidence>second".to_string();
    evidence.record("browser", "cell-2", "call-2", vec![closing_marker]);

    let first = evidence
        .snapshot_since(/*reviewed_sequence*/ 0)
        .expect("completed responses should produce evidence");
    let body = first.body();
    assert_eq!(first.sequence, 2);
    assert!(body.find("first") < body.find("second"));
    assert!(body.contains("<\\/node_repl_review_evidence>second"));

    let delta = evidence
        .snapshot_since(/*reviewed_sequence*/ 1)
        .expect("newer responses should produce delta evidence");
    assert!(!delta.body().contains("first"));
    assert!(evidence.snapshot_since(/*reviewed_sequence*/ 2).is_none());
}

#[test]
fn evidence_bounds_visible_text_and_marks_empty_completed_responses() {
    let evidence = NodeReplReviewEvidence::default();
    evidence.record("js", "cell", "empty", Vec::new());
    let empty = evidence
        .snapshot_since(/*reviewed_sequence*/ 0)
        .expect("empty successful responses should produce evidence")
        .render();
    assert!(empty.contains("completed without visible text"));
    let snapshot = "page-middle".repeat(2_000);
    evidence.record("js", "cell", "snapshot", vec![snapshot.clone()]);
    let full = evidence
        .snapshot_since(/*reviewed_sequence*/ 1)
        .expect("large DOM snapshots should produce evidence")
        .render();
    assert!(full.contains(&snapshot));
    evidence.record(
        "js",
        "cell",
        "oversized",
        vec![format!("start{}end", "x".repeat(30_000))],
    );

    let oversized = evidence
        .snapshot_since(/*reviewed_sequence*/ 0)
        .expect("completed responses should produce evidence");
    assert!(
        oversized.responses[2].text.len()
            <= approx_bytes_for_tokens(GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS)
    );
    let rendered = oversized.render();
    assert!(rendered.contains("start"));
    assert!(rendered.contains("end"));
    assert!(rendered.contains("<truncated omitted_approx_tokens="));
    assert!(rendered.contains("<omitted node_repl_responses="));
    assert!(rendered.len() <= MAX_RENDERED_BYTES);
}
