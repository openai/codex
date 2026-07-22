use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::StartThreadOptions;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

const TEST_WAV_SAMPLE_RATE: u32 = 8_000;
const OMITTED_AUDIO_MARKER: &str = "[omitted 1 audio items ...]";

fn pcm_wav_data_url(sample_count: u32) -> String {
    let padding = sample_count % 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + sample_count + padding).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&TEST_WAV_SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&TEST_WAV_SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&sample_count.to_le_bytes());
    bytes.resize(
        bytes.len() + sample_count as usize + padding as usize,
        /*value*/ 0,
    );
    format!("data:audio/wav;base64,{}", BASE64_STANDARD.encode(bytes))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dynamic_tool_audio_exceeding_the_output_budget_is_omitted() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let call_id = "audio-call";
    let tool_name = "recording";
    let responses_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(call_id, "codex_app", tool_name, "{}"),
                responses::ev_completed("resp-1"),
            ]),
            sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_model_info_override("gpt-5.5", |model_info| {
        model_info.input_modalities.push(InputModality::Audio);
        model_info.truncation_policy = TruncationPolicyConfig::tokens(/*limit*/ 50);
    });
    let base_test = builder.build_with_auto_env(&server).await?;
    let dynamic_tool = DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: "codex_app".to_string(),
        description: "Audio tools.".to_string(),
        tools: vec![DynamicToolNamespaceTool::Function(
            DynamicToolFunctionSpec {
                name: tool_name.to_string(),
                description: "Returns a recording.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
                defer_loading: false,
            },
        )],
    });
    let new_thread = base_test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![dynamic_tool],
            ..StartThreadOptions::new(base_test.config.clone())
        })
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Return a recording".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    let EventMsg::DynamicToolCallRequest(request) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::DynamicToolCallRequest(_))
    })
    .await
    else {
        unreachable!("event guard guarantees DynamicToolCallRequest");
    };
    test.codex
        .submit(Op::DynamicToolResponse {
            id: request.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputAudio {
                    audio_url: pcm_wav_data_url(/*sample_count*/ 80_000),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1]
        .function_call_output(call_id)
        .get("output")
        .cloned()
        .expect("follow-up request should contain the dynamic tool output");
    assert_eq!(
        serde_json::from_value::<FunctionCallOutputPayload>(output)?,
        FunctionCallOutputPayload {
            body: FunctionCallOutputBody::ContentItems(vec![
                FunctionCallOutputContentItem::InputText {
                    text: OMITTED_AUDIO_MARKER.to_string(),
                },
            ]),
            success: None,
        }
    );

    Ok(())
}
