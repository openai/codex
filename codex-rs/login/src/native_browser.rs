use std::path::Path;
use std::process::Stdio;

use crate::AuthDotJson;
use crate::CLIENT_ID;
use crate::LoginError;
use crate::get_auth_file;
use crate::pkce::generate_pkce;
use crate::token_data::TokenData;
use crate::token_data::parse_id_token;

#[cfg(target_os = "macos")]
pub async fn login_with_native_browser(codex_home: &Path) -> Result<(), LoginError> {
    // Build PKCE + state
    let pkce = generate_pkce();
    // Allow forcing the OAuth state via env (used in tests). Defaults to random.
    let state = std::env::var("CODEX_LOGIN_FORCE_STATE").unwrap_or_else(|_| generate_state());

    // Use the existing localhost callback URI, but we won't actually run a server.
    let redirect_uri = format!("http://localhost:{}/auth/callback", 1455u16);
    // Allow overriding the issuer base via env (used in tests). Defaults to prod issuer.
    let issuer =
        std::env::var("CODEX_LOGIN_ISSUER_BASE").unwrap_or_else(|_| DEFAULT_ISSUER.to_string());
    let auth_url = build_authorize_url(
        &issuer,
        CLIENT_ID,
        &redirect_uri,
        &pkce.code_challenge,
        &state,
    );

    // Compile Swift helper and run it to open a WKWebView and intercept the callback.
    // Allow bypassing the helper UI via an injected capture (used in tests).
    let capture = match std::env::var("CODEX_LOGIN_TEST_HELPER_JSON") {
        Ok(val) => {
            if val == "ABORT" {
                return Err(LoginError::Aborted);
            }
            serde_json::from_str::<AuthCodeCapture>(&val)
                .map_err(|_| LoginError::InvalidHelperResponse)?
        }
        Err(_) => compile_and_run_swift_helper(&auth_url, &state).await?,
    };
    if capture.state != state {
        return Err(LoginError::StateMismatch);
    }
    if capture.code.is_empty() {
        return Err(LoginError::InvalidHelperResponse);
    }

    // Exchange code for tokens
    let tokens = exchange_code_for_tokens(
        &issuer,
        CLIENT_ID,
        &redirect_uri,
        &pkce.code_verifier,
        &capture.code,
    )
    .await?;

    // Optionally obtain API key via token-exchange
    let api_key = obtain_api_key(&issuer, CLIENT_ID, &tokens.id_token)
        .await
        .ok();

    // Persist tokens
    persist_tokens(
        codex_home,
        api_key,
        &tokens.id_token,
        &tokens.access_token,
        &tokens.refresh_token,
    )
    .await
}

#[cfg(not(target_os = "macos"))]
pub async fn login_with_native_browser(_codex_home: &Path) -> Result<(), LoginError> {
    Err(LoginError::UnsupportedOs)
}

const DEFAULT_ISSUER: &str = "https://auth.openai.com";

#[cfg(target_os = "macos")]
fn build_authorize_url(
    issuer: &str,
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> String {
    let query = vec![
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("scope", "openid profile email offline_access"),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
    ];
    let qs = query
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{issuer}/oauth/authorize?{qs}")
}

#[cfg(target_os = "macos")]
fn generate_state() -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

// (first version removed; see embedded-first version below)

#[cfg(target_os = "macos")]
#[derive(serde::Deserialize)]
struct AuthCodeCapture {
    code: String,
    state: String,
}

#[cfg(target_os = "macos")]
struct ExchangedTokens {
    id_token: String,
    access_token: String,
    refresh_token: String,
}

#[cfg(target_os = "macos")]
async fn exchange_code_for_tokens(
    issuer: &str,
    client_id: &str,
    redirect_uri: &str,
    code_verifier: &str,
    code: &str,
) -> Result<ExchangedTokens, LoginError> {
    #[derive(serde::Deserialize)]
    struct TokenResponse {
        id_token: String,
        access_token: String,
        refresh_token: String,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{issuer}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
            urlencoding::encode(code),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(client_id),
            urlencoding::encode(code_verifier)
        ))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(LoginError::TokenExchangeFailed(resp.status().to_string()));
    }
    let tokens: TokenResponse = resp.json().await?;
    Ok(ExchangedTokens {
        id_token: tokens.id_token,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    })
}

#[cfg(target_os = "macos")]
async fn obtain_api_key(
    issuer: &str,
    client_id: &str,
    id_token: &str,
) -> Result<String, LoginError> {
    #[derive(serde::Deserialize)]
    struct ExchangeResp {
        access_token: String,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{issuer}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type={}&client_id={}&requested_token={}&subject_token={}&subject_token_type={}",
            urlencoding::encode("urn:ietf:params:oauth:grant-type:token-exchange"),
            urlencoding::encode(client_id),
            urlencoding::encode("openai-api-key"),
            urlencoding::encode(id_token),
            urlencoding::encode("urn:ietf:params:oauth:token-type:id_token")
        ))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(LoginError::TokenExchangeFailed(resp.status().to_string()));
    }
    let body: ExchangeResp = resp.json().await?;
    Ok(body.access_token)
}

