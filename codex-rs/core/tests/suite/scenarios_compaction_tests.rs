//! Snapshots pending mail across remote compaction, restart, and normal inference.

use super::*;
use core_test_support::responses::strip_metadata_from_json;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_slash_compact_preserves_context_across_windows() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("plan", "The release plan has three steps."),
                ev_completed("plan-response"),
            ]),
            sse(vec![
                compaction_output("REMOTE_SLASH_COMPACT_CHECKPOINT"),
                ev_completed("compact-response"),
            ]),
            sse(vec![
                compaction_output("RECOMPACTED_CHECKPOINT"),
                ev_completed("recompact-response"),
            ]),
            sse(vec![
                ev_assistant_message("continue", "Start by validating the release candidate."),
                ev_completed("continue-response"),
            ]),
            sse(vec![
                compaction_output("AFTER_MAIL_WAS_SAMPLED"),
                ev_completed("sampled-compact-response"),
            ]),
        ],
    )
    .await;
    let configure = |config: &mut Config| {
        configure_scenario_catalog(config);
        config.workspace_roots = vec![config.cwd.clone()];
        config.model_auto_compact_token_limit = Some(i64::MAX);
    };
    let mut builder = test_codex()
        .with_model("gpt-6-astra")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(configure);
    let mut test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Plan the release.").await?;
    let mail = json!({
        "type": "agent_message", "id": "agent_pending-result",
        "author": "/root/worker", "recipient": "/root",
        "content": [{"type": "input_text", "text": "Message Type: FINAL_ANSWER\nThe worker found a release blocker."}],
    });
    test.codex
        .inject_response_items(vec![serde_json::from_value(mail.clone())?])
        .await?;
    for compaction in 0..2 {
        test.codex.submit(Op::Compact).await?;
        wait_for_event(&test.codex, |event| {
            assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        if compaction == 0 {
            builder = builder.with_model("gpt-6-astra").with_config(configure);
            test = builder.restart_with_auto_env(&server, &test).await?;
        }
    }

    test.submit_turn("Continue with the first step.").await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let history = test.codex.conversation_history_snapshot().await;
    assert!(
        !history
            .items()
            .any(|item| matches!(item, ResponseItem::AgentMessage { .. }))
    );
    let requests = mock.requests();
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[1].inputs_of_type("compaction_trigger").len(), 1);
    assert_eq!(
        requests[3].inputs_of_type("compaction")[0]["encrypted_content"],
        "RECOMPACTED_CHECKPOINT"
    );
    assert_eq!(
        strip_metadata_from_json(json!(requests[3].inputs_of_type("agent_message"))),
        json!([mail]),
    );

    insta::assert_snapshot!(
        "remote_slash_compact",
        context_snapshot::format_context_snapshot(
            "Pending mail survives repeated /compact and cold resume, then normal inference acknowledges it for subsequent compaction.",
            &[
                SnapshotEntry::captured(&requests[0]).labeled("Before /compact"),
                SnapshotEntry::captured(&requests[1]).labeled("Remote /compact request"),
                SnapshotEntry::captured(&requests[2]).labeled("Repeated /compact after resume"),
                SnapshotEntry::captured(&requests[3]).labeled("First turn after /compact"),
                SnapshotEntry::captured(&requests[4]).labeled("Compaction after mail was sampled"),
            ],
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
