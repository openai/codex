//! Exercises streamed remote compaction, including the retained checkpoint and continuing history.

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_history::CodexHarnessMetadata;
use codex_history::InitialHistory;
use codex_history::RolloutItem;
use codex_login::CodexAuth;
use codex_login::auth::BedrockApiKeyAuth;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_5_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::AgentPath;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex as base_test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::Path;
use test_case::test_case;
use tokio::time::Duration;
use wiremock::ResponseTemplate;

#[path = "compact_remote_trimming.rs"]
mod trimming;

const DUMMY_FUNCTION_NAME: &str = "test_tool";
const TURN_STATE_HEADER: &str = "x-codex-turn-state";
const REMOTE_COMPACT_TURN_COMPLETE_TIMEOUT: Duration = Duration::from_secs(30);
const CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE: &str =
    "Output exceeded the available model context and was truncated";

fn test_codex() -> TestCodexBuilder {
    base_test_codex().with_config(|config| {
        config.update_plan_enabled = true;
    })
}

fn remote_realtime_test_codex_builder(
    realtime_server: &responses::WebSocketTestServer,
) -> TestCodexBuilder {
    let realtime_base_url = realtime_server.uri().to_string();
    test_codex()
        .with_auth(CodexAuth::from_api_key("dummy"))
        .with_config(move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        })
}

async fn start_remote_realtime_server() -> responses::WebSocketTestServer {
    start_websocket_server(vec![vec![
        vec![json!({
            "type": "session.updated",
            "session": { "id": "sess_remote_compact", "instructions": "backend prompt" }
        })],
        // Keep the websocket open after startup so routed transcript items during the test do not
        // exhaust the scripted responses and mark realtime inactive before the assertions run.
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    ]])
    .await
}

async fn start_realtime_conversation(codex: &codex_core::CodexThread) -> Result<()> {
    codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            client_managed_handoffs: false,
            delegation_ack_filler: None,
            flush_transcript_tail_on_session_end: false,
            codex_responses_as_items: false,
            codex_response_item_prefix: None,
            codex_response_handoff_mode:
                codex_protocol::protocol::CodexResponseHandoffMode::Thinking,
            codex_response_handoff_channel_prefixes: None,
            model: None,
            output_modality: RealtimeOutputModality::Audio,
            include_startup_context: true,
            initial_items: Vec::new(),
            realtime_start_instructions: None,
            realtime_end_instructions: None,
            prompt: Some(Some("backend prompt".to_string())),
            realtime_session_id: None,
            transport: None,
            version: None,
            voice: None,
        }))
        .await?;

    wait_for_event_match(codex, |msg| match msg {
        EventMsg::RealtimeConversationStarted(started) => Some(Ok(started.clone())),
        EventMsg::Error(err) => Some(Err(err.clone())),
        _ => None,
    })
    .await
    .expect("conversation start failed");

    wait_for_event_match(codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload:
                RealtimeEvent::SessionUpdated {
                    realtime_session_id: session_id,
                    ..
                },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;

    Ok(())
}

async fn close_realtime_conversation(codex: &codex_core::CodexThread) -> Result<()> {
    codex.submit(Op::RealtimeConversationClose).await?;
    wait_for_event_match(codex, |msg| match msg {
        EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
        _ => None,
    })
    .await;
    Ok(())
}

fn assert_request_contains_custom_realtime_start(
    request: &responses::ResponsesRequest,
    instructions: &str,
) {
    let body = request.body_json().to_string();
    assert!(
        body.contains("<realtime_conversation>"),
        "expected request to preserve the realtime wrapper"
    );
    assert!(
        body.contains(instructions),
        "expected request to use custom realtime start instructions"
    );
    assert!(
        !body.contains("Realtime conversation started."),
        "expected request to replace the default realtime start instructions"
    );
}

async fn wait_for_turn_complete(codex: &codex_core::CodexThread) {
    wait_for_event_with_timeout(
        codex,
        |ev| matches!(ev, EventMsg::TurnComplete(_)),
        REMOTE_COMPACT_TURN_COMPLETE_TIMEOUT,
    )
    .await;
}

fn amazon_bedrock_test_codex() -> TestCodexBuilder {
    let auth = CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
        api_key: "bedrock-test-api-key".to_string(),
        region: "us-east-1".to_string(),
    });
    test_codex()
        .with_auth(auth)
        .with_model(AMAZON_BEDROCK_GPT_5_5_MODEL_ID)
        .with_config(|config| {
            config.model_provider = ModelProviderInfo {
                base_url: config.model_provider.base_url.clone(),
                ..ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None)
            };
            config.model_provider_id = AMAZON_BEDROCK_PROVIDER_ID.to_string();
        })
}

fn is_retained_user_message(item: &ResponseItem, retained_text: &str) -> bool {
    matches!(
        item,
        ResponseItem::Message { role, content, .. }
            if role == "user"
                && content.iter().any(|item| {
                    matches!(item, ContentItem::InputText { text } if text == retained_text)
                })
    )
}

fn annotate_retained_user_in_rollout(path: &Path, retained_text: &str) -> Result<()> {
    let mut rollout = fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(codex_rollout::parse_rollout_line)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rollout
        .iter_mut()
        .find_map(|line| match &mut line.item {
            RolloutItem::ResponseItem(envelope)
                if is_retained_user_message(&envelope.item, retained_text) =>
            {
                Some(envelope)
            }
            _ => None,
        })
        .context("persisted user response missing from rollout")?
        .metadata = Some(CodexHarnessMetadata::default());

    let contents = rollout
        .iter()
        .map(serde_json::to_string)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join("\n");
    fs::write(path, format!("{contents}\n"))?;
    Ok(())
}