#[cfg(target_os = "macos")]
async fn persist_tokens(
    codex_home: &Path,
    api_key: Option<String>,
    id_token: &str,
    access_token: &str,
    refresh_token: &str,
) -> Result<(), LoginError> {
    // Own strings for 'static spawn_blocking closure
    let id_token_owned = id_token.to_owned();
    let access_token_owned = access_token.to_owned();
    let refresh_token_owned = refresh_token.to_owned();
    let codex_home = codex_home.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        use chrono::Utc;
        use std::fs;
        let auth_file = get_auth_file(&codex_home);
        if let Some(parent) = auth_file.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }
        let mut auth = match crate::try_read_auth_json(&auth_file) {
            Ok(a) => a,
            Err(_) => AuthDotJson {
                openai_api_key: None,
                tokens: None,
                last_refresh: None,
            },
        };
        if let Some(key) = api_key {
            auth.openai_api_key = Some(key);
        }
        let tokens = auth.tokens.get_or_insert_with(TokenData::default);
        tokens.id_token = parse_id_token(&id_token_owned).map_err(std::io::Error::other)?;
        // Extract account ID if present
        if let Some(acc) = jwt_auth_claims(&id_token_owned)
            .get("chatgpt_account_id")
            .and_then(|v| v.as_str())
        {
            tokens.account_id = Some(acc.to_string());
        }
        tokens.access_token = access_token_owned;
        tokens.refresh_token = refresh_token_owned;
        auth.last_refresh = Some(Utc::now());

        // Write file (0600)
        let json_data = serde_json::to_string_pretty(&auth)?;
        let mut opts = std::fs::OpenOptions::new();
        opts.truncate(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&auth_file)?;
        use std::io::Write as _;
        file.write_all(json_data.as_bytes())?;
        file.flush()?;
        Ok::<(), std::io::Error>(())
    })
    .await
    .map_err(|e| std::io::Error::other(format!("persist task failed: {e}")))?;
    result.map_err(LoginError::from)
}

#[cfg(target_os = "macos")]
fn jwt_auth_claims(jwt: &str) -> serde_json::Map<String, serde_json::Value> {
    use base64::Engine;
    let mut parts = jwt.split('.');
    let (_h, payload_b64, _s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => {
            return serde_json::Map::new();
        }
    };
    if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64)
        && let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && let Some(obj) = v
            .get_mut("https://api.openai.com/auth")
            .and_then(|x| x.as_object_mut())
    {
        return obj.clone();
    }
    serde_json::Map::new()
}

#[cfg(target_os = "macos")]
const SWIFT_HELPER_SRC: &str = include_str!("native_browser_helper.swift");

// Prefer an embedded helper produced at build time (see build.rs). If the embedded
// bytes are empty or execution fails, we fall back to compiling the helper on demand.
#[cfg(target_os = "macos")]
static EMBEDDED_HELPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/codex-auth-helper"));

#[cfg(target_os = "macos")]
async fn compile_and_run_swift_helper(
    authorize_url: &str,
    state: &str,
) -> Result<AuthCodeCapture, LoginError> {
    // Testing seam: allow injecting a fake helper result or abort.
    #[cfg(any(test, debug_assertions))]
    if let Ok(val) = std::env::var("CODEX_LOGIN_TEST_HELPER_JSON") {
        if val == "ABORT" {
            return Err(LoginError::Aborted);
        }
        return serde_json::from_str::<AuthCodeCapture>(&val)
            .map_err(|_| LoginError::InvalidHelperResponse);
    }

    // If embedded helper is present (non-empty), try running it first unless disabled.
    let disable_embedded = std::env::var("CODEX_LOGIN_DISABLE_EMBEDDED_HELPER").is_ok();
    if !disable_embedded && !EMBEDDED_HELPER.is_empty() {
        use std::fs;
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = std::env::temp_dir().join(format!("codex-embedded-{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let helper_bin = temp_dir.join("codex-auth-helper");
        let mut f = fs::File::create(&helper_bin)?;
        f.write_all(EMBEDDED_HELPER)?;
        f.flush()?;
        let mut perms = fs::metadata(&helper_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&helper_bin, perms)?;

        let mut cmd = tokio::process::Command::new(&helper_bin);
        cmd.arg("--authorize-url")
            .arg(authorize_url)
            .arg("--state")
            .arg(state);
        cmd.env("NSUnbufferedIO", "YES");
        cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
        match cmd.output().await {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Ok(capture) = serde_json::from_str::<AuthCodeCapture>(stdout.trim()) {
                        return Ok(capture);
                    }
                    // Ran successfully but output malformed → treat as error, do NOT reopen.
                    return Err(LoginError::InvalidHelperResponse);
                } else {
                    // Helper exited non‑zero (likely user closed the window). Do NOT reopen.
                    return Err(LoginError::Aborted);
                }
            }
            Err(_) => { /* fall through to on-demand compile */ }
        }
    }

    // Fallback: compile Swift source now and run it
    compile_and_run_swift_helper_fallback(authorize_url, state).await
}

