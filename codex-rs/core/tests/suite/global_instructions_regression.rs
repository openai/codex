use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use codex_core::ForkSnapshot;
use codex_features::Feature;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

const GLOBAL_AGENTS_FILENAME: &str = "AGENTS.md";
const GLOBAL_AGENTS_OVERRIDE_FILENAME: &str = "AGENTS.override.md";
const GLOBAL_INSTRUCTIONS: &str = "global instructions";
const NEW_GLOBAL_INSTRUCTIONS: &str = "new global instructions";
const OLD_GLOBAL_INSTRUCTIONS: &str = "old global instructions";
const PROJECT_INSTRUCTIONS: &str = "project instructions";
const PROJECT_SEPARATOR: &str = "--- project-doc ---";
const SPAWN_CALL_ID: &str = "spawn-global-instructions-child";
const SPAWN_CHILD_PROMPT: &str = "inspect inherited global instructions";
const SPAWN_PARENT_PROMPT: &str = "spawn a child with the parent context";
const SPAWN_SEED_PROMPT: &str = "seed parent history";

fn write_global(home: &TempDir, contents: impl AsRef<[u8]>) -> Result<AbsolutePathBuf> {
    write_global_file(home, GLOBAL_AGENTS_FILENAME, contents)
}

fn write_global_override(home: &TempDir, contents: impl AsRef<[u8]>) -> Result<AbsolutePathBuf> {
    write_global_file(home, GLOBAL_AGENTS_OVERRIDE_FILENAME, contents)
}

fn write_global_file(
    home: &TempDir,
    filename: &str,
    contents: impl AsRef<[u8]>,
) -> Result<AbsolutePathBuf> {
    let path = home.path().join(filename);
    std::fs::write(&path, contents)?;
    AbsolutePathBuf::try_from(path).map_err(Into::into)
}

fn instruction_fragments(request: &responses::ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("# AGENTS.md instructions for "))
        .collect()
}

fn expected_instruction_fragment(cwd: &AbsolutePathBuf, contents: &str) -> String {
    let cwd = cwd.as_path().display();
    format!("# AGENTS.md instructions for {cwd}\n\n<INSTRUCTIONS>\n{contents}\n</INSTRUCTIONS>")
}

fn assert_single_instruction_fragment(request: &responses::ResponsesRequest, expected: &str) {
    assert_eq!(instruction_fragments(request), vec![expected.to_string()]);
}

