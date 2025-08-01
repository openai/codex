use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::fs as tokio_fs;
use tokio::process::Command;

const SOURCE_FOR_PYTHON_SERVER: &str = include_str!("./login_with_chatgpt.py");

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AZURE_OPENAI: &str = "azure";
const BUFFER_REFRESH_TOKEN_SECONDS: u64 = 300; // 5 minutes
const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
const AZURE_TOKEN_CACHE_FILE: &str = "azure_token_cache.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CachedAzureToken {
    pub access_token: String,
    pub expires_on: u64, // Unix timestamp
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthMode {
    ApiKey,
    MicrosoftEntraID,
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

    pub async fn get_token(&self, codex_home: &PathBuf) -> Result<String, std::io::Error> {
        match self.mode {
            AuthMode::ApiKey => Ok(self.api_key.clone().unwrap_or_default()),
            AuthMode::MicrosoftEntraID => {
                if let Some(token) = try_refresh_azure_token(codex_home).await? {
                    return Ok(token);
                }
                Err(std::io::Error::other(
                    "Failed to get Azure token. Make sure you've downloaded the Azure CLI and logged in with 'az login'.",
                ))
            }
            AuthMode::ChatGPT => {
                let id_token = self.get_token_data().await?.access_token;

                Ok(id_token)
            }
        }
    }
}

// Loads the available auth information from the auth.json or OPENAI_API_KEY environment variable.
pub fn load_auth(
    codex_home: &Path,
    model_provider_name: &str,
    env_key: &Option<String>,
) -> std::io::Result<Option<CodexAuth>> {
    let auth_file = codex_home.join("auth.json");

    let auth_dot_json = try_read_auth_json(&auth_file).ok();

    let auth_json_api_key = auth_dot_json
        .as_ref()
        .and_then(|a| a.openai_api_key.clone())
        .filter(|s| !s.is_empty());

    let openai_api_key = env::var(env_key.as_deref().unwrap_or(OPENAI_API_KEY_ENV_VAR))
        .ok()
        .filter(|s| !s.is_empty())
        .or(auth_json_api_key);

    if model_provider_name.to_lowercase() != AZURE_OPENAI
        && openai_api_key.is_none()
        && auth_dot_json.is_none()
    {
        return Ok(None);
    }

    let mode = if openai_api_key.is_some() {
        AuthMode::ApiKey
    } else if model_provider_name.to_lowercase() == AZURE_OPENAI {
        AuthMode::MicrosoftEntraID
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
        tokio_fs::write(auth_file, json_data).await?;
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

async fn try_refresh_azure_token(codex_home: &PathBuf) -> std::io::Result<Option<String>> {
    // Ensure cache directory exists asynchronously
    tokio_fs::create_dir_all(&codex_home).await?;

    let cache_file = codex_home.join(AZURE_TOKEN_CACHE_FILE);

    // Check if we have a valid cached token
    if tokio_fs::metadata(&cache_file).await.is_ok() {
        let cached_data = tokio_fs::read_to_string(&cache_file).await?;
        if let Ok(token) = serde_json::from_str::<CachedAzureToken>(&cached_data) {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| {
                    std::io::Error::other("Failed to get current time for token validation")
                })?
                .as_secs();

            // Add a buffer to avoid using an expired token
            if current_time + BUFFER_REFRESH_TOKEN_SECONDS < token.expires_on {
                return Ok(Some(token.access_token));
            }
        }
    }

    // Token is expired or doesn't exist, fetch a new one
    let output = Command::new("az")
        .args([
            "account",
            "get-access-token",
            "--output",
            "json",
            "--resource",
            "https://cognitiveservices.azure.com",
        ])
        .output()
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Azure CLI not found. Please install the Azure CLI.",
            )
        })?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Failed to parse Azure CLI output. Please ensure you are logged in with 'az login'.",
        )
    })?;

    let access_token = json["accessToken"]
        .as_str()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Azure CLI response missing 'accessToken' field",
            )
        })?
        .to_string();

    let expires_on = json["expires_on"].as_u64().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Azure CLI response missing or invalid 'expires_on' field",
        )
    })?;

    // Cache the new token to file asynchronously with strict permissions
    let new_token = CachedAzureToken {
        access_token: access_token.clone(),
        expires_on,
    };
    let cache_data = serde_json::to_string(&new_token)?;
    tokio_fs::write(&cache_file, cache_data).await?;

    Ok(Some(access_token))
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
