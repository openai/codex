//! Host budget reminders use generic World State and refresh after compaction spending.

use super::*;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_extension_api::input_text;
use core_test_support::responses::received_responses_requests;
use core_test_support::responses::sse_response;
use core_test_support::responses::strip_metadata_from_json;
use core_test_support::responses::strip_response_item_ids_from_json;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

struct HostBudgetContext {
    remaining: Arc<AtomicI64>,
}

impl ContextContributor for HostBudgetContext {
    fn contribute_world_state<'a>(
        &'a self,
        _input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        let remaining = self.remaining.load(Ordering::SeqCst);
        let threshold = [750, 400, 100]
            .into_iter()
            .filter(|threshold| remaining <= *threshold)
            .count();
        Box::pin(async move {
            vec![
                WorldStateSectionContribution::new("host_budget_reminder", move |previous| {
                    if let PreviousWorldStateSection::Known(previous) = previous
                        && previous.as_u64().is_some_and(|index| index >= threshold as u64)
                    {
                        return (None, None);
                    }
                    (
                        Some(json!(threshold)),
                        Some(RenderedWorldStateFragment::new(
                            "developer",
                            ("<rollout_budget>\n", "\n</rollout_budget>"),
                            input_text(format!(
                                "You have {remaining} weighted tokens left in the shared session token budget."
                            )),
                        )),
                    )
                })
                .with_retained_fragment_matcher(|role, text| {
                    role == "developer" && text.contains("<rollout_budget>")
                })
                .without_full_context(),
            ]
        })
    }
}

// At 400 the cutoff section opts out. At 350 and 325 the install section opts out.
// Both later values share a comparison bucket, so rendering 325 proves the markers were cleared.
pub(super) struct PrefixBudgetReminders {
    pub(super) remaining: AtomicI64,
}

impl ContextContributor for PrefixBudgetReminders {
    fn contribute_world_state<'a>(
        &'a self,
        _input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        let remaining = self.remaining.load(Ordering::SeqCst);
        let section = move |id: &'static str| {
            WorldStateSectionContribution::new(id, move |previous| {
                let threshold = json!(remaining / 100);
                if matches!(previous, PreviousWorldStateSection::Known(value) if value == &threshold)
                {
                    return (None, None);
                }
                (
                    Some(threshold),
                    Some(RenderedWorldStateFragment::new(
                        "developer",
                        ("", ""),
                        input_text(format!("{id}: {remaining} tokens remain.")),
                    )),
                )
            })
        };
        Box::pin(async move {
            let cutoff = section("cutoff_budget");
            let install = section("install_budget");
            if remaining == 400 {
                vec![cutoff.without_full_context(), install]
            } else {
                vec![cutoff, install.without_full_context()]
            }
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_budget_context_refreshes_after_remote_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    // This fixture supplies host-owned policy and controlled shared spending. It does
    // not implement usage reporting or configure Local's native rollout budget.
    let remaining = Arc::new(AtomicI64::new(1_000));
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.prompt_contributor(Arc::new(HostBudgetContext {
        remaining: Arc::clone(&remaining),
    }));
    let test = test_codex()
        .with_model("gpt-6-astra")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            configure_scenario_catalog(config);
            config.workspace_roots = vec![config.cwd.clone()];
            config.rollout_budget = None;
        })
        .build_with_auto_env(&server)
        .await?;

    mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("initial")]),
            sse(vec![ev_completed("same-threshold")]),
            sse(vec![ev_completed("crossed-threshold")]),
        ],
    )
    .await;
    test.submit_turn("Start planning the release.").await?;
    remaining.store(900, Ordering::SeqCst);
    test.submit_turn("Continue planning.").await?;
    remaining.store(400, Ordering::SeqCst);
    test.submit_turn("Check the release plan.").await?;

    let compaction_budget = Arc::clone(&remaining);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(move |_: &wiremock::Request| {
            // World State was already captured at 400 before this request. Simulate
            // compaction spending before Core installs that captured snapshot.
            compaction_budget.store(350, Ordering::SeqCst);
            sse_response(sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "BUDGET_CONTEXT_CHECKPOINT",
                    },
                }),
                ev_completed("compact-response"),
            ]))
        })
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("fresh-window")]),
            sse(vec![ev_completed("same-fresh-threshold")]),
        ],
    )
    .await;
    test.submit_turn("Continue from the compacted plan.")
        .await?;
    remaining.store(300, Ordering::SeqCst);
    test.submit_turn("Finish the release plan.").await?;

    let requests = received_responses_requests(&server).await;
    assert_eq!(requests.len(), 6);
    // Compare the actual wire input without normalization: both unchanged and
    // threshold-crossing reminders must only append within a context window.
    for (previous, next) in [(0, 1), (1, 2), (4, 5)] {
        let previous_input = requests[previous].input();
        let next_input = requests[next].input();
        assert_eq!(
            next_input.get(..previous_input.len()),
            Some(previous_input.as_slice()),
            "request {next} must preserve request {previous}'s entire input prefix",
        );
        for field in ["prompt_cache_key", "instructions", "tools"] {
            assert_eq!(
                requests[next].body_json()[field],
                requests[previous].body_json()[field],
                "request {next} must preserve request {previous}'s {field}",
            );
        }
    }
    let resumed_input = requests[4].input();
    // Compaction rebuilds item IDs and metadata while preserving the tool
    // declarations and base instructions ahead of the conversation.
    let initial_prefix = serde_json::Value::Array(requests[0].input()[..2].to_vec());
    let resumed_prefix = serde_json::Value::Array(resumed_input[..2].to_vec());
    assert_eq!(
        strip_metadata_from_json(strip_response_item_ids_from_json(initial_prefix)),
        strip_metadata_from_json(strip_response_item_ids_from_json(resumed_prefix)),
    );
    insta::assert_snapshot!(
        "host_budget_context_after_compaction",
        context_snapshot::format_request_history_snapshot(
            "A host contributes budget reminders at thresholds. Remote compaction spends within the same threshold, so its captured reminder is omitted and the next sampling step inserts the fresh remainder once.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}