fn request_body_contains(request: &wiremock::Request, text: &str) -> bool {
    let is_zstd = request
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    let body = if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&request.body)).ok()
    } else {
        Some(request.body.clone())
    };
    body.and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
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
    let global_source = write_global(home.as_ref(), GLOBAL_INSTRUCTIONS)?;

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_workspace_setup(|cwd, fs| async move {
            fs.write_file(
                &cwd.join("AGENTS.md"),
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
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
    let expected_contents =
        format!("{GLOBAL_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}");
    let expected_fragment = expected_instruction_fragment(&test.config.cwd, &expected_contents);
    let fragments = instruction_fragments(&requests[0]);
    assert_eq!(fragments, vec![expected_fragment]);
    let rendered = fragments
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("expected one rendered instruction fragment"))?;
    let global_position = rendered.find(GLOBAL_INSTRUCTIONS).ok_or_else(|| {
        anyhow!(
            "expected rendered instructions to contain {GLOBAL_INSTRUCTIONS:?}; observed: {rendered}"
        )
    })?;
    let project_position = rendered.find(PROJECT_INSTRUCTIONS).ok_or_else(|| {
        anyhow!(
            "expected rendered instructions to contain {PROJECT_INSTRUCTIONS:?}; observed: {rendered}"
        )
    })?;
    assert!(
        global_position < project_position,
        "global instructions should precede project instructions: {rendered}"
    );
    assert!(
        rendered.contains(PROJECT_SEPARATOR),
        "expected rendered instructions to contain {PROJECT_SEPARATOR:?}; observed: {rendered}"
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
    assert!(
        warning.contains("invalid UTF-8"),
        "expected warning to contain \"invalid UTF-8\"; observed: {warning}"
    );

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
    let old_source = write_global(home.as_ref(), OLD_GLOBAL_INSTRUCTIONS)?;

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

    let new_source = write_global_override(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_ne!(old_source, new_source);
    let mut resume_builder = test_codex().with_home(Arc::clone(&home));
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;

    assert_eq!(
        resumed.codex.instruction_sources().await,
        vec![new_source],
        "resume reports sources from the newly loaded config"
    );

    resumed.submit_turn("continue resumed thread").await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let initial_input = requests[0].input();
    let resumed_input = requests[1].input();
    assert_eq!(
        resumed_input.get(..initial_input.len()),
        Some(initial_input.as_slice()),
        "cold resume should replay the original structured input prefix"
    );
    let expected_fragment =
        expected_instruction_fragment(&initial.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[0], &expected_fragment);
    assert_single_instruction_fragment(&requests[1], &expected_fragment);

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
    let source = write_global(home.as_ref(), OLD_GLOBAL_INSTRUCTIONS)?;
    let mut builder = test_codex().with_home(Arc::clone(&home));
    let parent = builder.build(&server).await?;
    parent.submit_turn("persist instructions").await?;
    parent.codex.ensure_rollout_materialized().await;
    parent.codex.flush_rollout().await?;
    let rollout_path = parent.codex.rollout_path().expect("rollout path");

    let new_source = write_global_override(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_ne!(source, new_source);
    let mut fork_config = load_default_config_for_test(home.as_ref()).await;
    fork_config.cwd = parent.config.cwd.clone();
    fork_config.model = parent.config.model.clone();
    fork_config.model_provider = parent.config.model_provider.clone();
    fork_config.model_catalog = parent.config.model_catalog.clone();
    fork_config.codex_self_exe = parent.config.codex_self_exe.clone();
    let forked = parent
        .thread_manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            fork_config,
            rollout_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await?;
    assert_eq!(
        forked.thread.instruction_sources().await,
        vec![new_source],
        "fork config should reflect the newly loaded global source"
    );

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

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let parent_input = requests[0].input();
    let fork_input = requests[1].input();
    assert_eq!(
        fork_input.get(..parent_input.len()),
        Some(parent_input.as_slice()),
        "fork should replay the parent's original structured input prefix"
    );
    let expected_fragment =
        expected_instruction_fragment(&parent.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[0], &expected_fragment);
    assert_single_instruction_fragment(&requests[1], &expected_fragment);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forked_subagent_replays_one_creation_time_global_instruction_fragment() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let seed_mock = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| request_body_contains(request, SPAWN_SEED_PROMPT),
        responses::sse(vec![
            responses::ev_response_created("seed-response"),
            responses::ev_assistant_message("seed-message", "seeded"),
            responses::ev_completed("seed-response"),
        ]),
    )
    .await;
    let spawn_args = serde_json::to_string(&json!({
        "message": SPAWN_CHILD_PROMPT,
        "fork_context": true,
    }))?;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| request_body_contains(request, SPAWN_PARENT_PROMPT),
        responses::sse(vec![
            responses::ev_response_created("spawn-response"),
            responses::ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                "multi_agent_v1",
                "spawn_agent",
                &spawn_args,
            ),
            responses::ev_completed("spawn-response"),
        ]),
    )
    .await;
    let child_mock = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            request_body_contains(request, SPAWN_CHILD_PROMPT)
                && !request_body_contains(request, SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("child-response"),
            responses::ev_assistant_message("child-message", "done"),
            responses::ev_completed("child-response"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| request_body_contains(request, SPAWN_CALL_ID),
        responses::sse(vec![
            responses::ev_response_created("spawn-follow-up-response"),
            responses::ev_assistant_message("spawn-follow-up-message", "child started"),
            responses::ev_completed("spawn-follow-up-response"),
        ]),
    )
    .await;

    let home = Arc::new(TempDir::new()?);
    write_global(home.as_ref(), OLD_GLOBAL_INSTRUCTIONS)?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(|config| {
            let _ = config.features.enable(Feature::Collab);
            let _ = config.features.disable(Feature::EnableRequestCompression);
        });
    let test = builder.build(&server).await?;
    test.submit_turn(SPAWN_SEED_PROMPT).await?;
    let seed_request = seed_mock.single_request();

    write_global_override(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    test.submit_turn(SPAWN_PARENT_PROMPT).await?;
    let child_request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(request) = child_mock.requests().into_iter().next() {
                break request;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| anyhow!("timed out waiting for the forked subagent request"))?;

    let expected_fragment =
        expected_instruction_fragment(&test.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&seed_request, &expected_fragment);
    assert_single_instruction_fragment(&child_request, &expected_fragment);
    let seed_input = seed_request.input();
    let child_input = child_request.input();
    assert_eq!(
        child_input.get(..seed_input.len()),
        Some(seed_input.as_slice()),
        "forked subagent should replay the parent's original structured input prefix"
    );

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
    let source = write_global(home.as_ref(), OLD_GLOBAL_INSTRUCTIONS)?;
    let provider = local_compaction_provider(&server);
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| {
            config.model_provider = provider;
        });
    let test = builder.build(&server).await?;

    test.submit_turn("first turn").await?;
    std::fs::write(source.as_path(), NEW_GLOBAL_INSTRUCTIONS)?;

    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("after compact").await?;

    let requests = response_mock.requests();
    let expected_fragment =
        expected_instruction_fragment(&test.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[2], &expected_fragment);

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
    let source = write_global(home.as_ref(), OLD_GLOBAL_INSTRUCTIONS)?;
    let provider = local_compaction_provider(&server);
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| {
            config.model_provider = provider;
            config.model_context_window = Some(100);
            config.model_auto_compact_token_limit = Some(90);
        });
    let test = builder.build(&server).await?;

    std::fs::write(source.as_path(), NEW_GLOBAL_INSTRUCTIONS)?;
    test.submit_turn("trigger mid-turn compaction").await?;

    let requests = response_mock.requests();
    let expected_fragment =
        expected_instruction_fragment(&test.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[2], &expected_fragment);

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
    let source = write_global(home.as_ref(), OLD_GLOBAL_INSTRUCTIONS)?;
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

    std::fs::write(source.as_path(), NEW_GLOBAL_INSTRUCTIONS)?;
    let mut resume_builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| config.model_provider = provider);
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;
    resumed.submit_turn("resume legacy history").await?;
    let requests = response_mock.requests();
    let old_fragment = expected_instruction_fragment(&initial.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[1], &old_fragment);

    resumed.codex.submit(Op::Compact).await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    resumed.submit_turn("rebuild full context").await?;

    let requests = response_mock.requests();
    let new_fragment = expected_instruction_fragment(&resumed.config.cwd, NEW_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[3], &new_fragment);

    Ok(())
}
