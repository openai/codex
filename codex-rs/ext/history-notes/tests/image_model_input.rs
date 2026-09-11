//! Verify history attachments survive native preparation into the next model request.

use std::sync::Arc;

use codex_core::config::Config;
use codex_core::config::TokenBudgetConfig;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_history_notes_extension::install;
use codex_login::AuthHeaders;
use codex_login::AuthManager;
use codex_login::CodexAuth;
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
    let test = test_codex()
        .with_auth(auth)
        .with_model_info_override("gpt-5.5", |model_info| {
            model_info.supports_experimental_context = true;
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
    assert_eq!(
        requests[1].function_call_output("read-history")["output"],
        json!([
                {"type": "encrypted_content", "encrypted_content": "opaque-history-text"},
                {"type": "input_image", "image_url": format!("data:image/png;base64,{PNG}"), "detail": "original"},
                {"type": "input_image", "image_url": format!("data:image/png;base64,{PNG}"), "detail": "high"},
                {"type": "input_image", "image_url": format!("data:image/png;base64,{PNG}"), "detail": "auto"}
        ])
    );
    Ok(())
}
