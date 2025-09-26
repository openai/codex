use reqwest::StatusCode;
use serde::Deserialize;
use serde::de::Deserializer;
use serde::de::{self};
use std::time::Duration;
use std::time::Instant;

use crate::pkce::PkceCodes;
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

/// Request the user code and polling interval.
async fn request_user_code(
    client: &reqwest::Client,
    auth_base_url: &str,
) -> std::io::Result<UserCodeResp> {
    let url = format!("{auth_base_url}/deviceauth/usercode");
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .body("{}")
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
    client_id: &str,
    user_code: &str,
    interval: u64,
) -> std::io::Result<CodeSuccessResp> {
    let url = format!("{auth_base_url}/deviceauth/token");
    let max_wait = Duration::from_secs(15 * 60);
    let start = Instant::now();

    loop {
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(format!(
                "{{\"client_id\":\"{client_id}\",\"user_code\":\"{user_code}\"}}"
            ))
            .send()
            .await
            .map_err(std::io::Error::other)?;

        if resp.status().is_success() {
            return resp.json().await.map_err(std::io::Error::other);
        }

        if resp.status() == StatusCode::NOT_FOUND {
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

/// Full device code login flow.
pub async fn run_device_code_login(opts: ServerOptions) -> std::io::Result<()> {
    let client = reqwest::Client::new();
    let auth_base_url = opts.issuer.trim_end_matches('/').to_owned();

    let uc = request_user_code(&client, &auth_base_url).await?;
    println!(
        "To authenticate, visit: {}/deviceauth/authorize and enter code: {}",
        opts.issuer.trim_end_matches('/'),
        uc.user_code
    );
    // eprintln!(
    //     "To authenticate, enter this code when prompted: {} (interval {}s)",
    //     uc.user_code, uc.interval
    // );

    let code_resp = poll_for_token(
        &client,
        &auth_base_url,
        &opts.client_id,
        &uc.user_code,
        uc.interval,
    )
    .await?;

    let empty_pkce = PkceCodes {
        code_verifier: String::new(),
        code_challenge: String::new(),
    };

    let tokens = crate::server::exchange_code_for_tokens(
        &opts.issuer,
        &opts.client_id,
        "",
        &empty_pkce,
        &code_resp.code,
    )
    .await
    .map_err(|err| std::io::Error::other(format!("device code exchange failed: {err}")))?;

    // Try to exchange for an API key (optional)
    let api_key = crate::server::obtain_api_key(&opts.issuer, &opts.client_id, &tokens.id_token)
        .await
        .ok();

    crate::server::persist_tokens_async(
        &opts.codex_home,
        api_key,
        tokens.id_token,
        tokens.access_token,
        tokens.refresh_token,
    )
    .await
}
