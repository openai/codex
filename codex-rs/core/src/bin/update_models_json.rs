use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_api::AuthProvider;
use codex_api::ModelsClient;
use codex_api::ReqwestTransport;
use codex_app_server_protocol::AuthMode;
use codex_core::ModelProviderInfo;
use codex_core::auth::OPENAI_API_KEY_ENV_VAR;
use codex_core::auth::read_openai_api_key_from_env;
use codex_core::default_client::build_reqwest_client;
use http::HeaderMap;
use std::env;
use std::fs;
use std::path::PathBuf;

const HIGHEST_SUPPORTED_CLIENT_VERSION: &str = "99.99.99";

#[derive(Clone)]
struct ApiKeyAuthProvider {
    token: String,
}

impl AuthProvider for ApiKeyAuthProvider {
    fn bearer_token(&self) -> Option<String> {
        Some(self.token.clone())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let output_path = parse_output_path()?;
    let token = require_openai_api_key()?;

    let provider = ModelProviderInfo::create_openai_provider();
    let api_provider = provider.to_api_provider(Some(AuthMode::ChatGPT))?;
    let transport = ReqwestTransport::new(build_reqwest_client());
    let auth = ApiKeyAuthProvider { token };
    let client = ModelsClient::new(transport, api_provider, auth);
    let response = client
        .list_models(HIGHEST_SUPPORTED_CLIENT_VERSION, HeaderMap::new())
        .await
        .context("failed to fetch /models response")?;

    let payload =
        serde_json::to_string_pretty(&response).context("failed to serialize /models response")?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create models.json parent directory {path}",
                path = parent.display()
            )
        })?;
    }
    fs::write(&output_path, format!("{payload}\n"))
        .with_context(|| format!("failed to write {path}", path = output_path.display()))?;

    println!(
        "Wrote {} models to {}",
        response.models.len(),
        output_path.display()
    );
    Ok(())
}

fn parse_output_path() -> Result<PathBuf> {
    let mut output_path = default_output_path();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("--output requires a path"))?;
                output_path = PathBuf::from(value);
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(anyhow!("unknown argument: {arg}")),
        }
    }

    Ok(output_path)
}

fn print_usage() {
    let default_path = default_output_path();
    println!(
        "usage: update-models-json [--output <path>]\n\nDefault output: {}",
        default_path.display()
    );
}

fn default_output_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models.json")
}

fn require_openai_api_key() -> Result<String> {
    read_openai_api_key_from_env().ok_or_else(|| anyhow!("{OPENAI_API_KEY_ENV_VAR} must be set"))
}
