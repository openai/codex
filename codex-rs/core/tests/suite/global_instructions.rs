use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use codex_core::ForkSnapshot;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

const GLOBAL_AGENTS_FILENAME: &str = "AGENTS.md";
const GLOBAL_AGENTS_OVERRIDE_FILENAME: &str = "AGENTS.override.md";
const GLOBAL_INSTRUCTIONS: &str = "global instructions";
const NEW_GLOBAL_INSTRUCTIONS: &str = "new global instructions";
const NEW_PROJECT_INSTRUCTIONS: &str = "new project instructions";
const OLD_GLOBAL_INSTRUCTIONS: &str = "old global instructions";
const PROJECT_INSTRUCTIONS: &str = "project instructions";
const PROJECT_SEPARATOR: &str = "--- project-doc ---";
const REMOTE_V2_SUMMARY: &str = "global-instructions-remote-v2-summary";
const SPAWN_CALL_ID: &str = "spawn-global-instructions-child";
const SPAWN_CHILD_PROMPT: &str = "inspect inherited global instructions";
const SPAWN_FRESH_PARENT_PROMPT: &str = "spawn a child with fresh context";
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

fn instruction_fragments_in_items(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .filter(|item| {
            item.get("type").and_then(Value::as_str) == Some("message")
                && item.get("role").and_then(Value::as_str) == Some("user")
        })
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter_map(|span| span.get("text").and_then(Value::as_str))
        .filter(|text| text.starts_with("# AGENTS.md instructions for "))
        .map(str::to_string)
        .collect()
}

fn expected_instruction_fragment(cwd: &AbsolutePathBuf, contents: &str) -> String {
    let cwd = cwd.as_path().display();
    format!("# AGENTS.md instructions for {cwd}\n\n<INSTRUCTIONS>\n{contents}\n</INSTRUCTIONS>")
}

fn assert_single_instruction_fragment(request: &responses::ResponsesRequest, expected: &str) {
    assert_eq!(instruction_fragments(request), vec![expected.to_string()]);
}

fn replacement_history_from_rollout(path: &Path) -> Result<Vec<Value>> {
    let rollout_text = fs::read_to_string(path)?;
    let mut replacement_history = None;
    for line in rollout_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let entry: RolloutLine = serde_json::from_str(line)?;
        if let RolloutItem::Compacted(compacted) = entry.item
            && let Some(items) = compacted.replacement_history
        {
            replacement_history = Some(
                items
                    .into_iter()
                    .map(serde_json::to_value)
                    .collect::<std::result::Result<Vec<_>, _>>()?,
            );
        }
    }
    replacement_history.ok_or_else(|| anyhow!("expected rollout replacement history"))
}

fn rewrite_compaction_as_legacy(path: &Path) -> Result<()> {
    let rollout_text = fs::read_to_string(path)?;
    let mut rewritten = Vec::new();
    let mut compacted_items = 0;
    for line in rollout_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let mut entry: RolloutLine = serde_json::from_str(line)?;
        if let RolloutItem::Compacted(compacted) = &mut entry.item {
            compacted.replacement_history = None;
            compacted_items += 1;
        }
        rewritten.push(serde_json::to_string(&entry)?);
    }
    if compacted_items != 1 {
        return Err(anyhow!(
            "expected exactly one compaction to rewrite as legacy; observed {compacted_items}"
        ));
    }
    fs::write(path, format!("{}\n", rewritten.join("\n")))?;
    Ok(())
}

