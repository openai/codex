use assert_matches::assert_matches;
use codex_core::CodexThread;
use codex_core::RecoverTurnRequest;
use codex_core::SuspendTurnOutcome;
use codex_core::TurnInputRequest;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_features::Feature;
use codex_history::RolloutItem;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::user_input::UserInput;
use core_test_support::ThreadIdle;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_handler;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_completed;
use core_test_support::responses::sse_failed;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::json;
use test_case::test_case;

/// Integration test: spawn a long‑running exec_command tool via a mocked Responses SSE
/// function call, then interrupt the session and expect TurnAborted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_long_running_tool_emits_turn_aborted() {
    let command = "sleep 60";

    let args = json!({
        "cmd": command,
        "yield_time_ms": 60_000
    })
    .to_string();
    let body = sse(vec![
        ev_function_call("call_sleep", "exec_command", &args),
        ev_completed("done"),
    ]);

    let server = start_mock_server().await;
    mount_sse_once(&server, body).await;

    let fixture = test_codex()
        .with_model("gpt-5.4")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    // Kick off a turn that triggers the function call.
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start sleep".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    // Wait until the exec begins to avoid a race, then interrupt.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    codex.submit(Op::Interrupt).await.unwrap();

    // Expect TurnAborted soon after.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;
    codex.submit(Op::CleanBackgroundTerminals).await.unwrap();
}

const CHILD_RESULT: &str = "the original worker found the issue";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ChildHandoff {
    #[default]
    QueuedBeforeSuspend,
    FinishesDuringSuspend,
    ErrorsDuringSuspend,
    WakesIdleRoot,
    WakeReservedBeforeSuspend,
    ParentWriterClosed,
    GrandchildFinishes,
    ParentEvicted,
}

impl ChildHandoff {
    fn has_grandchild(self) -> bool {
        matches!(self, Self::GrandchildFinishes | Self::ParentEvicted)
    }
    fn wakes_root(self) -> bool {
        matches!(self, Self::WakesIdleRoot | Self::WakeReservedBeforeSuspend)
    }
}

#[derive(Default)]
struct ChildCompletionGate {
    scenario: ChildHandoff,
    child: std::sync::OnceLock<String>,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl ChildCompletionGate {
    fn pause_if(&self, selected: bool) -> ExtensionFuture<'_, ()> {
        Box::pin(async move {
            if selected {
                self.entered.notify_one();
                self.release.notified().await;
            }
        })
    }
}

impl TurnLifecycleContributor for ChildCompletionGate {
    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        let is_root = input.thread_store.level_id() == input.session_store.level_id();
        self.pause_if(
            is_root
                && self.scenario == ChildHandoff::WakeReservedBeforeSuspend
                && self.child.get().is_some(),
        )
    }

    fn on_turn_stop<'a>(&'a self, input: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
        self.pause_if(self.child.get().map(String::as_str) == Some(input.thread_store.level_id()))
    }
}

