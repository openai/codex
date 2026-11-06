//! Base instructions stay fixed within a context window and refresh when a new window opens.

use anyhow::Result;
use codex_core::ForkSnapshot;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::EventMsg::TurnComplete;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest as Request;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use wiremock::MockServer;

const MODEL: &str = "gpt-5.4";
const SWITCHED_MODEL: &str = "gpt-5.2";
const ORIGINAL: &str = "original managed base instructions";
const UPDATED: &str = "updated managed base instructions";
const SWITCHED: &str = "switched managed base instructions";

fn managed_model(slug: &str, instructions: &str) -> ModelInfo {
    let mut model = model_info_from_slug(slug);
    model.use_responses_lite = true;
    model.visibility = ModelVisibility::List;
    model
        .model_messages
        .as_mut()
        .expect("model messages")
        .instructions_template = Some(instructions.to_string());
    model
}

async fn refresh_catalog(test: &TestCodex, server: &MockServer, model: &str, instructions: &str) {
    let etag = format!("\"{model}-{}\"", instructions.len());
    let models = vec![managed_model(model, instructions)];
    responses::mount_models_once_with_etag(server, ModelsResponse { models }, &etag).await;
    let client_factory = codex_core::test_support::default_http_client_factory();
    test.thread_manager
        .get_models_manager()
        .list_models(RefreshStrategy::Online, client_factory)
        .await;
}

async fn send_turn(test: &TestCodex, server: &MockServer, text: &str) -> Result<Request> {
    let response = responses::mount_sse_once(server, responses::sse_completed(text)).await;
    test.submit_text_turn(text).await?;
    Ok(response.single_request())
}

async fn switch_model(test: &TestCodex, server: &MockServer, model: &str) -> Result<Request> {
    let overrides = ThreadSettingsOverrides {
        model: Some(model.to_string()),
        ..Default::default()
    };
    core_test_support::submit_thread_settings(&test.codex, overrides).await?;
    send_turn(test, server, "switch models").await
}

async fn compact(test: &TestCodex, server: &MockServer) -> Result<()> {
    let events = vec![
        json!({"type": "response.output_item.done", "item": {
            "type": "compaction", "encrypted_content": "summary"
        }}),
        responses::ev_completed("compaction"),
    ];
    responses::mount_sse_once(server, responses::sse(events)).await;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| matches!(event, TurnComplete(_))).await;
    Ok(())
}

fn model_updates(request: &Request) -> Vec<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.contains("<model_instructions>"))
        .collect()
}

fn assert_request_base(request: &Request, base: &str) {
    let developer = request.message_input_texts("developer");
    assert!(request.body_json().get("instructions").is_none());
    if !base.is_empty() {
        let first_message = request
            .input()
            .into_iter()
            .find(|item| item["type"] != "additional_tools")
            .expect("base instruction prefix");
        assert_eq!(first_message["content"][0]["text"], base);
    }
    assert_eq!(
        developer
            .iter()
            .filter(|text| text.as_str() == base)
            .count(),
        usize::from(!base.is_empty())
    );
}