fn assert_compacted_user_metadata(path: &Path, retained_text: &str) -> Result<()> {
    let replacement_history = fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(codex_rollout::parse_rollout_line)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .rev()
        .find_map(|line| match line.item {
            RolloutItem::Compacted(compacted) => compacted.replacement_history,
            _ => None,
        })
        .context("compacted replacement history missing")?;
    assert!(
        replacement_history.iter().any(|envelope| {
            is_retained_user_message(&envelope.item, retained_text) && envelope.metadata.is_some()
        }),
        "compacted user message should retain its aligned harness metadata"
    );
    Ok(())
}

fn assert_compact_request_omits_harness_metadata(request: &responses::ResponsesRequest) {
    for item in request.input() {
        assert!(
            item.get("metadata").is_none() && item.get("replacement_history_metadata").is_none(),
            "provider request must not receive harness history metadata: {item}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_retains_metadata_from_resumed_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    let retained_text = "annotated v2 user message";
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                responses::ev_assistant_message("message-1", "before compaction"),
                responses::ev_completed("response-1"),
            ]),
            sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "ANNOTATED_V2_COMPACTION_SUMMARY",
                    },
                }),
                responses::ev_completed("response-compact"),
            ]),
            sse(vec![responses::ev_completed("response-after")]),
        ],
    )
    .await;

    let builder = || test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let initial = builder()
        .with_pre_build_hook(|home| {
            fs::write(
                home.join("config.toml"),
                "[features]\nremote_compaction_v2 = false\n",
            )
            .expect("write saved config");
        })
        .build(&server)
        .await?;
    initial.submit_turn(retained_text).await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .context("rollout path")?;
    initial.codex.shutdown_and_wait().await?;
    annotate_retained_user_in_rollout(&rollout_path, retained_text)?;

    let resumed = builder()
        .resume(&server, home, rollout_path.clone())
        .await?;
    resumed.codex.submit(Op::Compact).await?;
    wait_for_turn_complete(&resumed.codex).await;
    resumed.submit_turn("continue after compaction").await?;
    resumed.codex.shutdown_and_wait().await?;

    let requests = response_mock.requests();
    let compact_request = &requests[1];
    assert_eq!(
        compact_request.inputs_of_type("compaction_trigger").len(),
        1
    );
    assert_compact_request_omits_harness_metadata(compact_request);
    assert_compacted_user_metadata(&rollout_path, retained_text)?;
    assert_eq!(
        requests[2].inputs_of_type("compaction")[0]["encrypted_content"],
        "ANNOTATED_V2_COMPACTION_SUMMARY"
    );
    assert!(requests[2].body_contains_text("continue after compaction"));

    Ok(())
}

