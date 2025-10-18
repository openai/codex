use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::LoginApiKeyResponse;
use codex_app_server_protocol::RequestId;
use codex_core::auth::AuthDotJson;
use codex_core::auth::get_auth_file;
use codex_core::auth::write_auth_json;
use codex_core::protocol::AskForApproval;
use codex_core::token_data::TokenData;
use codex_core::token_data::parse_id_token;
use codex_protocol::config_types::SandboxMode;
use serde_json::json;
use tokio::time::timeout;

use crate::McpProcess;
use crate::to_response;

pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn login_with_api_key_via_mcp(mcp: &mut McpProcess, api_key: &str) -> anyhow::Result<()> {
    let request_id = mcp
        .send_login_api_key_request(LoginApiKeyParams {
            api_key: api_key.to_string(),
        })
        .await?;

    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await
    .context("loginApiKey timeout")??;

    let _: LoginApiKeyResponse = to_response(response)?;
    Ok(())
}

pub fn write_chatgpt_auth(
    codex_home: &Path,
    access_token: &str,
    account_id: &str,
    plan_type: &str,
) -> std::io::Result<()> {
    let auth_path = get_auth_file(codex_home);
    let id_token_raw = encode_chatgpt_id_token(plan_type)?;
    let id_token = parse_id_token(&id_token_raw).map_err(std::io::Error::other)?;
    let auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token,
            access_token: access_token.to_string(),
            refresh_token: "refresh-token".to_string(),
            account_id: Some(account_id.to_string()),
        }),
        last_refresh: Some(Utc::now()),
    };
    write_auth_json(&auth_path, &auth)
}

fn encode_chatgpt_id_token(plan_type: &str) -> std::io::Result<String> {
    let header = serde_json::to_vec(&json!({ "alg": "none", "typ": "JWT" }))
        .map_err(std::io::Error::other)?;
    let payload = serde_json::to_vec(&json!({
        "https://api.openai.com/auth": {
            "chatgpt_plan_type": plan_type
        }
    }))
    .map_err(std::io::Error::other)?;
    let header_b64 = URL_SAFE_NO_PAD.encode(header);
    let payload_b64 = URL_SAFE_NO_PAD.encode(payload);
    let signature_b64 = URL_SAFE_NO_PAD.encode(b"signature");
    Ok(format!("{header_b64}.{payload_b64}.{signature_b64}"))
}

#[derive(Default)]
pub struct ConfigBuilder {
    model: Option<String>,
    approval_policy: Option<AskForApproval>,
    sandbox_mode: Option<SandboxMode>,
    chatgpt_base_url: Option<String>,
    mock_provider: Option<MockProviderConfig>,
    extra_lines: Vec<String>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults(mut self) -> Self {
        self.model = Some("mock-model".to_string());
        self.approval_policy = Some(AskForApproval::Never);
        self.sandbox_mode = Some(SandboxMode::DangerFullAccess);
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn approval_policy(mut self, policy: AskForApproval) -> Self {
        self.approval_policy = Some(policy);
        self
    }

    pub fn sandbox_mode(mut self, mode: Option<SandboxMode>) -> Self {
        self.sandbox_mode = mode;
        self
    }

    pub fn chatgpt_base_url(mut self, url: impl Into<String>) -> Self {
        self.chatgpt_base_url = Some(url.into());
        self
    }

    pub fn with_mock_provider(mut self, provider: MockProviderConfig) -> Self {
        self.mock_provider = Some(provider);
        self
    }

    pub fn extra_line(mut self, line: impl Into<String>) -> Self {
        self.extra_lines.push(line.into());
        self
    }

    fn build_contents(self) -> String {
        let mut lines = Vec::new();

        if let Some(model) = self.model {
            lines.push(format!(r#"model = "{model}""#));
        }

        if let Some(policy) = self.approval_policy {
            lines.push(format!(r#"approval_policy = "{}""#, policy));
        }

        if let Some(mode) = self.sandbox_mode {
            lines.push(format!(r#"sandbox_mode = "{}""#, mode));
        }

        if let Some(base_url) = self.chatgpt_base_url {
            lines.push(format!(r#"chatgpt_base_url = "{base_url}""#));
        }

        for line in self.extra_lines {
            lines.push(line);
        }

        if let Some(provider) = self.mock_provider {
            lines.push(String::new());
            lines.push(format!(r#"model_provider = "{}""#, provider.id));
            lines.push(String::new());
            lines.push(format!(r#"[model_providers.{}]"#, provider.id));
            lines.push(format!(r#"name = "{}""#, provider.display_name));
            lines.push(format!(r#"base_url = "{}""#, provider.base_url));
            lines.push(format!(r#"wire_api = "{}""#, provider.wire_api));
            if let Some(retries) = provider.request_max_retries {
                lines.push(format!("request_max_retries = {retries}"));
            }
            if let Some(retries) = provider.stream_max_retries {
                lines.push(format!("stream_max_retries = {retries}"));
            }
            if let Some(timeout_ms) = provider.stream_idle_timeout_ms {
                lines.push(format!("stream_idle_timeout_ms = {timeout_ms}"));
            }
            if provider.requires_openai_auth {
                lines.push("requires_openai_auth = true".to_string());
            }
        }

        if !lines.is_empty() {
            lines.push(String::new());
        }

        let mut contents = lines.join("\n");
        if !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents
    }

    pub fn write(self, codex_home: &Path) -> std::io::Result<()> {
        let contents = self.build_contents();
        std::fs::write(codex_home.join("config.toml"), contents)
    }
}

#[derive(Clone)]
pub struct MockProviderConfig {
    id: String,
    display_name: String,
    base_url: String,
    wire_api: String,
    request_max_retries: Option<u64>,
    stream_max_retries: Option<u64>,
    stream_idle_timeout_ms: Option<u64>,
    requires_openai_auth: bool,
}

impl MockProviderConfig {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            id: "mock_provider".to_string(),
            display_name: "Mock provider for test".to_string(),
            base_url: base_url.into(),
            wire_api: "chat".to_string(),
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    pub fn wire_api(mut self, wire_api: impl Into<String>) -> Self {
        self.wire_api = wire_api.into();
        self
    }

    pub fn request_max_retries(mut self, retries: Option<u64>) -> Self {
        self.request_max_retries = retries;
        self
    }

    pub fn stream_max_retries(mut self, retries: Option<u64>) -> Self {
        self.stream_max_retries = retries;
        self
    }

    pub fn stream_idle_timeout_ms(mut self, timeout_ms: Option<u64>) -> Self {
        self.stream_idle_timeout_ms = timeout_ms;
        self
    }

    pub fn requires_openai_auth(mut self, requires: bool) -> Self {
        self.requires_openai_auth = requires;
        self
    }
}
