use chrono::DateTime;

use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::remove_file;
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::process::Command;

pub use crate::token_data::TokenData;
use crate::token_data::parse_id_token;

mod token_data;

const SOURCE_FOR_PYTHON_SERVER: &str = include_str!("./login_with_chatgpt.py");

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";

#[derive(Clone, Debug, PartialEq, Copy)]
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
            Some(AuthDotJson {
                tokens: Some(mut tokens),
                last_refresh: Some(last_refresh),
                ..
            }) => {
                if last_refresh < Utc::now() - chrono::Duration::days(28) {
                    let refresh_response = tokio::time::timeout(
                        Duration::from_secs(60),
                        try_refresh_token(tokens.refresh_token.clone()),
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

                    tokens = updated_auth_dot_json
                        .tokens
                        .clone()
                        .ok_or(std::io::Error::other(
                            "Token data is not available after refresh.",
                        ))?;

                    #[expect(clippy::unwrap_used)]
                    let mut auth_lock = self.auth_dot_json.lock().unwrap();
                    *auth_lock = Some(updated_auth_dot_json);
                }

                Ok(tokens)
            }
            _ => Err(std::io::Error::other("Token data is not available.")),
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

    pub async fn get_account_id(&self) -> Option<String> {
        match self.mode {
            AuthMode::ApiKey => None,
            AuthMode::ChatGPT => {
                let token_data = self.get_token_data().await.ok()?;

                token_data.account_id.clone()
            }
        }
    }
}

// Loads the available auth information from the auth.json or OPENAI_API_KEY environment variable.
pub fn load_auth(codex_home: &Path) -> std::io::Result<Option<CodexAuth>> {
    _load_auth(codex_home, true)
}