#[test_case(false; "feature_disabled")]
#[test_case(true; "feature_enabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_retains_only_client_developer_messages_when_enabled(
    enabled: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_auto_env_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(move |config| {
                if enabled {
                    config
                        .features
                        .enable(Feature::RetainClientDeveloperMessages)
                        .expect("client developer retention should be configurable");
                }
            }),
    )
    .await?;
    let developer = |text: &str| ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    let codex = &harness.test().codex;
    let rollout_path = codex.rollout_path().context("rollout path")?;
    codex
        .inject_response_items(vec![developer("INJECTED_CLIENT_DEVELOPER")])
        .await?;
    let response_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![responses::ev_completed("before-compact")]),
            sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "CLIENT_RETENTION_SUMMARY",
                    },
                }),
                responses::ev_completed("compact"),
            ]),
            sse(vec![responses::ev_completed("after-compact")]),
        ],
    )
    .await;

    harness.test().submit_turn("before compact").await?;
    codex.submit(Op::Compact).await?;
    wait_for_turn_complete(codex).await;
    harness.test().submit_turn("after compact").await?;

    let requests = response_mock.requests();
    let follow_up = requests.last().context("follow-up request")?;
    assert_eq!(
        follow_up.body_contains_text("INJECTED_CLIENT_DEVELOPER"),
        enabled
    );
    requests
        .iter()
        .for_each(assert_compact_request_omits_harness_metadata);

    codex.shutdown_and_wait().await?;
    let replacement_history = fs::read_to_string(&rollout_path)?
        .lines()
        .filter_map(|line| codex_rollout::parse_rollout_line(line).ok())
        .filter_map(|line| match line.item {
            RolloutItem::Compacted(compacted) => compacted.replacement_history,
            _ => None,
        })
        .next_back()
        .context("remote compaction should persist a checkpoint")?;
    let retained_client_developers = replacement_history
        .iter()
        .filter(|item| {
            item.metadata
                .as_ref()
                .is_some_and(|metadata| metadata.client_authored)
        })
        .count();
    assert_eq!(retained_client_developers, usize::from(enabled));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_records_usage_before_output_validation() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_auto_env_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_auto_compact_token_limit = Some(200);
            }),
    )
    .await?;
    let codex = &harness.test().codex;
    let rollout_path = codex.rollout_path().context("rollout path")?;
    let responses_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![responses::ev_completed_with_tokens(
                "before-compact",
                /*total_tokens*/ 500,
            )]),
            sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "FIRST_SUMMARY",
                    },
                }),
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "SECOND_SUMMARY",
                    },
                }),
                responses::ev_completed_with_tokens("invalid-compact", /*total_tokens*/ 8_200),
            ]),
        ],
    )
    .await;

    harness.test().submit_turn("before compact").await?;
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "turn that triggers auto compact".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut preserved_prompts = Vec::new();
    wait_for_event(codex, |event| {
        if let EventMsg::UserMessage(message) = event {
            preserved_prompts.push(message.message.clone());
        }
        matches!(event, EventMsg::Error(_))
    })
    .await;
    assert_eq!(preserved_prompts, vec!["turn that triggers auto compact"]);
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert_eq!(responses_mock.requests().len(), 2);

    codex.flush_rollout().await?;
    let history = codex.load_history(/*include_archived*/ false).await?;
    assert_eq!(
        history
            .items
            .iter()
            .filter(|item| matches!(
                item,
                RolloutItem::ResponseItem(envelope)
                    if is_retained_user_message(
                        &envelope.item,
                        "turn that triggers auto compact",
                    )
            ))
            .count(),
        1,
        "the accepted prompt should be saved exactly once after compaction fails"
    );
    codex.shutdown_and_wait().await?;

    let record = fs::read_to_string(&rollout_path)?
        .lines()
        .filter_map(|line| codex_rollout::parse_rollout_line(line).ok())
        .filter_map(|line| match line.item {
            RolloutItem::TokenUsageRecord(record) => Some(record),
            _ => None,
        })
        .find(|record| record.response_id == "invalid-compact")
        .context("remote compaction usage record")?;
    assert_eq!(record.usage.total_tokens, 8_200);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn amazon_bedrock_automatic_compaction_uses_v2_responses_endpoint() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_auto_env_builder(amazon_bedrock_test_codex().with_config(
        |config| {
            config.model_auto_compact_token_limit = Some(200);
        },
    ))
    .await?;
    let response_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                responses::ev_assistant_message("message-1", "before automatic compaction"),
                responses::ev_completed_with_tokens("response-1", /*total_tokens*/ 500),
            ]),
            sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "BEDROCK_AUTOMATIC_REMOTE_COMPACTED_SUMMARY",
                    }
                }),
                responses::ev_completed("response-compact"),
            ]),
            sse(vec![
                responses::ev_assistant_message("message-2", "after automatic compaction"),
                responses::ev_completed("response-2"),
            ]),
        ],
    )
    .await;

    harness
        .test()
        .submit_turn("before automatic compact")
        .await?;
    harness
        .test()
        .submit_turn("after automatic compact")
        .await?;

    let response_requests = response_mock.requests();
    assert_eq!(response_requests.len(), 3);
    assert!(
        response_requests
            .iter()
            .all(|request| request.path() == "/v1/responses")
    );
    let compact_request = &response_requests[1];
    assert_eq!(
        compact_request.header("authorization").as_deref(),
        Some("Bearer bedrock-test-api-key")
    );
    assert_eq!(
        compact_request
            .header("x-amzn-mantle-client-agent")
            .as_deref(),
        Some("codex")
    );
    assert_eq!(
        compact_request.body_json()["model"],
        AMAZON_BEDROCK_GPT_5_5_MODEL_ID
    );
    assert_eq!(
        compact_request.inputs_of_type("compaction_trigger").len(),
        1
    );
    assert!(response_requests[2].input().iter().any(|item| {
        item["type"] == "compaction"
            && item["encrypted_content"] == "BEDROCK_AUTOMATIC_REMOTE_COMPACTED_SUMMARY"
    }));

    Ok(())
}

