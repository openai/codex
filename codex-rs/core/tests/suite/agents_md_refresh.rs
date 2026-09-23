//! Exercises instruction refreshes and concurrent step preparation through real turns.

use super::*;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::SelectedPluginSnapshot;
use codex_extension_api::ToolContributor;
use codex_extension_api::TurnErrorInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tokio::sync::Notify;
use tokio::sync::oneshot;

struct StepPreparationObserver {
    instructions: RecordingThreadInstructionsProvider,
    tools_ready: Mutex<Option<oneshot::Sender<()>>>,
    waiting_for_tools: Mutex<Option<oneshot::Receiver<()>>>,
    published_plugins: Mutex<Vec<(&'static str, bool)>>,
}

impl StepPreparationObserver {
    fn record_plugins(&self, phase: &'static str, turn_store: &ExtensionData) {
        self.published_plugins
            .lock()
            .expect("plugin observations")
            .push((phase, turn_store.get::<SelectedPluginSnapshot>().is_some()));
    }
}

impl ThreadInstructionsProvider for StepPreparationObserver {
    fn load_thread_instructions(&self) -> LoadInstructionsFuture<'_> {
        let ready = self
            .waiting_for_tools
            .lock()
            .expect("instruction gate")
            .take();
        Box::pin(async move {
            if let Some(ready) = ready {
                ready.await.expect("tool preparation must complete");
            }
            self.instructions.load_thread_instructions().await
        })
    }
}

impl ToolContributor for StepPreparationObserver {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        if let Some(ready) = self.tools_ready.lock().expect("tool gate").take() {
            ready
                .send(())
                .expect("instruction refresh must remain live");
        }
        Vec::new()
    }
}

impl TurnLifecycleContributor for StepPreparationObserver {
    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move { self.record_plugins("start", input.turn_store) })
    }

    fn on_turn_error<'a>(&'a self, input: TurnErrorInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move { self.record_plugins("error", input.turn_store) })
    }

    fn on_turn_stop<'a>(&'a self, input: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move { self.record_plugins("stop", input.turn_store) })
    }
}

#[derive(Clone, Copy)]
enum PreparationOutcome {
    Success,
    ToolCollision,
    InstructionError,
    BothErrors,
}

