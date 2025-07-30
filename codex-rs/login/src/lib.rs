use chrono::DateTime;

use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::process::Command;

const SOURCE_FOR_PYTHON_SERVER: &str = include_str!("./login_with_chatgpt.py");

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";

#[derive(Clone, Debug, PartialEq)]
pub enum AuthMode {
    ApiKey,
    ChatGPT,
}

#[derive(Debug, Clone)]
pub struct CodexAuth {
    pub api_key: Option<String>,
    pub mode: AuthMode,
    auth_dot_json: Arc<Mutex<Option<AuthDotJson>>>,
    auth_file: PathBuf,
}

impl PartialEq for CodexAuth {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
    }
}

impl CodexAuth {
    pub fn new(
        api_key: Option<String>,
        mode: AuthMode,
        auth_file: PathBuf,
        auth_dot_json: Option<AuthDotJson>,
    ) -> Self {
        let auth_dot_json = Arc::new(Mutex::new(auth_dot_json));
        Self {
            api_key,
            mode,
            auth_file,
            auth_dot_json,
        }
    }

    pub fn from_api_key(api_key: String) -> Self {
        Self {
            api_key: Some(api_key),
            mode: AuthMode::ApiKey,
            auth_file: PathBuf::new(),
            auth_dot_json: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_token_data(&self) -> Result<TokenData, std::io::Error> {
        #[expect(clippy::unwrap_used)]
        let auth_dot_json = self.auth_dot_json.lock().unwrap().clone();

        match auth_dot_json {
            Some(auth_dot_json) => {
                if auth_dot_json.last_refresh < Utc::now() - chrono::Duration::days(28) {
                    let refresh_response = tokio::time::timeout(
                        Duration::from_secs(60),
                        try_refresh_token(auth_dot_json.tokens.refresh_token.clone()),
                    )
                    .await
                    .map_err(|_| {
                        std::io::Error::other("timed out while refreshing OpenAI API key")
                    })?
                    .map_err(std::io::Error::other)?;

                    let updated_auth_dot_json = update_tokens(
                        &self.auth_file,
                        refresh_response.id_token,
                        refresh_response.access_token,
                        refresh_response.refresh_token,
                    )
                    .await?;

                    #[expect(clippy::unwrap_used)]
                    let mut auth_dot_json = self.auth_dot_json.lock().unwrap();
                    *auth_dot_json = Some(updated_auth_dot_json);
                }
                Ok(auth_dot_json.tokens.clone())
            }
            None => Err(std::io::Error::other("Token data is not available.")),
        }
    }

    pub async fn get_token(&self) -> Result<String, std::io::Error> {
        match self.mode {
            AuthMode::ApiKey => Ok(self.api_key.clone().unwrap_or_default()),
            AuthMode::ChatGPT => {
                let id_token = self.get_token_data().await?.access_token;

                Ok(id_token)
            }
        }
    }
}

// Loads the available auth information from the auth.json or OPENAI_API_KEY environment variable.
pub fn load_auth(codex_home: &Path) -> std::io::Result<Option<CodexAuth>> {
    let auth_file = codex_home.join("auth.json");

    let auth_dot_json = try_read_auth_json(&auth_file).ok();

    let auth_json_api_key = auth_dot_json
        .as_ref()
        .and_then(|a| a.openai_api_key.clone())
        .filter(|s| !s.is_empty());

    let openai_api_key = env::var(OPENAI_API_KEY_ENV_VAR)
        .ok()
        .filter(|s| !s.is_empty())
        .or(auth_json_api_key);

    if openai_api_key.is_none() && auth_dot_json.is_none() {
        return Ok(None);
    }

    let mode = if openai_api_key.is_some() {
        AuthMode::ApiKey
    } else {
        AuthMode::ChatGPT
    };

    Ok(Some(CodexAuth {
        api_key: openai_api_key,
        mode,
        auth_file,
        auth_dot_json: Arc::new(Mutex::new(auth_dot_json)),
    }))
}

/// Run `python3 -c {{SOURCE_FOR_PYTHON_SERVER}}` with the CODEX_HOME
/// environment variable set to the provided `codex_home` path. If the
/// subprocess exits 0, read the OPENAI_API_KEY property out of
/// CODEX_HOME/auth.json and return Ok(OPENAI_API_KEY). Otherwise, return Err
/// with any information from the subprocess.
///
/// If `capture_output` is true, the subprocess's output will be captured and
/// recorded in memory. Otherwise, the subprocess's output will be sent to the
/// current process's stdout/stderr.
pub async fn login_with_chatgpt(codex_home: &Path, capture_output: bool) -> std::io::Result<()> {
    let child = Command::new("python3")
        .arg("-c")
        .arg(SOURCE_FOR_PYTHON_SERVER)
        .env("CODEX_HOME", codex_home)
        .env("CODEX_CLIENT_ID", CLIENT_ID)
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
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(std::io::Error::other(format!(
            "login_with_chatgpt subprocess failed: {stderr}"
        )))
    }
}

/// Attempt to read and refresh the `auth.json` file in the given `CODEX_HOME` directory.
/// Returns the full AuthDotJson structure after refreshing if necessary.
pub fn try_read_auth_json(auth_file: &Path) -> std::io::Result<AuthDotJson> {
    let mut file = std::fs::File::open(auth_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let auth_dot_json: AuthDotJson = serde_json::from_str(&contents)?;

    Ok(auth_dot_json)
}

async fn update_tokens(
    auth_file: &Path,
    id_token: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> std::io::Result<AuthDotJson> {
    let mut options = OpenOptions::new();
    options.truncate(true).write(true).create(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut auth_dot_json = try_read_auth_json(auth_file)?;

    auth_dot_json.tokens.id_token = id_token.to_string();
    if let Some(access_token) = access_token {
        auth_dot_json.tokens.access_token = access_token.to_string();
    }
    if let Some(refresh_token) = refresh_token {
        auth_dot_json.tokens.refresh_token = refresh_token.to_string();
    }
    auth_dot_json.last_refresh = Utc::now();

    let json_data = serde_json::to_string_pretty(&auth_dot_json)?;
    {
        let mut file = options.open(auth_file)?;
        file.write_all(json_data.as_bytes())?;
        file.flush()?;
    }
    Ok(auth_dot_json)
}

async fn try_refresh_token(refresh_token: String) -> std::io::Result<RefreshResponse> {
    let refresh_request = RefreshRequest {
        client_id: CLIENT_ID,
        grant_type: "refresh_token",
        refresh_token,
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

#[derive(Deserialize, Clone)]
struct RefreshResponse {
    id_token: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

/// Expected structure for $CODEX_HOME/auth.json.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AuthDotJson {
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,

    pub tokens: TokenData,

    pub last_refresh: DateTime<Utc>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct TokenData {
    /// This is a JWT.
    pub id_token: String,

    /// This is a JWT.
    pub access_token: String,

    pub refresh_token: String,

    pub account_id: Option<String>,
}