#[test_case(None; "default_trims_images")]
#[test_case(Some(false); "disabled_preserves_images")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_charges_retained_images_to_token_budget(
    image_budget_enabled: Option<bool>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_auto_env_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(move |config| {
                let _ = config.features.enable(Feature::UnifiedImageBudget);
                if let Some(enabled) = image_budget_enabled {
                    let _ = config
                        .features
                        .set_enabled(Feature::CompactionImageBudget, enabled);
                }
            }),
    )
    .await?;
    let codex = &harness.test().codex;
    // Each original-detail image costs 10,000 estimated patch tokens.
    let image_inputs = (1..=8)
        .map(|number| {
            let image = image::ImageBuffer::from_pixel(
                /*width*/ 3200,
                /*height*/ 3200,
                image::Luma([number as u8]),
            );
            let mut bytes = std::io::Cursor::new(Vec::new());
            image.write_to(&mut bytes, image::ImageFormat::Png)?;
            Ok(UserInput::Image {
                image_url: format!(
                    "data:image/png;base64,{}",
                    BASE64_STANDARD.encode(bytes.get_ref())
                ),
                detail: Some(codex_protocol::models::ImageDetail::Original),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut input = image_inputs[..7].to_vec();
    input.push(UserInput::Text {
        text: "Compare these images".to_string(),
        text_elements: Vec::new(),
    });
    let initial_mock = mount_sse_once(
        harness.server(),
        sse(vec![
            responses::ev_assistant_message("initial", "done"),
            responses::ev_completed("initial"),
        ]),
    )
    .await;
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(input))
        .await?;
    wait_for_turn_complete(codex).await;
    let initial_request = initial_mock.single_request();
    let prepared_images = initial_request.message_input_image_urls("user");
    assert_eq!(prepared_images.len(), 7);

    for cycle in 1..=2 {
        let compact_mock = mount_sse_once(
            harness.server(),
            sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": { "type": "compaction", "encrypted_content": "IMAGE_BUDGET_SUMMARY" },
                }),
                responses::ev_completed("compact-images"),
            ]),
        )
        .await;
        codex.submit(Op::Compact).await?;
        wait_for_turn_complete(codex).await;
        let compact_request = compact_mock.single_request();
        assert_eq!(compact_request.path(), "/v1/responses");
        assert_eq!(
            compact_request.inputs_of_type("compaction_trigger").len(),
            1
        );

        let follow_up_mock = mount_sse_once(
            harness.server(),
            sse(vec![
                responses::ev_assistant_message("after", "done"),
                responses::ev_completed("after"),
            ]),
        )
        .await;
        codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "after compact".to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_turn_complete(codex).await;
        let follow_up = follow_up_mock.single_request();
        assert_eq!(
            follow_up.inputs_of_type("compaction")[0]["encrypted_content"],
            "IMAGE_BUDGET_SUMMARY"
        );
        let dropped = if image_budget_enabled.unwrap_or(true) {
            cycle
        } else {
            0
        };
        let mut expected_images = prepared_images[dropped..].to_vec();
        if cycle == 2 {
            let UserInput::Image { image_url, .. } = &image_inputs[7] else {
                unreachable!()
            };
            expected_images.push(image_url.clone());
        }
        assert_eq!(follow_up.message_input_image_urls("user"), expected_images);
        assert!(
            follow_up
                .message_input_texts("user")
                .iter()
                .any(|text| text == "Compare these images")
        );

        if cycle == 1 {
            let append_mock = mount_sse_once(
                harness.server(),
                sse(vec![
                    responses::ev_assistant_message("append", "done"),
                    responses::ev_completed("append"),
                ]),
            )
            .await;
            codex
                .start_or_steer_turn(TurnInputRequest::user_input(vec![image_inputs[7].clone()]))
                .await?;
            wait_for_turn_complete(codex).await;
            let _ = append_mock.single_request();
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_reuses_compaction_trigger_for_followups() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR4nGNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=";
    let user_notice = "<image_resize_notice>retained user image</image_resize_notice>";
    let tool_notice = "<image_resize_notice>discarded tool image</image_resize_notice>";
    let unlisted_notice = "<unlisted_notice>discarded developer notice</unlisted_notice>";
    let developer_message = |text| {
        json!({
            "type": "message",
            "role": "developer",
            "content": [{ "type": "input_text", "text": text }]
        })
    };
    let initial_history = [
        json!({
            "type": "message",
            "role": "user",
            "content": [
                { "type": "input_text", "text": "retained image source" },
                { "type": "input_image", "image_url": image_url }
            ]
        }),
        developer_message(user_notice),
        developer_message(unlisted_notice),
        json!({
            "type": "function_call",
            "name": "view_image",
            "arguments": "{}",
            "call_id": "image-call"
        }),
        json!({
            "type": "function_call_output",
            "call_id": "image-call",
            "output": [{ "type": "input_image", "image_url": image_url }]
        }),
        developer_message(tool_notice),
    ]
    .into_iter()
    .map(|item| {
        serde_json::from_value::<ResponseItem>(item)
            .map(|item| RolloutItem::ResponseItem(item.into()))
    })
    .collect::<serde_json::Result<Vec<_>>>()?;
    let codex = harness
        .test()
        .thread_manager
        .start_thread(StartThreadOptions {
            initial_history: InitialHistory::Forked(initial_history),
            ..StartThreadOptions::new(harness.test().config.clone())
        })
        .await?
        .thread;

    let responses_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("m1", "FIRST_REMOTE_REPLY"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m-agent", "DELEGATED_TASK_REPLY"),
                responses::ev_completed("resp-agent"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m-descendant", "DESCENDANT_FOLLOWUP_REPLY"),
                responses::ev_completed("resp-descendant"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m-compact-noise", "IGNORED_COMPACT_REPLY"),
                serde_json::json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "ENCRYPTED_CONTEXT_COMPACTION_SUMMARY",
                    }
                }),
                responses::ev_completed("resp-compact"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m2", "AFTER_COMPACT_REPLY"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello remote compact".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&codex).await;

    codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new(
                AgentPath::root().join("child").expect("valid child path"),
                AgentPath::root(),
                Vec::new(),
                "Message Type: MESSAGE\nTask name: /root\nSender: /root/child\nPayload:\nchild progress"
                    .to_string(),
                /*trigger_turn*/ false,
            ),
            start_options: Default::default(),
        })
        .await?;
    codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new(
                AgentPath::root().join("child").expect("valid child path"),
                AgentPath::root(),
                Vec::new(),
                "Message Type: FINAL_ANSWER\nTask name: /root\nSender: /root/child\nPayload:\nchild completion".to_string(),
                /*trigger_turn*/ false,
            ),
            start_options: Default::default(),
        })
        .await?;
    let delegated_task_ciphertext = format!("delegated compact task{}", "x".repeat(40_000));
    codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new_encrypted(
                AgentPath::root(),
                AgentPath::root().join("worker").expect("valid worker path"),
                Vec::new(),
                delegated_task_ciphertext.clone(),
                /*trigger_turn*/ true,
            ),
            start_options: Default::default(),
        })
        .await?;
    wait_for_turn_complete(&codex).await;

    let descendant_followup_ciphertext = "descendant follow-up task";
    let worker_path = AgentPath::root().join("worker").expect("valid worker path");
    codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new_encrypted(
                worker_path.join("child").expect("valid grandchild path"),
                worker_path,
                Vec::new(),
                descendant_followup_ciphertext.to_string(),
                /*trigger_turn*/ true,
            ),
            start_options: Default::default(),
        })
        .await?;
    wait_for_turn_complete(&codex).await;

    let compact_turn_id = codex.submit(Op::Compact).await?;
    wait_for_turn_complete(&codex).await;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "after compact".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&codex).await;

    let response_requests = responses_mock.requests();
    let compact_request = &response_requests[3];
    let item_create_time = |request: &responses::ResponsesRequest, text: &str| {
        request
            .input()
            .into_iter()
            .find(|item| {
                item["content"].as_array().is_some_and(|content| {
                    content.iter().any(|part| {
                        part["text"].as_str() == Some(text)
                            || part["encrypted_content"].as_str() == Some(text)
                    })
                })
            })
            .and_then(|item| {
                item.pointer("/internal_chat_message_metadata_passthrough/create_time")
                    .cloned()
            })
            .expect("matching message should include a creation timestamp")
    };
    let original_user_create_time = item_create_time(&response_requests[0], "hello remote compact");
    let delegated_task_create_time =
        item_create_time(&response_requests[1], &delegated_task_ciphertext);
    assert!(
        compact_request
            .inputs_of_type("agent_message")
            .iter()
            .any(|item| item["content"][1]["encrypted_content"].as_str()
                == Some(delegated_task_ciphertext.as_str())),
        "expected v2 compaction input to include the encrypted delegated task"
    );
    assert!(
        compact_request
            .inputs_of_type("agent_message")
            .iter()
            .any(|item| item["content"][1]["encrypted_content"].as_str()
                == Some(descendant_followup_ciphertext)),
        "expected v2 compaction input to include the descendant-authored follow-up task"
    );
    assert!(
        compact_request
            .inputs_of_type("agent_message")
            .iter()
            .any(|item| item.to_string().contains("child progress")),
        "expected v2 compaction input to include the child progress update"
    );
    assert!(
        compact_request
            .inputs_of_type("agent_message")
            .iter()
            .any(|item| item.to_string().contains("child completion")),
        "expected v2 compaction input to include the child completion"
    );
    assert!(
        compact_request
            .header("x-codex-beta-features")
            .as_deref()
            .is_some_and(|value| value
                .split(',')
                .any(|feature| feature == "remote_compaction_v2")),
        "expected compact request to advertise the remote_compaction_v2 beta feature"
    );
    assert_eq!(compact_request.path(), "/v1/responses");
    let compact_metadata: Value = serde_json::from_str(
        &compact_request
            .header("x-codex-turn-metadata")
            .expect("v2 compact request should include turn metadata"),
    )
    .expect("v2 compact turn metadata should be valid json");
    assert_eq!(compact_metadata["turn_id"], compact_turn_id);
    assert_eq!(compact_metadata["root_turn_id"], compact_turn_id);
    assert_eq!(
        compact_metadata["request_kind"].as_str(),
        Some("compaction")
    );
    assert_eq!(
        compact_metadata["window_id"].as_str(),
        compact_request.header("x-codex-window-id").as_deref()
    );
    assert_eq!(
        compact_request.body_json()["client_metadata"]["x-codex-window-id"].as_str(),
        compact_metadata["window_id"].as_str()
    );
    assert_eq!(
        compact_metadata["compaction"],
        json!({
            "trigger": "manual",
            "reason": "user_requested",
            "implementation": "responses_compaction_v2",
            "phase": "standalone_turn",
            "strategy": "memento",
        })
    );
    let compact_body = compact_request.body_json().to_string();
    assert!(
        compact_body.contains("\"type\":\"compaction_trigger\""),
        "expected v2 compaction request to include the compaction_trigger item"
    );
    assert!(
        !compact_body.contains("ENCRYPTED_CONTEXT_COMPACTION_SUMMARY"),
        "expected v2 compaction trigger item to omit encrypted_content"
    );

    let follow_up_request = response_requests.last().expect("follow-up request missing");
    assert_eq!(
        item_create_time(follow_up_request, "hello remote compact"),
        original_user_create_time
    );
    assert_eq!(
        item_create_time(follow_up_request, &delegated_task_ciphertext),
        delegated_task_create_time
    );
    assert!(
        item_create_time(follow_up_request, "after compact")
            .as_f64()
            .is_some_and(|create_time| create_time > 0.0)
    );
    assert!(
        follow_up_request
            .inputs_of_type("agent_message")
            .iter()
            .any(|item| item["content"][1]["encrypted_content"].as_str()
                == Some(delegated_task_ciphertext.as_str())),
        "expected v2 follow-up request to retain the encrypted delegated task"
    );
    assert!(
        follow_up_request
            .inputs_of_type("agent_message")
            .iter()
            .any(|item| item["content"][1]["encrypted_content"].as_str()
                == Some(descendant_followup_ciphertext)),
        "expected v2 follow-up request to retain the descendant-authored follow-up task"
    );
    assert!(
        follow_up_request
            .inputs_of_type("agent_message")
            .iter()
            .all(|item| !item.to_string().contains("child progress")),
        "expected v2 follow-up request to omit the child progress update"
    );
    assert!(
        follow_up_request
            .inputs_of_type("agent_message")
            .iter()
            .all(|item| !item.to_string().contains("child completion")),
        "expected v2 follow-up request to omit the child completion"
    );
    let follow_up_body = follow_up_request.body_json().to_string();
    assert!(
        follow_up_body.contains("\"type\":\"compaction\""),
        "expected follow-up request to preserve the compaction item"
    );
    assert!(
        follow_up_body.contains("ENCRYPTED_CONTEXT_COMPACTION_SUMMARY"),
        "expected follow-up request to include the compaction payload"
    );
    assert!(!follow_up_body.contains("IGNORED_COMPACT_REPLY"));
    assert!(
        follow_up_body.contains("hello remote compact"),
        "expected v2 follow-up request to preserve retained original user messages"
    );
    assert!(
        follow_up_request.input().windows(2).any(|items| {
            items[0]["role"] == "user"
                && items[0]["content"][0]["text"] == "retained image source"
                && items[1]["role"] == "developer"
                && items[1]["content"][0]["text"] == user_notice
        }),
        "expected v2 compaction to retain the user image and its adjacent resize notice"
    );
    assert!(
        !follow_up_body.contains(unlisted_notice),
        "expected v2 compaction to drop unlisted developer notices"
    );
    assert!(
        !follow_up_body.contains(tool_notice),
        "expected v2 compaction to drop the resize notice with its tool output"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_retries_failures_with_stream_retry_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_history_mode(ThreadHistoryMode::Paginated)
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_provider.request_max_retries = Some(0);
                config.model_provider.stream_max_retries = Some(2);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_response_sequence(
        harness.server(),
        vec![
            responses::sse_response(responses::sse(vec![
                responses::ev_assistant_message("m1", "FIRST_REMOTE_REPLY"),
                responses::ev_completed("resp-1"),
            ])),
            ResponseTemplate::new(500).set_body_string("first compact open failed"),
            responses::sse_response(responses::sse(vec![serde_json::json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "compaction",
                    "encrypted_content": "FAILED_COMPACT_SUMMARY",
                }
            })])),
            responses::sse_response(responses::sse(vec![
                serde_json::json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "RETRIED_COMPACT_SUMMARY",
                    }
                }),
                responses::ev_completed("resp-compact-retry"),
            ])),
            responses::sse_response(responses::sse(vec![
                responses::ev_assistant_message("m2", "AFTER_COMPACT_REPLY"),
                responses::ev_completed("resp-2"),
            ])),
        ],
    )
    .await;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello remote compact".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&codex).await;

    codex.submit(Op::Compact).await?;
    wait_for_turn_complete(&codex).await;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "after compact".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&codex).await;

    let response_requests = responses_mock.requests();
    assert_eq!(
        5,
        response_requests.len(),
        "expected initial turn, failed open, failed stream, compact retry, and follow-up turn"
    );
    for compact_request in &response_requests[1..=3] {
        assert_eq!("/v1/responses", compact_request.path());
        let compact_metadata: Value = serde_json::from_str(
            &compact_request
                .header("x-codex-turn-metadata")
                .expect("v2 compact request should include turn metadata"),
        )?;
        assert_eq!(compact_metadata["window_number"].as_u64(), Some(0));
        assert!(compact_metadata["context_window_id"].as_str().is_some());
        assert!(
            compact_request
                .body_json()
                .to_string()
                .contains("\"type\":\"compaction_trigger\""),
            "expected v2 compaction request to include the compaction_trigger item"
        );
    }

    let follow_up_request = response_requests.last().expect("follow-up request missing");
    let follow_up_body = follow_up_request.body_json().to_string();
    assert!(
        follow_up_body.contains("RETRIED_COMPACT_SUMMARY"),
        "expected follow-up request to include the retried compaction payload"
    );
    assert!(
        !follow_up_body.contains("FAILED_COMPACT_SUMMARY"),
        "expected failed compaction attempt output to be discarded"
    );

    Ok(())
}

