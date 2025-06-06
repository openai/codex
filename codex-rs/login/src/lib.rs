use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

const SOURCE_FOR_PYTHON_SERVER: &str = include_str!("./login_with_chatgpt.py");

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// Run `python3 -c {{SOURCE_FOR_PYTHON_SERVER}}` with the CODEX_HOME
/// environment variable set to the provided `codex_home` path. If the
/// subprocess exits 0, read the OPENAI_API_KEY property out of
/// CODEX_HOME/auth.json and return Ok(OPENAI_API_KEY). Otherwise, return Err
/// with any information from the subprocess.
///
/// If `capture_output` is true, the subprocess's output will be captured and
/// recorded in memory. Otherwise, the subprocess's output will be sent to the
/// current process's stdout/stderr.
pub async fn login_with_chatgpt(
    codex_home: &Path,
    capture_output: bool,
) -> std::io::Result<String> {
    let child = Command::new("python3")
        .arg("-c")
        .arg(SOURCE_FOR_PYTHON_SERVER)
        .env("CODEX_HOME", codex_home)
        .stdin(Stdio::null())
        .stdout(if capture_output {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .stderr(if capture_output {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .spawn()?;

    let output = child.wait_with_output().await?;
    if output.status.success() {
        try_read_openai_api_key(codex_home).await
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(std::io::Error::other(format!(
            "login_with_chatgpt subprocess failed: {stderr}"
        )))
    }
}

/// Attempt to read the `OPENAI_API_KEY` from the `auth.json` file in the given
/// `CODEX_HOME` directory, refreshing it, if necessary.
pub async fn try_read_openai_api_key(codex_home: &Path) -> std::io::Result<String> {
    let auth_path = codex_home.join("auth.json");
    let mut file = std::fs::File::open(&auth_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let auth_dot_json: AuthDotJson = serde_json::from_str(&contents)?;

    if is_expired(&auth_dot_json) {
        let refresh_response = try_refresh_token(&auth_dot_json).await?;
        let mut auth_dot_json = auth_dot_json;
        auth_dot_json.tokens.id_token = refresh_response.id_token;
        if let Some(refresh_token) = refresh_response.refresh_token {
            auth_dot_json.tokens.refresh_token = refresh_token;
        }
        auth_dot_json.last_refresh = Utc::now();

        let mut options = OpenOptions::new();
        options.truncate(true).write(true).create(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
        }

        let json_data = serde_json::to_string(&auth_dot_json)?;
        {
            let mut file = options.open(&auth_path)?;
            file.write_all(json_data.as_bytes())?;
            file.flush()?;
        }

        Ok(auth_dot_json.openai_api_key)
    } else {
        Ok(auth_dot_json.openai_api_key)
    }
}

fn is_expired(auth_dot_json: &AuthDotJson) -> bool {
    let last_refresh = auth_dot_json.last_refresh;
    last_refresh < Utc::now() - chrono::Duration::days(28)
}

async fn try_refresh_token(auth_dot_json: &AuthDotJson) -> std::io::Result<RefreshResponse> {
    let refresh_request = RefreshRequest {
        client_id: CLIENT_ID,
        grant_type: "refresh_token",
        refresh_token: auth_dot_json.tokens.refresh_token.clone(),
        scope: "openid profile email",
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://auth.openai.com/oauth/token")
        .header("Content-Type", "application/json")
        .json(&refresh_request)
        .send()
        .await
        .map_err(std::io::Error::other)?;

    if response.status().is_success() {
        let refresh_response = response
            .json::<RefreshResponse>()
            .await
            .map_err(std::io::Error::other)?;
        Ok(refresh_response)
    } else {
        Err(std::io::Error::other(format!(
            "Failed to refresh token: {}",
            response.status()
        )))
    }
}

#[derive(Serialize)]
struct RefreshRequest {
    client_id: &'static str,
    grant_type: &'static str,
    refresh_token: String,
    scope: &'static str,
}

#[derive(Deserialize)]
struct RefreshResponse {
    id_token: String,
    refresh_token: Option<String>,
}

/// Expected structure for $CODEX_HOME/auth.json.
#[derive(Deserialize, Serialize)]
struct AuthDotJson {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: String,

    tokens: TokenData,

    last_refresh: DateTime<Utc>,
}

#[derive(Deserialize, Serialize)]
struct TokenData {
    /// This is a JWT.
    id_token: String,

    /// This is a JWT.
    #[allow(dead_code)]
    access_token: String,

    refresh_token: String,
}
