//! Snapshots remote `/compact`, its persisted replacement, and the next user turn.

use super::*;
use codex_history::RolloutItem;
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
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "REMOTE_SLASH_COMPACT_CHECKPOINT",
                    },
                }),
                ev_completed("compact-response"),
            ]),
            sse(vec![
                ev_assistant_message("continue", "Start by validating the release candidate."),
                ev_completed("continue-response"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_model("gpt-6-astra")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            configure_scenario_catalog(config);
            config.workspace_roots = vec![config.cwd.clone()];
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn("Plan the release.").await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    // Observe the checkpoint before a later turn can initialize or update its context.
    test.codex.flush_rollout().await?;
    let rollout = fs::read_to_string(test.codex.rollout_path().expect("rollout path"))?;
    let replacement = rollout
        .lines()
        .map(codex_rollout::parse_rollout_line)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find_map(|line| match line.item {
            RolloutItem::Compacted(compacted) => compacted.replacement_history,
            _ => None,
        })
        .expect("persisted compaction replacement")
        .into_iter()
        .map(|envelope| serde_json::to_value(envelope.item))
        .collect::<Result<Vec<_>, _>>()?;

    test.submit_turn("Continue with the first step.").await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1].inputs_of_type("compaction_trigger").len(), 1);
    assert_eq!(
        requests[2].inputs_of_type("compaction")[0]["encrypted_content"],
        "REMOTE_SLASH_COMPACT_CHECKPOINT"
    );

    insta::assert_snapshot!(
        "remote_slash_compact",
        context_snapshot::format_context_snapshot(
            "A user invokes /compact through remote compaction, persists the replacement history, then continues in the new window.",
            &[
                SnapshotEntry::captured(&requests[0]).labeled("Before /compact"),
                SnapshotEntry::captured(&requests[1]).labeled("Remote /compact request"),
                SnapshotEntry::items(&replacement).labeled("Stored replacement before next turn"),
                SnapshotEntry::captured(&requests[2]).labeled("First turn after /compact"),
            ],
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
