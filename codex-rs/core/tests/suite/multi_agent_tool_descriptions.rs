//! Verifies V2 catalog tool messages change only their selected description or parameter schema.

use anyhow::Result;
use codex_core::config::AgentRoleConfig;
use codex_features::Feature;
use codex_protocol::openai_models::ToolMessages;
use codex_protocol::protocol::MultiAgentVersion;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;

const TOOL_NAMES: [&str; 6] = [
    "spawn_agent",
    "send_message",
    "followup_task",
    "wait_agent",
    "interrupt_agent",
    "list_agents",
];

const CATALOG_PARAMETERS: &str = r#"{
    "type": "object",
    "title": "Catalog parameters",
    "properties": {
        "catalog_limit": {"type": "integer", "description": "Catalog limit."},
        "message": {"type": "string", "description": "Catalog message.", "encrypted": false}
    },
    "required": ["catalog_limit"],
    "additionalProperties": false,
    "$defs": {"unused": {"type": "string"}}
}"#;

// The existing JsonSchema subset retains supported fields and drops unknown keywords.
const EXPECTED_CATALOG_PARAMETERS: &str = r#"{
    "type": "object",
    "properties": {
        "catalog_limit": {"type": "integer", "description": "Catalog limit."},
        "message": {"type": "string", "description": "Catalog message.", "encrypted": false}
    },
    "required": ["catalog_limit"],
    "additionalProperties": false,
    "$defs": {"unused": {"type": "string"}}
}"#;

#[derive(Clone, Copy)]
enum Exposure {
    Namespaced,
    Plain,
    CodeMode,
    V1,
}

fn all_tool_messages(message: Value) -> Value {
    json!({
        "multi_agent": TOOL_NAMES
            .map(|name| {
                let mut message = message.clone();
                if let Some(description) = message["description"].as_str() {
                    message["description"] = json!(description.replace("TOOL_NAME", name));
                }
                (name.to_string(), message)
            })
            .into_iter()
            .collect::<serde_json::Map<String, Value>>()
    })
}

