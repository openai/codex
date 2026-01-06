use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde::de::Deserializer;
use serde::de::{self};
use std::time::Duration;
use std::time::Instant;

use crate::pkce::PkceCodes;
use crate::server::ServerOptions;
use std::io;

const ANSI_BLUE: &str = "\x1b[94m";
const ANSI_GRAY: &str = "\x1b[90m";
const ANSI_RESET: &str = "\x1b[0m";

#[derive(Debug, Clone)]
pub struct DeviceCode {
    pub verification_url: String,
    pub user_code: String,
    device_auth_id: String,
    interval: u64,
}

#[derive(Deserialize)]
struct UserCodeResp {
    device_auth_id: String,
    #[serde(alias = "user_code", alias = "usercode")]
    user_code: String,
    #[serde(default, deserialize_with = "deserialize_interval")]
    interval: u64,
}

#[derive(Serialize)]
struct UserCodeReq {
    client_id: String,
}

#[derive(Serialize)]
struct TokenPollReq {
    device_auth_id: String,
    user_code: String,
}

fn deserialize_interval<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.trim()
        .parse::<u64>()
        .map_err(|e| de::Error::custom(format!("invalid u64 string: {e}")))
}

#[derive(Deserialize)]
struct CodeSuccessResp {
    authorization_code: String,
    code_challenge: String,
    code_verifier: String,
}

/// Request the user code and polling interval.
async fn request_user_code(
    client: &reqwest::Client,
    auth_base_url: &str,
    client_id: &str,
) -> std::io::Result<UserCodeResp> {
    let url = format!("{auth_base_url}/deviceauth/usercode");
    let body = serde_json::to_string(&UserCodeReq {
        client_id: client_id.to_string(),
    })
    .map_err(std::io::Error::other)?;
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(std::io::Error::other)?;

    if !resp.status().is_success() {
        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "device code login is not enabled for this Codex server. Use the browser login or verify the server URL.",
            ));
        }

        return Err(std::io::Error::other(format!(
            "device code request failed with status {status}"
        )));
    }

    let body = resp.text().await.map_err(std::io::Error::other)?;
    serde_json::from_str(&body).map_err(std::io::Error::other)
}

/// Poll token endpoint until a code is issued or timeout occurs.
async fn poll_for_token(
    client: &reqwest::Client,
    auth_base_url: &str,
    device_auth_id: &str,
    user_code: &str,
    interval: u64,
) -> std::io::Result<CodeSuccessResp> {
    let url = format!("{auth_base_url}/deviceauth/token");
    let max_wait = Duration::from_secs(15 * 60);
    let start = Instant::now();

    loop {
        let body = serde_json::to_string(&TokenPollReq {
            device_auth_id: device_auth_id.to_string(),
            user_code: user_code.to_string(),
        })
        .map_err(std::io::Error::other)?;
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(std::io::Error::other)?;

        let status = resp.status();

        if status.is_success() {
            return resp.json().await.map_err(std::io::Error::other);
        }

        if status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND {
            if start.elapsed() >= max_wait {
                return Err(std::io::Error::other(
                    "device auth timed out after 15 minutes",
                ));
            }
            let sleep_for = Duration::from_secs(interval).min(max_wait - start.elapsed());
            tokio::time::sleep(sleep_for).await;
            continue;
        }

        return Err(std::io::Error::other(format!(
            "device auth failed with status {}",
            resp.status()
        )));
    }
}

fn print_device_code_prompt(verification_url: &str, code: &str) {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "\nWelcome to Codex [v{ANSI_GRAY}{version}{ANSI_RESET}]\n{ANSI_GRAY}OpenAI's command-line coding agent{ANSI_RESET}\n\
\nFollow these steps to sign in with ChatGPT using device code authorization:\n\
\n1. Open this link in your browser and sign in to your account\n   {ANSI_BLUE}{verification_url}{ANSI_RESET}\n\
\n2. Enter this one-time code {ANSI_GRAY}(expires in 15 minutes){ANSI_RESET}\n   {ANSI_BLUE}{code}{ANSI_RESET}\n\
\n{ANSI_GRAY}Device codes are a common phishing target. Never share this code.{ANSI_RESET}\n",
    );
}

pub async fn request_device_code(opts: &ServerOptions) -> std::io::Result<DeviceCode> {
    let client = reqwest::Client::new();
    let base_url = opts.issuer.trim_end_matches('/');
    let api_base_url = format!("{base_url}/api/accounts");
    let uc = request_user_code(&client, &api_base_url, &opts.client_id).await?;

    Ok(DeviceCode {
        verification_url: format!("{base_url}/codex/device"),
        user_code: uc.user_code,
        device_auth_id: uc.device_auth_id,
        interval: uc.interval,
    })
}

pub async fn complete_device_code_login(
    opts: ServerOptions,
    device_code: DeviceCode,
) -> std::io::Result<()> {
    let client = reqwest::Client::new();
    let base_url = opts.issuer.trim_end_matches('/');
    let api_base_url = format!("{base_url}/api/accounts");

    let code_resp = poll_for_token(
        &client,
        &api_base_url,
        &device_code.device_auth_id,
        &device_code.user_code,
        device_code.interval,
    )
    .await?;

    let pkce = PkceCodes {
        code_verifier: code_resp.code_verifier,
        code_challenge: code_resp.code_challenge,
    };
    let redirect_uri = format!("{base_url}/deviceauth/callback");

    let tokens = crate::server::exchange_code_for_tokens(
        base_url,
        &opts.client_id,
        &redirect_uri,
        &pkce,
        &code_resp.authorization_code,
    )
    .await
    .map_err(|err| std::io::Error::other(format!("device code exchange failed: {err}")))?;

    if let Err(message) = crate::server::ensure_workspace_allowed(
        opts.forced_chatgpt_workspace_id.as_deref(),
        &tokens.id_token,
    ) {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, message));
    }

    crate::server::persist_tokens_async(
        &opts.codex_home,
        None,
        tokens.id_token,
        tokens.access_token,
        tokens.refresh_token,
        opts.cli_auth_credentials_store_mode,
    )
    .await
}

/// Full device code login flow.
pub async fn run_device_code_login(opts: ServerOptions) -> std::io::Result<()> {
    let device_code = request_device_code(&opts).await?;
    print_device_code_prompt(&device_code.verification_url, &device_code.user_code);
    complete_device_code_login(opts, device_code).await
}