async fn start_active_tree(
    server: &wiremock::MockServer,
    finishing: Option<&Arc<ChildCompletionGate>>,
    history_mode: ThreadHistoryMode,
) -> anyhow::Result<(TestCodex, String, Arc<CodexThread>, String)> {
    let spawn = |name, message| {
        sse(vec![
            ev_response_created(name),
            ev_function_call_with_namespace(
                name,
                "collaboration",
                "spawn_agent",
                &json!({"message": message, "task_name": name, "fork_turns": "none"}).to_string(),
            ),
            ev_completed(name),
        ])
    };
    mount_sse_once(server, spawn("worker", "keep working until shutdown")).await;
    let wake_root = finishing.is_some_and(|gate| gate.scenario.wakes_root());
    let mut builder = test_codex()
        .with_history_mode(history_mode)
        .with_model("gpt-5.6-sol")
        .with_config(move |config| {
            config.multi_agent_v2.max_concurrent_threads_per_session = 3;
            config
                .features
                .set_enabled(Feature::WakeOnAgentMessages, wake_root)
                .expect("configure root wake");
            config
                .features
                .enable(Feature::Collab)
                .expect("enable collaboration for the suspension test");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent v2 for the suspension test");
        });
    if let Some(gate) = finishing {
        let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
        extensions.turn_lifecycle_contributor(gate.clone());
        extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
        builder = builder.with_extensions(Arc::new(extensions.build()));
    }
    let test = builder.build_with_auto_env(server).await?;
    let root_id = test.session_configured.thread_id.to_string();
    let gate = finishing.cloned();
    mount_response_handler(server, move |request| {
        let done = sse_response(sse_completed("done"));
        let Some(gate) = &gate else {
            return done.set_delay(Duration::from_secs(60));
        };
        let metadata = &request.body_json()["client_metadata"];
        let Some(parent) = metadata["x-codex-parent-thread-id"].as_str() else {
            return done;
        };
        if gate.scenario.has_grandchild() && parent == root_id {
            if request.inputs_of_type("function_call_output").is_empty() {
                return sse_response(spawn("leaf", "investigate the leaf"));
            }
            return if gate.scenario == ChildHandoff::GrandchildFinishes {
                done.set_delay(Duration::from_secs(60))
            } else {
                done
            };
        }
        let id = metadata["thread_id"].as_str().expect("child ID");
        gate.child.set(id.to_owned()).expect("one finishing child");
        if gate.scenario == ChildHandoff::ErrorsDuringSuspend {
            return sse_response(sse_failed("child", "insufficient_quota", "quota exceeded"));
        }
        sse_response(sse(vec![
            ev_assistant_message("child", CHILD_RESULT),
            ev_completed("child"),
        ]))
    })
    .await;
    let codex = Arc::clone(&test.codex);
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    let submitted = codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "preserve this exact unfinished turn".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let codex_core::TurnInputSubmission::Started { turn_id, .. } = submitted else {
        panic!("expected a started root turn");
    };
    let child_id = tokio::time::timeout(Duration::from_secs(10), created_threads.recv()).await??;
    let descendant = test.thread_manager.get_thread(child_id).await?;
    let child_turn = wait_for_event_match(&descendant, |event| match event {
        EventMsg::TurnStarted(turn) => Some(turn.turn_id.clone()),
        _ => None,
    })
    .await;
    if let Some(gate) = finishing {
        ThreadIdle::wait(&test.codex).await;
        tokio::time::timeout(Duration::from_secs(10), gate.entered.notified()).await?;
    }
    Ok((test, turn_id, descendant, child_turn))
}

/// Suspending a root preserves its unfinished turn and initiates child shutdown. The host
/// separately waits for tree cleanup, including any child writer failure, before recovery.
#[test_case(false; "recovers root after stopping child")]
#[test_case(true; "reports child writer failure separately")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_turn_suspension_preserves_unfinished_turn_history(fail_child_writer: bool) {
    let server = start_mock_server().await;
    let (test, turn_id, descendant, _) =
        start_active_tree(&server, /*finishing*/ None, ThreadHistoryMode::Legacy)
            .await
            .expect("start the root and worker");
    let codex = Arc::clone(&test.codex);
    let child_id = descendant.startup_metadata().thread_id;
    if fail_child_writer {
        test.thread_store
            .shutdown_thread(child_id)
            .await
            .expect("close the child writer before shutdown to inject a persistence failure");
    }
    let suspension =
        tokio::time::timeout(Duration::from_secs(10), codex.suspend_turn_and_shutdown())
            .await
            .expect("suspension must not wait for the child turn to finish naturally");
    assert_eq!(
        suspension.expect("suspend the root independently of child cleanup"),
        SuspendTurnOutcome::Suspended {
            turn_id: turn_id.clone(),
        },
    );
    // Observe child termination before requesting the completion handle, so the test proves
    // that root suspension itself initiated child shutdown.
    tokio::time::timeout(Duration::from_secs(10), descendant.wait_until_terminated())
        .await
        .expect("root suspension must initiate child shutdown");
    let shutdown = test
        .thread_manager
        .request_agent_tree_shutdown(test.session_configured.thread_id)
        .await
        .expect("get the tree cleanup completion handle");
    let shutdown_result = tokio::time::timeout(Duration::from_secs(10), shutdown.wait_detailed())
        .await
        .expect("tree cleanup must finish");
    if fail_child_writer {
        let report =
            shutdown_result.expect_err("tree cleanup must report the child writer failure");
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.thread_id == Some(child_id))
        );
        return;
    }
    shutdown_result.expect("finish tree cleanup before handing off the root");
    let child_rollout =
        tokio::fs::read_to_string(descendant.rollout_path().expect("child rollout"))
            .await
            .expect("read closed child writer");
    assert!(child_rollout.lines().any(|line| matches!(
        codex_rollout::parse_rollout_line(line)
            .expect("parse child rollout")
            .item,
        RolloutItem::EventMsg(EventMsg::TurnAborted(_))
    )));
    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = tokio::fs::read_to_string(&rollout_path)
        .await
        .expect("read durable rollout");
    let items = rollout
        .lines()
        .map(|line| {
            codex_rollout::parse_rollout_line(line)
                .expect("parse durable rollout")
                .item
        })
        .collect::<Vec<_>>();
    assert!(items.iter().all(|item| !matches!(
        item,
        RolloutItem::EventMsg(EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_))
    )));
    test.thread_manager
        .remove_thread(&test.session_configured.thread_id)
        .await
        .expect("unload the suspended root");
    let recovery_server = start_mock_server().await;
    mount_sse_once(
        &recovery_server,
        sse(vec![
            ev_response_created("recovered_response"),
            ev_completed("recovered_response"),
        ]),
    )
    .await;
    let resumed = test_codex()
        .with_model("gpt-5.4")
        .resume(&recovery_server, Arc::clone(&test.home), rollout_path)
        .await
        .expect("resume the suspended root on a replacement runtime");

    assert_eq!(
        resumed
            .codex
            .recover_turn_if_idle(codex_core::RecoverTurnRequest {
                turn_id: turn_id.clone(),
                thread_settings: Default::default(),
                trace: None,
                prompt_cache_ttl: None,
                cyber_access_program: None,
            })
            .await
            .expect("recover the unfinished turn"),
        codex_core::StartIfIdleSubmission::Started {
            root_turn_id: turn_id.clone(),
            turn_id: turn_id.clone()
        },
    );
    let completed = wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let EventMsg::TurnComplete(completed) = completed else {
        unreachable!("wait_for_event returned unexpected event");
    };
    assert_eq!(completed.turn_id, turn_id);
    assert_eq!(
        resumed.thread_manager.list_thread_ids().await,
        vec![resumed.session_configured.thread_id],
        "root recovery must not automatically restart the interrupted child"
    );
}

