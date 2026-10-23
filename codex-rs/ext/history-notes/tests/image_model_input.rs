//! Verify captured catalog definitions and history attachments reach outgoing model requests.

use std::collections::BTreeMap;
use std::sync::Arc;

use codex_core::config::Config;
use codex_core::config::TokenBudgetConfig;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_history_notes_extension::install;
use codex_login::AuthHeaders;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use http::HeaderMap;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=";

const NATIVE_TOOLS: [(&str, &str, &[&str]); 2] = [
    (
        "history",
        "",
        &["list_windows", "list_items", "read_item", "search_contents"],
    ),
    (
        "notes",
        "   ",
        &[
            "list_files_by_prefix",
            "read_file",
            "search_contents",
            "append_to_file",
            "write_file",
        ],
    ),
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_images_reach_the_next_model_request() -> Result<(), Box<dyn std::error::Error>> {
    let server = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/alpha/history/v2/read_item"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "encrypted_output": "opaque-history-text",
            "images": [
                {"data": PNG, "mime_type": "image/png", "detail": "original"},
                {"data": PNG, "mime_type": "image/png", "detail": "high"},
                {"data": PNG, "mime_type": "image/png", "detail": "auto"}
            ]
        })))
        .mount(&server)
        .await;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "read-history",
                    "history",
                    "read_item",
                    &json!({"window_id": "window", "item_id": "item"}).to_string(),
                ),
                responses::ev_completed("first-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("assistant", "done"),
                responses::ev_completed("second-response"),
            ]),
        ],
    )
    .await;
    let auth = CodexAuth::Headers(AuthHeaders::new(HeaderMap::new()));
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install(
        &mut extensions,
        AuthManager::from_auth_for_testing(auth.clone()),
    );
    let parameters = json!({
        "type": "object", "description": "Catalog read parameters",
        "properties": {"window_id": {"type": "string"}, "item_id": {"type": "string"}},
        "required": ["window_id", "item_id"]
    });
    let catalog_parameters = parameters.clone();
    let test = test_codex()
        .with_auth(auth)
        .with_model_info_override("gpt-5.5", move |model_info| {
            model_info.supports_experimental_context = true;
            let mut messages = json!({});
            for (namespace, description, names) in NATIVE_TOOLS {
                messages[namespace] = json!({"namespace_description": description});
                for name in names {
                    messages[namespace][*name] =
                        json!({"description": format!("Catalog {namespace}.{name}")});
                }
            }
            messages["history"]["read_item"]["parameters"] = json!(catalog_parameters.to_string());
            model_info.model_messages.get_or_insert_default().tools =
                Some(serde_json::from_value(messages).expect("native catalog messages"));
        })
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.model_provider.name = "OpenAI".to_string();
            config.token_budget = Some(TokenBudgetConfig {
                use_history_notes_extension: true,
                ..TokenBudgetConfig::default()
            });
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("Read the image from history.")
        .await?;
    let requests = requests.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        let body = request.body_json();
        for (name, description, tool_names) in NATIVE_TOOLS {
            let namespace = body["tools"]
                .as_array()
                .expect("tools")
                .iter()
                .find(|tool| tool["name"] == name)
                .expect("native namespace");
            assert_eq!(namespace["description"], description);
            let tools = namespace["tools"].as_array().expect("namespace tools");
            assert_eq!(
                tools
                    .iter()
                    .map(|tool| (
                        tool["name"].as_str().unwrap(),
                        tool["description"].as_str().unwrap().to_string(),
                    ))
                    .collect::<BTreeMap<_, _>>(),
                tool_names
                    .iter()
                    .map(|tool| (*tool, format!("Catalog {name}.{tool}")))
                    .collect(),
            );
            if name == "history" {
                assert_eq!(
                    tools
                        .iter()
                        .find(|tool| tool["name"] == "read_item")
                        .unwrap()["parameters"],
                    parameters
                );
            }
        }
    }
    assert_eq!(
        requests[1].function_call_output("read-history")["output"],
        json!([
                {"type": "encrypted_content", "encrypted_content": "opaque-history-text"},
                {"type": "input_image", "image_url": format!("data:image/png;base64,{PNG}"), "detail": "original"},
                {"type": "input_image", "image_url": format!("data:image/png;base64,{PNG}"), "detail": "high"},
                {"type": "input_image", "image_url": format!("data:image/png;base64,{PNG}"), "detail": "auto"}
        ])
    );
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "History/Notes catalog definitions preserve empty and whitespace namespace descriptions, and history images reach the next model request.",
        &requests,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    Ok(())
}
