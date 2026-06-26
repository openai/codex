use std::collections::BTreeMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_features::Feature;
use codex_tools::JsonSchema;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;

use super::extract_running_cell_id;
use super::function_tool_output_items;
use super::test_codex;
use super::text_item;

const CANONICAL_SCRIPT_TOOL_NAME: &str = "canonical_script_tool";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
enum CanonicalToolCall {
    A,
    B,
    C,
    D,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum CanonicalToolEvent {
    Started(CanonicalToolCall),
    Completed(CanonicalToolCall),
    Cancelled(CanonicalToolCall),
}

#[derive(Deserialize)]
struct CanonicalToolArgs {
    call: CanonicalToolCall,
}

#[derive(Clone)]
struct CanonicalScriptTool {
    events_tx: mpsc::UnboundedSender<CanonicalToolEvent>,
    parallel_call_permits: Arc<Semaphore>,
}

impl CanonicalScriptTool {
    fn new() -> (
        Self,
        mpsc::UnboundedReceiver<CanonicalToolEvent>,
        Arc<Semaphore>,
    ) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let parallel_call_permits = Arc::new(Semaphore::new(0));
        (
            Self {
                events_tx,
                parallel_call_permits: Arc::clone(&parallel_call_permits),
            },
            events_rx,
            parallel_call_permits,
        )
    }
}

impl ToolContributor for CanonicalScriptTool {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        vec![Arc::new(self.clone())]
    }
}

impl ToolExecutor<ToolCall> for CanonicalScriptTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(CANONICAL_SCRIPT_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(ResponsesApiTool {
            name: CANONICAL_SCRIPT_TOOL_NAME.to_string(),
            description: "Controllable tool for the canonical code-mode script test.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "call".to_string(),
                    JsonSchema::string(Some("Canonical call identifier.".to_string())),
                )]),
                Some(vec!["call".to_string()]),
                Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, call: ToolCall) -> codex_extension_api::ToolExecutorFuture<'_> {
        let args = call.function_arguments().and_then(|arguments| {
            serde_json::from_str::<CanonicalToolArgs>(arguments).map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to parse canonical script tool arguments: {err}"
                ))
            })
        });
        let events_tx = self.events_tx.clone();
        let parallel_call_permits = Arc::clone(&self.parallel_call_permits);
        Box::pin(async move {
            let args = args?;
            let mut in_flight = InFlightCall::new(args.call, events_tx);
            match args.call {
                CanonicalToolCall::A | CanonicalToolCall::B => {
                    let permit = parallel_call_permits.acquire().await.map_err(|_| {
                        FunctionCallError::Fatal(
                            "canonical script parallel-call gate closed".to_string(),
                        )
                    })?;
                    drop(permit);
                    in_flight.complete();
                    Ok(Box::new(JsonToolOutput::new(json!({ "ok": true }))) as Box<dyn ToolOutput>)
                }
                CanonicalToolCall::C | CanonicalToolCall::D => {
                    std::future::pending::<
                        std::result::Result<Box<dyn ToolOutput>, FunctionCallError>,
                    >()
                    .await
                }
            }
        })
    }
}

struct InFlightCall {
    call: CanonicalToolCall,
    events_tx: mpsc::UnboundedSender<CanonicalToolEvent>,
    completed: bool,
}

impl InFlightCall {
    fn new(call: CanonicalToolCall, events_tx: mpsc::UnboundedSender<CanonicalToolEvent>) -> Self {
        let _ = events_tx.send(CanonicalToolEvent::Started(call));
        Self {
            call,
            events_tx,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
        let _ = self
            .events_tx
            .send(CanonicalToolEvent::Completed(self.call));
    }
}

impl Drop for InFlightCall {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self
                .events_tx
                .send(CanonicalToolEvent::Cancelled(self.call));
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canonical_script_observes_frames_notifications_and_callback_cancellation() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (tool, mut events_rx, parallel_call_permits) = CanonicalScriptTool::new();
    let mut extension_builder = ExtensionRegistryBuilder::<Config>::new();
    extension_builder.tool_contributor(Arc::new(tool));
    let mut builder = test_codex()
        .with_extensions(Arc::new(extension_builder.build()))
        .with_model("test-gpt-5.1-codex")
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("code mode should be enabled");
        });
    let test = builder.build(&server).await?;

    let code = r#"
const tool_call_a = () => tools.canonical_script_tool({ call: "a" });
const tool_call_b = () => tools.canonical_script_tool({ call: "b" });
const tool_call_c = () => tools.canonical_script_tool({ call: "c" });
const tool_call_d = () => tools.canonical_script_tool({ call: "d" });

