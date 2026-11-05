//! Denial resampling repeats the last request and publishes only the final decision.

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_denial_reconsideration() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    let builder = test_codex().with_model("gpt-5.5").with_config(|config| {
        super::configure_scenario_catalog(config);
        config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config
            .permissions
            .set_permission_profile(PermissionProfile::read_only())
            .expect("set read-only permission profile");
        // Apply to the selected reviewer too, including per-model reviewer overrides.
        for model in &mut config
            .model_catalog
            .as_mut()
            .expect("scenario model catalog")
            .models
        {
            model.context_window = Some(200_000);
            model.auto_compact_token_limit = Some(200_000);
        }
        // The provisional denial crosses the background threshold. It must
        // never reach post-turn work or start compaction of that answer.
        config.features.enable(Feature::AsyncCompaction).unwrap();
        config.async_compaction_threshold_percent = 50;
        config
            .features
            .enable(Feature::GuardianDenialReconsideration)
            .expect("enable Guardian denial reconsideration");
    });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let test = harness.test();
    let mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_function_call(
                    "reviewed-action",
                    "exec_command",
                    r#"{"cmd":"echo reconsidered > reconsideration.txt","sandbox_permissions":"require_escalated","justification":"Write the requested marker."}"#,
                ),
                ev_completed("parent-action"),
            ]),
            sse(vec![
                responses::ev_custom_tool_call(
                    "review-evidence",
                    "exec",
                    "text('retained evidence');",
                ),
                ev_completed("evidence-read"),
            ]),
            sse(vec![
                responses::ev_reasoning_item("first-reasoning", &["Assessing authorization."], &[]),
                ev_assistant_message("first-assessment", r#"{"outcome":"deny"}"#),
                ev_completed_with_tokens("first-review", /*total_tokens*/ 100_001),
            ]),
            sse(vec![
                ev_assistant_message("second-assessment", r#"{"outcome":"allow"}"#),
                ev_completed_with_tokens("second-review", /*total_tokens*/ 1_000),
            ]),
            sse(vec![
                ev_function_call(
                    "followup-action",
                    "exec_command",
                    r#"{"cmd":"echo followup > followup.txt","sandbox_permissions":"require_escalated","justification":"Write the followup marker."}"#,
                ),
                ev_completed("parent-followup"),
            ]),
            sse(vec![
                ev_assistant_message("followup-assessment", r#"{"outcome":"allow"}"#),
                ev_completed("followup-review"),
            ]),
            sse(vec![
                ev_assistant_message("parent-result", "Finished reviewing the request."),
                ev_completed("parent-done"),
            ]),
        ],
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![super::text(
            "Write the marker file if the security policy permits it.",
        )]))
        .await?;
    while !matches!(
        test.codex.next_event().await?.msg,
        EventMsg::TurnComplete(_)
    ) {}
    assert!(harness.path_exists("reconsideration.txt").await?);
    assert!(harness.path_exists("followup.txt").await?);
    let mut requests = mock.requests();
    assert_eq!(requests.len(), 7);
    let input = requests[2].input();
    assert_eq!(requests[3].input(), input);
    assert_eq!(&requests[5].input()[..input.len()], input.as_slice());
    assert_eq!(
        requests[5].input()[input.len()]["content"][0]["text"],
        r#"{"outcome":"allow"}"#
    );
    // Group independent streams so the snapshot only repeats actual retries.
    requests.sort_by_key(|request| {
        request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian"
    });
    let mut snapshot = context_snapshot::format_request_history_snapshot(
        "Guardian retries the same request after a denial and preserves evidence for the next review. Only selected assessments enter history. Parent and reviewer streams are grouped.",
        &requests,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    let environment_id = &test.executor_environment().selection().environment_id;
    snapshot = snapshot.replace(&format!("{environment_id:?}"), "\"<ENVIRONMENT>\"");
    for (pattern, replacement) in [
        (r"Chunk ID: [a-z0-9]+", "Chunk ID: <CHUNK_ID>"),
        (
            r"Wall time: [0-9]+(?:\.[0-9]+)? seconds",
            "Wall time: <DURATION> seconds",
        ),
        (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
        (
            r#""command": \[\s*(?:"[^"]*",\s*)*"(echo [^"]*)"\s*\]"#,
            r#""command": ["<SHELL>", "$1"]"#,
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    insta::assert_snapshot!("guardian_denial_reconsideration", snapshot);
    Ok(())
}
