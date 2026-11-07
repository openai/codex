//! Wait descriptions and parameter schemas resolve independently without changing other tools.

use anyhow::Result;
use codex_features::Feature;
use codex_protocol::openai_models::ToolMessages;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::responses;
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

#[test_case(false; "disabled")]
#[test_case(true; "enabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_tools_follow_incremental_exec_flag(incremental_exec_enabled: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![sse_completed("response-0"), sse_completed("response-1")],
    )
    .await;
    let mut builder = test_codex().with_config(move |config| {
        config
            .features
            .enable(Feature::IncrementalTools)
            .expect("enable incremental tools");
        config
            .features
            .set_enabled(Feature::IncrementalExecTools, incremental_exec_enabled)
            .expect("set incremental exec tools");
        config.code_mode.disable_in_process_fallback = true;
    });
    for (slug, instructions) in [
        ("gate-initial", "Initial code instructions."),
        ("gate-updated", "Updated code instructions."),
    ] {
        builder = builder.with_model_info_override(slug, move |model| {
            model.use_responses_lite = true;
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.supports_search_tool = false;
            model.model_messages.as_mut().expect("model messages").tools = Some(
                serde_json::from_value(json!({
                    "code_mode": {"exec": {"description": instructions}},
                }))
                .expect("code mode messages"),
            );
        });
    }
    let test = builder
        .with_model("gate-initial")
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Begin with the original instructions.")
        .await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("gate-updated".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("Continue with changed instructions.")
        .await?;
    let definitions = mock
        .requests()
        .iter()
        .map(|request| {
            request
                .inputs_of_type("additional_tools")
                .iter()
                .rev()
                .find_map(|item| responses::namespace_child_tool(item, "functions", "exec"))
                .expect("exec definition")
                .clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 2);
    let initial_description = definitions[0]["description"].as_str().expect("description");
    assert!(initial_description.starts_with("Initial code instructions."));
    let mut expected = definitions[0].clone();
    if !incremental_exec_enabled {
        expected["description"] = json!(initial_description.replacen(
            "Initial code instructions.",
            "Updated code instructions.",
            1
        ));
    }
    assert_eq!(definitions[1], expected);
    Ok(())
}