fn remote_v2_compaction_response() -> String {
    responses::sse(vec![
        json!({
            "type": "response.output_item.done",
            "item": {
                "type": "compaction",
                "encrypted_content": REMOTE_V2_SUMMARY,
            }
        }),
        responses::ev_completed("remote-v2-compact-response"),
    ])
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
    // Set up one global source, one project source, and two ordinary model turns.
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
    let project_source = test.config.cwd.join(GLOBAL_AGENTS_FILENAME);
    let creation_sources = vec![global_source.clone(), project_source.clone()];

    // Confirm the thread records both creation-time sources in composition order.
    assert_eq!(test.codex.instruction_sources().await, creation_sources);

    // Materialize the initial snapshot, then rewrite both selected files in place before another
    // ordinary turn.
    test.submit_turn("first turn").await?;
    let rewritten_global_source = write_global(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    test.fs()
        .write_file(
            &project_source,
            NEW_PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(
        rewritten_global_source, global_source,
        "same-path mutation should retain the selected global source path"
    );
    test.submit_turn("second turn").await?;

    // Assert the running thread keeps its original rendering and structured prefix even though
    // both files at the reported source paths now contain different text.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let expected_contents =
        format!("{GLOBAL_INSTRUCTIONS}\n\n{PROJECT_SEPARATOR}\n\n{PROJECT_INSTRUCTIONS}");
    let expected_fragment = expected_instruction_fragment(&test.config.cwd, &expected_contents);
    let fragments = instruction_fragments(&requests[0]);
    assert_eq!(fragments, vec![expected_fragment.clone()]);
    assert_single_instruction_fragment(&requests[1], &expected_fragment);
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
        test.codex.instruction_sources().await,
        creation_sources,
        "ordinary turns retain the creation-time source list"
    );
    let first_input = requests[0].input();
    let second_input = requests[1].input();
    assert_eq!(
        second_input.get(..first_input.len()),
        Some(first_input.as_slice()),
        "the ordinary second turn should retain the cached prefix"
    );

    Ok(())
}

// TODO(anp): Enforce an independent hard limit for the global instruction context item, then
// update this characterization to assert that oversized global instructions are bounded.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_instruction_context_item_is_currently_not_limited_by_project_doc_budget()
-> Result<()> {
    // Set a one-byte project-doc budget, then create a much larger global instruction file.
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("oversized-global-response"),
            responses::ev_completed("oversized-global-response"),
        ]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let oversized_global = vec!["global instruction item remains uncapped"; 512].join("\n");
    let source = write_global(home.as_ref(), &oversized_global)?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(|config| config.project_doc_max_bytes = 1);
    let test = builder.build(&server).await?;

    // Submit a turn so the complete global instruction item is rendered into model input.
    test.submit_turn("inspect current global item limit behavior")
        .await?;

    // Characterize the current gap: the project-doc budget does not cap the global item.
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![source],
        "the oversized global file should still be selected as the sole source"
    );
    let expected_fragment = expected_instruction_fragment(&test.config.cwd, &oversized_global);
    assert_single_instruction_fragment(&response_mock.single_request(), &expected_fragment);
    assert!(
        expected_fragment.len() > test.config.project_doc_max_bytes,
        "characterization requires a global item larger than the configured project-doc budget"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_loading_warning_surfaces_during_thread_creation() -> Result<()> {
    // Set up a malformed global instruction file and one model response.
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("warning-response"),
            responses::ev_completed("warning-response"),
        ]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global(home.as_ref(), b"global\xFFinstructions")?;

    // Create the thread, capture its load warning, and submit one turn for rendered output.
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
    test.submit_turn("inspect lossy global instructions")
        .await?;

    // Assert the source is reported, the warning is specific, and rendering is lossily decoded.
    assert_eq!(test.codex.instruction_sources().await, vec![source.clone()]);
    assert!(
        warning.contains("invalid UTF-8"),
        "expected warning to contain \"invalid UTF-8\"; observed: {warning}"
    );
    let expected_fragment =
        expected_instruction_fragment(&test.config.cwd, "global\u{FFFD}instructions");
    assert_single_instruction_fragment(&response_mock.single_request(), &expected_fragment);

    Ok(())
}

