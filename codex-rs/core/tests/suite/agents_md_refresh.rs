//! Exercises global-instruction refreshes through real turns and provider reads.

use super::*;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tokio::sync::Notify;

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