#[test_case(ChildHandoff::QueuedBeforeSuspend; "result already queued")]
#[test_case(ChildHandoff::FinishesDuringSuspend; "result arrives during suspend")]
#[test_case(ChildHandoff::ErrorsDuringSuspend; "error arrives during suspend")]
#[test_case(ChildHandoff::WakesIdleRoot; "result wakes idle root")]
#[test_case(ChildHandoff::WakeReservedBeforeSuspend; "fence wins before wake registration")]
#[test_case(ChildHandoff::ParentWriterClosed; "parent cannot save result")]
#[test_case(ChildHandoff::GrandchildFinishes; "unfinished parent")]
#[test_case(ChildHandoff::ParentEvicted; "evicted idle parent")]
#[tokio::test]
async fn graceful_suspend_preserves_a_finishing_child_result(
    scenario: ChildHandoff,
) -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let gate = Arc::new(ChildCompletionGate {
        scenario,
        ..Default::default()
    });
    let mode = if scenario.wakes_root() || scenario.has_grandchild() {
        ThreadHistoryMode::Legacy
    } else {
        ThreadHistoryMode::Paginated
    };
    let (source, root_turn, worker, worker_turn) =
        start_active_tree(&server, Some(&gate), mode).await?;
    let worker_id = worker.startup_metadata().thread_id;
    match scenario {
        ChildHandoff::QueuedBeforeSuspend => {
            gate.release.notify_one();
            ThreadIdle::wait(&worker).await;
        }
        ChildHandoff::WakeReservedBeforeSuspend => {
            gate.release.notify_one();
            gate.entered.notified().await;
        }
        ChildHandoff::ParentEvicted => {
            ThreadIdle::wait(&worker).await;
            assert_eq!(
                source
                    .thread_manager
                    .try_evict_v2_thread(Arc::clone(&worker))
                    .await?,
                codex_core::ThreadEvictionOutcome::Evicted
            );
        }
        ChildHandoff::ParentWriterClosed => {
            source
                .thread_store
                .shutdown_thread(source.session_configured.thread_id)
                .await?;
        }
        _ => {}
    }
    let drain = source.thread_manager.suspend_agent_tree(&source.codex);
    tokio::pin!(drain);
    if scenario != ChildHandoff::QueuedBeforeSuspend {
        assert!(futures::poll!(drain.as_mut()).is_pending());
        gate.release.notify_one();
    }
    let outcome = tokio::time::timeout(Duration::from_secs(10), drain).await?;
    if scenario == ChildHandoff::ParentWriterClosed {
        assert!(outcome.is_err(), "the result could not be saved");
        return Ok(());
    }
    let tree = outcome?.expect("durable root");
    let child_turns = if scenario == ChildHandoff::GrandchildFinishes {
        HashMap::from([(worker_id, worker_turn.clone())])
    } else {
        HashMap::new()
    };
    assert_eq!(
        tree,
        codex_core::SuspendedAgentTree {
            root_turn_id: None,
            wake_root_after: scenario.wakes_root().then(|| root_turn.clone()),
            child_turns,
        }
    );
    let (parent, sender) = if scenario.has_grandchild() {
        (&worker, "/root/worker/leaf")
    } else {
        (&source.codex, "/root/worker")
    };
    let (history, _, _) = codex_rollout::RolloutRecorder::load_rollout_items(
        &parent.rollout_path().expect("closed parent writer"),
    )
    .await?;
    let messages: Vec<_> = history
        .iter()
        .filter_map(|entry| match entry {
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::AgentMessage {
                    author, content, ..
                } if author == sender => {
                    codex_protocol::models::plaintext_agent_message_content(content)
                }
                _ => None,
            },
            _ => None,
        })
        .collect();
    let [result] = messages.as_slice() else {
        panic!("expected one result, got {messages:?}")
    };
    let payload = if scenario == ChildHandoff::ErrorsDuringSuspend {
        "Agent errored:"
    } else {
        CHILD_RESULT
    };
    assert!(result.contains(payload), "{result}");

    if !scenario.wakes_root() && !scenario.has_grandchild() {
        return Ok(());
    }
    let server = start_mock_server().await;
    let response = mount_sse_once(&server, sse(vec![ev_completed("received")])).await;
    let replacement = test_codex()
        .with_model("gpt-5.6-sol")
        .with_config(|config| {
            config.multi_agent_v2.max_concurrent_threads_per_session = 2;
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable V2");
        })
        .restart_with_auto_env(&server, &source)
        .await?;
    let root = replacement.codex.clone();
    if scenario == ChildHandoff::ParentEvicted {
        // The cold worker has one child slot. Load the grandchild through its real
        // parent and release the parent's slot before admitting the grandchild.
        let leaf_id =
            codex_protocol::ThreadId::from_string(gate.child.get().expect("finishing leaf ID"))?;
        let unrelated = replacement
            .thread_manager
            .start_thread(replacement.start_thread_options().await)
            .await?;
        assert!(
            replacement
                .thread_manager
                .load_suspended_agent(&root, unrelated.thread_id)
                .await
                .is_err()
        );
        let leaf = replacement
            .thread_manager
            .load_suspended_agent(&root, leaf_id)
            .await?;
        assert_eq!(
            leaf.thread.session_source().parent_thread_id(),
            Some(worker_id)
        );
        assert!(
            replacement
                .thread_manager
                .get_thread(worker_id)
                .await
                .is_err()
        );
        return Ok(());
    }
    let parent = if scenario == ChildHandoff::GrandchildFinishes {
        let loaded = replacement
            .thread_manager
            .load_suspended_agent(&root, worker_id)
            .await?;
        assert_matches!(loaded.thread.recover_turn_if_idle(RecoverTurnRequest { turn_id: worker_turn.clone(), thread_settings: Default::default(), trace: None, prompt_cache_ttl: None, cyber_access_program: None }).await?,
            codex_core::StartIfIdleSubmission::Started { turn_id, .. } if turn_id == worker_turn);
        Arc::clone(&loaded.thread)
    } else {
        assert_eq!(
            history
                .iter()
                .filter(|item| matches!(item, RolloutItem::EventMsg(EventMsg::TurnStarted(_))))
                .count(),
            1
        );
        assert_matches!(root.continue_turn_if_idle(TurnInputRequest::user_input(vec![]), root_turn.clone()).await?,
            codex_core::TurnInputSubmission::Started { turn_id, .. } if turn_id != root_turn);
        root
    };
    wait_for_event(&parent, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert_eq!(
        response
            .single_request()
            .inputs_of_type("agent_message")
            .into_iter()
            .filter(|item| item["author"] == sender)
            .map(|item| item["content"].clone())
            .collect::<Vec<_>>(),
        [json!([{"type":"input_text", "text":result}])]
    );
    Ok(())
}

