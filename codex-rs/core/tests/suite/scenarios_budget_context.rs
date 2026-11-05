//! Host budgets and agent session reads use World State.

use super::super::agent_control::HostSessionReader;
use super::super::rollout_budget::HostAccounting;
use super::super::rollout_budget::rollout_budget;
use super::*;
use codex_core::AgentRef;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::PreviousWorldStateSection;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_extension_api::input_text;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentRequests;
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
    // Host accounting and context must override the configured native budget.
    let remaining = Arc::new(AtomicI64::new(1_000));
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.thread_lifecycle_contributor(Arc::new(HostAccounting::default()));
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
            config.rollout_budget = Some(rollout_budget());
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent V2");
        })
        .build_with_auto_env(&server)
        .await?;
    let reader = test
        .codex
        .thread_extension_data()
        .get::<HostSessionReader>()
        .expect("host agent session reader");
    let child = AgentRef {
        thread_id: ThreadId::new(),
        path: AgentPath::root().join("release").expect("child path"),
    };
    *reader.children.lock().expect("host roster") = Some(vec![child.clone()]);

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
    // An unchanged environment does not need to read the roster.
    let reads = reader.children_reads.load(Ordering::Relaxed);
    test.submit_turn("Continue planning.").await?;
    assert_eq!(reader.children_reads.load(Ordering::Relaxed), reads);
    remaining.store(400, Ordering::SeqCst);
    *reader.children.lock().expect("host roster") = Some(vec![
        child,
        AgentRef {
            thread_id: ThreadId::new(),
            path: AgentPath::root().join("testing").expect("child path"),
        },
    ]);
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
            sse(vec![ev_completed("roster-unavailable")]),
        ],
    )
    .await;
    test.submit_turn("Continue from the compacted plan.")
        .await?;
    remaining.store(300, Ordering::SeqCst);
    test.submit_turn("Finish the release plan.").await?;

    // Change the working directory to refresh the environment fragment while
    // preserving the executor and its tools.
    let mut environment = test.executor_environment().request();
    environment.cwd = test.workspace_path_uri("roster-check")?;
    test.fs()
        .create_directory(
            &environment.cwd,
            CreateDirectoryOptions {
                recursive: false,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    *reader.children.lock().expect("host roster") = None;
    let reads = reader.children_reads.load(Ordering::Relaxed);
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Check the unavailable roster.".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.join("roster-check"),
                    vec![environment],
                )),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(reader.children_reads.load(Ordering::Relaxed), reads + 1);

    let requests = received_responses_requests(&server).await;
    assert_eq!(requests.len(), 7);
    for request in &requests {
        assert!(!request.has_content_kinds(&["rollout_budget.remaining_tokens"]));
        let input = serde_json::to_string(&request.input())?;
        assert!(input.contains("You are `/root`"));
    }
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

    let snapshot = context_snapshot::format_request_history_snapshot(
        "Core renders the host's roster with native World State cadence. Compaction rebuilds it from current membership, and the next sampling step refreshes the budget after compaction spending.",
        &requests,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .preserve_environment_context(),
    );
    insta::assert_snapshot!("host_budget_context_after_compaction", snapshot);
    Ok(())
}
