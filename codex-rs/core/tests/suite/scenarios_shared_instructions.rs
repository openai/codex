//! Exercises provider sharing and model-visible updates across parent unload and restart.

use super::super::agents_md::RecordingThreadInstructionsProvider;
use super::super::agents_md::expected_provider_only_instruction_fragment;
use super::super::agents_md::instruction_fragments;
use super::super::agents_md::persisted_resume_history;
use super::super::agents_md::submit_thread_turn;
use super::*;
use codex_extension_api::Instructions;
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
    let (_, history) = persisted_resume_history(&root.thread).await?;
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
