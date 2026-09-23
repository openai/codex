//! Observe real dispatch, completion, cancellation, and nested code-mode execution.
use super::*;
use codex_extension_api::ToolDispatchInput;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolTimingBoundary;
use codex_extension_api::ToolTimingInput;
use codex_protocol::protocol::Op;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[derive(Default)]
struct Observations {
    events: Mutex<Vec<(String, &'static str)>>,
    pause: bool,
    started: Notify,
    timed: Notify,
}

impl ToolLifecycleContributor for Observations {
    fn on_tool_dispatch(&self, input: ToolDispatchInput<'_>) {
        self.events
            .lock()
            .expect("observation lock should not be poisoned")
            .push((input.call_id.into(), "dispatch"));
    }

    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            self.events
                .lock()
                .expect("observation lock should not be poisoned")
                .push((input.call_id.into(), "start"));
            self.started.notify_one();
            if self.pause {
                std::future::pending::<()>().await;
            }
        })
    }

    fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            self.events
                .lock()
                .expect("observation lock should not be poisoned")
                .push((input.call_id.into(), "finish"));
        })
    }

    fn on_tool_timing(&self, input: ToolTimingInput<'_>) {
        let boundary = match input.boundary {
            ToolTimingBoundary::Handler => "handler",
            ToolTimingBoundary::HostOperation => "host",
        };
        self.events
            .lock()
            .expect("observation lock should not be poisoned")
            .push((input.call_id.into(), boundary));
        self.timed.notify_one();
    }
}

#[test_case("update_plan", &["dispatch", "start", "finish", "handler"]; "completed")]
#[test_case("missing_tool", &["dispatch", "handler"]; "rejected_before_start")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dispatch_and_timing_surround_tool_execution(
    tool_name: &str,
    expected: &[&str],
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call("plan", tool_name, &json!({"plan": []}).to_string()),
                responses::ev_completed("first"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("answer", "done"),
                responses::ev_completed("last"),
            ]),
        ],
    )
    .await;
    let observations = Arc::new(Observations::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(observations.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| config.update_plan_enabled = true)
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("update the plan").await?;
    assert_eq!(requests.requests().len(), 2);
    assert_eq!(
        *observations
            .events
            .lock()
            .expect("observation lock should not be poisoned"),
        expected
            .iter()
            .map(|event| ("plan".into(), *event))
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_tool_still_reports_handler_timing() -> Result<()> {
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_function_call("plan", "update_plan", &json!({"plan": []}).to_string()),
            responses::ev_completed("first"),
        ]),
    )
    .await;
    let observations = Arc::new(Observations {
        pause: true,
        ..Default::default()
    });
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(observations.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| config.update_plan_enabled = true)
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "update the plan".into(),
            text_elements: vec![],
        }]))
        .await?;
    timeout(Duration::from_secs(10), observations.started.notified()).await?;
    test.codex.submit(Op::Interrupt).await?;
    timeout(Duration::from_secs(10), observations.timed.notified()).await?;
    assert_eq!(requests.requests().len(), 1);
    assert_eq!(
        *observations
            .events
            .lock()
            .expect("observation lock should not be poisoned"),
        vec![
            ("plan".into(), "dispatch"),
            ("plan".into(), "start"),
            ("plan".into(), "finish"),
            ("plan".into(), "handler"),
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_reports_host_timing_without_nested_dispatch_timing() -> Result<()> {
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_custom_tool_call(
                    "exec",
                    "exec",
                    "text(await tools.test_sync_tool({sleep_after_ms: 1}));",
                ),
                responses::ev_completed("first"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("answer", "done"),
                responses::ev_completed("last"),
            ]),
        ],
    )
    .await;
    let observations = Arc::new(Observations::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(observations.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model("test-gpt-5.1-codex")
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("code mode should be enabled");
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("run the nested tool").await?;
    let requests = requests.requests();
    assert_eq!(requests.len(), 2);
    let events = observations
        .events
        .lock()
        .expect("observation lock should not be poisoned");
    let outer = events
        .iter()
        .filter(|(id, _)| id == "exec")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        outer,
        vec![
            ("exec".into(), "dispatch"),
            ("exec".into(), "start"),
            ("exec".into(), "host"),
            ("exec".into(), "finish"),
            ("exec".into(), "handler"),
        ],
        "tool output: {:?}",
        requests[1].custom_tool_call_output("exec")
    );
    let nested = events
        .iter()
        .filter(|(id, _)| id != "exec")
        .map(|(_, event)| *event)
        .collect::<Vec<_>>();
    assert_eq!(nested, vec!["start", "finish"]);
    Ok(())
}