text("hello world");
yield_control();
text("second frame");
notify("boo");
await Promise.all([tool_call_a(), tool_call_b()]);
text("tool calls done");
setTimeout(tool_call_c, 99999);
tool_call_d();
"#;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_frame_mock = responses::mount_response_once(
        &server,
        responses::sse_response(sse(vec![
            ev_assistant_message("msg-1", "waiting"),
            ev_completed("resp-2"),
        ]))
        .set_delay(Duration::from_secs(2)),
    )
    .await;
    let notification_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "notification received"),
            ev_completed("resp-3"),
        ]),
    )
    .await;

    let turn = test.submit_turn("run the canonical code-mode script");
    tokio::pin!(turn);
    let mut events = Vec::new();
    let mut started_parallel_calls = HashSet::new();
    while started_parallel_calls.len() < 2 {
        let event = tokio::select! {
            result = &mut turn => panic!("turn completed before calls A and B started: {result:?}"),
            event = events_rx.recv() => event.expect("canonical tool event channel closed"),
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                panic!("timed out waiting for canonical calls A and B")
            }
        };
        if let CanonicalToolEvent::Started(call @ (CanonicalToolCall::A | CanonicalToolCall::B)) =
            event
        {
            started_parallel_calls.insert(call);
        }
        events.push(event);
    }
    assert_eq!(
        started_parallel_calls,
        HashSet::from([CanonicalToolCall::A, CanonicalToolCall::B])
    );

    parallel_call_permits.add_permits(2);
    while !events.contains(&CanonicalToolEvent::Cancelled(CanonicalToolCall::D)) {
        let event = tokio::select! {
            result = &mut turn => panic!("turn completed before call D was cancelled: {result:?}"),
            event = events_rx.recv() => event.expect("canonical tool event channel closed"),
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                panic!("timed out waiting for canonical call D cancellation")
            }
        };
        events.push(event);
    }
    turn.await?;

    while let Ok(event) = events_rx.try_recv() {
        events.push(event);
    }
    assert_eq!(events.len(), 6);
    assert_eq!(
        events.iter().copied().collect::<HashSet<_>>(),
        HashSet::from([
            CanonicalToolEvent::Started(CanonicalToolCall::A),
            CanonicalToolEvent::Started(CanonicalToolCall::B),
            CanonicalToolEvent::Started(CanonicalToolCall::D),
            CanonicalToolEvent::Completed(CanonicalToolCall::A),
            CanonicalToolEvent::Completed(CanonicalToolCall::B),
            CanonicalToolEvent::Cancelled(CanonicalToolCall::D),
        ])
    );

    let first_frame_request = first_frame_mock.single_request();
    let first_outputs = exec_outputs(&first_frame_request, "call-1");
    assert_eq!(first_outputs.len(), 1);
    let first_items = output_items(&first_outputs[0]);
    assert_eq!(first_items.len(), 2);
    super::assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&first_items, /*index*/ 0),
    );
    assert_eq!(text_item(&first_items, /*index*/ 1), "hello world");
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    let notification_request = notification_mock.single_request();
    let notification_outputs = exec_outputs(&notification_request, "call-1");
    assert_eq!(notification_outputs.len(), 2);
    assert_eq!(notification_outputs[1]["name"], "exec");
    assert_eq!(notification_outputs[1]["output"], "boo");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-4"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&json!({
                    "cell_id": cell_id,
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-4"),
        ]),
    )
    .await;
    let completion_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "done"),
            ev_completed("resp-5"),
        ]),
    )
    .await;

    test.submit_turn("collect the completed canonical script")
        .await?;

    let completion_request = completion_mock.single_request();
    let completion_items = function_tool_output_items(&completion_request, "call-2");
    assert_eq!(completion_items.len(), 3);
    super::assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&completion_items, /*index*/ 0),
    );
    assert_eq!(text_item(&completion_items, /*index*/ 1), "second frame");
    assert_eq!(text_item(&completion_items, /*index*/ 2), "tool calls done");
    while let Ok(event) = events_rx.try_recv() {
        events.push(event);
    }
    assert_eq!(events.len(), 6);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, CanonicalToolEvent::Started(CanonicalToolCall::C)))
    );

    Ok(())
}

fn exec_outputs(request: &responses::ResponsesRequest, call_id: &str) -> Vec<Value> {
    request
        .inputs_of_type("custom_tool_call_output")
        .into_iter()
        .filter(|item| item.get("call_id").and_then(Value::as_str) == Some(call_id))
        .collect()
}

fn output_items(output: &Value) -> Vec<Value> {
    output
        .get("output")
        .and_then(Value::as_array)
        .expect("exec output should contain content items")
        .clone()
}
