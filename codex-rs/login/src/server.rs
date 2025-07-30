use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::DateTime;
use chrono::Utc;
use http_body_util::Full as BodyFull;
use hyper::Method;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use rand::RngCore;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::CLIENT_ID;
use crate::TokenData;

const REQUIRED_PORT: u16 = 1455;
const URL_BASE: &str = "http://localhost:1455";
const DEFAULT_ISSUER: &str = "https://auth.openai.com";

#[derive(Clone)]
struct PkceCodes {
    code_verifier: String,
    code_challenge: String,
}

impl PkceCodes {
    fn generate() -> Self {
        let code_verifier = random_hex(64);
        let digest = Sha256::digest(code_verifier.as_bytes());
        let code_challenge = URL_SAFE_NO_PAD.encode(digest);
        Self {
            code_verifier,
            code_challenge,
        }
    }
}

#[derive(Clone)]
struct ServerState {
    codex_home: PathBuf,
    issuer: String,
    client_id: String,
    redirect_uri: String,
    pkce: PkceCodes,
    state: String,
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl ServerState {
    fn auth_url(&self) -> String {
        let params: Vec<(String, String)> = vec![
            ("response_type".into(), "code".into()),
            ("client_id".into(), self.client_id.clone()),
            ("redirect_uri".into(), self.redirect_uri.clone()),
            ("scope".into(), "openid profile email offline_access".into()),
            ("code_challenge".into(), self.pkce.code_challenge.clone()),
            ("code_challenge_method".into(), "S256".into()),
            ("id_token_add_organizations".into(), "true".into()),
            ("state".into(), self.state.clone()),
        ];
        let query = serde_urlencode(&params);
        format!("{}/oauth/authorize?{}", self.issuer, query)
    }
}

fn serde_urlencode(params: &[(String, String)]) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in params.iter() {
        s.append_pair(k, v);
    }
    s.finish()
}

fn random_hex(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

// Public entry point used by lib.rs
pub async fn run_login_server(codex_home: &Path) -> std::io::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], REQUIRED_PORT));
    let listener = TcpListener::bind(addr).await?;

    let pkce = PkceCodes::generate();
    let state = random_hex(32);
    let redirect_uri = format!("{URL_BASE}/auth/callback");

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server_state = ServerState {
        codex_home: codex_home.to_path_buf(),
        issuer: DEFAULT_ISSUER.to_string(),
        client_id: CLIENT_ID.to_string(),
        redirect_uri,
        pkce,
        state,
        shutdown_tx: Arc::new(Mutex::new(Some(shutdown_tx))),
    };

    let auth_url = server_state.auth_url();

    // Try to open a browser, but don't fail if we can't.
    if let Err(err) = open::that_detached(&auth_url) {
        eprintln!("Failed to open browser: {err}");
    }

    eprintln!("If your browser did not open, navigate to this URL to authenticate:\n\n{auth_url}");

    let state_arc = Arc::new(server_state);

    let accept_task = tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Accept error: {e}");
                    continue;
                }
            };
            let io = TokioIo::new(stream);
            let state_inner = state_arc.clone();
            tokio::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(|req| handle_request(req, state_inner.clone())),
                    )
                    .await
                {
                    eprintln!("server connection error: {err}");
                }
            });
        }
    });

    // Wait for shutdown signal
    let _ = shutdown_rx.await;
    accept_task.abort();
    let _ = accept_task.await;
    Ok(())
}

#[allow(clippy::unwrap_used)]
async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: Arc<ServerState>,
) -> Result<Response<BodyFull<Bytes>>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let resp = match (method, path.as_str()) {
        (Method::GET, "/success") => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(BodyFull::from(Bytes::from(LOGIN_SUCCESS_HTML)))
            .unwrap(),
        (Method::GET, "/auth/callback") => match handle_auth_callback(req, state.clone()).await {
            Ok(resp) => resp,
            Err((status, msg)) => {
                let builder = Response::builder().status(status);
                if let Ok(resp) = builder.body(BodyFull::from(Bytes::from(msg.clone()))) {
                    // On error, shut down the server after responding
                    send_shutdown(&state);
                    resp
                } else {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(BodyFull::from(Bytes::from_static(b"Internal Server Error")))
                        .unwrap()
                }
            }
        },
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(BodyFull::from(Bytes::from_static(b"Not Found")))
            .unwrap(),
    };

    Ok(resp)
}

