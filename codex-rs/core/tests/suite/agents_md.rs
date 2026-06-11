use anyhow::Result;
use anyhow::anyhow;
use codex_core::ForkSnapshot;
use codex_core::StartThreadOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_home::CodexHomeUserInstructionsProvider;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use core_test_support::create_directory_symlink;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::RecordingUserInstructionsProvider;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

const GLOBAL_AGENTS_FILENAME: &str = "AGENTS.md";
const GLOBAL_AGENTS_OVERRIDE_FILENAME: &str = "AGENTS.override.md";
const GLOBAL_INSTRUCTIONS: &str = "global instructions";
const NEW_GLOBAL_INSTRUCTIONS: &str = "new global instructions";
const NEW_PROJECT_INSTRUCTIONS: &str = "new project instructions";
const OLD_GLOBAL_INSTRUCTIONS: &str = "old global instructions";
const PROJECT_INSTRUCTIONS: &str = "project instructions";
const PROJECT_SEPARATOR: &str = "--- project-doc ---";
const SPAWN_CALL_ID: &str = "spawn-global-instructions-child";
const SPAWN_CHILD_PROMPT: &str = "inspect inherited global instructions";
const SPAWN_FRESH_PARENT_PROMPT: &str = "spawn a child with fresh context";
const SPAWN_PARENT_PROMPT: &str = "spawn a child with the parent context";
const SPAWN_SEED_PROMPT: &str = "seed parent history";

async fn agents_instructions(mut builder: TestCodexBuilder) -> Result<String> {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let test = builder.build_with_remote_env(&server).await?;
    test.submit_turn("hello").await?;

    let request = resp_mock.single_request();
    request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions for "))
        .ok_or_else(|| anyhow::anyhow!("instructions message not found"))
}