#[test_case(true, None; "managed incremental instructions")]
#[test_case(false, None; "flag disabled")]
#[test_case(true, Some("user-supplied instructions"); "custom instructions")]
#[test_case(true, Some(""); "empty custom instructions")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_and_model_changes_preserve_the_window_base(
    incremental_tools: bool,
    custom_instructions: Option<&'static str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let models = vec![managed_model(MODEL, ORIGINAL)];
    responses::mount_models_once_with_etag(&server, ModelsResponse { models }, "\"v1\"").await;
    let test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model(MODEL)
        .with_config(move |config| {
            config.base_instructions = custom_instructions.map(str::to_string);
            if incremental_tools {
                config
                    .features
                    .enable(Feature::IncrementalTools)
                    .expect("enable incremental tools");
            } else {
                config
                    .features
                    .disable(Feature::IncrementalTools)
                    .expect("disable incremental tools");
                config
                    .features
                    .enable(Feature::IncrementalToolsLegacy)
                    .expect("enable only the regular Responses flag");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let first_request = send_turn(&test, &server, "first turn").await?;
    assert!(model_updates(&first_request).is_empty());

    refresh_catalog(&test, &server, MODEL, UPDATED).await;
    let updated_request = send_turn(&test, &server, "use refreshed instructions").await?;
    let mut updates = Vec::new();
    if incremental_tools && custom_instructions.is_none() {
        updates.push(format!("<model_instructions>\nThe following instructions replace the previous base instructions:\n\n{UPDATED}\n</model_instructions>"));
    }
    assert_eq!(model_updates(&updated_request), updates);
    assert_eq!(
        updated_request.has_content_kinds(&["model.instructions"]),
        incremental_tools && custom_instructions.is_none()
    );
    assert!(updated_request.input().starts_with(&first_request.input()));
    let unchanged_request = send_turn(&test, &server, "keep refreshed instructions").await?;
    assert_eq!(model_updates(&unchanged_request), updates);

    refresh_catalog(&test, &server, MODEL, "").await;
    let cleared_request = send_turn(&test, &server, "clear managed instructions").await?;
    if incremental_tools && custom_instructions.is_none() {
        updates.push("<model_instructions>\nThe previous base instructions no longer apply. No replacement base instructions are active.\n</model_instructions>".to_string());
    }
    assert_eq!(model_updates(&cleared_request), updates);

    for request in [
        first_request,
        updated_request,
        unchanged_request,
        cleared_request,
    ] {
        assert_request_base(&request, custom_instructions.unwrap_or(ORIGINAL));
    }

    compact(&test, &server).await?;
    let compacted_request = send_turn(&test, &server, "keep cleared instructions").await?;
    if !incremental_tools {
        assert_request_base(&compacted_request, ORIGINAL);
        assert!(model_updates(&compacted_request).is_empty());
        return Ok(());
    }
    assert_request_base(&compacted_request, custom_instructions.unwrap_or_default());
    assert!(model_updates(&compacted_request).is_empty());
    assert!(!compacted_request.body_contains_text(ORIGINAL));
    assert!(!compacted_request.body_contains_text(UPDATED));

    refresh_catalog(&test, &server, SWITCHED_MODEL, SWITCHED).await;
    let switched_request = switch_model(&test, &server, SWITCHED_MODEL).await?;
    let updates = vec![match custom_instructions {
        None => format!(
            "<model_instructions>\nThe current model is {SWITCHED_MODEL}.\nThe following instructions replace the previous base instructions:\n\n{SWITCHED}\n</model_instructions>"
        ),
        Some(_) => format!(
            "<model_instructions>\nThe current model is {SWITCHED_MODEL}.\n</model_instructions>"
        ),
    }];
    assert_eq!(model_updates(&switched_request), updates);
    assert_request_base(&switched_request, custom_instructions.unwrap_or_default());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rebuilding_retained_context_reasserts_current_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(Ok(()), "the rollout fixture is edited on the host");

    let server = MockServer::start().await;
    let builder = |instructions: &'static str| {
        test_codex()
            .with_model(MODEL)
            .with_history_mode(Some(ThreadHistoryMode::Legacy))
            .with_config(move |config| {
                config.model_catalog = Some(ModelsResponse {
                    models: vec![managed_model(MODEL, instructions)],
                });
                config
                    .features
                    .enable(Feature::IncrementalTools)
                    .expect("enable incremental tools");
            })
    };
    let initial = builder(ORIGINAL).build_with_auto_env(&server).await?;
    let first = send_turn(&initial, &server, "start the conversation").await?;
    initial.codex.shutdown_and_wait().await?;
    let path = initial.codex.rollout_path().expect("rollout path");

    // A missing turn-context record forces full reassertion within the retained window.
    let mut lines = Vec::new();
    for line in std::fs::read_to_string(&path)?.lines() {
        let item: serde_json::Value = serde_json::from_str(line)?;
        if item["type"] == "turn_context" {
            continue;
        }
        lines.push(line.to_string());
    }
    std::fs::write(&path, format!("{}\n", lines.join("\n")))?;

    let resumed = builder(UPDATED)
        .resume(&server, initial.home.clone(), path)
        .await?;
    let request = send_turn(&resumed, &server, "resume with updated instructions").await?;
    assert!(request.input().starts_with(&first.input()));
    assert_request_base(&request, ORIGINAL);
    assert_eq!(
        request
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| text.as_str() == ORIGINAL || text.as_str() == UPDATED)
            .collect::<Vec<_>>(),
        vec![ORIGINAL, UPDATED],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_switches_refresh_the_window_base_across_compaction_and_restart() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const SAME_PROMPT_MODEL: &str = "gpt-5.1";
    let server = MockServer::start().await;
    let builder = || {
        test_codex().with_model(MODEL).with_config(move |config| {
            config
                .features
                .enable(Feature::IncrementalTools)
                .expect("enable incremental tools");
            config.model_catalog = Some(ModelsResponse {
                models: vec![
                    managed_model(MODEL, ORIGINAL),
                    managed_model(SWITCHED_MODEL, SWITCHED),
                    managed_model(SAME_PROMPT_MODEL, SWITCHED),
                ],
            });
        })
    };
    let test = builder().build_with_auto_env(&server).await?;
    let first_request = send_turn(&test, &server, "first turn").await?;
    assert!(model_updates(&first_request).is_empty());
    let switched_request = switch_model(&test, &server, SWITCHED_MODEL).await?;
    let mut expected_updates = vec![format!(
        "<model_instructions>\nThe current model is {SWITCHED_MODEL}.\nThe following instructions replace the previous base instructions:\n\n{SWITCHED}\n</model_instructions>"
    )];
    assert_eq!(model_updates(&switched_request), expected_updates);

    let same_prompt_request = switch_model(&test, &server, SAME_PROMPT_MODEL).await?;
    expected_updates.push(format!(
        "<model_instructions>\nThe current model is {SAME_PROMPT_MODEL}.\n</model_instructions>"
    ));
    assert_eq!(model_updates(&same_prompt_request), expected_updates);

    compact(&test, &server).await?;
    let compacted_request = send_turn(&test, &server, "continue after compaction").await?;
    assert_request_base(&compacted_request, SWITCHED);
    assert!(model_updates(&compacted_request).is_empty());
    assert!(!compacted_request.body_contains_text(ORIGINAL));
    test.codex.flush_rollout().await?;
    let mut fork_config = test.config.clone();
    fork_config.model = Some(SAME_PROMPT_MODEL.to_string());
    let fork = test
        .thread_manager
        .fork_legacy_thread(
            ForkSnapshot::Interrupted,
            StartThreadOptions::new(fork_config),
            test.codex.rollout_path().expect("rollout path"),
        )
        .await?;
    let fork_response =
        responses::mount_sse_once(&server, responses::sse_completed("fork after compaction")).await;
    fork.thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "continue in fork".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&fork.thread, |event| matches!(event, TurnComplete(_))).await;
    let fork_request = fork_response.single_request();
    assert_request_base(&fork_request, SWITCHED);
    assert!(model_updates(&fork_request).is_empty());
    assert!(!fork_request.body_contains_text(ORIGINAL));

    let resumed = builder()
        .with_model(SAME_PROMPT_MODEL)
        .restart(&server, &test)
        .await?;
    let resumed_request = send_turn(&resumed, &server, "continue after restart").await?;
    assert_request_base(&resumed_request, SWITCHED);
    assert!(model_updates(&resumed_request).is_empty());
    assert!(!resumed_request.body_contains_text(ORIGINAL));
    let resumed_input = resumed_request.input();
    let compacted_input = compacted_request.input();
    if test.executor_environment().exec_server_url().is_some() {
        assert!(resumed_input[1..].starts_with(&compacted_input[1..]));
    } else {
        assert!(resumed_input.starts_with(&compacted_input));
    }

    for request in [first_request, switched_request, same_prompt_request] {
        assert_request_base(&request, ORIGINAL);
    }
    Ok(())
}