fn send_shutdown(state: &ServerState) {
    if let Ok(mut guard) = state.shutdown_tx.lock() {
        if let Some(tx) = guard.take() {
            let _ = tx.send(());
        }
    }
}

fn decode_jwt_segment(segment: &str) -> serde_json::Value {
    let data = URL_SAFE_NO_PAD
        .decode(segment)
        .map_err(|_| ())
        .and_then(|bytes| String::from_utf8(bytes).map_err(|_| ()))
        .ok();
    if let Some(s) = data {
        serde_json::from_str::<serde_json::Value>(&s).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    }
}

#[allow(clippy::unwrap_used)]
async fn handle_auth_callback(
    req: Request<hyper::body::Incoming>,
    state: Arc<ServerState>,
) -> Result<Response<BodyFull<Bytes>>, (StatusCode, String)> {
    let query_str = req.uri().query().unwrap_or("");
    let query: HashMap<String, String> = url::form_urlencoded::parse(query_str.as_bytes())
        .into_owned()
        .collect();

    // Validate state
    if query.get("state").map(String::as_str) != Some(&state.state) {
        return Err((StatusCode::BAD_REQUEST, "State parameter mismatch".into()));
    }

    let code = match query.get("code") {
        Some(c) if !c.is_empty() => c.clone(),
        _ => return Err((StatusCode::BAD_REQUEST, "Missing authorization code".into())),
    };

    // 1. Authorization-code -> (id_token, access_token, refresh_token)
    let token_endpoint = format!("{}/oauth/token", state.issuer);
    let client = reqwest::Client::new();
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", state.redirect_uri.as_str()),
        ("client_id", state.client_id.as_str()),
        ("code_verifier", state.pkce.code_verifier.as_str()),
    ];
    let resp = client
        .post(&token_endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Token request failed: {e}"),
            )
        })?;
    if !resp.status().is_success() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Token request failed: {}", resp.status()),
        ));
    }
    let payload: serde_json::Value = resp.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Invalid token response: {e}"),
        )
    })?;

    let id_token = payload
        .get("id_token")
        .and_then(|v| v.as_str())
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Missing id_token".into()))?
        .to_string();
    let access_token = payload
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Missing access_token".into(),
        ))?
        .to_string();
    let refresh_token = payload
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Missing refresh_token".into(),
        ))?
        .to_string();

    // Extract chatgpt_account_id from id_token claims
    let id_token_parts: Vec<&str> = id_token.split('.').collect();
    if id_token_parts.len() != 3 {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid ID token".into()));
    }
    let id_token_claims = decode_jwt_segment(id_token_parts[1]);
    let auth_claims = id_token_claims
        .get("https://api.openai.com/auth")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let account_id = auth_claims
        .get("chatgpt_account_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let token_data = TokenData {
        id_token: id_token.clone(),
        access_token: access_token.clone(),
        refresh_token: refresh_token.clone(),
        account_id,
    };

    // Parse access_token claims
    let access_token_parts: Vec<&str> = access_token.split('.').collect();
    if access_token_parts.len() != 3 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Invalid access token".into(),
        ));
    }
    let access_token_claims = decode_jwt_segment(access_token_parts[1]);

    let token_claims = id_token_claims
        .get("https://api.openai.com/auth")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let access_claims = access_token_claims
        .get("https://api.openai.com/auth")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let org_id = token_claims
        .get("organization_id")
        .and_then(|v| v.as_str())
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Missing organization in id_token claims".into(),
        ))?
        .to_string();
    let project_id = token_claims
        .get("project_id")
        .and_then(|v| v.as_str())
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Missing project in id_token claims".into(),
        ))?
        .to_string();

    // 2. Token exchange to obtain API key
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let rand_id = random_hex(12);
    let exchange_name = format!("Codex CLI [auto-generated] ({today}) [{rand_id}]");
    let exchange_form: Vec<(String, String)> = vec![
        (
            "grant_type".into(),
            "urn:ietf:params:oauth:grant-type:token-exchange".into(),
        ),
        ("client_id".into(), state.client_id.clone()),
        ("requested_token".into(), "openai-api-key".into()),
        ("subject_token".into(), id_token.clone()),
        (
            "subject_token_type".into(),
            "urn:ietf:params:oauth:token-type:id_token".into(),
        ),
        ("name".into(), exchange_name),
    ];
    let exchange_resp = reqwest::Client::new()
        .post(&token_endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&exchange_form)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Exchange request failed: {e}"),
            )
        })?;
    if !exchange_resp.status().is_success() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Exchange request failed: {}", exchange_resp.status()),
        ));
    }
    let exchange_payload: serde_json::Value = exchange_resp.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Invalid exchange response: {e}"),
        )
    })?;
    let exchanged_access_token = exchange_payload
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Missing access_token in exchange".into(),
        ))?
        .to_string();

    let completed_onboarding = token_claims
        .get("completed_platform_onboarding")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let is_org_owner = token_claims
        .get("is_org_owner")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let needs_setup = !completed_onboarding && is_org_owner;
    let chatgpt_plan_type = access_claims
        .get("chatgpt_plan_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let platform_url = if state.issuer == "https://auth.openai.com" {
        "https://platform.openai.com"
    } else {
        "https://platform.api.openai.org"
    };

    let success_params: Vec<(String, String)> = vec![
        ("id_token".into(), id_token.clone()),
        (
            "needs_setup".into(),
            if needs_setup { "true" } else { "false" }.into(),
        ),
        ("org_id".into(), org_id.clone()),
        ("project_id".into(), project_id.clone()),
        ("plan_type".into(), chatgpt_plan_type.clone()),
        ("platform_url".into(), platform_url.into()),
    ];
    let success_url = format!("{}/success?{}", URL_BASE, serde_urlencode(&success_params));

    // Best-effort credit redemption; errors are logged but do not interrupt flow.
    if let Err(err) = maybe_redeem_credits(
        &state.issuer,
        &state.client_id,
        Some(&id_token),
        &refresh_token,
        &state.codex_home,
        &access_claims,
        &token_claims,
    )
    .await
    {
        eprintln!("Unable to redeem ChatGPT subscriber API credits: {err}");
    }

    // Persist auth.json
    let last_refresh: DateTime<Utc> = Utc::now();
    let auth_json_value = json!({
        "OPENAI_API_KEY": exchanged_access_token,
        "tokens": {
            "id_token": token_data.id_token,
            "access_token": token_data.access_token,
            "refresh_token": token_data.refresh_token,
            "account_id": token_data.account_id,
        },
        "last_refresh": last_refresh.to_rfc3339().replace("+00:00", "Z"),
    });
    if let Err(err) = write_auth_file(&state.codex_home, auth_json_value).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unable to persist auth file: {err}"),
        ));
    }

    // Redirect to success URL
    let resp = Response::builder()
        .status(StatusCode::FOUND)
        .header("Location", success_url)
        .body(BodyFull::from(Bytes::new()))
        .unwrap();

    // Signal shutdown afterwards
    send_shutdown(&state);

    Ok(resp)
}

