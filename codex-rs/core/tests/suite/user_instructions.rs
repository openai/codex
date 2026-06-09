use std::fs;
use std::sync::Arc;

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_home::CodexHomeUserInstructionsProvider;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses;
use core_test_support::test_codex::RecordingUserInstructionsProvider;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const USER_INSTRUCTIONS: &str = "global instructions";
const PROJECT_INSTRUCTIONS: &str = "project instructions";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loads_user_instructions_without_a_primary_environment() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("no-primary-environment-response"),
            responses::ev_completed("no-primary-environment-response"),
        ]),
    )
    .await;
    let home = Arc::new(TempDir::new()?);
    let global_path = home.path().join("AGENTS.md");
    fs::write(&global_path, USER_INSTRUCTIONS)?;
    let global_source = AbsolutePathBuf::try_from(global_path)?;
    let provider = Arc::new(RecordingUserInstructionsProvider::new(Arc::new(
        CodexHomeUserInstructionsProvider::new(AbsolutePathBuf::try_from(
            home.path().to_path_buf(),
        )?),
    )));

    let mut builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_user_instructions_provider(provider.clone())
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

    let instruction_fragments = response_mock
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("# AGENTS.md instructions for "))
        .collect::<Vec<_>>();
    assert_eq!(instruction_fragments.len(), 1);
    assert!(instruction_fragments[0].contains(USER_INSTRUCTIONS));
    assert!(!instruction_fragments[0].contains(PROJECT_INSTRUCTIONS));

    Ok(())
}