#[tokio::test]
async fn suspended_tree_records_the_original_unfinished_turns() -> anyhow::Result<()> {
    use codex_core::SuspendedAgentTree;

    let source_server = start_mock_server().await;
    let (source, root_turn, child, child_turn) = start_active_tree(
        &source_server,
        /*finishing*/ None,
        ThreadHistoryMode::Paginated,
    )
    .await?;
    // A single poll establishes the writer fence. Dropping the caller on the
    // current-thread runtime leaves all the actual teardown work to the sessions.
    {
        let pending = source.thread_manager.suspend_agent_tree(&source.codex);
        tokio::pin!(pending);
        assert!(futures::poll!(pending.as_mut()).is_pending());
    }
    let expected = SuspendedAgentTree {
        root_turn_id: Some(root_turn.clone()),
        wake_root_after: None,
        child_turns: HashMap::from([(child.startup_metadata().thread_id, child_turn)]),
    };
    for _ in 0..2 {
        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(10),
                source.thread_manager.suspend_agent_tree(&source.codex)
            )
            .await??,
            Some(expected.clone())
        );
    }
    Ok(())
}

/// After an interrupt we expect the next request to the model to include both
/// the original tool call and an `"aborted"` `function_call_output`. This test
/// exercises the follow-up flow: it sends another user turn, inspects the mock
/// responses server, and ensures the model receives the synthesized abort.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_tool_records_history_entries() {
    let command = "sleep 60";
    let call_id = "call-history";

    let args = json!({
        "cmd": command,
        "yield_time_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-history"),
        ev_function_call(call_id, "exec_command", &args),
        ev_completed("resp-history"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-followup"),
        ev_completed("resp-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.4")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start history recording".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "follow up".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = response_mock.requests();
    assert!(
        requests.len() == 2,
        "expected two calls to the responses API, got {}",
        requests.len()
    );

    assert!(
        response_mock.saw_function_call(call_id),
        "function call not recorded in responses payload"
    );
    let output = response_mock
        .function_call_output_text(call_id)
        .expect("missing function_call_output text");
    let re = Regex::new(r"^Wall time: ([0-9]+(?:\.[0-9])?) seconds\naborted by user$")
        .expect("compile regex");
    let captures = re.captures(&output);
    assert_matches!(
        captures.as_ref(),
        Some(caps) if caps.get(1).is_some(),
        "aborted message with elapsed seconds"
    );
    let secs: f32 = captures
        .expect("aborted message with elapsed seconds")
        .get(1)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    assert!(
        secs >= 0.1,
        "expected at least one tenth of a second of elapsed time, got {secs}"
    );
    codex.submit(Op::CleanBackgroundTerminals).await.unwrap();
}

/// After an interrupt we persist a model-visible `<turn_aborted>` marker in the conversation
/// history. This test asserts that the marker is included in the next `/responses` request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_persists_turn_aborted_marker_in_next_request() {
    let command = "sleep 60";
    let call_id = "call-turn-aborted-marker";

    let args = json!({
        "cmd": command,
        "yield_time_ms": 60_000
    })
    .to_string();
    let first_body = sse(vec![
        ev_response_created("resp-marker"),
        ev_function_call(call_id, "exec_command", &args),
        ev_completed("resp-marker"),
    ]);
    let follow_up_body = sse(vec![
        ev_response_created("resp-followup"),
        ev_completed("resp-followup"),
    ]);

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(&server, vec![first_body, follow_up_body]).await;

    let fixture = test_codex()
        .with_model("gpt-5.4")
        .build(&server)
        .await
        .unwrap();
    let codex = Arc::clone(&fixture.codex);

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start interrupt marker".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    codex.submit(Op::Interrupt).await.unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "follow up".into(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2, "expected two calls to the responses API");

    let follow_up_request = &requests[1];
    let user_texts = follow_up_request.message_input_texts("user");
    assert!(
        user_texts
            .iter()
            .any(|text| text.contains("<turn_aborted>")),
        "expected <turn_aborted> marker in follow-up request"
    );
    codex.submit(Op::CleanBackgroundTerminals).await.unwrap();
}
