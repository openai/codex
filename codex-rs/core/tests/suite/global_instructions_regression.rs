use std::sync::Arc;

use anyhow::Result;
use codex_core::ForkSnapshot;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const GLOBAL_AGENTS_FILENAME: &str = "AGENTS.md";

fn write_global(home: &TempDir, contents: impl AsRef<[u8]>) -> Result<AbsolutePathBuf> {
    let path = home.path().join(GLOBAL_AGENTS_FILENAME);
    std::fs::write(&path, contents)?;
    AbsolutePathBuf::try_from(path).map_err(Into::into)
}

fn user_instructions(request: &responses::ResponsesRequest) -> String {
    request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions for "))
        .expect("global instructions message")
}

fn local_compaction_provider(server: &wiremock::MockServer) -> ModelProviderInfo {
    let mut provider = built_in_model_providers(/*openai_base_url*/ None)["openai"].clone();
    provider.name = "OpenAI-compatible test provider".to_string();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.supports_websockets = false;
    provider
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_thread_composes_global_before_project_and_reports_sources() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("response-1"),
                responses::ev_completed("response-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("response-2"),
                responses::ev_completed("response-2"),
            ]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_source = write_global(home.as_ref(), "global instructions")?;

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &cwd.join("AGENTS.md"),
                b"project instructions".to_vec(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_remote_env(&server).await?;

    assert_eq!(
        test.codex.instruction_sources().await,
        vec![global_source, test.config.cwd.join("AGENTS.md")]
    );

    test.submit_turn("first turn").await?;
    test.submit_turn("second turn").await?;

    let requests = response_mock.requests();
    let rendered = user_instructions(&requests[0]);
    assert!(
        rendered.find("global instructions") < rendered.find("project instructions"),
        "global instructions should precede project instructions: {rendered}"
    );
    assert!(
        rendered.contains("--- project-doc ---"),
        "global/project boundary should retain the project separator: {rendered}"
    );
    assert_eq!(
        &requests[1].input()[..requests[0].input().len()],
        requests[0].input(),
        "the ordinary second turn should retain the cached prefix"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_loading_warning_surfaces_during_thread_creation() -> Result<()> {
    let server = responses::start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global(home.as_ref(), b"global\xFFinstructions")?;

    let mut builder = test_codex().with_home(home);
    let test = builder.build(&server).await?;

    let warning = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Warning(warning)
            if warning
                .message
                .contains(source.as_path().display().to_string().as_str()) =>
        {
            Some(warning.message.clone())
        }
        _ => None,
    })
    .await;
    assert!(warning.contains("invalid UTF-8"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_resume_replays_rendered_instructions_but_reports_current_config_sources() -> Result<()>
{
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("initial-response"),
                responses::ev_completed("initial-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resumed-response"),
                responses::ev_completed("resumed-response"),
            ]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let old_source = write_global(home.as_ref(), "old global instructions")?;

    let mut initial_builder = test_codex().with_home(Arc::clone(&home));
    let initial = initial_builder.build(&server).await?;
    initial.submit_turn("persist instructions").await?;
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    initial.codex.submit(Op::Shutdown).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;

    std::fs::remove_file(old_source.as_path())?;
    let new_source = write_global(home.as_ref(), "new global instructions")?;
    let mut resume_builder = test_codex().with_home(Arc::clone(&home));
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;

    assert_eq!(
        resumed.codex.instruction_sources().await,
        vec![new_source],
        "resume currently reports sources from the newly loaded config"
    );

    resumed.submit_turn("continue resumed thread").await?;

    let resumed_request = response_mock.requests()[1].body_json().to_string();
    assert!(resumed_request.contains("old global instructions"));
    assert!(!resumed_request.contains("new global instructions"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_replays_rendered_instructions_from_shared_history() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("parent-response"),
                responses::ev_completed("parent-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("fork-response"),
                responses::ev_completed("fork-response"),
            ]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global(home.as_ref(), "old global instructions")?;
    let mut builder = test_codex().with_home(Arc::clone(&home));
    let parent = builder.build(&server).await?;
    parent.submit_turn("persist instructions").await?;
    parent.codex.ensure_rollout_materialized().await;
    parent.codex.flush_rollout().await?;
    let rollout_path = parent.codex.rollout_path().expect("rollout path");

    std::fs::write(source.as_path(), "new global instructions")?;
    let forked = parent
        .thread_manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            parent.config.clone(),
            rollout_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await?;

    forked
        .thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "continue fork".to_string(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&forked.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let rendered = user_instructions(&response_mock.requests()[1]);
    assert!(rendered.contains("old global instructions"));
    assert!(!rendered.contains("new global instructions"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compaction_keeps_the_creation_time_global_instructions() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("first-response"),
                responses::ev_completed("first-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("compact-response"),
                responses::ev_assistant_message("compact-message", "summary"),
                responses::ev_completed("compact-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("follow-up-response"),
                responses::ev_completed("follow-up-response"),
            ]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global(home.as_ref(), "old global instructions")?;
    let provider = local_compaction_provider(&server);
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| {
            config.model_provider = provider;
        });
    let test = builder.build(&server).await?;

    test.submit_turn("first turn").await?;
    std::fs::write(source.as_path(), "new global instructions")?;

    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("after compact").await?;

    let follow_up = user_instructions(&response_mock.requests()[2]);
    assert!(follow_up.contains("old global instructions"));
    assert!(!follow_up.contains("new global instructions"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_compaction_keeps_the_creation_time_global_instructions() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call("call-1", "unsupported_tool", "{}"),
                responses::ev_completed_with_tokens("first-response", /*total_tokens*/ 96),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("compact-message", "summary"),
                responses::ev_completed_with_tokens("compact-response", /*total_tokens*/ 10),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("final-message", "done"),
                responses::ev_completed_with_tokens("follow-up-response", /*total_tokens*/ 10),
            ]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global(home.as_ref(), "old global instructions")?;
    let provider = local_compaction_provider(&server);
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| {
            config.model_provider = provider;
            config.model_context_window = Some(100);
            config.model_auto_compact_token_limit = Some(90);
        });
    let test = builder.build(&server).await?;

    std::fs::write(source.as_path(), "new global instructions")?;
    test.submit_turn("trigger mid-turn compaction").await?;

    let continuation = user_instructions(&response_mock.requests()[2]);
    assert!(continuation.contains("old global instructions"));
    assert!(!continuation.contains("new global instructions"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_resume_rebuilds_from_current_config_after_manual_compaction() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("initial-response"),
                responses::ev_completed("initial-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resumed-response"),
                responses::ev_completed("resumed-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("compact-response"),
                responses::ev_assistant_message("compact-message", "summary"),
                responses::ev_completed("compact-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("post-compact-response"),
                responses::ev_completed("post-compact-response"),
            ]),
        ],
    )
    .await;
    let provider = local_compaction_provider(&server);
    let home = Arc::new(TempDir::new()?);
    let source = write_global(home.as_ref(), "old global instructions")?;
    let mut initial_builder = test_codex().with_home(Arc::clone(&home)).with_config({
        let provider = provider.clone();
        move |config| config.model_provider = provider
    });
    let initial = initial_builder.build(&server).await?;
    initial.submit_turn("persist legacy history").await?;
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    initial.codex.submit(Op::Shutdown).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;

    std::fs::write(source.as_path(), "new global instructions")?;
    let mut resume_builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| config.model_provider = provider);
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;
    resumed.submit_turn("resume legacy history").await?;
    let resumed_rendered = response_mock.requests()[1].body_json().to_string();
    assert!(resumed_rendered.contains("old global instructions"));
    assert!(!resumed_rendered.contains("new global instructions"));

    resumed.codex.submit(Op::Compact).await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    resumed.submit_turn("rebuild full context").await?;

    let rebuilt = user_instructions(&response_mock.requests()[3]);
    assert!(rebuilt.contains("new global instructions"));
    assert!(!rebuilt.contains("old global instructions"));

    Ok(())
}