#[test_case::test_case(PreparationOutcome::Success; "publish after both branches succeed")]
#[test_case::test_case(PreparationOutcome::ToolCollision; "warning survives tool failure")]
#[test_case::test_case(PreparationOutcome::InstructionError; "instruction error prevents publication")]
#[test_case::test_case(PreparationOutcome::BothErrors; "instruction error takes precedence")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_preparation_preserves_warnings_errors_and_plugin_publication(
    outcome: PreparationOutcome,
) -> Result<()> {
    let server = start_mock_server().await;
    let request = mount_sse_once(&server, responses::sse_completed("prepared")).await;
    let observer = Arc::new(StepPreparationObserver {
        instructions: RecordingThreadInstructionsProvider::with_text(TASK_USER_INSTRUCTIONS),
        tools_ready: Mutex::default(),
        waiting_for_tools: Mutex::default(),
        published_plugins: Mutex::default(),
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.tool_contributor(observer.clone());
    extensions.turn_lifecycle_contributor(observer.clone());
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.tool_registry.error_on_tool_collisions = true;
            config.update_plan_enabled = true;
        });
    let test = builder.build_with_auto_env(&server).await?;
    let succeeds = matches!(outcome, PreparationOutcome::Success);
    let dynamic_tools = if matches!(
        outcome,
        PreparationOutcome::ToolCollision | PreparationOutcome::BothErrors
    ) {
        vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
            name: "update_plan".to_string(),
            description: "Collides with the built-in planning tool.".to_string(),
            input_schema: json!({"type": "object", "properties": {}}),
            defer_loading: false,
        })]
    } else {
        Vec::new()
    };
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![test.executor_environment().selection().clone()]),
            dynamic_tools,
            thread_instructions_provider: Some(observer.clone()),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    observer
        .instructions
        .set_warnings(vec![PROVIDER_WARNING.to_string()]);
    if matches!(
        outcome,
        PreparationOutcome::InstructionError | PreparationOutcome::BothErrors
    ) {
        observer.instructions.set_instructions(Some(Instructions {
            text: "x".repeat(approx_bytes_for_tokens(/*tokens*/ 10_001)),
            source: None,
        }));
    }
    // The provider finishes only once the other branch has reached tool construction.
    // A tool collision must not cancel the provider or hide its warning/error.
    let (tools_ready, waiting_for_tools) = oneshot::channel();
    *observer.tools_ready.lock().expect("tool gate") = Some(tools_ready);
    *observer.waiting_for_tools.lock().expect("instruction gate") = Some(waiting_for_tools);
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "prepare instructions and tools".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut notices = Vec::new();
    wait_for_event(&thread, |event| {
        match event {
            EventMsg::Warning(warning) => notices.push(("warning", warning.message.clone())),
            EventMsg::Error(error) => notices.push(("error", error.message.clone())),
            _ => {}
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let mut expected_notices = vec![("warning", PROVIDER_WARNING.to_string())];
    let mut expected_publication = vec![("start", false)];
    match outcome {
        PreparationOutcome::Success => {}
        PreparationOutcome::ToolCollision => {
            expected_notices.push(("error", "duplicate tool: functions.update_plan".to_string()));
            expected_publication.push(("error", false));
        }
        PreparationOutcome::InstructionError | PreparationOutcome::BothErrors => {
            expected_notices.push((
                "error",
                "thread instructions exceed the limit of 10000 estimated tokens (10001 estimated tokens provided)".to_string(),
            ));
            expected_publication.push(("error", false));
        }
    }
    expected_publication.push(("stop", succeeds));
    assert_eq!(notices, expected_notices);
    assert_eq!(
        *observer
            .published_plugins
            .lock()
            .expect("plugin observations"),
        expected_publication
    );
    assert_eq!(request.requests().len(), usize::from(succeeds));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_global_read_keeps_instructions_until_recovery() -> Result<()> {
    let server = start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        ["initial", "failed-read", "recovered"]
            .map(responses::sse_completed)
            .to_vec(),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global_file(&home, GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let mut builder = test_codex().with_home(Arc::clone(&home));
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("initial instructions").await?;

    std::fs::remove_file(&source)?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(GLOBAL_AGENTS_FILENAME, &source)?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(GLOBAL_AGENTS_FILENAME, &source)?;
    test.submit_turn("keep instructions through the read failure")
        .await?;
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![PathUri::from_abs_path(&source)],
    );

    std::fs::remove_file(&source)?;
    write_global_file(&home, GLOBAL_AGENTS_FILENAME, NEW_GLOBAL_INSTRUCTIONS)?;
    test.submit_turn("load recovered instructions").await?;
    let initial = expected_provider_only_instruction_fragment(GLOBAL_INSTRUCTIONS);
    let replacement = expected_provider_only_instruction_fragment(&format!(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{NEW_GLOBAL_INSTRUCTIONS}"
    ));
    assert_eq!(
        requests
            .requests()
            .iter()
            .map(instruction_fragments)
            .collect::<Vec<_>>(),
        vec![
            vec![initial.clone()],
            vec![initial.clone()],
            vec![initial, replacement]
        ],
    );
    Ok(())
}

#[test_case::test_case(None; "deleted")]
#[test_case::test_case(Some(" \n"); "blank")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_global_removal_preserves_repository_instructions(
    contents: Option<&str>,
) -> Result<()> {
    let server = start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        ["initial", "removed", "unchanged"]
            .map(responses::sse_completed)
            .to_vec(),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global_file(&home, GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &executor_path_uri(cwd.join(GLOBAL_AGENTS_FILENAME))?,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("initial instructions").await?;
    match contents {
        Some(contents) => std::fs::write(&source, contents)?,
        None => std::fs::remove_file(&source)?,
    }
    test.submit_turn("remove global instructions").await?;
    test.submit_turn("keep repository instructions").await?;
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![test.workspace_path_uri(GLOBAL_AGENTS_FILENAME)?],
    );
    let cwd = &test.executor_environment().selection().cwd;
    let initial = expected_instruction_fragment(
        cwd,
        &format!("{GLOBAL_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}"),
    );
    let replacement = expected_instruction_fragment(
        cwd,
        &format!(
            "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{PROJECT_INSTRUCTIONS}"
        ),
    );
    assert_eq!(
        requests
            .requests()
            .iter()
            .map(instruction_fragments)
            .collect::<Vec<_>>(),
        vec![
            vec![initial.clone()],
            vec![initial.clone(), replacement.clone()],
            vec![initial, replacement]
        ],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_instructions_refresh_after_a_tool_in_the_same_turn() -> Result<()> {
    let server = start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
            ev_response_created("waiting"),
            responses::ev_function_call("pause", "request_user_input", &json!({
                "questions": [{"id": "continue", "header": "Continue", "question": "Continue?",
                    "options": [{"label": "Yes", "description": "Continue the turn."},
                        {"label": "No", "description": "Stop the turn."}]}]
            }).to_string()),
            ev_completed("waiting"),
        ]),
            responses::sse_completed("continued"),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    write_global_file(&home, GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(|config| {
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .expect("test config should allow request-user-input feature");
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "ask before continuing".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let EventMsg::RequestUserInput(request) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RequestUserInput(_))
    })
    .await
    else {
        unreachable!()
    };
    write_global_file(&home, GLOBAL_AGENTS_FILENAME, NEW_GLOBAL_INSTRUCTIONS)?;
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let initial = expected_provider_only_instruction_fragment(GLOBAL_INSTRUCTIONS);
    let replacement = expected_provider_only_instruction_fragment(&format!(
        "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.\n\n{NEW_GLOBAL_INSTRUCTIONS}"
    ));
    assert_eq!(
        requests
            .requests()
            .iter()
            .map(instruction_fragments)
            .collect::<Vec<_>>(),
        vec![vec![initial.clone()], vec![initial, replacement]],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupting_a_provider_read_allows_the_next_turn_to_refresh() -> Result<()> {
    struct GatedProvider {
        inner: CodexHomeUserInstructionsProvider,
        block_next: AtomicBool,
        started: Notify,
    }
    impl UserInstructionsProvider for GatedProvider {
        fn load_user_instructions(&self) -> LoadInstructionsFuture<'_> {
            Box::pin(async move {
                if self.block_next.swap(/*val*/ false, Ordering::SeqCst) {
                    self.started.notify_one();
                    std::future::pending::<()>().await;
                }
                self.inner.load_user_instructions().await
            })
        }
    }
    let server = start_mock_server().await;
    let request = mount_sse_once(&server, responses::sse_completed("next-turn")).await;
    let home = Arc::new(TempDir::new()?);
    write_global_file(&home, GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let provider = Arc::new(GatedProvider {
        inner: CodexHomeUserInstructionsProvider::new(home.path().to_path_buf().abs()),
        block_next: AtomicBool::new(/*v*/ false),
        started: Notify::new(),
    });
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_user_instructions_provider(provider.clone());
    let test = builder.build_with_auto_env(&server).await?;
    provider.block_next.store(/*val*/ true, Ordering::SeqCst);
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "interrupt this blocked read".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    tokio::time::timeout(Duration::from_secs(10), provider.started.notified()).await?;
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    assert!(request.requests().is_empty());
    write_global_file(&home, GLOBAL_AGENTS_FILENAME, NEW_GLOBAL_INSTRUCTIONS)?;
    test.submit_turn("use the latest instructions").await?;
    assert_single_instruction_fragment(
        &request.single_request(),
        &expected_provider_only_instruction_fragment(NEW_GLOBAL_INSTRUCTIONS),
    );
    Ok(())
}
