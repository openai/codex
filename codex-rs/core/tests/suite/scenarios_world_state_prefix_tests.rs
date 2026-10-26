//! Exercises extension prefixes through live updates, compaction, and persisted history.

use super::*;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_protocol::models::input_text;
use pretty_assertions::assert_eq;
use std::sync::Mutex;

struct PrefixInstructions {
    current: Mutex<&'static str>,
}

impl ContextContributor for PrefixInstructions {
    fn contribute_world_state<'a>(
        &'a self,
        _input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        let current = *self.current.lock().expect("current instructions");
        Box::pin(async move {
            [
                ("scenario_prefix", "developer"),
                ("rejected_user_prefix", "user"),
                ("rejected_assistant_prefix", "assistant"),
            ]
            .into_iter()
            .map(|(id, role)| {
                let body = format!("{id}: {current}");
                let rendered = format!("<scenario_instructions>{body}</scenario_instructions>");
                WorldStateSectionContribution::new(id, move |previous| {
                    let snapshot = json!(body);
                    let unchanged = matches!(previous, PreviousWorldStateSection::Known(previous) if previous == &snapshot);
                    (
                        Some(snapshot),
                        (!unchanged).then(|| {
                            RenderedWorldStateFragment::new(
                                role,
                                ("<scenario_instructions>", "</scenario_instructions>"),
                                input_text(body.clone()),
                            )
                        }),
                    )
                })
                .with_window_prefix()
                .with_retained_fragment_matcher(move |candidate_role, text| {
                    candidate_role == role && text == rendered
                })
            })
            .collect()
        })
    }
}

#[test_case::test_case("local"; "local")]
#[test_case::test_case("remote"; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn extension_window_prefix_survives_compaction(compaction: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let compact_response = match compaction {
        "local" => ev_assistant_message("summary", "The user is planning a release."),
        "remote" => json!({
            "type": "response.output_item.done",
            "item": {"type": "compaction", "encrypted_content": "PREFIX_CHECKPOINT"},
        }),
        _ => unreachable!(),
    };
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("first", "Plan ready."),
                ev_completed("first-response"),
            ]),
            sse(vec![
                ev_assistant_message("second", "Plan checked."),
                ev_completed("second-response"),
            ]),
            sse(vec![
                ev_assistant_message("third", "Plan updated."),
                ev_completed("third-response"),
            ]),
            sse(vec![compact_response, ev_completed("compact-response")]),
            sse(vec![
                ev_assistant_message("continue", "Continue the release."),
                ev_completed("continue-response"),
            ]),
        ],
    )
    .await;
    let instructions = Arc::new(PrefixInstructions {
        current: Mutex::new("A"),
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.prompt_contributor(instructions.clone());
    let local_compaction = compaction == "local";
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model("gpt-6-astra")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            configure_scenario_catalog(config);
            config.workspace_roots = vec![config.cwd.clone()];
            if local_compaction {
                config.model_provider.name = "Local compaction test".to_string();
            }
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn("Plan the release.").await?;
    test.submit_turn("Check the plan.").await?;
    *instructions.current.lock().expect("current instructions") = "B";
    test.submit_turn("Update the plan.").await?;
    // Capture a new policy during compaction, not only one already rendered in history.
    *instructions.current.lock().expect("current instructions") = "C";
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

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

    test.submit_turn("Continue with the release.").await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 5);
    let prefix_texts = |items: &[serde_json::Value]| {
        items
            .iter()
            .flat_map(|item| item["content"].as_array().into_iter().flatten())
            .filter_map(|content| content["text"].as_str())
            .filter(|text| text.starts_with("<scenario_instructions>"))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let prefix_a = "<scenario_instructions>scenario_prefix: A</scenario_instructions>".to_string();
    let prefix_b = "<scenario_instructions>scenario_prefix: B</scenario_instructions>".to_string();
    let prefix_c = "<scenario_instructions>scenario_prefix: C</scenario_instructions>".to_string();
    assert_eq!(prefix_texts(&requests[0].input()), vec![prefix_a.clone()]);
    assert_eq!(prefix_texts(&requests[1].input()), vec![prefix_a.clone()]);
    assert_eq!(prefix_texts(&requests[2].input()), vec![prefix_a, prefix_b]);
    assert_eq!(prefix_texts(&replacement), vec![prefix_c.clone()]);
    assert_eq!(prefix_texts(&requests[4].input()), vec![prefix_c]);
    // Updating the policy preserves the entire old request prefix byte-for-byte.
    let unchanged_input = requests[1].input();
    assert_eq!(
        &requests[2].input()[..unchanged_input.len()],
        unchanged_input
    );
    for items in [
        requests[0].input(),
        replacement.clone(),
        requests[4].input(),
    ] {
        let prefix_index = items
            .iter()
            .position(|item| !prefix_texts(std::slice::from_ref(item)).is_empty())
            .expect("prefix");
        let user_index = items
            .iter()
            .position(|item| item["role"] == "user")
            .expect("user context");
        assert!(prefix_index < user_index);
    }
    for request in &requests {
        let body = request.body_json().to_string();
        assert!(!body.contains("rejected_user_prefix"));
        assert!(!body.contains("rejected_assistant_prefix"));
    }

    insta::assert_snapshot!(
        format!("extension_window_prefix_{compaction}"),
        context_snapshot::format_context_snapshot(
            "An extension supplies A, stays unchanged, appends B, then supplies C during compaction. Only C starts the new window. Non-developer prefix fragments are never rendered.",
            &[
                SnapshotEntry::captured(&requests[0]).labeled("Initial policy A"),
                SnapshotEntry::captured(&requests[1]).labeled("Unchanged policy A"),
                SnapshotEntry::captured(&requests[2])
                    .labeled("Policy B appends to existing history"),
                SnapshotEntry::captured(&requests[3]).labeled("Compaction request"),
                SnapshotEntry::items(&replacement).labeled("Stored replacement before next turn"),
                SnapshotEntry::captured(&requests[4])
                    .labeled("Unchanged policy C after compaction"),
            ],
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