#[test_case(false; "manual")]
#[test_case(true; "automatic")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_v2_rewrites_multiple_trailing_function_call_outputs(
    automatic: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let first_user_message = "turn with retained tool call";
    let second_user_message = "turn with parallel tool calls";
    let retained_call_id = "retained-call";
    let first_trimmed_call_id = "first-trimmed-call";
    let second_trimmed_call_id = "second-trimmed-call";
    let retained_output = "retained tool output";

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_context_window = Some(2_000);
                config.model_auto_compact_token_limit = Some(200_000);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let initial_mock = mount_sse_once(
        harness.server(),
        sse(vec![responses::ev_completed_with_tokens(
            "initial-response",
            if automatic { 500_000 } else { 100 },
        )]),
    )
    .await;
    harness.test().submit_turn("initial turn").await?;
    let _ = initial_mock.single_request();
    let history = [
        json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": first_user_message}]}),
        json!({"type": "function_call", "call_id": retained_call_id, "name": "exec_command", "arguments": "{}"}),
        json!({"type": "function_call_output", "call_id": retained_call_id, "output": retained_output}),
        json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": second_user_message}]}),
        json!({"type": "function_call", "call_id": first_trimmed_call_id, "name": "exec_command", "arguments": "{}"}),
        json!({"type": "function_call", "call_id": second_trimmed_call_id, "name": "exec_command", "arguments": "{}"}),
        json!({"type": "function_call_output", "call_id": first_trimmed_call_id, "output": "x".repeat(12_000)}),
        json!({"type": "function_call_output", "call_id": second_trimmed_call_id, "output": "y".repeat(12_000)}),
    ]
    .into_iter()
    .map(serde_json::from_value)
    .collect::<serde_json::Result<Vec<ResponseItem>>>()?;
    codex.inject_response_items(history).await?;

    let mut response_bodies = vec![sse(vec![
        json!({
            "type": "response.output_item.done",
            "item": {"type": "compaction", "encrypted_content": "REMOTE_COMPACT_SUMMARY"},
        }),
        responses::ev_completed("response-compact"),
    ])];
    if automatic {
        response_bodies.push(responses::sse_completed("after-compact"));
    }
    let compact_mock = responses::mount_sse_sequence(harness.server(), response_bodies).await;

    if automatic {
        harness.test().submit_text_turn("after compact").await?;
    } else {
        codex.submit(Op::Compact).await?;
        wait_for_turn_complete(&codex).await;
    }

    let requests = compact_mock.requests();
    assert_eq!(requests.len(), if automatic { 2 } else { 1 });
    let compact_request = &requests[0];
    assert_eq!(
        compact_request.inputs_of_type("compaction_trigger").len(),
        1
    );
    let user_messages = compact_request.message_input_texts("user");
    assert!(
        user_messages
            .iter()
            .any(|message| message == first_user_message)
    );
    assert!(
        user_messages
            .iter()
            .any(|message| message == second_user_message)
    );
    assert!(
        !user_messages
            .iter()
            .any(|message| message == "after compact")
    );
    if automatic {
        let followup_metadata: Value = serde_json::from_str(
            &requests[1]
                .header("x-codex-turn-metadata")
                .context("follow-up request metadata")?,
        )?;
        assert_eq!(followup_metadata["request_kind"], "turn");
    }
    assert!(compact_request.has_function_call(retained_call_id));
    assert_eq!(
        compact_request
            .function_call_output_text(retained_call_id)
            .as_deref(),
        Some(retained_output),
        "expected compact request to keep the older function output unchanged"
    );
    assert!(
        compact_request.has_function_call(first_trimmed_call_id)
            && compact_request.has_function_call(second_trimmed_call_id),
        "expected compact request to retain both trailing parallel function calls"
    );
    assert_eq!(
        compact_request.function_call_output_text(first_trimmed_call_id),
        Some(CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE.to_string()),
        "expected compact request to rewrite the first trailing function call output"
    );
    assert_eq!(
        compact_request.function_call_output_text(second_trimmed_call_id),
        Some(CONTEXT_WINDOW_TRUNCATED_OUTPUT_MESSAGE.to_string()),
        "expected compact request to rewrite the second trailing function call output"
    );

    assert_eq!(
        compact_request.inputs_of_type("function_call").len(),
        3,
        "expected all function calls after rewriting trailing outputs"
    );
    assert_eq!(
        compact_request.inputs_of_type("function_call_output").len(),
        3,
        "expected all function call outputs after rewriting trailing outputs"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_realtime_refreshes_changed_start_instructions_only_after_compaction() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    let initial_realtime_server = start_remote_realtime_server().await;
    let initial_instructions = "initial custom realtime start instructions";
    let mut initial_builder = remote_realtime_test_codex_builder(&initial_realtime_server)
        .with_config({
            let initial_instructions = initial_instructions.to_string();
            move |config| {
                config.experimental_realtime_start_instructions = Some(initial_instructions);
            }
        });
    let initial = initial_builder.build(&server).await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    let responses_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![responses::ev_completed("r1")]),
            responses::sse(vec![responses::ev_completed("r2")]),
            responses::sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {"type": "compaction", "encrypted_content": "realtime-summary"},
                }),
                responses::ev_completed("r-compact"),
            ]),
            responses::sse(vec![responses::ev_completed("r3")]),
        ],
    )
    .await;

    start_realtime_conversation(initial.codex.as_ref()).await?;
    initial.submit_turn("USER_ONE").await?;
    close_realtime_conversation(initial.codex.as_ref()).await?;
    initial.codex.submit(Op::Shutdown).await?;
    wait_for_event(&initial.codex, |ev| {
        matches!(ev, EventMsg::ShutdownComplete)
    })
    .await;
    initial_realtime_server.shutdown().await;

    let resumed_realtime_server = start_remote_realtime_server().await;
    let changed_instructions = "changed custom realtime start instructions";
    let mut resume_builder = remote_realtime_test_codex_builder(&resumed_realtime_server)
        .with_config({
            let changed_instructions = changed_instructions.to_string();
            move |config| {
                config.experimental_realtime_start_instructions = Some(changed_instructions);
            }
        });
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;

    start_realtime_conversation(resumed.codex.as_ref()).await?;
    resumed.submit_turn("USER_TWO").await?;
    resumed.codex.submit(Op::Compact).await?;
    wait_for_turn_complete(&resumed.codex).await;
    resumed.submit_turn("USER_THREE").await?;

    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 4);
    assert_request_contains_custom_realtime_start(&requests[0], initial_instructions);
    let resumed_body = requests[1].body_json().to_string();
    assert!(
        resumed_body.contains(initial_instructions),
        "expected resumed history to retain the original realtime instructions"
    );
    assert!(
        !resumed_body.contains(changed_instructions),
        "did not expect an active-to-active instruction change to emit a diff"
    );
    assert_eq!(requests[2].inputs_of_type("compaction_trigger").len(), 1);
    assert_eq!(
        requests[3].inputs_of_type("compaction")[0]["encrypted_content"],
        "realtime-summary"
    );
    assert_request_contains_custom_realtime_start(&requests[3], changed_instructions);
    assert!(!requests[3].body_contains_text(initial_instructions));

    close_realtime_conversation(resumed.codex.as_ref()).await?;
    resumed_realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_mid_turn_compact_v2_sends_turn_state_over_http() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_auto_compact_token_limit = Some(200);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();
    let responses_mock = responses::mount_response_sequence(
        harness.server(),
        vec![
            responses::sse_response(responses::sse(vec![
                responses::ev_function_call("call-before-compact", DUMMY_FUNCTION_NAME, "{}"),
                responses::ev_completed_with_tokens("r1", /*total_tokens*/ 500),
            ]))
            .insert_header(TURN_STATE_HEADER, "sampling-state"),
            responses::sse_response(responses::sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "V2_COMPACT_SUMMARY",
                    }
                }),
                responses::ev_completed("r-compact"),
            ]))
            .insert_header(TURN_STATE_HEADER, "compact-state"),
            responses::sse_response(responses::sse(vec![
                responses::ev_function_call("call-after-compact", DUMMY_FUNCTION_NAME, "{}"),
                responses::ev_completed_with_tokens("r2", /*total_tokens*/ 80),
            ]))
            .insert_header(TURN_STATE_HEADER, "continuation-state"),
            responses::sse_response(responses::sse(vec![
                responses::ev_assistant_message("m1", "FINAL_REPLY"),
                responses::ev_completed_with_tokens("r3", /*total_tokens*/ 80),
            ])),
        ],
    )
    .await;

    // Phase 1: sampling mints state and schedules inline v2 compaction.
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "RUN_WITH_MID_TURN_COMPACT_V2".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&codex).await;

    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 4);
    assert!(
        requests
            .iter()
            .all(|request| request.path() == "/v1/responses")
    );
    assert_eq!(requests[0].header(TURN_STATE_HEADER), None);

    // Phase 2: the v2 compaction request replays the state already established by sampling.
    assert!(
        requests[1]
            .body_json()
            .to_string()
            .contains("\"type\":\"compaction_trigger\"")
    );
    assert_eq!(
        requests[1].header(TURN_STATE_HEADER).as_deref(),
        Some("sampling-state")
    );

    // Phase 3: later response headers do not replace the first value in the OnceLock.
    assert_eq!(
        requests[2].header(TURN_STATE_HEADER).as_deref(),
        Some("sampling-state")
    );
    assert_eq!(
        requests[3].header(TURN_STATE_HEADER).as_deref(),
        Some("sampling-state")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_mid_turn_compact_v2_sends_turn_state_over_websocket() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![vec![
        vec![
            responses::ev_response_created("warm-1"),
            responses::ev_completed("warm-1"),
        ],
        vec![
            json!({
                "type": "response.metadata",
                "headers": {(TURN_STATE_HEADER): "sampling-state"},
            }),
            responses::ev_function_call("call-before-compact", DUMMY_FUNCTION_NAME, "{}"),
            responses::ev_completed_with_tokens("r1", /*total_tokens*/ 500),
        ],
        vec![
            json!({
                "type": "response.metadata",
                "headers": {(TURN_STATE_HEADER): "compact-state"},
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "compaction",
                    "encrypted_content": "V2_WS_COMPACT_SUMMARY",
                }
            }),
            responses::ev_completed("r-compact"),
        ],
        vec![
            json!({
                "type": "response.metadata",
                "headers": {(TURN_STATE_HEADER): "continuation-state"},
            }),
            responses::ev_function_call("call-after-compact", DUMMY_FUNCTION_NAME, "{}"),
            responses::ev_completed_with_tokens("r2", /*total_tokens*/ 80),
        ],
        vec![
            responses::ev_assistant_message("m1", "FINAL_REPLY"),
            responses::ev_completed_with_tokens("r3", /*total_tokens*/ 80),
        ],
    ]])
    .await;
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.model_auto_compact_token_limit = Some(200);
        });
    let test = builder.build_with_websocket_server(&server).await?;

    // Phase 1: startup prewarm stays empty, then WebSocket sampling mints state and schedules
    // inline v2 compaction.
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "RUN_WITH_WS_MID_TURN_COMPACT_V2".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&test.codex).await;

    let requests = server.single_connection();
    assert_eq!(requests.len(), 5);
    assert_eq!(requests[0].body_json()["generate"].as_bool(), Some(false));
    // Phase 2: the v2 compact request replays the state already established by sampling.
    assert!(
        requests[2]
            .body_json()
            .to_string()
            .contains("\"type\":\"compaction_trigger\"")
    );
    // Phase 3: both post-compact requests keep replaying that first value.
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["client_metadata"][TURN_STATE_HEADER].clone())
            .collect::<Vec<_>>(),
        vec![
            json!(null),
            json!(null),
            json!("sampling-state"),
            json!("sampling-state"),
            json!("sampling-state"),
        ]
    );

    server.shutdown().await;
    Ok(())
}
