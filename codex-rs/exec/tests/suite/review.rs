#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::prelude::*;

/// Verify that `codex exec review` triggers the review flow and renders
/// a formatted finding block when the reviewer returns a structured JSON result.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_review_uncommitted_renders_findings() -> anyhow::Result<()> {
    let test = test_codex_exec();

    // Structured review output returned by the reviewer model (as a JSON string).
    let review_json = serde_json::json!({
        "findings": [
            {
                "title": "Prefer Stylize helpers",
                "body": "Use .dim()/.bold() chaining instead of manual Style where possible.",
                "confidence_score": 0.9,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/file.rs",
                    "line_range": {"start": 10, "end": 20}
                }
            }
        ],
        "overall_correctness": "good",
        "overall_explanation": "All good with some improvements suggested.",
        "overall_confidence_score": 0.8
    })
    .to_string();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("m-1", &review_json),
        responses::ev_completed("resp-1"),
    ]);
    responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.cwd_path())
        .arg("review")
        .arg("--uncommitted")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            ">> Code review started: current changes <<",
        ))
        .stderr(predicate::str::contains(
            "- Prefer Stylize helpers — /tmp/file.rs:10-20",
        ))
        .stderr(predicate::str::contains("<< Code review finished >>"));

    Ok(())
}
