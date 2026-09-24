//! Exercises provider sharing and model-visible updates across parent unload and restart.

use super::super::agents_md::RecordingThreadInstructionsProvider;
use super::super::agents_md::expected_provider_only_instruction_fragment;
use super::super::agents_md::instruction_fragments;
use super::super::agents_md::persisted_resume_history;
use super::super::agents_md::submit_thread_turn;
use super::*;
use codex_core::StartThreadOptions;
use codex_core::config::Constrained;
use codex_extension_api::Instructions;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use pretty_assertions::assert_eq;

const INITIAL: &str = "Ask before sending email.";
const UPDATED: &str = "Never send email.";

#[test_case::test_case(false; "snapshot_by_default")]
#[test_case::test_case(true; "shared_when_opted_in")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn running_descendants_refresh_only_shared_thread_instructions(shared: bool) -> Result<()> {
    let server = start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let provider = RecordingThreadInstructionsProvider::with_text(INITIAL);
    let provider = Arc::new(if shared { provider.shared() } else { provider });
    let root = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![test.executor_environment().selection().clone()]),
            thread_instructions_provider: Some(provider.clone()),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    let mut parent_id = root.thread_id;
    let mut descendants = Vec::new();
    for depth in 1..=2 {
        let child = test
            .thread_manager
            .start_thread(StartThreadOptions {
                environments: Some(vec![test.executor_environment().selection().clone()]),
                session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: parent_id,
                    depth,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
                ..StartThreadOptions::new(test.config.clone())
            })
            .await?;
        parent_id = child.thread_id;
        descendants.push(child.thread);
    }
    let other_provider =
        Arc::new(RecordingThreadInstructionsProvider::with_text("Other root rules").shared());
    let other_root = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![test.executor_environment().selection().clone()]),
            thread_instructions_provider: Some(other_provider),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    let root_response = mount_sse_once(&server, responses::sse_completed("root-step")).await;
    submit_thread_turn(&root.thread, "persist the root before it unloads").await?;
    root_response.single_request();
    let (_, history) = persisted_resume_history(&root.thread, test.thread_store.as_ref()).await?;
    root.thread.shutdown_and_wait().await?;
    assert!(
        test.thread_manager
            .remove_thread(&root.thread_id)
            .await
            .is_some()
    );
    assert!(
        test.thread_manager
            .get_thread(root.thread_id)
            .await
            .is_err()
    );
    drop(root);
    let previous_provider = provider;
    let provider = RecordingThreadInstructionsProvider::with_text(INITIAL);
    let provider = Arc::new(if shared { provider.shared() } else { provider });
    let _restarted_root = test
        .thread_manager
        .start_thread(StartThreadOptions {
            initial_history: history,
            environments: Some(vec![test.executor_environment().selection().clone()]),
            thread_instructions_provider: Some(provider.clone()),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    previous_provider.set_instructions(Some(Instructions {
        text: "Old provider must not be used".to_string(),
        source: None,
    }));
    let previous_loads = previous_provider.load_count();

    let replacement = format!(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{UPDATED}"
    );
    let updates = [
        (Some(INITIAL), INITIAL),
        (Some(UPDATED), replacement.as_str()),
        (
            None,
            "The previously provided AGENTS.md instructions no longer apply.",
        ),
        (None, ""),
    ];
    let loads_before = provider.load_count();
    let mut request_history = Vec::new();
    // Both descendants stayed alive across the restart and must switch without another root turn.
    for descendant in descendants {
        let mut expected = Vec::new();
        for (contents, appended_fragment) in updates {
            provider.set_instructions(contents.map(|text| Instructions {
                text: text.to_owned(),
                source: None,
            }));
            let response = mount_sse_once(&server, responses::sse_completed("child-step")).await;
            submit_thread_turn(&descendant, "continue with the current instructions").await?;
            if expected.is_empty() || (shared && !appended_fragment.is_empty()) {
                expected.push(expected_provider_only_instruction_fragment(
                    appended_fragment,
                ));
            }
            let request = response.single_request();
            assert_eq!(instruction_fragments(&request), expected);
            request_history.push(request);
        }
    }
    assert_eq!(provider.load_count() > loads_before, shared);
    assert_eq!(previous_provider.load_count(), previous_loads);
    let other_response = mount_sse_once(&server, responses::sse_completed("other-root-step")).await;
    submit_thread_turn(&other_root.thread, "check unrelated rules").await?;
    assert_eq!(
        instruction_fragments(&other_response.single_request()),
        vec![expected_provider_only_instruction_fragment(
            "Other root rules"
        )]
    );
    if shared {
        insta::assert_snapshot!(
            "shared_instructions_update_running_descendants",
            context_snapshot::format_request_history_snapshot(
                "A host replaces the provider after the parent restarts, then updates and clears instructions for surviving children and grandchildren.",
                &request_history,
                &ContextSnapshotOptions::default(),
            )
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_tracks_shared_instruction_updates_in_running_descendants() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Change the host's rules after each action is sampled, before its review.
    // The worker must refresh on its next request; Guardian must review the
    // applied snapshot, without independently polling the live provider.
    struct UpdateRulesOnAction(Arc<RecordingThreadInstructionsProvider>);

    impl ToolLifecycleContributor for UpdateRulesOnAction {
        fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
            Box::pin(async move {
                let instructions = match input.call_id {
                    "initial-action" => Some(Instructions {
                        text: UPDATED.to_owned(),
                        source: None,
                    }),
                    "updated-action" => None,
                    _ => return,
                };
                self.0.set_instructions(instructions);
            })
        }
    }

    let server = start_mock_server().await;
    let provider = Arc::new(RecordingThreadInstructionsProvider::with_text(INITIAL).shared());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(Arc::new(UpdateRulesOnAction(provider.clone())));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
    let root = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![test.executor_environment().selection().clone()]),
            thread_instructions_provider: Some(provider),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    let mut descendant = root;
    for depth in 1..=2 {
        descendant = test
            .thread_manager
            .start_thread(StartThreadOptions {
                environments: Some(vec![test.executor_environment().selection().clone()]),
                session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: descendant.thread_id,
                    depth,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
                ..StartThreadOptions::new(test.config.clone())
            })
            .await?;
    }

    let mut responses = Vec::new();
    for call_id in ["initial-action", "updated-action", "cleared-action"] {
        responses.push(sse(vec![
            ev_response_created(call_id),
            responses::ev_function_call(
                call_id,
                "exec_command",
                &json!({
                    "cmd": format!("echo {call_id}"),
                    "sandbox_permissions": "require_escalated",
                    "justification": "Review this action against the current rules.",
                })
                .to_string(),
            ),
            ev_completed(call_id),
        ]));
        responses.push(sse(vec![
            ev_assistant_message("guardian", r#"{"outcome":"deny"}"#),
            ev_completed(&format!("review-{call_id}")),
        ]));
        responses.push(responses::sse_completed(&format!("finished-{call_id}")));
    }
    let requests = mount_sse_sequence(&server, responses).await;
    // Neither ancestor takes another turn during the update and removal.
    for prompt in [
        "Check the initial rules.",
        "Check the updated rules.",
        "Check after removal.",
    ] {
        submit_thread_turn(&descendant.thread, prompt).await?;
    }
    let requests = requests.requests();
    assert_eq!(requests.len(), 9);
    let initial = format!("# AGENTS.md instructions\n\n<INSTRUCTIONS>\n{INITIAL}\n</INSTRUCTIONS>");
    let updated = format!("# AGENTS.md instructions\n\n<INSTRUCTIONS>\n{UPDATED}\n</INSTRUCTIONS>");
    let replacement = format!(
        "# AGENTS.md instructions\n\n<INSTRUCTIONS>\nThese AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{UPDATED}\n</INSTRUCTIONS>"
    );
    let cleared = "# AGENTS.md instructions\n\n<INSTRUCTIONS>\nThe previously provided AGENTS.md instructions no longer apply.\n</INSTRUCTIONS>".to_owned();
    assert_eq!(
        requests
            .iter()
            .map(instruction_fragments)
            .collect::<Vec<_>>(),
        vec![
            vec![initial.clone()],
            vec![initial.clone()],
            vec![initial.clone(), replacement.clone()],
            vec![initial.clone(), replacement.clone()],
            vec![updated],
            vec![initial.clone(), replacement.clone(), cleared.clone()],
            vec![initial.clone(), replacement.clone(), cleared.clone()],
            vec![],
            vec![initial, replacement, cleared],
        ],
    );
    let reviewers = [1, 4, 7].map(|index| {
        let metadata = requests[index].body_json()["client_metadata"].clone();
        assert_eq!(metadata["x-openai-subagent"], "guardian");
        metadata["thread_id"]
            .as_str()
            .expect("reviewer thread id")
            .to_owned()
    });
    assert_ne!(reviewers[0], reviewers[1]);
    assert_ne!(reviewers[1], reviewers[2]);
    Ok(())
}
