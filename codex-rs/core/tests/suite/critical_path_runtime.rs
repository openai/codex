//! Exercises per-turn timing through real tool dispatch and admission gates.
//! Channels hold work open and establish overlap without scheduling sleeps.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::IsolatedSessionExtensions;
use codex_extension_api::SessionIsolation;
use codex_extension_api::ToolContributor;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use codex_tools::JsonToolOutput;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use codex_tools::ToolExecutorFuture;
use codex_tools::ToolName;
use codex_tools::ToolOutput;
use codex_tools::ToolSpec;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;

#[derive(Clone, Copy)]
enum Admission {
    Serial,
    Parallel,
    Unsupported,
}

#[derive(Clone, Copy)]
enum CaptureSource {
    Root,
    Guardian,
    Subagent,
    MemoryConsolidation,
}

#[derive(Clone)]
struct GatedTool {
    admission: Admission,
    entered: mpsc::UnboundedSender<(String, Instant)>,
    release: Arc<Semaphore>,
}

impl ToolContributor for GatedTool {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        vec![Arc::new(self.clone())]
    }
}

impl<'call> ToolExecutor<ToolCall<'call>> for GatedTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(if matches!(self.admission, Admission::Unsupported) {
            "followup_task"
        } else {
            "timing_gate"
        })
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(codex_tools::ResponsesApiTool {
            name: self.tool_name().name,
            description: "Wait for the host.".to_string(),
            strict: false,
            parameters: codex_tools::JsonSchema::default(),
            output_schema: None,
            defer_loading: None,
        })
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        matches!(self.admission, Admission::Parallel)
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            self.entered
                .send((call.call_id.to_string(), Instant::now()))
                .expect("test receiver");
            self.release.acquire().await.expect("test gate").forget();
            Ok(
                Box::new(JsonToolOutput::new(serde_json::json!({"done": true})))
                    as Box<dyn ToolOutput>,
            )
        })
    }
}

