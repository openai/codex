use std::time::Duration;

use serde::Deserialize;

use crate::server::ServerOptions;

#[derive(Deserialize)]
struct UserCodeResp {
    #[serde(alias = "user_code", alias = "usercode")]
    user_code: String,
    #[serde(
        default,
        alias = "interval_secs",
        alias = "polling_interval",
        alias = "poll_interval"
    )]
    interval: Option<u64>,
    #[allow(dead_code)]
    #[serde(default, alias = "device_code")]
    device_code: Option<String>,
}

#[derive(Deserialize)]
struct TokenSuccessResp {
    id_token: String,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenErrorResp {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
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

    // Step 1: request a user code and polling interval
    let usercode_url = format!("{}/devicecode/usercode", opts.issuer.trim_end_matches('/'));
    let uc_resp = client
        .post(usercode_url)
        .header("Content-Type", "application/json")
        .body(format!("{{\"client_id\":\"{}\"}}", opts.client_id))
        .send()
        .await
        .map_err(std::io::Error::other)?;

    if !uc_resp.status().is_success() {
        return Err(std::io::Error::other(format!(
            "device code request failed with status {}",
            uc_resp.status()
        )));
    }
    let uc: UserCodeResp = uc_resp.json().await.map_err(std::io::Error::other)?;
    let interval = uc.interval.unwrap_or(5);

    eprintln!(
        "To authenticate, enter this code when prompted: {}",
        uc.user_code
    );

    // Step 2: poll the token endpoint until success or failure
    let token_url = format!("{}/deviceauth/token", opts.issuer.trim_end_matches('/'));
    loop {
        let resp = client
            .post(&token_url)
            .header("Content-Type", "application/json")
            .body(format!(
                "{{\"client_id\":\"{}\",\"user_code\":\"{}\"}}",
                opts.client_id, uc.user_code
            ))
            .send()
            .await
            .map_err(std::io::Error::other)?;

        if resp.status().is_success() {
            let tokens: TokenSuccessResp = resp.json().await.map_err(std::io::Error::other)?;

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
            let maybe_err: Result<TokenErrorResp, _> = resp.json().await;
            if let Ok(err) = maybe_err {
                if err.error == "token_pending" {
                    tokio::time::sleep(Duration::from_secs(interval)).await;
                    continue;
                }
                return Err(std::io::Error::other(match err.error_description {
                    Some(desc) => format!("device auth failed: {}: {}", err.error, desc),
                    None => format!("device auth failed: {}", err.error),
                }));
            } else {
                return Err(std::io::Error::other(format!(
                    "device auth failed with status {}",
                    status
                )));
            }
        }
    }
}
