use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use serde::de::Deserializer;
use serde::de::{self};

use crate::server::ServerOptions;

#[derive(Deserialize)]
struct UserCodeResp {
    #[serde(alias = "user_code", alias = "usercode")]
    user_code: String,
    #[serde(default, deserialize_with = "deserialize_interval")]
    interval: u64,
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
    #[serde(alias = "device_code")]
    code: String,
}

#[derive(Deserialize)]
struct TokenSuccessResp {
    id_token: String,
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
}

/// Run a device code login flow using the configured issuer and client id.
///
/// Flow:
/// - Request a user code and polling interval from `{issuer}/devicecode/usercode`.
/// - Display the user code to the terminal.
/// - Poll `{issuer}/deviceauth/token` at the provided interval until a token is issued.
///   - If the response indicates `token_pending`, continue polling.
///   - Any other error aborts the flow.
/// - On success, persist tokens and attempt an API key exchange for convenience.
pub async fn run_device_code_login(opts: ServerOptions) -> std::io::Result<()> {
    let client = reqwest::Client::new();
    let auth_base_url = opts.issuer.trim_end_matches('/').to_owned();

    // Step 1: request a user code and polling interval
    let usercode_url = format!("{auth_base_url}/deviceauth/usercode");
    let payload: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let body = serde_json::Value::Object(payload).to_string();

    let uc_resp = client
        .post(usercode_url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(std::io::Error::other)?;

    let status = uc_resp.status();
    let body_text = uc_resp.text().await.map_err(std::io::Error::other)?;

    if !status.is_success() {
        return Err(std::io::Error::other(format!(
            "device code request failed with status {status}"
        )));
    }
    let uc: UserCodeResp = serde_json::from_str(&body_text).map_err(std::io::Error::other)?;
    let interval: u64 = uc.interval;

    eprintln!(
        "To authenticate, enter this code when prompted: {} with interval {}",
        uc.user_code, uc.interval
    );

    // Step 2: poll the token endpoint until success or failure
    // Cap the polling duration to 15 minutes.
    let max_wait = Duration::from_secs(15 * 60);
    let start = std::time::Instant::now();

    let token_url = format!("{auth_base_url}/deviceauth/token");
    loop {
        let resp = client
            .post(&token_url)
            .header("Content-Type", "application/json")
            .body({
                let client_id = &opts.client_id;
                let user_code: &String = &uc.user_code;
                format!("{{\"client_id\":\"{client_id}\",\"user_code\":\"{user_code}\"}}")
            })
            .send()
            .await
            .map_err(std::io::Error::other)?;

        if resp.status().is_success() {
            let code_resp: CodeSuccessResp = resp.json().await.map_err(std::io::Error::other)?;
            let tokens = exchange_device_code_for_tokens(
                &client,
                &opts.issuer,
                &opts.client_id,
                &code_resp.code,
            )
            .await?;

            // Try to exchange for an API key (optional best-effort)
            let api_key =
                crate::server::obtain_api_key(&opts.issuer, &opts.client_id, &tokens.id_token)
                    .await
                    .ok();

            crate::server::persist_tokens_async(
                &opts.codex_home,
                api_key,
                tokens.id_token,
                tokens.access_token,
                tokens.refresh_token,
            )
            .await?;

            return Ok(());
        } else {
            // Try to parse an error payload; if it's token_pending, sleep and retry
            let status = resp.status();
            if status == StatusCode::NOT_FOUND {
                let elapsed = start.elapsed();
                if elapsed >= max_wait {
                    return Err(std::io::Error::other(
                        "device auth timed out after 15 minutes",
                    ));
                }
                let remaining = max_wait - elapsed;
                let sleep_for = Duration::from_secs(interval).min(remaining);
                tokio::time::sleep(sleep_for).await;
                continue;
            } else {
                return Err(std::io::Error::other(format!(
                    "device auth failed with status {status}"
                )));
            }
        }
    }
}

async fn exchange_device_code_for_tokens(
    client: &reqwest::Client,
    issuer: &str,
    client_id: &str,
    code: &str,
) -> std::io::Result<TokenSuccessResp> {
    let issuer_trimmed = issuer.trim_end_matches('/');
    let resp = client
        .post(format!("{issuer_trimmed}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type={}&device_code={}&client_id={}",
            urlencoding::encode("urn:ietf:params:oauth:grant-type:device_code"),
            urlencoding::encode(code),
            urlencoding::encode(client_id)
        ))
        .send()
        .await
        .map_err(std::io::Error::other)?;

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(std::io::Error::other(format!(
            "device code exchange failed with status {status}: {body_text}"
        )));
    }

    resp.json().await.map_err(std::io::Error::other)
}