#[test_case::test_case(Admission::Serial, false, CaptureSource::Root; "default_disabled")]
#[test_case::test_case(Admission::Unsupported, true, CaptureSource::Root; "unsupported_enabled")]
#[test_case::test_case(Admission::Serial, true, CaptureSource::Root; "serial_enabled")]
#[test_case::test_case(Admission::Parallel, true, CaptureSource::Root; "overlapping_enabled")]
#[test_case::test_case(Admission::Serial, true, CaptureSource::Guardian; "guardian_enabled")]
#[test_case::test_case(Admission::Serial, false, CaptureSource::Guardian; "guardian_disabled")]
#[test_case::test_case(Admission::Parallel, true, CaptureSource::Subagent; "subagent_enabled")]
#[test_case::test_case(Admission::Unsupported, true, CaptureSource::Subagent; "subagent_unsupported")]
#[test_case::test_case(Admission::Serial, true, CaptureSource::MemoryConsolidation; "memory_excluded")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn critical_path_follows_runtime_admission(
    admission: Admission,
    critical_path_timing: bool,
    source: CaptureSource,
) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let (entered, mut entries) = mpsc::unbounded_channel();
    let gate = GatedTool {
        admission,
        entered,
        release: Arc::new(Semaphore::new(/* permits */ 0)),
    };
    let name = gate.tool_name().name;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.tool_contributor(Arc::new(gate.clone()));
    let extensions = Arc::new(extensions.build());
    let test = test_codex()
        .with_extensions(extensions.clone())
        .with_config(move |config| {
            if critical_path_timing {
                config
                    .features
                    .enable(Feature::CriticalPathTiming)
                    .expect("critical-path capture is configurable");
            } else {
                config
                    .features
                    .disable(Feature::CriticalPathTiming)
                    .expect("disable capture");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let session_source = match source {
        CaptureSource::Root => SessionSource::Exec,
        CaptureSource::Guardian => SessionSource::Internal(InternalSessionSource::Guardian),
        CaptureSource::Subagent => SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: test.session_configured.thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        }),
        CaptureSource::MemoryConsolidation => {
            SessionSource::Internal(InternalSessionSource::MemoryConsolidation)
        }
    };
    let mut options = StartThreadOptions {
        session_source: Some(session_source),
        environments: Some(vec![test.executor_environment().request()]),
        ..StartThreadOptions::new(test.config.clone())
    };
    if matches!(source, CaptureSource::Guardian) {
        // Guardian inherits neither host extensions nor an unrestricted tool policy.
        options
            .thread_extension_init
            .insert(SessionIsolation::Isolated);
        options
            .thread_extension_init
            .insert(IsolatedSessionExtensions(extensions));
        let mut policy = codex_guardian_reviewer::reviewer_tool_policy();
        policy
            .allowed_tools
            .as_mut()
            .expect("Guardian tool allowlist")
            .push(gate.tool_name());
        options.thread_extension_init.insert(policy);
    }
    let thread = test.thread_manager.start_thread(options).await?.thread;
    let first = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("tools"),
            responses::ev_function_call("a", &name, "{}"),
            responses::ev_function_call("b", &name, "{}"),
            responses::ev_completed("tools"),
        ]),
    )
    .await;
    let start = Instant::now();
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run both tools".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let (first_id, first_start) =
        tokio::time::timeout(Duration::from_secs(/* secs */ 20), entries.recv())
            .await
            .expect("first tool should enter its handler")
            .expect("first admitted tool");
    let observe = codex_core::test_support::observe_turn_timing(&thread)
        .await
        .expect("active turn");
    let reply = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("reply"),
            responses::ev_assistant_message("answer", "Done"),
            responses::ev_completed("reply"),
        ]),
    )
    .await;
    let (second_start, held_start, held_end) = if matches!(admission, Admission::Parallel) {
        let (second_id, second_start) =
            tokio::time::timeout(Duration::from_secs(/* secs */ 20), entries.recv())
                .await
                .expect("second tool should overlap the first")
                .expect("second overlapping tool");
        assert_ne!(first_id, second_id);
        // Both handlers are inside their gate before either can finish.
        let held_end = Instant::now();
        gate.release.add_permits(/* n */ 2);
        (second_start, second_start, held_end)
    } else {
        assert!(
            entries.try_recv().is_err(),
            "exclusive handler still owns admission"
        );
        let held_end = Instant::now();
        gate.release.add_permits(/* n */ 1);
        let (_, second_start) =
            tokio::time::timeout(Duration::from_secs(/* secs */ 20), entries.recv())
                .await
                .expect("second tool should enter after the first releases admission")
                .expect("second serial tool");
        gate.release.add_permits(/* n */ 1);
        (second_start, first_start, held_end)
    };
    wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let first_request = first.single_request();
    if matches!(source, CaptureSource::Guardian) {
        assert_eq!(
            first_request.body_json()["client_metadata"]["x-openai-subagent"],
            "guardian"
        );
    }
    let request = reply.single_request();
    for id in ["a", "b"] {
        assert!(
            request
                .function_call_output(id)
                .to_string()
                .contains("done")
        );
    }
    let end = Instant::now();
    if matches!(admission, Admission::Unsupported)
        || !critical_path_timing
        || matches!(source, CaptureSource::MemoryConsolidation)
    {
        assert_eq!(observe(start, end), None);
    } else {
        let [inference, review, tools, overhead, unknown] =
            observe(start, end).expect("complete direct-tool path");
        assert_eq!(
            observe(held_start, held_end),
            Some([
                Duration::ZERO,
                Duration::ZERO,
                held_end.duration_since(held_start),
                Duration::ZERO,
                Duration::ZERO,
            ])
        );
        assert!(inference > Duration::ZERO);
        assert!(tools >= second_start.duration_since(first_start));
        assert_eq!((review, unknown), (Duration::ZERO, Duration::ZERO));
        assert_eq!(
            inference + review + tools + overhead + unknown,
            end.duration_since(start)
        );
        // Retaining both branches would count their common prefix twice.
        assert!(tools < end.duration_since(start));
    }
    Ok(())
}

