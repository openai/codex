//! Verifies catalog tool descriptions reach the V2 outbound tool without changing its parameters.

use anyhow::Result;
use codex_core::config::AgentRoleConfig;
use codex_features::Feature;
use codex_protocol::openai_models::ToolMessages;
use codex_protocol::protocol::MultiAgentVersion;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;

#[test_case(json!(null); "missing_tools")]
#[test_case(json!({}); "missing_multi_agent")]
#[test_case(json!({"multi_agent": null}); "null_multi_agent")]
#[test_case(json!({"multi_agent": {}}); "missing_spawn_agent")]
#[test_case(json!({"multi_agent": {"spawn_agent": null}}); "null_spawn_agent")]
#[test_case(json!({"multi_agent": {"spawn_agent": {}}}); "missing_description")]
#[test_case(json!({"multi_agent": {"spawn_agent": {"description": null}}}); "null_description")]
#[test_case(json!({"multi_agent": {"spawn_agent": {"description": "Catalog spawn."}}}); "catalog_description")]
#[test_case(json!({"multi_agent": {"spawn_agent": {"description": ""}}}); "empty_description")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_catalog_descriptions_preserve_outbound_schema(
    tool_messages: Value,
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
                model.multi_agent_version = Some(MultiAgentVersion::V2);
                model.model_messages.as_mut().expect("model messages").tools = messages;
            })
            .with_config(|config| {
                config
                    .features
                    .enable(Feature::MultiAgentV2)
                    .expect("enable V2");
                config.multi_agent_v2.tool_namespace = Some("delegation".to_string());
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
    let mut expected = namespace_child_tool(&requests[0].body_json(), "delegation", "spawn_agent")
        .expect("default spawn agent tool")
        .clone();
    let actual = namespace_child_tool(&requests[1].body_json(), "delegation", "spawn_agent")
        .expect("catalog spawn agent tool")
        .clone();
    let messages = &tool_messages["multi_agent"]["spawn_agent"];
    if let Some(description) = messages["description"].as_str() {
        let actual_description = actual["description"].as_str().expect("tool description");
        assert!(actual_description.contains(description));
        assert!(!actual_description.contains("Spawns an agent to work on the specified task."));
        assert!(actual_description.ends_with("Local delegation hint."));
        expected["description"] = actual["description"].clone();
    }
    assert_eq!(actual, expected);
    Ok(())
}
