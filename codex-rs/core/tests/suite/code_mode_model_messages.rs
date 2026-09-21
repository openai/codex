//! Wait descriptions and parameter schemas resolve independently without changing other tools.

use anyhow::Result;
use codex_protocol::openai_models::ToolMessages;
use codex_protocol::openai_models::ToolMode;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;

const PARAMETERS: &str = r#"{"type":"object","properties":{"cell_id":{"type":"string","description":"Catalog cell identifier."}},"required":["cell_id"],"additionalProperties":false}"#;

#[test_case(ToolMode::CodeMode, json!({"description":"  Catalog wait. {{ literal }}\n"}), false; "description_only")]
#[test_case(ToolMode::CodeMode, json!({"parameters":PARAMETERS}), true; "parameters_only")]
#[test_case(ToolMode::CodeModeOnly, json!({"description":"Catalog wait.","parameters":PARAMETERS}), true; "both_in_code_mode_only")]
#[test_case(ToolMode::CodeModeOnly, json!({"description":"","parameters":""}), false; "empty_description_and_invalid_schema")]
#[test_case(ToolMode::CodeMode, json!({"description":"Catalog wait.","parameters":"{"}), false; "invalid_parameters_preserve_description")]
#[test_case(ToolMode::CodeMode, json!({"description":null,"parameters":null}), false; "null_fields_fall_back")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_overrides_are_independent(
    tool_mode: ToolMode,
    overrides: Value,
    valid_parameters: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![sse_completed("bundled"), sse_completed("catalog")],
    )
    .await;
    for messages in [
        None,
        Some(serde_json::from_value::<ToolMessages>(
            json!({"code_mode": {"wait": overrides}}),
        )?),
    ] {
        let test = test_codex()
            .with_model_info_override("gpt-5.5", move |model| {
                model.tool_mode = Some(tool_mode);
                model.use_responses_lite = false;
                model.model_messages.as_mut().expect("model messages").tools = messages;
            })
            .with_config(|config| {
                config.code_mode.disable_in_process_fallback = true;
            })
            .build_with_auto_env(&server)
            .await?;
        test.submit_turn("Inspect the available tools.").await?;
    }
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let mut expected = requests[0].body_json()["tools"].clone();
    let wait = expected
        .as_array_mut()
        .expect("tools")
        .iter_mut()
        .find(|tool| tool["name"] == "wait")
        .expect("wait");
    if let Some(description) = overrides["description"].as_str() {
        wait["description"] = json!(description);
    }
    if valid_parameters {
        wait["parameters"] = serde_json::from_str(PARAMETERS)?;
    }
    assert_eq!(requests[1].body_json()["tools"], expected);
    Ok(())
}
