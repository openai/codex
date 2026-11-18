//! Verifies prepared context reaches both the first model request and a follow-up.

use super::*;
use anyhow::Context as _;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::turn_input::AnnotatedResponseItem;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case("message")]
#[test_case("function_call_output")]
#[test_case("custom_tool_call_output")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_injection_rejects_uploaded_audio_before_modifying_history(kind: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let before = test.codex.conversation_history_snapshot().await;
    let parts = json!([{"type": "input_audio", "file_id": "file_injected"}]);
    let audio: ResponseItem = serde_json::from_value(if kind == "message" {
        json!({"type": kind, "role": "developer", "content": parts})
    } else {
        json!({"type": kind, "call_id": "untrusted", "output": parts})
    })?;
    let batch = vec![
        core_test_support::responses::user_message_item("must not be partially written"),
        audio.clone(),
    ];

    let history_error = test
        .codex
        .inject_response_items(batch.clone())
        .await
        .err()
        .context("raw history accepted a caller-supplied audio file")?;
    let preparation_error = test
        .codex
        .inject_response_items_for_turn(batch)
        .await
        .err()
        .context("prepared history accepted a caller-supplied audio file")?;
    for error in [history_error, preparation_error] {
        assert!(matches!(
            error.details(),
            CodexErrorDetails::InvalidRequest(_)
        ));
    }
    for input in [
        TurnInput::ResponseItem(audio.clone()),
        TurnInput::AnnotatedResponseItem(AnnotatedResponseItem {
            item: audio,
            annotations: Default::default(),
        }),
    ] {
        let error = test
            .codex
            .start_or_steer_turn(TurnInputRequest::new(input))
            .await
            .err()
            .context("turn input accepted an untyped audio file")?;
        assert!(matches!(
            error.details(),
            CodexErrorDetails::InvalidRequest(_)
        ));
    }
    let after = test.codex.conversation_history_snapshot().await;
    assert_eq!(after.history_version(), before.history_version());
    assert_eq!(
        after.items().collect::<Vec<_>>(),
        before.items().collect::<Vec<_>>()
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(ThreadHistoryMode::Legacy; "legacy")]
#[test_case(ThreadHistoryMode::Paginated; "paginated")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_context_reaches_first_request_and_survives_follow_up(
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    const PREPARED_CONTEXT: &str = "Prepared project context: the launch owner is Mira.";

    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(history_mode)
        .build_with_auto_env(&server)
        .await?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("first-response"),
                ev_assistant_message("first-message", "The launch owner is Mira."),
                ev_completed("first-response"),
            ]),
            sse(vec![
                ev_response_created("follow-up-response"),
                ev_assistant_message("follow-up-message", "Mira will send the launch update."),
                ev_completed("follow-up-response"),
            ]),
        ],
    )
    .await;

    // A hosted caller can inject preparation before the user starts the first turn.
    test.codex
        .inject_response_items(vec![serde_json::from_value(json!({
            "type": "message",
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": PREPARED_CONTEXT
            }],
        }))?])
        .await?;
    assert!(mock.requests().is_empty());

    test.submit_turn("Who owns the launch?").await?;
    test.submit_turn("Who will send the launch update?").await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    for request in requests {
        let prepared_context = request
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| text == PREPARED_CONTEXT)
            .collect::<Vec<_>>();
        assert_eq!(prepared_context, vec![PREPARED_CONTEXT]);
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(true; "turn_prepares_first")]
#[test_case(false; "injection_prepares_first")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_turn_and_raw_injection_publish_one_initial_prefix(
    turn_prepares_first: bool,
) -> Result<()> {
    use codex_extension_api::ContextContributor;
    use codex_extension_api::ExtensionFuture;
    use codex_extension_api::PreviousWorldStateSection;
    use codex_extension_api::RenderedWorldStateFragment;
    use codex_extension_api::WorldStateContributionInput;
    use codex_extension_api::WorldStateSectionContribution;
    use codex_extension_api::input_text;
    use core_test_support::responses;
    use futures::FutureExt;
    use std::sync::OnceLock;
    use tokio::sync::Notify;

    const INITIAL: &str = "Instructions for the new window.";
    const INJECTED: &str = "Caller-provided context.";
    skip_if_no_network!(Ok(()));

    struct OverlappingInitializers {
        paused_turn_id: OnceLock<String>,
        entered: Notify,
        resume: Notify,
    }
    impl ContextContributor for OverlappingInitializers {
        fn contribute_world_state<'a>(
            &'a self,
            input: WorldStateContributionInput<'a>,
        ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
            Box::pin(async move {
                // Hold the first caller after it observes the missing reference context.
                if self.paused_turn_id.set(input.turn_id.to_owned()).is_ok() {
                    self.entered.notify_one();
                    self.resume.notified().await;
                }
                let current =
                    if self.paused_turn_id.get().map(String::as_str) == Some(input.turn_id) {
                        "paused"
                    } else {
                        "other"
                    };
                vec![WorldStateSectionContribution::new(
                    "race_state",
                    move |previous| {
                        let previous = match previous {
                            PreviousWorldStateSection::Known(value) => {
                                value.as_str().unwrap_or("invalid")
                            }
                            PreviousWorldStateSection::Absent => "absent",
                            PreviousWorldStateSection::Unknown => "unknown",
                        };
                        if previous == current {
                            return (None, None);
                        }
                        (
                            Some(json!(current)),
                            Some(RenderedWorldStateFragment::new(
                                "developer",
                                ("", ""),
                                input_text(format!("Race state: {previous} -> {current}")),
                            )),
                        )
                    },
                )]
            })
        }
    }

    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    let overlap = Arc::new(OverlappingInitializers {
        paused_turn_id: OnceLock::new(),
        entered: Notify::new(),
        resume: Notify::new(),
    });
    extensions.prompt_contributor(overlap.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| config.developer_instructions = Some(INITIAL.to_string()))
        .build_with_auto_env(&server)
        .await?;
    let mock = responses::mount_response_handler(&server, |_| {
        responses::sse_response(sse(vec![ev_completed("done")]))
    })
    .await;
    let item = serde_json::from_value(json!({
        "type": "message", "role": "developer",
        "content": [{"type": "input_text", "text": INJECTED}]
    }))?;
    let turn = test.submit_text_turn("First turn.").boxed();
    let injection = async { Ok(test.codex.inject_response_items(vec![item]).await?) }.boxed();
    let (first, second) = if turn_prepares_first {
        (turn, injection)
    } else {
        (injection, turn)
    };
    tokio::try_join!(first, async {
        overlap.entered.notified().await;
        let result = second.await;
        overlap.resume.notify_one();
        result
    })?;
    let transitions: Vec<_> = mock.requests()[0]
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("Race state:"))
        .collect();
    let mut expected = vec!["Race state: absent -> other"];
    if turn_prepares_first {
        // Injection installed its snapshot. The real turn must publish its diff before sampling.
        expected.push("Race state: other -> paused");
    }
    assert_eq!(transitions, expected);
    test.submit_text_turn("Follow up.").await?;
    let texts = mock
        .last_request()
        .expect("follow-up request")
        .message_input_texts("developer");
    for expected in [INITIAL, INJECTED] {
        assert_eq!(
            texts.iter().filter(|text| text.contains(expected)).count(),
            1,
            "{expected}"
        );
    }
    if !turn_prepares_first {
        // A losing injection appends only caller-provided items.
        assert!(
            !texts
                .iter()
                .any(|text| text == "Race state: other -> paused")
        );
    }
    let history = test
        .codex
        .load_history(/*include_archived*/ false)
        .await?
        .items;
    let full_snapshots = history
        .iter()
        .filter(|item| matches!(item, RolloutItem::WorldState(state) if state.full))
        .count();
    assert_eq!(full_snapshots, 1);
    Ok(())
}