#[cfg(target_os = "macos")]
async fn compile_and_run_swift_helper_fallback(
    authorize_url: &str,
    state: &str,
) -> Result<AuthCodeCapture, LoginError> {
    use rand::RngCore;
    use std::fs;
    use std::io::Write;

    // Create a temp dir and paths
    let mut rand_bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut rand_bytes);
    let temp_dir =
        std::env::temp_dir().join(format!("codex-auth-{:x}", u64::from_le_bytes(rand_bytes)));
    fs::create_dir_all(&temp_dir)?;
    let swift_src = temp_dir.join("CodexAuthHelper.swift");
    let helper_bin = temp_dir.join("codex-auth-helper");

    // Write Swift source
    let mut f = fs::File::create(&swift_src)?;
    f.write_all(SWIFT_HELPER_SRC.as_bytes())?;
    f.flush()?;

    // Try to compile via swiftc (or xcrun swiftc)
    let compile_cmds: Vec<Vec<String>> = vec![
        vec![
            "swiftc".into(),
            "-O".into(),
            "-framework".into(),
            "Cocoa".into(),
            "-framework".into(),
            "WebKit".into(),
            swift_src.to_string_lossy().into(),
            "-o".into(),
            helper_bin.to_string_lossy().into(),
        ],
        vec![
            "/usr/bin/swiftc".into(),
            "-O".into(),
            "-framework".into(),
            "Cocoa".into(),
            "-framework".into(),
            "WebKit".into(),
            swift_src.to_string_lossy().into(),
            "-o".into(),
            helper_bin.to_string_lossy().into(),
        ],
        vec![
            "xcrun".into(),
            "swiftc".into(),
            "-O".into(),
            "-framework".into(),
            "Cocoa".into(),
            "-framework".into(),
            "WebKit".into(),
            swift_src.to_string_lossy().into(),
            "-o".into(),
            helper_bin.to_string_lossy().into(),
        ],
    ];

    let mut compiled = false;
    for argv in compile_cmds {
        let mut cmd = tokio::process::Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        match cmd
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .await
        {
            Ok(status) if status.success() => {
                compiled = true;
                break;
            }
            Ok(_) | Err(_) => { /* try next */ }
        }
    }
    if !compiled {
        return Err(LoginError::HelperCompileFailed(
            "swiftc not found or compile failed".to_string(),
        ));
    }

    // Run helper
    let mut cmd = tokio::process::Command::new(&helper_bin);
    cmd.arg("--authorize-url")
        .arg(authorize_url)
        .arg("--state")
        .arg(state);
    cmd.env("NSUnbufferedIO", "YES");
    cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
    let output = cmd.output().await?;
    if !output.status.success() {
        return Err(LoginError::Aborted);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let capture: AuthCodeCapture =
        serde_json::from_str(stdout.trim()).map_err(|_| LoginError::InvalidHelperResponse)?;
    Ok(capture)
}

#[cfg(all(test, target_os = "macos"))]
mod tests_macos {
    use super::*;
    use base64::Engine;

    #[test]
    fn authorize_url_contains_required_params() {
        let url = build_authorize_url(
            "https://issuer.example",
            "client123",
            "http://localhost:1455/auth/callback",
            "challengeXYZ",
            "stateABC",
        );
        assert!(url.starts_with("https://issuer.example/oauth/authorize?"));
        for needle in [
            "response_type=code",
            "client_id=client123",
            "redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback",
            "scope=openid%20profile%20email%20offline_access",
            "code_challenge=challengeXYZ",
            "code_challenge_method=S256",
            "id_token_add_organizations=true",
            "codex_cli_simplified_flow=true",
            "state=stateABC",
        ] {
            assert!(url.contains(needle), "missing query param: {needle}");
        }
    }

    #[test]
    fn jwt_auth_claims_extracts_nested_fields() {
        #[derive(serde::Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }
        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_plan_type": "pro",
                "chatgpt_account_id": "acc-42"
            }
        });
        let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        let jwt = format!(
            "{}.{}.{}",
            b64(&serde_json::to_vec(&header).unwrap()),
            b64(&serde_json::to_vec(&payload).unwrap()),
            b64(b"sig")
        );
        let claims = jwt_auth_claims(&jwt);
        assert_eq!(
            claims.get("chatgpt_plan_type").and_then(|v| v.as_str()),
            Some("pro")
        );
        assert_eq!(
            claims.get("chatgpt_account_id").and_then(|v| v.as_str()),
            Some("acc-42")
        );
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests_non_macos {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn native_browser_is_unsupported() {
        let dir = tempdir().unwrap();
        let res = login_with_native_browser(dir.path()).await;
        match res {
            Err(LoginError::UnsupportedOs) => {}
            other => panic!("unexpected result: {:?}", other),
        }
    }
}
