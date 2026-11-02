//! Exercises provider discovery through the public app-server request.
#![cfg(test)]

use anyhow::Result;
use app_test_support::TestAppServer;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

const CLEARED_AUTH_ENV: &[(&str, Option<&str>)] = &[
    ("OPENAI_API_KEY", None),
    ("CODEX_API_KEY", None),
    ("CODEX_ACCESS_TOKEN", None),
    ("OPENAI_FEDERATION_RULE_ID", None),
    ("OPENAI_IDENTITY_TOKEN_FILE", None),
    ("OPENAI_WORKLOAD_IDENTITY_CONTEXT", None),
];

#[tokio::test]
async fn list_without_provider_definitions_only_returns_openai() -> Result<()> {
    for (config, default_id, base_url) in [
        ("", "openai", None),
        ("model_provider = 'ollama'", "ollama", None),
        (
            "openai_base_url = 'https://proxy.example.test/v1'",
            "openai",
            Some("https://proxy.example.test/v1"),
        ),
        (
            "openai_base_url = 'https://proxy.example.test/v1?token=test-only'",
            "openai",
            None,
        ),
    ] {
        let home = TempDir::new()?;
        std::fs::write(home.path().join("config.toml"), config)?;
        let mut server = TestAppServer::builder()
            .with_codex_home(home.path())
            .with_env_overrides(CLEARED_AUTH_ENV)
            .build_initialized()
            .await?;
        let id = server
            .send_request("modelProvider/list", Some(json!({})))
            .await?;
        let list: serde_json::Value = server.read_response(id).await?;
        assert_eq!(
            list,
            json!({
                "data": [{"id":"openai", "name":"OpenAI", "canEdit":false,
                    "baseUrl":base_url, "settings":{"type":"openai"}}],
                "defaultProviderId":default_id, "canChangeDefault":true,
            })
        );
    }
    Ok(())
}

#[tokio::test]
async fn list_builtin_and_custom_providers_with_editable_settings() -> Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        r#"
[model_providers.amazon-bedrock]
base_url = "https://bedrock.example.test/v1"
[model_providers.amazon-bedrock.aws]
profile = "work"
region = "us-east-1"
[model_providers.amazon-bedrock-runtime.aws]
region = "us-east-1"
[model_providers.gcp]
[model_providers.basic]
name = "Basic auth gateway"
base_url = "https://user:password@basic.example.test/v1"
[model_providers.external]
name = "External gateway"
base_url = "https://external.example.test/v1?token=test-only"
env_key = "EXTERNAL_KEY"
http_headers = { Authorization = "Bearer test-only" }
[model_providers.query]
name = "Query gateway"
base_url = "https://query.example.test/v1"
query_params = { api_key = "test-only-query-key" }
[model_providers.team]
name = "Team Gateway"
base_url = "https://gateway.example.test/v1"
env_key = "TEAM_KEY"
"#,
    )?;
    std::fs::write(
        home.path().join("requirements.toml"),
        r#"
model_provider = "corp.gateway"
[model_providers."corp.gateway"]
name = "Managed gateway"
base_url = "https://managed.example.test"
"#,
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(CLEARED_AUTH_ENV)
        .build_initialized()
        .await?;
    let list = server
        .send_request("modelProvider/list", Some(json!({})))
        .await?;
    let all: serde_json::Value = server.read_response(list).await?;
    assert_eq!(
        all,
        json!({
            "defaultProviderId":"corp.gateway", "canChangeDefault":false,
            "data": [
                {
                    "id":"amazon-bedrock", "name":"Amazon Bedrock Mantle", "canEdit":true,
                    "baseUrl":"https://bedrock.example.test/v1", "settings":{"type":"bedrock", "profile":"work", "region":"us-east-1"},
                },
                {
                    "id":"amazon-bedrock-runtime", "name":"Amazon Bedrock Runtime", "canEdit":true,
                    "baseUrl":null, "settings":{"type":"bedrock", "profile":null, "region":"us-east-1"},
                },
                {
                    "id":"basic", "name":"Basic auth gateway", "canEdit":true, "baseUrl":null,
                    "settings":{"type":"customGateway", "auth":{"mode":"externallyConfigured", "envKey":null}},
                },
                {
                    "id":"corp.gateway", "name":"Managed gateway", "canEdit":false,
                    "baseUrl":"https://managed.example.test", "settings":{"type":"customGateway", "auth":{"mode":"none"}},
                },
                {
                    "id":"external", "name":"External gateway", "canEdit":true,
                    "baseUrl":null, "settings":{"type":"customGateway", "auth":{"mode":"externallyConfigured", "envKey":"EXTERNAL_KEY"}},
                },
                {
                    "id":"gcp", "name":"Google Cloud Vertex AI", "canEdit":true,
                    "baseUrl":null, "settings":{"type":"gcp", "projectId":null, "location":null, "credentialFile":null},
                },
                {
                    "id":"openai", "name":"OpenAI", "canEdit":false,
                    "baseUrl":null, "settings":{"type":"openai"},
                },
                {
                    "id":"query", "name":"Query gateway", "canEdit":true,
                    "baseUrl":"https://query.example.test/v1", "settings":{"type":"customGateway", "auth":{"mode":"externallyConfigured", "envKey":null}},
                },
                {
                    "id":"team", "name":"Team Gateway", "canEdit":true,
                    "baseUrl":"https://gateway.example.test/v1", "settings":{"type":"customGateway", "auth":{"mode":"environmentVariable", "envKey":"TEAM_KEY"}},
                },
            ],
        })
    );
    Ok(())
}
