//! Exercises dependency capture through real tool dispatch and admission gates.
//! Channels hold work open and establish overlap without scheduling sleeps.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_core::TurnInputRequest;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolContributor;
use codex_features::Feature;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
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
            "wait_agent"
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

#[test_case::test_case(Admission::Serial, false; "default_disabled")]
#[test_case::test_case(Admission::Unsupported, true; "unsupported_enabled")]
#[test_case::test_case(Admission::Serial, true; "serial_enabled")]
#[test_case::test_case(Admission::Parallel, true; "overlapping_enabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn critical_path_follows_runtime_admission(
    admission: Admission,
    critical_path_timing: bool,
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
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            if critical_path_timing {
                config
                    .features
                    .enable(Feature::CriticalPathTiming)
                    .expect("critical-path capture is configurable");
            }
        })
        .build_with_auto_env(&server)
        .await?;
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
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run both tools".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let (first_id, first_start) = entries.recv().await.expect("first admitted tool");
    let observe = codex_core::test_support::observe_turn_timing(&test.codex)
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
        let (second_id, second_start) = entries.recv().await.expect("second overlapping tool");
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
        let (_, second_start) = entries.recv().await.expect("second serial tool");
        gate.release.add_permits(/* n */ 1);
        (second_start, first_start, held_end)
    };
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    first.single_request();
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
    if matches!(admission, Admission::Unsupported) || !critical_path_timing {
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