#[tokio::test]
async fn standalone_user_shell_omits_uncaptured_path() -> anyhow::Result<()> {
    core_test_support::skip_if_target_windows!(Ok(()), "Unix shell workload");
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::CriticalPathTiming)
                .expect("critical-path capture is configurable");
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .submit(Op::RunUserShellCommand {
            command: "sleep 30".to_string(),
            timeout_ms: None,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExecCommandBegin(_))
    })
    .await;
    let observe = codex_core::test_support::observe_turn_timing(&test.codex)
        .await
        .expect("active turn");
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    let now = Instant::now();
    assert_eq!(observe(now, now), None);
    Ok(())
}

#[derive(Clone, Copy)]
enum AgentMessageTiming {
    InitialOnly,
    DuringTurn,
}

#[test_case::test_case(true, AgentMessageTiming::InitialOnly; "initial_spawn_input")]
#[test_case::test_case(true, AgentMessageTiming::DuringTurn; "midturn_message")]
#[test_case::test_case(false, AgentMessageTiming::InitialOnly; "gate_disabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_child_has_independent_timing_and_parent_wait_is_tools(
    critical_path_timing: bool,
    message_timing: AgentMessageTiming,
) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let (entered, mut entries) = mpsc::unbounded_channel();
    let gate = GatedTool {
        admission: Admission::Serial,
        entered,
        release: Arc::new(Semaphore::new(/* permits */ 0)),
    };
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.tool_contributor(Arc::new(gate.clone()));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("test-gpt-5.1-codex", |model| {
            model.multi_agent_version = Some(MultiAgentVersion::V2);
        })
        .with_config(move |config| {
            for feature in [Feature::Collab, Feature::MultiAgentV2] {
                config.features.enable(feature).expect("enable multi-agent");
            }
            if critical_path_timing {
                config
                    .features
                    .enable(Feature::CriticalPathTiming)
                    .expect("enable capture");
            } else {
                config
                    .features
                    .disable(Feature::CriticalPathTiming)
                    .expect("disable capture");
            }
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("match uncompressed mock requests");
        })
        .build_with_auto_env(&server)
        .await?;
    let root_id = test.session_configured.thread_id;
    let is_root = move |request: &wiremock::Request| {
        request
            .body_json::<serde_json::Value>()
            .is_ok_and(|body| body["client_metadata"]["thread_id"] == serde_json::json!(root_id))
    };
    let is_child = move |request: &wiremock::Request| {
        request.body_json::<serde_json::Value>().is_ok_and(|body| {
            body["client_metadata"]["x-codex-parent-thread-id"] == serde_json::json!(root_id)
        })
    };
    responses::mount_sse_once_match(
        &server,
        is_root,
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "spawn-worker",
                "collaboration",
                "spawn_agent",
                r#"{"task_name":"worker","fork_turns":"none","message":"Run the timing gate."}"#,
            ),
            responses::ev_completed("root-spawn"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        is_root,
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "wait-worker",
                "collaboration",
                "wait_agent",
                "{}",
            ),
            responses::ev_completed("root-wait"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        is_root,
        responses::sse(vec![
            responses::ev_assistant_message("root-answer", "Worker started."),
            responses::ev_completed("root-done"),
        ]),
    )
    .await;
    let child_input = responses::mount_sse_once_match(
        &server,
        is_child,
        responses::sse(vec![
            responses::ev_function_call("child-gate", "timing_gate", "{}"),
            responses::ev_completed("child-tools"),
        ]),
    )
    .await;
    let (sampled, mut sampling_windows) = mpsc::unbounded_channel();
    let child_reply = responses::sse(vec![
        responses::ev_assistant_message("child-answer", "Done."),
        responses::ev_completed("child-done"),
    ]);
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path_regex(".*/responses$"))
        .and(is_child)
        .respond_with(move |request: &wiremock::Request| {
            // The child is sampling and the parent is waiting until this response is sent.
            let start = Instant::now();
            let response = wiremock::ResponseTemplate::new(/* status_code */ 200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(child_reply.clone());
            let end = Instant::now();
            sampled
                .send((
                    start,
                    end,
                    request
                        .body_json::<serde_json::Value>()
                        .expect("child request JSON"),
                ))
                .expect("sampling observer");
            response
        })
        .up_to_n_times(/* n */ 1)
        .mount(&server)
        .await;
    let mut created = test.thread_manager.subscribe_thread_created();
    let start = Instant::now();
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Spawn a worker.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let child_id = tokio::time::timeout(Duration::from_secs(20), created.recv()).await??;
    let child = test.thread_manager.get_thread(child_id).await?;
    let (call_id, _) = tokio::time::timeout(Duration::from_secs(20), entries.recv())
        .await?
        .expect("child tool should enter");
    assert_eq!(call_id, "child-gate");
    let observe = codex_core::test_support::observe_turn_timing(&child)
        .await
        .expect("active child turn");
    assert_eq!(
        observe(start, Instant::now()).is_some(),
        critical_path_timing
    );
    let initial_messages = child_input.single_request().inputs_of_type("agent_message");
    assert_eq!(initial_messages.len(), 1);
    assert_eq!(initial_messages[0]["author"], "/root");
    assert_eq!(initial_messages[0]["recipient"], "/root/worker");
    if matches!(message_timing, AgentMessageTiming::DuringTurn) {
        child
            .submit(Op::InterAgentCommunication {
                communication: InterAgentCommunication::new(
                    AgentPath::root(),
                    AgentPath::root().join("worker").expect("child path"),
                    Vec::new(),
                    "Use this later result too.".into(),
                    /*trigger_turn*/ true,
                ),
                start_options: Default::default(),
            })
            .await?;
        // The response fences the submission queue before the tool can finish.
        child.submit(Op::RealtimeConversationListVoices).await?;
        wait_for_event(&child, |event| {
            matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
        })
        .await;
    }
    wait_for_event(&test.codex, |event| matches!(event,
        EventMsg::ItemStarted(event) if matches!(&event.item, codex_protocol::items::TurnItem::CollabAgentToolCall(item) if item.id == "wait-worker")
    )).await;
    let observe_parent = codex_core::test_support::observe_turn_timing(&test.codex)
        .await
        .expect("waiting parent");
    gate.release.add_permits(/* n */ 1);
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let end = Instant::now();
    let expected_messages = match message_timing {
        AgentMessageTiming::InitialOnly => 1,
        AgentMessageTiming::DuringTurn => 2,
    };
    let (sampling_start, sampling_end, child_request) = sampling_windows
        .recv()
        .await
        .expect("child sampling window");
    assert_eq!(
        child_request["input"]
            .as_array()
            .expect("request input")
            .iter()
            .filter(|item| item["type"] == "agent_message")
            .count(),
        expected_messages
    );
    if critical_path_timing {
        let [inference, review, tools, overhead, unknown] =
            observe(start, end).expect("child's own turn has a complete path");
        assert!(inference > Duration::ZERO);
        assert!(tools > Duration::ZERO);
        assert_eq!((review, unknown), (Duration::ZERO, Duration::ZERO));
        assert_eq!(
            inference + review + tools + overhead + unknown,
            end.duration_since(start)
        );
    } else {
        assert_eq!(observe(start, end), None);
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let parent_end = Instant::now();
    let parent_path = observe_parent(start, parent_end);
    assert_eq!(parent_path.is_some(), critical_path_timing);
    if let Some([inference, review, tools, overhead, unknown]) = parent_path {
        assert_eq!(unknown, Duration::ZERO);
        assert_eq!(
            inference + review + tools + overhead + unknown,
            parent_end.duration_since(start)
        );
        let sampling = sampling_end.duration_since(sampling_start);
        assert!(sampling > Duration::ZERO);
        assert_eq!(
            observe(sampling_start, sampling_end),
            Some([
                sampling,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO
            ])
        );
        assert_eq!(
            observe_parent(sampling_start, sampling_end),
            Some([
                Duration::ZERO,
                Duration::ZERO,
                sampling,
                Duration::ZERO,
                Duration::ZERO
            ])
        );
    }
    child.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