fn write_global_file(
    home: &TempDir,
    filename: &str,
    contents: impl AsRef<[u8]>,
) -> Result<AbsolutePathBuf> {
    let path = home.path().join(filename);
    std::fs::write(&path, contents)?;
    Ok(path.abs())
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agents_override_is_preferred_over_agents_md() -> Result<()> {
    let instructions =
        agents_instructions(test_codex().with_workspace_setup(|cwd, fs| async move {
            let agents_md = cwd.join("AGENTS.md");
            let override_md = cwd.join("AGENTS.override.md");
            let agents_md_uri = PathUri::from_path(&agents_md)?;
            let override_md_uri = PathUri::from_path(&override_md)?;
            fs.write_file(&agents_md_uri, b"base doc".to_vec(), /*sandbox*/ None)
                .await?;
            fs.write_file(
                &override_md_uri,
                b"override doc".to_vec(),
                /*sandbox*/ None,
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        }))
        .await?;

    assert!(
        instructions.contains("override doc"),
        "expected AGENTS.override.md contents: {instructions}"
    );
    assert!(
        !instructions.contains("base doc"),
        "expected AGENTS.md to be ignored when override exists: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_fallback_is_used_when_agents_candidate_is_directory() -> Result<()> {
    let instructions = agents_instructions(
        test_codex()
            .with_config(|config| {
                config.project_doc_fallback_filenames = vec!["WORKFLOW.md".to_string()];
            })
            .with_workspace_setup(|cwd, fs| async move {
                let agents_dir = cwd.join("AGENTS.md");
                let fallback = cwd.join("WORKFLOW.md");
                let agents_dir_uri = PathUri::from_path(&agents_dir)?;
                let fallback_uri = PathUri::from_path(&fallback)?;
                fs.create_directory(
                    &agents_dir_uri,
                    CreateDirectoryOptions { recursive: true },
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &fallback_uri,
                    b"fallback doc".to_vec(),
                    /*sandbox*/ None,
                )
                .await?;
                Ok::<(), anyhow::Error>(())
            }),
    )
    .await?;

    assert!(
        instructions.contains("fallback doc"),
        "expected fallback doc contents: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agents_docs_are_concatenated_from_project_root_to_cwd() -> Result<()> {
    let instructions = agents_instructions(
        test_codex()
            .with_config(|config| {
                config.cwd = config.cwd.join("nested/workspace");
            })
            .with_workspace_setup(|cwd, fs| async move {
                let nested = cwd.clone();
                let root = nested
                    .parent()
                    .and_then(|parent| parent.parent())
                    .expect("nested workspace should have a project root ancestor");
                let root_agents = root.join("AGENTS.md");
                let git_marker = root.join(".git");
                let nested_agents = nested.join("AGENTS.md");
                let nested_uri = PathUri::from_path(&nested)?;
                let root_agents_uri = PathUri::from_path(&root_agents)?;
                let git_marker_uri = PathUri::from_path(&git_marker)?;
                let nested_agents_uri = PathUri::from_path(&nested_agents)?;

                fs.create_directory(
                    &nested_uri,
                    CreateDirectoryOptions { recursive: true },
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &root_agents_uri,
                    b"root doc".to_vec(),
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &git_marker_uri,
                    b"gitdir: /tmp/mock-git-dir\n".to_vec(),
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &nested_agents_uri,
                    b"child doc".to_vec(),
                    /*sandbox*/ None,
                )
                .await?;
                Ok::<(), anyhow::Error>(())
            }),
    )
    .await?;

    let root_pos = instructions
        .find("root doc")
        .expect("expected root doc in AGENTS instructions");
    let child_pos = instructions
        .find("child doc")
        .expect("expected child doc in AGENTS instructions");
    assert!(
        root_pos < child_pos,
        "expected root doc before child doc: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn symlinked_cwd_uses_logical_parent_for_agents_discovery() -> Result<()> {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let mut builder = test_codex()
        .with_config(|config| {
            config.cwd = config.cwd.join("logical-repo/workspace");
        })
        .with_workspace_setup(|cwd, _fs| async move {
            // Construct two sibling repositories with the configured cwd as a
            // directory symlink from the logical repository into the physical
            // repository:
            //
            // test-root/
            // |-- logical-repo/
            // |   |-- .git
            // |   |-- AGENTS.md              ("logical parent doc")
            // |   `-- workspace ------------> physical-repo/workspace/
            // `-- physical-repo/
            //     |-- .git
            //     |-- AGENTS.md              ("physical parent doc")
            //     `-- workspace/
            //         `-- AGENTS.md          ("workspace doc")
            //
            // Discovery should walk the lexical path through logical-repo,
            // while opening logical-repo/workspace/AGENTS.md still follows the
            // symlink into physical-repo/workspace.
            let logical_root = cwd.parent().expect("symlink should have a parent");
            let test_root = logical_root
                .parent()
                .expect("logical repository should have a parent");
            let physical_root = test_root.join("physical-repo");
            let physical_workspace = physical_root.join("workspace");

            std::fs::create_dir_all(logical_root.as_path())?;
            std::fs::write(logical_root.join(".git"), "")?;
            std::fs::write(logical_root.join("AGENTS.md"), "logical parent doc")?;

            std::fs::create_dir_all(physical_workspace.as_path())?;
            std::fs::write(physical_root.join(".git"), "")?;
            std::fs::write(physical_root.join("AGENTS.md"), "physical parent doc")?;
            std::fs::write(physical_workspace.join("AGENTS.md"), "workspace doc")?;

            create_directory_symlink(physical_workspace.as_path(), cwd.as_path());
            Ok(())
        });
    let test = builder.build(&server).await?;
    let logical_root = test
        .config
        .cwd
        .parent()
        .expect("symlink should have a parent");

    assert_eq!(
        test.codex.instruction_sources().await,
        vec![
            logical_root.join("AGENTS.md"),
            test.config.cwd.join("AGENTS.md")
        ]
    );

    test.submit_turn("hello").await?;
    let instructions = resp_mock
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions for "))
        .expect("instructions message");
    assert!(instructions.contains("logical parent doc"));
    assert!(instructions.contains("workspace doc"));
    assert!(!instructions.contains("physical parent doc"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_environment_sources_match_model_visible_instructions() -> Result<()> {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_agents = home.path().join("AGENTS.md");
    std::fs::write(&global_agents, "global doc")?;

    let mut builder = test_codex()
        .with_home(home)
        .with_workspace_setup(|cwd, fs| async move {
            let agents_md_uri = PathUri::from_path(cwd.join("AGENTS.md"))?;
            fs.write_file(
                &agents_md_uri,
                b"project doc".to_vec(),
                /*sandbox*/ None,
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        });
    let test = builder.build_with_remote_env(&server).await?;
    let project_agents = test.config.cwd.join("AGENTS.md");
    let global_agents = global_agents.abs();

    assert_eq!(
        test.codex.instruction_sources().await,
        vec![global_agents, project_agents]
    );

    test.submit_turn("hello").await?;
    let instructions = resp_mock
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions for "))
        .expect("instructions message");
    assert!(instructions.contains("global doc\n\n--- project-doc ---\n\nproject doc"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loads_user_instructions_without_a_primary_environment() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("no-primary-environment-response"),
            ev_completed("no-primary-environment-response"),
        ]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_source =
        write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;
    let provider = Arc::new(RecordingUserInstructionsProvider::new(Arc::new(
        CodexHomeUserInstructionsProvider::new(AbsolutePathBuf::try_from(
            home.path().to_path_buf(),
        )?),
    )));

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_user_instructions_provider(provider.clone())
        .with_workspace_setup(|cwd, fs| async move {
            let project_agents_uri = PathUri::from_path(cwd.join(GLOBAL_AGENTS_FILENAME))?;
            fs.write_file(
                &project_agents_uri,
                PROJECT_INSTRUCTIONS.as_bytes().to_vec(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_remote_env(&server).await?;
    assert_eq!(provider.load_count(), 1);

    let no_environment_thread = test
        .thread_manager
        .start_thread_with_options(StartThreadOptions {
            config: test.config.clone(),
            initial_history: InitialHistory::New,
            session_source: None,
            thread_source: None,
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: Vec::new(),
            thread_extension_init: Default::default(),
        })
        .await?;
    assert_eq!(provider.load_count(), 2);
    assert_eq!(
        no_environment_thread.thread.instruction_sources().await,
        vec![global_source]
    );

    no_environment_thread
        .thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "inspect global instructions without an environment".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&no_environment_thread.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let instruction_fragments = instruction_fragments(&response_mock.single_request());
    assert_eq!(instruction_fragments.len(), 1);
    assert!(instruction_fragments[0].contains(GLOBAL_INSTRUCTIONS));
    assert!(!instruction_fragments[0].contains(PROJECT_INSTRUCTIONS));

    Ok(())
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
    let global_source =
        write_global_file(home.as_ref(), GLOBAL_AGENTS_FILENAME, GLOBAL_INSTRUCTIONS)?;

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_workspace_setup(|cwd, fs| async move {
            let agents_md_uri = PathUri::from_path(cwd.join("AGENTS.md"))?;
            fs.write_file(
                &agents_md_uri,
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
    let rewritten_global_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        NEW_GLOBAL_INSTRUCTIONS,
    )?;
    test.fs()
        .write_file(
            &PathUri::from_path(&project_source)?,
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
    let source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        b"global\xFFinstructions",
    )?;

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
    let old_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        OLD_GLOBAL_INSTRUCTIONS,
    )?;

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
    let new_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_OVERRIDE_FILENAME,
        NEW_GLOBAL_INSTRUCTIONS,
    )?;
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
    let source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        OLD_GLOBAL_INSTRUCTIONS,
    )?;

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
    let new_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_OVERRIDE_FILENAME,
        NEW_GLOBAL_INSTRUCTIONS,
    )?;
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
    let source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_FILENAME,
        OLD_GLOBAL_INSTRUCTIONS,
    )?;
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
    let new_source = write_global_file(
        home.as_ref(),
        GLOBAL_AGENTS_OVERRIDE_FILENAME,
        NEW_GLOBAL_INSTRUCTIONS,
    )?;
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
