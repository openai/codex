//! Outgoing context-tool definitions follow the captured acting model.

use super::*;
use codex_protocol::openai_models::ContextManagementToolMessages;
use codex_protocol::openai_models::ModelInfo;
use pretty_assertions::assert_eq;

fn configure_tools(model: &mut ModelInfo, description: &str) {
    let message = ToolMessage {
        description: Some(description.to_string()),
        parameters: Some(r#"{"type":"object","properties":{},"additionalProperties":false,"description":"Catalog arguments."}"#.to_string()),
    };
    model
        .model_messages
        .get_or_insert_default()
        .tools
        .get_or_insert_default()
        .context_management = Some(ContextManagementToolMessages {
        new_context: Some(message.clone()),
        get_context_remaining: Some(message),
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_context_tool_definitions_follow_captured_models() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("first")]),
            sse(vec![ev_completed("second")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.use_responses_lite = false;
            model.tool_mode = Some(ToolMode::Direct);
            configure_tools(model, "Catalog context guidance.");
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("enable token budget");
            let catalog = config.model_catalog.as_mut().expect("model catalog");
            let mut updated = catalog
                .models
                .iter()
                .find(|model| model.slug == "gpt-5.4")
                .unwrap()
                .clone();
            updated.slug = "context-tools-updated".to_string();
            configure_tools(&mut updated, "");
            catalog.models.push(updated);
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("Begin work.").await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        codex_protocol::protocol::ThreadSettingsOverrides {
            model: Some("context-tools-updated".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("Continue with updated tools.")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    for (request, description) in requests.iter().zip(["Catalog context guidance.", ""]) {
        let body = request.body_json();
        for name in ["new_context", "get_context_remaining"] {
            let tool = body["tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|tool| tool["name"] == name)
                .unwrap();
            assert_eq!(tool["description"], description);
            assert_eq!(tool["parameters"]["description"], "Catalog arguments.");
        }
    }
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "Context tool descriptions and parameters use the captured model; selecting a model with explicit empty descriptions preserves empty text and prior history.",
        &requests,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    Ok(())
}