#[test_case(json!(null), Exposure::Namespaced, None; "missing_tools")]
#[test_case(json!({}), Exposure::Namespaced, None; "missing_multi_agent")]
#[test_case(json!({"multi_agent": null}), Exposure::Namespaced, None; "null_multi_agent")]
#[test_case(json!({"multi_agent": {}}), Exposure::Namespaced, None; "missing_tools_in_family")]
#[test_case(all_tool_messages(json!(null)), Exposure::Namespaced, None; "null_tool")]
#[test_case(all_tool_messages(json!({})), Exposure::Namespaced, None; "missing_description")]
#[test_case(all_tool_messages(json!({"description": null})), Exposure::Namespaced, None; "null_description")]
#[test_case(all_tool_messages(json!({"description": "  Catalog TOOL_NAME description.\n{{literal_placeholder}}  "})), Exposure::Namespaced, None; "catalog_description")]
#[test_case(all_tool_messages(json!({"description": ""})), Exposure::Namespaced, None; "empty_description")]
#[test_case(json!({"multi_agent": {"send_message": {"description": "Catalog send."}}}), Exposure::Namespaced, None; "sparse_sibling_fallback")]
#[test_case(all_tool_messages(json!({"description": "Catalog TOOL_NAME description."})), Exposure::Plain, None; "plain_tools")]
#[test_case(all_tool_messages(json!({"description": "Catalog TOOL_NAME description."})), Exposure::CodeMode, None; "code_mode_declarations")]
#[test_case(all_tool_messages(json!({"description": ""})), Exposure::CodeMode, None; "empty_code_mode_descriptions")]
#[test_case(all_tool_messages(json!({"description": "Catalog TOOL_NAME description."})), Exposure::V1, None; "v1_unchanged")]
#[test_case(all_tool_messages(json!({"parameters": CATALOG_PARAMETERS})), Exposure::Namespaced, Some(EXPECTED_CATALOG_PARAMETERS); "catalog_parameters")]
#[test_case(all_tool_messages(json!({"parameters": CATALOG_PARAMETERS})), Exposure::Plain, Some(EXPECTED_CATALOG_PARAMETERS); "plain_parameters")]
#[test_case(all_tool_messages(json!({"parameters": CATALOG_PARAMETERS})), Exposure::CodeMode, Some(EXPECTED_CATALOG_PARAMETERS); "code_mode_parameters")]
#[test_case(all_tool_messages(json!({"parameters": CATALOG_PARAMETERS})), Exposure::V1, None; "v1_parameters_unchanged")]
#[test_case(json!({"multi_agent": {
    "spawn_agent": {"parameters": null},
    "send_message": {"parameters": "{"},
    "followup_task": {"parameters": r#"{"type":"object","properties":{"message":true}}"#},
    "wait_agent": {"parameters": r#"{"type":["object"]}"#},
    "interrupt_agent": {"parameters": r#"[null,"object",null,null,null,null,null,{"message":{"type":"string"}},null,null,null,null,null,null,null]"#},
    "list_agents": {"parameters": "{}"}
}}), Exposure::Namespaced, None; "missing_or_invalid_parameters_fall_back")]
#[test_case(json!({"multi_agent": {
    "spawn_agent": {"parameters": r#"{"type":"object"}"#},
    "send_message": {"parameters": r#"{"type":"object"}"#},
    "followup_task": {"parameters": r#"{"type":"object"}"#}
}}), Exposure::Namespaced, None; "missing_encrypted_parameters_fall_back")]
#[test_case(json!({"multi_agent": {"send_message": {"parameters": CATALOG_PARAMETERS}}}), Exposure::Namespaced, Some(EXPECTED_CATALOG_PARAMETERS); "sparse_parameters")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_agent_catalog_messages_change_only_selected_tool_fields(
    tool_messages: Value,
    exposure: Exposure,
    expected_parameters: Option<&str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-default"), sse_completed("resp-catalog")],
    )
    .await;
    for messages in [
        None,
        serde_json::from_value::<Option<ToolMessages>>(tool_messages.clone())?,
    ] {
        let test = test_codex()
            .with_model_info_override("gpt-5.2", move |model| {
                model.multi_agent_version = Some(if matches!(exposure, Exposure::V1) {
                    MultiAgentVersion::V1
                } else {
                    MultiAgentVersion::V2
                });
                model.model_messages.as_mut().expect("model messages").tools = messages;
            })
            .with_config(move |config| {
                if matches!(exposure, Exposure::V1) {
                    config.features.enable(Feature::Collab).expect("enable V1");
                    config
                        .features
                        .disable(Feature::MultiAgentV2)
                        .expect("disable V2");
                } else {
                    config
                        .features
                        .enable(Feature::MultiAgentV2)
                        .expect("enable V2");
                }
                if matches!(exposure, Exposure::CodeMode) {
                    config
                        .features
                        .enable(Feature::CodeMode)
                        .expect("enable Code Mode");
                    // Inspect declarations even when the test has no Code Mode host to execute them.
                    config.code_mode.disable_in_process_fallback = true;
                    config.multi_agent_v2.non_code_mode_only = false;
                }
                config.multi_agent_v2.tool_namespace =
                    (!matches!(exposure, Exposure::Plain)).then(|| "delegation".to_string());
                config.multi_agent_v2.hide_spawn_agent_metadata = false;
                config.multi_agent_v2.expose_spawn_agent_model_overrides = true;
                config.multi_agent_v2.usage_hint_text = Some("Local delegation hint.".to_string());
                config.agent_roles.insert(
                    "researcher".to_string(),
                    AgentRoleConfig {
                        description: Some("Research the assigned question.".to_string()),
                        config_file: None,
                        nickname_candidates: None,
                    },
                );
            })
            .build_with_auto_env(&server)
            .await?;
        test.submit_turn("Inspect the available tools.").await?;
    }

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let mut expected = requests[0].body_json()["tools"].clone();
    let actual = requests[1].body_json()["tools"].clone();
    if !matches!(exposure, Exposure::V1) {
        let tools = if matches!(exposure, Exposure::Plain) {
            expected.as_array_mut().expect("plain tools")
        } else {
            expected
                .as_array_mut()
                .expect("tools")
                .iter_mut()
                .find(|tool| tool["name"] == "delegation")
                .expect("delegation namespace")["tools"]
                .as_array_mut()
                .expect("namespace tools")
        };
        let actual_tools = if matches!(exposure, Exposure::Plain) {
            actual.as_array().expect("plain tools")
        } else {
            actual
                .as_array()
                .expect("tools")
                .iter()
                .find(|tool| tool["name"] == "delegation")
                .expect("delegation namespace")["tools"]
                .as_array()
                .expect("namespace tools")
        };
        for name in TOOL_NAMES {
            let expected_tool = tools
                .iter_mut()
                .find(|tool| tool["name"] == name)
                .expect(name);
            let actual_tool = actual_tools
                .iter()
                .find(|tool| tool["name"] == name)
                .expect(name);
            if let Some(description) = tool_messages["multi_agent"][name]["description"].as_str() {
                let bundled = expected_tool["description"]
                    .as_str()
                    .expect("bundled description");
                let declaration = bundled
                    .find("\n\nexec tool declaration:")
                    .map(|index| &bundled[index..])
                    .unwrap_or_default();
                if matches!(exposure, Exposure::CodeMode) {
                    assert!(
                        declaration.contains(&format!("delegation__{name}")),
                        "Code Mode description for {name}: {bundled}"
                    );
                }
                let replacement = if name == "spawn_agent" {
                    let actual_description = actual_tool["description"]
                        .as_str()
                        .expect("spawn description");
                    assert!(actual_description.contains(description));
                    assert!(
                        !actual_description
                            .contains("Spawns an agent to work on the specified task.")
                    );
                    assert!(
                        actual_description
                            .strip_suffix(declaration)
                            .expect("unchanged declaration")
                            .ends_with("Local delegation hint.")
                    );
                    actual_description.to_string()
                } else {
                    format!("{description}{declaration}")
                };
                expected_tool["description"] = json!(replacement);
            }
            if let Some(parameters) = expected_parameters
                && tool_messages["multi_agent"][name]["parameters"].is_string()
            {
                expected_tool["parameters"] = serde_json::from_str(parameters)?;
                if matches!(name, "spawn_agent" | "send_message" | "followup_task") {
                    expected_tool["parameters"]["properties"]["message"]["encrypted"] = json!(true);
                }
                if matches!(exposure, Exposure::CodeMode) {
                    let description = expected_tool["description"].as_str().expect("description");
                    let (prefix, signature) =
                        description.rsplit_once("(args: ").expect("arguments");
                    let (_, result) = signature.split_once("): Promise").expect("result type");
                    expected_tool["description"] = json!(format!(
                        "{prefix}(args: {{\n  // Catalog limit.\n  catalog_limit: number;\n  // Catalog message.\n  message?: string;\n}}): Promise{result}"
                    ));
                }
            }
        }
    }
    assert_eq!(actual, expected);
    Ok(())
}
