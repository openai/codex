use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde::de::Deserializer;
use serde::de::{self};
use std::time::Duration;
use std::time::Instant;

use crate::pkce::PkceCodes;
use crate::server::ServerOptions;
use std::io::Write;
use std::io::{self};

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
        return Err(std::io::Error::other(format!(
            "device code request failed with status {}",
            resp.status()
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

// Helper to print colored text if terminal supports ANSI
fn print_colored_warning_device_code() {
    // ANSI escape code for bright yellow
    const YELLOW: &str = "\x1b[93m";
    const RESET: &str = "\x1b[0m";
    let warning = "WARN!!! device code authentication has potential risks and\n\
        should be used with caution only in cases where browser support \n\
        is missing. This is prone to attacks.\n\
        \n\
        - This code is valid for 15 minutes.\n\
        - Do not share this code with anyone.\n\
        ";
    let mut stdout = io::stdout().lock();
    let _ = write!(stdout, "{YELLOW}{warning}{RESET}");
    let _ = stdout.flush();
}

/// Full device code login flow.
pub async fn run_device_code_login(opts: ServerOptions) -> std::io::Result<()> {
    let client = reqwest::Client::new();
    let base_url = opts.issuer.trim_end_matches('/');
    let api_base_url = format!("{}/api/accounts", opts.issuer.trim_end_matches('/'));
    print_colored_warning_device_code();
    println!("⏳ Generating a new 9-digit device code for authentication...\n");
    let uc = request_user_code(&client, &api_base_url, &opts.client_id).await?;

    println!(
        "To authenticate, visit: {}/deviceauth/authorize and enter code: {}",
        api_base_url, uc.user_code
    );

    let code_resp = poll_for_token(
        &client,
        &api_base_url,
        &uc.device_auth_id,
        &uc.user_code,
        uc.interval,
    )
    .await?;

    let pkce = PkceCodes {
        code_verifier: code_resp.code_verifier,
        code_challenge: code_resp.code_challenge,
    };
    println!("authorization code received");
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

    crate::server::persist_tokens_async(
        &opts.codex_home,
        None,
        tokens.id_token,
        tokens.access_token,
        tokens.refresh_token,
    )
    .await
}