// TODO(anp): Align cold-resume instruction sources with the historical instructions replayed to
// the model so the API source list and model-visible context describe the same files.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_resume_replays_rendered_instructions_but_reports_current_config_sources() -> Result<()>
{
    // Set up an initial turn and a later cold-resumed turn against the same rollout.
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

    // Create the initial thread and persist its creation-time instruction snapshot.
    let mut initial_builder = test_codex().with_home(Arc::clone(&home));
    let initial = initial_builder.build(&server).await?;

    // Assert the pre-resume thread reports the source used to create its snapshot.
    assert_eq!(
        initial.codex.instruction_sources().await,
        vec![old_source.clone()],
        "initial thread reports the creation-time global source"
    );
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

    // Add a preferred override source, then cold-resume with freshly loaded configuration.
    let new_source = write_global_override(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_ne!(old_source, new_source);
    let mut resume_builder = test_codex().with_home(Arc::clone(&home));
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;

    // Assert the API reports the new source while model history replays the old structured prefix.
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

// TODO(anp): Align fork instruction sources with the historical instructions replayed to the
// model so the reported source list and model-visible context describe the same files.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_replays_rendered_instructions_from_shared_history() -> Result<()> {
    // Set up a parent turn and a later fork turn against the parent's rollout.
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

    // Create the parent and persist its creation-time instruction snapshot.
    let mut builder = test_codex().with_home(Arc::clone(&home));
    let parent = builder.build(&server).await?;

    // Assert the parent reports the source used to create its snapshot.
    assert_eq!(
        parent.codex.instruction_sources().await,
        vec![source.clone()],
        "parent reports the creation-time global source"
    );
    parent.submit_turn("persist instructions").await?;
    parent.codex.ensure_rollout_materialized().await;
    parent.codex.flush_rollout().await?;
    let rollout_path = parent.codex.rollout_path().expect("rollout path");

    // Add a preferred override source, then fork with freshly loaded configuration.
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

    // Assert the fork reports the new source before issuing its first turn.
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

    // Assert the forked model request replays the parent's exact structured history.
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
    run_subagent_global_instruction_case(/*fork_context*/ true).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_subagent_uses_creation_time_instructions_without_parent_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    run_subagent_global_instruction_case(/*fork_context*/ false).await
}

async fn run_subagent_global_instruction_case(fork_context: bool) -> Result<()> {
    // Set up matched responses for the parent seed, spawn call, child turn, and parent follow-up.
    let server = responses::start_mock_server().await;
    let parent_prompt = if fork_context {
        SPAWN_PARENT_PROMPT
    } else {
        SPAWN_FRESH_PARENT_PROMPT
    };
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
        "fork_context": fork_context,
    }))?;
    let spawn_mock = responses::mount_sse_once_match(
        &server,
        move |request: &wiremock::Request| request_body_contains(request, parent_prompt),
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

    // Create the parent thread, record its source, and seed the history inherited by the child.
    let home = Arc::new(TempDir::new()?);
    let source = write_global(home.as_ref(), OLD_GLOBAL_INSTRUCTIONS)?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(|config| {
            let _ = config.features.enable(Feature::Collab);
            let _ = config.features.disable(Feature::EnableRequestCompression);
        });
    let test = builder.build(&server).await?;

    // Assert the parent reports the creation-time source before spawning.
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![source.clone()],
        "parent reports the creation-time global source before spawning"
    );
    test.submit_turn(SPAWN_SEED_PROMPT).await?;
    let seed_request = seed_mock.single_request();

    // Add a preferred override, then spawn a full-history child while observing its thread ID.
    let new_source = write_global_override(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_ne!(source, new_source);
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    test.submit_turn(parent_prompt).await?;
    let child_thread_id = tokio::time::timeout(Duration::from_secs(10), created_threads.recv())
        .await
        .map_err(|_| anyhow!("timed out waiting for the subagent thread"))??;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    let spawn_request = spawn_mock.single_request();
    let child_request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(request) = child_mock.requests().into_iter().find(|request| {
                request
                    .message_input_texts("user")
                    .iter()
                    .any(|text| text == SPAWN_CHILD_PROMPT)
            }) {
                break request;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| anyhow!("timed out waiting for the subagent request"))?;

    // Assert parent and child report and render the parent's creation-time snapshot exactly once.
    let expected_fragment =
        expected_instruction_fragment(&test.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&seed_request, &expected_fragment);
    assert_single_instruction_fragment(&spawn_request, &expected_fragment);
    assert_single_instruction_fragment(&child_request, &expected_fragment);
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![source.clone()],
        "running parent retains the creation-time global source after spawning"
    );
    assert_eq!(
        child_thread.instruction_sources().await,
        vec![source],
        "subagent reports the parent's creation-time source"
    );
    if fork_context {
        let seed_input = seed_request.input();
        let child_input = child_request.input();
        assert_eq!(
            child_input.get(..seed_input.len()),
            Some(seed_input.as_slice()),
            "forked subagent should replay the parent's original structured input prefix"
        );
    } else {
        let child_user_texts = child_request.message_input_texts("user");
        assert_eq!(
            child_user_texts
                .iter()
                .filter(|text| text.as_str() == SPAWN_SEED_PROMPT)
                .count(),
            0,
            "fresh-context subagent should omit parent user history; observed: {child_user_texts:?}"
        );
        assert_eq!(
            child_user_texts
                .iter()
                .filter(|text| text.as_str() == SPAWN_CHILD_PROMPT)
                .count(),
            1,
            "fresh-context subagent should contain its own prompt exactly once; observed: {child_user_texts:?}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compaction_keeps_the_creation_time_global_instructions() -> Result<()> {
    // Set up an initial turn, a manual compaction response, and a post-compaction turn.
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

    // Create the thread with the old global source loaded into its instruction snapshot.
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| {
            config.model_provider = provider;
        });
    let test = builder.build(&server).await?;

    // Assert the pre-compaction source list points at the creation-time file.
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![source.clone()],
        "thread reports the creation-time global source before compaction"
    );

    // Materialize the old snapshot, rewrite the selected file in place, and manually compact.
    test.submit_turn("first turn").await?;
    let rewritten_source = write_global(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_eq!(source, rewritten_source);

    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("after compact").await?;

    // Assert ordinary and compact turns keep the old rendering even though the reported source
    // path now contains new text.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let expected_fragment =
        expected_instruction_fragment(&test.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[0], &expected_fragment);
    assert_single_instruction_fragment(&requests[1], &expected_fragment);
    assert_single_instruction_fragment(&requests[2], &expected_fragment);
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![source],
        "thread retains the creation-time global source after compaction"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mid_turn_compaction_keeps_the_creation_time_global_instructions() -> Result<()> {
    // Set up a turn that crosses the auto-compaction limit and a post-compaction response.
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

    // Create the thread with the old global source loaded into its instruction snapshot.
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| {
            config.model_provider = provider;
            config.model_context_window = Some(100);
            config.model_auto_compact_token_limit = Some(90);
        });
    let test = builder.build(&server).await?;

    // Assert the pre-compaction source list points at the creation-time file.
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![source.clone()],
        "thread reports the creation-time global source before mid-turn compaction"
    );

    // Add a preferred override before the turn triggers automatic mid-turn compaction.
    let new_source = write_global_override(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_ne!(source, new_source);
    test.submit_turn("trigger mid-turn compaction").await?;

    // Assert the initial, compact, and resumed requests all keep the old snapshot and source.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let expected_fragment =
        expected_instruction_fragment(&test.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[0], &expected_fragment);
    assert_single_instruction_fragment(&requests[1], &expected_fragment);
    assert_single_instruction_fragment(&requests[2], &expected_fragment);
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![source],
        "thread retains the creation-time global source after mid-turn compaction"
    );

    Ok(())
}