async fn write_auth_file(codex_home: &Path, contents: serde_json::Value) -> std::io::Result<()> {
    if !codex_home.is_dir() {
        std::fs::create_dir_all(codex_home)?;
    }
    let auth_path = codex_home.join("auth.json");
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&auth_path)?;
    let data = serde_json::to_vec_pretty(&contents)?;
    use std::io::Write as _;
    file.write_all(&data)?;
    file.flush()?;
    Ok(())
}

async fn maybe_redeem_credits(
    issuer: &str,
    client_id: &str,
    id_token_opt: Option<&str>,
    refresh_token: &str,
    codex_home: &Path,
    access_claims: &serde_json::Value,
    token_claims: &serde_json::Value,
) -> Result<(), String> {
    let mut id_token = id_token_opt.unwrap_or("").to_string();
    let mut id_claims = parse_id_token_claims(&id_token);

    let token_expired = match id_claims
        .as_ref()
        .and_then(|c| c.get("exp").and_then(|v| v.as_i64()))
    {
        Some(exp) => Utc::now().timestamp_millis() >= exp * 1000,
        None => true,
    };

    if token_expired {
        eprintln!("Refreshing credentials...");
        let payload = json!({
            "client_id": client_id,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "scope": "openid profile email",
        });
        let resp = reqwest::Client::new()
            .post("https://auth.openai.com/oauth/token")
            .header("Content-Type", "application/json")
            .body(payload.to_string())
            .send()
            .await
            .map_err(|e| format!("Unable to refresh ID token via token-exchange: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "Unable to refresh ID token via token-exchange: {}",
                resp.status()
            ));
        }
        let refresh_data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Invalid refresh response: {e}"))?;
        let new_id_token = refresh_data
            .get("id_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let new_refresh_token = refresh_data
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !new_id_token.is_empty() && !new_refresh_token.is_empty() {
            id_token = new_id_token.clone();
            id_claims = parse_id_token_claims(&new_id_token);
            // Update auth.json tokens
            let auth_path = codex_home.join("auth.json");
            if let Ok(mut file) = std::fs::File::open(&auth_path) {
                let mut s = String::new();
                use std::io::Read as _;
                let _ = file.read_to_string(&mut s);
                if let Ok(mut existing) = serde_json::from_str::<serde_json::Value>(&s) {
                    if existing.get("tokens").and_then(|t| t.as_object()).is_none() {
                        existing["tokens"] = json!({});
                    }
                    let tokens = existing["tokens"]
                        .as_object_mut()
                        .ok_or_else(|| format!("Invalid auth.json: {s}"))?;
                    tokens.insert("id_token".into(), json!(new_id_token));
                    tokens.insert("refresh_token".into(), json!(new_refresh_token));
                    existing["last_refresh"] =
                        json!(Utc::now().to_rfc3339().replace("+00:00", "Z"));
                    // write back
                    let _ = write_auth_file(codex_home, existing).await;
                }
            }
        } else {
            // Couldn't refresh; proceed without redeeming
            return Ok(());
        }
    }

    if id_token.is_empty() {
        eprintln!("No ID token available, cannot redeem credits.");
        return Ok(());
    }

    let auth_claims = id_claims
        .as_ref()
        .and_then(|v| v.get("https://api.openai.com/auth"))
        .cloned()
        .unwrap_or_else(|| json!({}));

    // Subscription eligibility check (Plus or Pro, >7 days active)
    if let Some(sub_start_str) = auth_claims
        .get("chatgpt_subscription_active_start")
        .and_then(|v| v.as_str())
    {
        if let Ok(sub_start_ts) = chrono::DateTime::parse_from_rfc3339(sub_start_str) {
            if Utc::now() - sub_start_ts.with_timezone(&Utc) < chrono::Duration::days(7) {
                eprintln!(
                    "Sorry, your subscription must be active for more than 7 days to redeem credits."
                );
                return Ok(());
            }
        }
    }

    let completed_onboarding = token_claims
        .get("completed_platform_onboarding")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let is_org_owner = token_claims
        .get("is_org_owner")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let needs_setup = !completed_onboarding && is_org_owner;
    let plan_type = access_claims
        .get("chatgpt_plan_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if needs_setup || (plan_type != "plus" && plan_type != "pro") {
        eprintln!("Only users with Plus or Pro subscriptions can redeem free API credits.");
        return Ok(());
    }

    let api_host = if issuer == "https://auth.openai.com" {
        "https://api.openai.com"
    } else {
        "https://api.openai.org"
    };

    let redeem_payload = json!({"id_token": id_token});
    let resp = reqwest::Client::new()
        .post(format!("{api_host}/v1/billing/redeem_credits"))
        .header("Content-Type", "application/json")
        .body(redeem_payload.to_string())
        .send()
        .await
        .map_err(|e| format!("Credit redemption request failed: {e}"))?;

    let redeem_data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid redeem response: {e}"))?;
    let granted = redeem_data
        .get("granted_chatgpt_subscriber_api_credits")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if granted > 0 {
        eprintln!(
            "Thanks for being a ChatGPT {} subscriber!\nIf you haven't already redeemed, you should receive {} in API credits.\n\nCredits: https://platform.openai.com/settings/organization/billing/credit-grants\nMore info: https://help.openai.com/en/articles/11381614",
            if plan_type == "plus" { "Plus" } else { "Pro" },
            if plan_type == "plus" { "$5" } else { "$50" }
        );
    } else {
        eprintln!(
            "It looks like no credits were granted:\n\n{}\n\nCredits: https://platform.openai.com/settings/organization/billing/credit-grants\nMore info: https://help.openai.com/en/articles/11381614",
            serde_json::to_string_pretty(&redeem_data).unwrap_or_default()
        );
    }

    Ok(())
}

fn parse_id_token_claims(id_token: &str) -> Option<serde_json::Value> {
    if !id_token.is_empty() {
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() == 3 {
            return Some(decode_jwt_segment(parts[1]));
        }
    }
    None
}

const LOGIN_SUCCESS_HTML: &str = include_str!("static/success.html");
