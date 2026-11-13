//! Exercises independently opted-in async allocation through a complete classifier request.

use super::*;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::GuardianV2TranscriptModelConfig;
use core_test_support::responses::ev_function_call;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers;

#[tokio::test(flavor = "multi_thread")]
async fn async_elastic_classifier_request_history() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    const FIXED_ENTRY_TOKENS: usize = 2_000;
    const REPEATS: usize = 800;
    const EVIDENCE_TEXT: &str = "Untrusted assistant evidence. ";
    const CLASSIFIER_TIMEOUT: Duration = Duration::from_secs(30);
    const TEMPORARY_REDIRECT_STATUS: u16 = 307;
    let server = start_mock_server().await;
    let (release, gate) = oneshot::channel();
    let (parent, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_response_created("action"),
                ev_assistant_message("evidence", &EVIDENCE_TEXT.repeat(REPEATS)),
                // Keep raw argument bytes stable across serde_json map-order features.
                ev_function_call(
                    "plan",
                    "update_plan",
                    r#"{"plan":[{"status":"in_progress","step":"Inspect README.md"}]}"#,
                ),
                ev_completed("action"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: Some(gate),
            body: sse(vec![
                ev_assistant_message("done", "Done."),
                ev_completed("done"),
            ]),
        }],
    ])
    .await;
    let (classifier, mut finished) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
        gate: None,
        body: sse(vec![
            ev_response_created("score"),
            ev_assistant_message("score", "low"),
            ev_completed("score"),
        ]),
    }]])
    .await;
    for (model, destination) in [
        ("gpt-5.5", parent.uri()),
        ("gpt-5.6-luna", classifier.uri()),
    ] {
        Mock::given(matchers::method("POST"))
            .and(matchers::path("/v1/responses"))
            .and(matchers::body_partial_json(json!({"model": model})))
            .respond_with(
                ResponseTemplate::new(TEMPORARY_REDIRECT_STATUS)
                    .insert_header("location", format!("{destination}/v1/responses")),
            )
            .mount(&server)
            .await;
    }
    let settings = format!(
        "[features.guardianv2]\nenabled = true\nasync_elastic_budgeting = true\n{}\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n",
        core_test_support::guardian::async_elastic_sections_toml(),
    );
    let cloud_config = CloudConfigBundleFixture::enterprise_config(settings).into_loader();
    let test = test_codex()
        .with_model("gpt-5.5")
        .with_cloud_config_bundle(cloud_config)
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            config.update_plan_enabled = true;
            config.approvals_reviewer = codex_protocol::config_types::ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = codex_core::config::Constrained::allow_any(
                codex_protocol::protocol::AskForApproval::OnRequest,
            );
            config
                .permissions
                .set_permission_profile(codex_protocol::models::PermissionProfile::read_only())
                .unwrap();
            assert!(config.guardian_async_elastic_budget.is_some());
            assert!(config.guardian_sync_elastic_budget.is_none());
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.guardian = None;
            model.model_messages.as_mut().unwrap().guardian_v2 = Some(GuardianV2ModelConfig {
                classifier_instructions: Some(
                    "Classify risk using the supplied evidence.".to_owned(),
                ),
                transcript: Some(GuardianV2TranscriptModelConfig {
                    max_message_entry_tokens: Some(FIXED_ENTRY_TOKENS),
                    ..Default::default()
                }),
                ..Default::default()
            });
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Inspect README.md. Do not modify files.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    tokio::time::timeout(CLASSIFIER_TIMEOUT, &mut finished[0]).await??;
    release.send(()).unwrap();
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = classifier.requests().await;
    pretty_assertions::assert_eq!(requests.len(), 1);
    let request: serde_json::Value = serde_json::from_slice(&requests[0])?;
    let text = request.to_string();
    let transcript = text.split(">>> TRANSCRIPT END").next().unwrap();
    let retained_repetitions = transcript.matches(EVIDENCE_TEXT.trim_end()).count();
    let snapshot = context_snapshot::format_context_snapshot(
        "Async elastic Guardian with sync budgeting disabled",
        &[SnapshotEntry::body(&request)],
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    let snapshot = snapshot
        .replace("environment \"local\"", "environment \"<ENVIRONMENT>\"")
        .replace("environment \"remote\"", "environment \"<ENVIRONMENT>\"");
    insta::assert_snapshot!("async_elastic_classifier", snapshot);
    assert!(
        retained_repetitions > 0 && retained_repetitions < REPEATS,
        "async entry cap must survive model defaults: {retained_repetitions}"
    );
    assert!(
        retained_repetitions * EVIDENCE_TEXT.len()
            > codex_protocol::protocol::TruncationPolicy::Tokens(FIXED_ENTRY_TOKENS).byte_budget(),
        "elastic evidence must exceed the old Fixed entry limit"
    );
    classifier.shutdown().await;
    parent.shutdown().await;
    Ok(())
}