// TODO(anp): Preserve the persisted model-visible instruction item across later full-context
// rebuilds. Reloading file contents into historical context rewrites model-visible history and
// invalidates the cached prefix; future behavior should keep the original item stable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_resume_then_full_context_rebuild_uses_current_instructions() -> Result<()> {
    // Set up an initial turn, a cold-resumed turn, manual compaction, and the later full-context
    // rebuild.
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

    // Create the initial thread and persist its creation-time instruction snapshot.
    let mut initial_builder = test_codex().with_home(Arc::clone(&home)).with_config({
        let provider = provider.clone();
        move |config| config.model_provider = provider
    });
    let initial = initial_builder.build(&server).await?;

    // Assert the initial thread reports the source used for its historical snapshot.
    assert_eq!(
        initial.codex.instruction_sources().await,
        vec![source.clone()],
        "initial thread reports the creation-time global source"
    );
    initial.submit_turn("persist resume history").await?;
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

    // Rewrite the selected AGENTS.md in place, then cold-resume with freshly loaded configuration.
    let rewritten_source = write_global(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_eq!(source, rewritten_source);
    let mut resume_builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| config.model_provider = provider);
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;

    // Assert the same source path now resolves new file contents while cold resume replays the
    // exact old historical prefix.
    assert_eq!(
        resumed.codex.instruction_sources().await,
        vec![source.clone()],
        "resumed thread reports the same file path after in-place mutation"
    );
    assert_eq!(
        fs::read_to_string(source.as_path())?,
        NEW_GLOBAL_INSTRUCTIONS,
        "the reported source path should contain the rewritten text"
    );
    resumed.submit_turn("resume historical context").await?;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let old_fragment = expected_instruction_fragment(&initial.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[0], &old_fragment);
    assert_single_instruction_fragment(&requests[1], &old_fragment);
    let initial_input = requests[0].input();
    let resumed_input = requests[1].input();
    assert_eq!(
        resumed_input.get(..initial_input.len()),
        Some(initial_input.as_slice()),
        "cold resume should replay the original structured input prefix"
    );

    // Compact the resumed thread, then issue a turn that rebuilds full context.
    resumed.codex.submit(Op::Compact).await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    resumed.submit_turn("rebuild full context").await?;

    // Characterize the current cache-breaking behavior: compaction sees old history, but the
    // following full-context rebuild injects the newly loaded same-path contents.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    let new_fragment = expected_instruction_fragment(&resumed.config.cwd, NEW_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[2], &old_fragment);
    assert_single_instruction_fragment(&requests[3], &new_fragment);
    assert_eq!(
        resumed.codex.instruction_sources().await,
        vec![source],
        "resumed thread retains the same current source path after compaction"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_compaction_without_replacement_history_rebuilds_current_instructions_on_resume()
-> Result<()> {
    // Create a current-format compacted rollout that can be rewritten to the legacy shape.
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("legacy-initial-response"),
                responses::ev_completed("legacy-initial-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("legacy-compact-response"),
                responses::ev_assistant_message("legacy-compact-message", "legacy summary"),
                responses::ev_completed("legacy-compact-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("legacy-resumed-response"),
                responses::ev_completed("legacy-resumed-response"),
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

    // Persist one compaction, shut down, and remove its replacement history to emulate an older
    // rollout whose compacted item contains only the summary message.
    initial.submit_turn("persist legacy-shaped history").await?;
    initial.codex.submit(Op::Compact).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
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
    rewrite_compaction_as_legacy(&rollout_path)?;

    // Rewrite the selected file in place and cold-resume with current configuration.
    let rewritten_source = write_global(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_eq!(source, rewritten_source);
    let mut resume_builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_config(move |config| config.model_provider = provider);
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;
    resumed.submit_turn("resume legacy compaction").await?;

    // Legacy reconstruction has no complete historical checkpoint, so it injects the newly loaded
    // same-path instructions rather than replaying the old item.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let old_fragment = expected_instruction_fragment(&initial.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    let new_fragment = expected_instruction_fragment(&resumed.config.cwd, NEW_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[0], &old_fragment);
    assert_single_instruction_fragment(&requests[1], &old_fragment);
    assert_single_instruction_fragment(&requests[2], &new_fragment);
    assert_eq!(
        resumed.codex.instruction_sources().await,
        vec![source],
        "legacy resume reports the rewritten same-path source"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_v2_compaction_keeps_creation_time_instructions_after_same_path_mutation()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    // Set up an ordinary turn, a remote-v2 compact response, and a post-compaction turn.
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("remote-v2-initial-response"),
                responses::ev_completed("remote-v2-initial-response"),
            ]),
            remote_v2_compaction_response(),
            responses::sse(vec![
                responses::ev_response_created("remote-v2-follow-up-response"),
                responses::ev_completed("remote-v2-follow-up-response"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("remote-v2-resumed-response"),
                responses::ev_completed("remote-v2-resumed-response"),
            ]),
        ],
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let source = write_global(home.as_ref(), OLD_GLOBAL_INSTRUCTIONS)?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            let _ = config.features.enable(Feature::RemoteCompactionV2);
        });
    let test = builder.build(&server).await?;

    // Materialize the old snapshot, rewrite the selected file in place, and compact remotely.
    test.submit_turn("before remote v2 compaction").await?;
    let rewritten_source = write_global(home.as_ref(), NEW_GLOBAL_INSTRUCTIONS)?;
    assert_eq!(source, rewritten_source);
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("after remote v2 compaction").await?;
    test.codex.flush_rollout().await?;

    // Assert the compact request, installed replacement history, and follow-up all keep the
    // creation-time item despite the file-backed source now containing new text.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let old_fragment = expected_instruction_fragment(&test.config.cwd, OLD_GLOBAL_INSTRUCTIONS);
    assert_single_instruction_fragment(&requests[0], &old_fragment);
    assert_single_instruction_fragment(&requests[1], &old_fragment);
    assert_single_instruction_fragment(&requests[2], &old_fragment);
    assert_eq!(
        requests[1].input().last(),
        Some(&json!({"type": "compaction_trigger"})),
        "remote-v2 compact request should append exactly one compaction trigger"
    );
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let replacement_history = replacement_history_from_rollout(&rollout_path)?;
    assert_eq!(
        instruction_fragments_in_items(&replacement_history),
        Vec::<String>::new(),
        "remote-v2 replacement history currently omits the global-instruction fragment"
    );
    assert_eq!(
        test.codex.instruction_sources().await,
        vec![source.clone()],
        "running thread retains the selected same-path source"
    );
    assert_eq!(
        fs::read_to_string(source.as_path())?,
        NEW_GLOBAL_INSTRUCTIONS,
        "the selected source path should contain the rewritten text"
    );

    // Cold-resume the persisted replacement history with freshly loaded same-path configuration.
    test.codex.submit(Op::Shutdown).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;
    let resumed_cwd = test.config.cwd.clone();
    let mut resume_builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.cwd = resumed_cwd;
            let _ = config.features.enable(Feature::RemoteCompactionV2);
        });
    let resumed = resume_builder
        .resume(&server, Arc::clone(&home), rollout_path)
        .await?;
    resumed
        .submit_turn("after remote v2 compaction cold resume")
        .await?;

    // Modern replacement-history resume replays the persisted checkpoint and its later old-context
    // suffix even though the same source path now contains new text.
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    assert_single_instruction_fragment(&requests[3], &old_fragment);
    let resumed_input = requests[3].input();
    assert_eq!(
        resumed_input.get(..replacement_history.len()),
        Some(replacement_history.as_slice()),
        "remote-v2 cold resume should replay persisted replacement history verbatim"
    );
    let post_compact_input = requests[2].input();
    assert_eq!(
        resumed_input.get(..post_compact_input.len()),
        Some(post_compact_input.as_slice()),
        "remote-v2 cold resume should replay the complete post-compaction structured prefix"
    );
    assert_eq!(
        resumed.codex.instruction_sources().await,
        vec![source],
        "cold-resumed thread reports the same rewritten source path"
    );

    Ok(())
}
