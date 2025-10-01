use std::path::Path;

use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SetDefaultModelParams;
use codex_app_server_protocol::SetDefaultModelResponse;
use codex_core::config::ConfigToml;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_default_model_persists_overrides() {
    let codex_home = TempDir::new().expect("create tempdir");
    create_config_toml(codex_home.path()).expect("write config.toml");

    let mut mcp = McpProcess::new(codex_home.path())
        .await
        .expect("spawn mcp process");
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize())
        .await
        .expect("init timeout")
        .expect("init failed");

    let params = SetDefaultModelParams {
        model: Some("gpt-4.1".to_string()),
        reasoning_effort: None,
    };

    let request_id = mcp
        .send_set_default_model_request(params)
        .await
        .expect("send setDefaultModel");

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .expect("setDefaultModel timeout")
    .expect("setDefaultModel response");

    let _: SetDefaultModelResponse =
        to_response(resp).expect("deserialize setDefaultModel response");

    let config_path = codex_home.path().join("config.toml");
    let config_contents = tokio::fs::read_to_string(&config_path)
        .await
        .expect("read config.toml");
    let config_toml: ConfigToml = toml::from_str(&config_contents).expect("parse config.toml");

    assert_eq!(
        ConfigToml {
            model: Some("gpt-4.1".to_string()),
            model_reasoning_effort: None,
            ..Default::default()
        },
        config_toml,
    );
}

// Helper to create a config.toml; mirrors create_conversation.rs
fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "gpt-5-codex"
model_reasoning_effort = "medium"
"#,
    )
}