fn _load_auth(codex_home: &Path, include_env_var: bool) -> std::io::Result<Option<CodexAuth>> {
    let auth_file = get_auth_file(codex_home);

    let auth_dot_json = try_read_auth_json(&auth_file).ok();

    let auth_json_api_key = auth_dot_json
        .as_ref()
        .and_then(|a| a.openai_api_key.clone())
        .filter(|s| !s.is_empty());

    let openai_api_key = if include_env_var {
        env::var(OPENAI_API_KEY_ENV_VAR)
            .ok()
            .filter(|s| !s.is_empty())
            .or(auth_json_api_key)
    } else {
        auth_json_api_key
    };

    let has_tokens = auth_dot_json
        .as_ref()
        .and_then(|a| a.tokens.as_ref())
        .is_some();

    if openai_api_key.is_none() && !has_tokens {
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

pub fn get_auth_file(codex_home: &Path) -> PathBuf {
    codex_home.join("auth.json")
}

/// Delete the auth.json file inside `codex_home` if it exists. Returns `Ok(true)`
/// if a file was removed, `Ok(false)` if no auth file was present.
pub fn logout(codex_home: &Path) -> std::io::Result<bool> {
    let auth_file = get_auth_file(codex_home);
    match remove_file(&auth_file) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

/// Represents a running login subprocess. The child can be killed by holding
/// the mutex and calling `kill()`.
#[derive(Debug, Clone)]
pub struct SpawnedLogin {
    pub child: Arc<Mutex<Child>>,
    pub stdout: Arc<Mutex<Vec<u8>>>,
    pub stderr: Arc<Mutex<Vec<u8>>>,
}

/// Spawn the ChatGPT login Python server as a child process and return a handle to its process.
pub fn spawn_login_with_chatgpt(codex_home: &Path) -> std::io::Result<SpawnedLogin> {
    let mut cmd = std::process::Command::new("python3");
    cmd.arg("-c")
        .arg(SOURCE_FOR_PYTHON_SERVER)
        .env("CODEX_HOME", codex_home)
        .env("CODEX_CLIENT_ID", CLIENT_ID)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    let stdout_buf = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf = Arc::new(Mutex::new(Vec::new()));

    if let Some(mut out) = child.stdout.take() {
        let buf = stdout_buf.clone();
        std::thread::spawn(move || {
            let mut tmp = Vec::new();
            let _ = std::io::copy(&mut out, &mut tmp);
            if let Ok(mut b) = buf.lock() {
                b.extend_from_slice(&tmp);
            }
        });
    }
    if let Some(mut err) = child.stderr.take() {
        let buf = stderr_buf.clone();
        std::thread::spawn(move || {
            let mut tmp = Vec::new();
            let _ = std::io::copy(&mut err, &mut tmp);
            if let Ok(mut b) = buf.lock() {
                b.extend_from_slice(&tmp);
            }
        });
    }

    Ok(SpawnedLogin {
        child: Arc::new(Mutex::new(child)),
        stdout: stdout_buf,
        stderr: stderr_buf,
    })
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

pub fn login_with_api_key(codex_home: &Path, api_key: &str) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson {
        openai_api_key: Some(api_key.to_string()),
        tokens: None,
        last_refresh: None,
    };
    write_auth_json(&get_auth_file(codex_home), &auth_dot_json)
}

/// Attempt to read and refresh the `auth.json` file in the given `CODEX_HOME` directory.
/// Returns the full AuthDotJson structure after refreshing if necessary.
pub fn try_read_auth_json(auth_file: &Path) -> std::io::Result<AuthDotJson> {
    let mut file = File::open(auth_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let auth_dot_json: AuthDotJson = serde_json::from_str(&contents)?;

    Ok(auth_dot_json)
}

fn write_auth_json(auth_file: &Path, auth_dot_json: &AuthDotJson) -> std::io::Result<()> {
    let json_data = serde_json::to_string_pretty(auth_dot_json)?;
    let mut options = OpenOptions::new();
    options.truncate(true).write(true).create(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut file = options.open(auth_file)?;
    file.write_all(json_data.as_bytes())?;
    file.flush()?;
    Ok(())
}

async fn update_tokens(
    auth_file: &Path,
    id_token: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> std::io::Result<AuthDotJson> {
    let mut auth_dot_json = try_read_auth_json(auth_file)?;

    let tokens = auth_dot_json.tokens.get_or_insert_with(TokenData::default);
    tokens.id_token = parse_id_token(&id_token).map_err(std::io::Error::other)?;
    if let Some(access_token) = access_token {
        tokens.access_token = access_token.to_string();
    }
    if let Some(refresh_token) = refresh_token {
        tokens.refresh_token = refresh_token.to_string();
    }
    auth_dot_json.last_refresh = Some(Utc::now());
    write_auth_json(auth_file, &auth_dot_json)?;
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenData>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_data::IdTokenInfo;
    use base64::Engine;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    #[expect(clippy::unwrap_used)]
    fn writes_api_key_and_loads_auth() {
        let dir = tempdir().unwrap();
        login_with_api_key(dir.path(), "sk-test-key").unwrap();
        let auth = _load_auth(dir.path(), false).unwrap().unwrap();
        assert_eq!(auth.mode, AuthMode::ApiKey);
        assert_eq!(auth.api_key.as_deref(), Some("sk-test-key"));
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn loads_from_env_var_if_env_var_exists() {
        let dir = tempdir().unwrap();

        let env_var = std::env::var(OPENAI_API_KEY_ENV_VAR);

        if let Ok(env_var) = env_var {
            let auth = _load_auth(dir.path(), true).unwrap().unwrap();
            assert_eq!(auth.mode, AuthMode::ApiKey);
            assert_eq!(auth.api_key, Some(env_var));
        }
    }

    #[tokio::test]
    #[expect(clippy::expect_used, clippy::unwrap_used)]
    async fn loads_token_data_from_auth_json() {
        let dir = tempdir().unwrap();
        let auth_file = dir.path().join("auth.json");
        // Create a minimal valid JWT for the id_token field.
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }
        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = serde_json::json!({
            "email": "user@example.com",
            "email_verified": true,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "bc3618e3-489d-4d49-9362-1561dc53ba53",
                "chatgpt_plan_type": "pro",
                "chatgpt_user_id": "user-12345",
                "user_id": "user-12345",
            }
        });
        let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        let header_b64 = b64(&serde_json::to_vec(&header).unwrap());
        let payload_b64 = b64(&serde_json::to_vec(&payload).unwrap());
        let signature_b64 = b64(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");
        std::fs::write(
            auth_file,
            format!(
                r#"
        {{
            "OPENAI_API_KEY": null,
            "tokens": {{
                "id_token": "{fake_jwt}",
                "access_token": "test-access-token",
                "refresh_token": "test-refresh-token"
            }},
            "last_refresh": "2025-08-06T20:41:36.232376Z"
        }}
        "#,
            ),
        )
        .unwrap();

        let CodexAuth {
            api_key,
            mode,
            auth_dot_json,
            auth_file,
        } = _load_auth(dir.path(), false).unwrap().unwrap();
        assert_eq!(None, api_key);
        assert_eq!(AuthMode::ChatGPT, mode);
        assert_eq!(dir.path().join("auth.json"), auth_file);

        let guard = auth_dot_json.lock().unwrap();
        let auth_dot_json = guard.as_ref().expect("AuthDotJson should exist");

        assert_eq!(
            &AuthDotJson {
                openai_api_key: None,
                tokens: Some(TokenData {
                    id_token: IdTokenInfo {
                        email: Some("user@example.com".to_string()),
                        chatgpt_plan_type: Some("pro".to_string()),
                    },
                    access_token: "test-access-token".to_string(),
                    refresh_token: "test-refresh-token".to_string(),
                    account_id: None,
                }),
                last_refresh: Some(
                    DateTime::parse_from_rfc3339("2025-08-06T20:41:36.232376Z")
                        .unwrap()
                        .with_timezone(&Utc)
                ),
            },
            auth_dot_json
        )
    }

    #[test]
    #[expect(clippy::expect_used, clippy::unwrap_used)]
    fn id_token_info_handles_missing_fields() {
        // Payload without email or plan should yield None values.
        let header = serde_json::json!({"alg": "none", "typ": "JWT"});
        let payload = serde_json::json!({"sub": "123"});
        let header_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&header).unwrap());
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&payload).unwrap());
        let signature_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"sig");
        let jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let info = parse_id_token(&jwt).expect("should parse");
        assert!(info.email.is_none());
        assert!(info.chatgpt_plan_type.is_none());
    }

    #[tokio::test]
    #[expect(clippy::unwrap_used)]
    async fn loads_api_key_from_auth_json() {
        let dir = tempdir().unwrap();
        let auth_file = dir.path().join("auth.json");
        std::fs::write(
            auth_file,
            r#"
        {
            "OPENAI_API_KEY": "sk-test-key",
            "tokens": null,
            "last_refresh": null
        }
        "#,
        )
        .unwrap();

        let auth = _load_auth(dir.path(), false).unwrap().unwrap();
        assert_eq!(auth.mode, AuthMode::ApiKey);
        assert_eq!(auth.api_key, Some("sk-test-key".to_string()));

        assert!(auth.get_token_data().await.is_err());
    }

    #[test]
    fn logout_removes_auth_file() -> Result<(), std::io::Error> {
        let dir = tempdir()?;
        login_with_api_key(dir.path(), "sk-test-key")?;
        assert!(dir.path().join("auth.json").exists());
        let removed = logout(dir.path())?;
        assert!(removed);
        assert!(!dir.path().join("auth.json").exists());
        Ok(())
    }
}
