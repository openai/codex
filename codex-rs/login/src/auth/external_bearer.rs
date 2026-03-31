use super::manager::ExternalAuth;
use super::manager::ExternalAuthRefreshContext;
use super::manager::ExternalAuthTokens;
use async_trait::async_trait;
use codex_protocol::config_types::ModelProviderAuthInfo;
use std::fmt;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct BearerTokenRefresher {
    state: Arc<ExternalBearerAuthState>,
}

impl BearerTokenRefresher {
    pub(crate) fn new(config: ModelProviderAuthInfo) -> Self {
        Self {
            state: Arc::new(ExternalBearerAuthState::new(config)),
        }
    }

    pub(crate) async fn resolve_access_token(&self) -> io::Result<String> {
        let mut cached = self.state.cached_token.lock().await;
        if let Some(cached_token) = cached.as_ref()
            && cached_token.fetched_at.elapsed() < self.state.config.refresh_interval()
        {
            return Ok(cached_token.access_token.clone());
        }

        let access_token = run_provider_auth_command(&self.state.config).await?;
        *cached = Some(CachedExternalBearerToken {
            access_token: access_token.clone(),
            fetched_at: Instant::now(),
        });
        Ok(access_token)
    }

    pub(crate) async fn refresh_after_unauthorized(&self) -> io::Result<()> {
        let access_token = run_provider_auth_command(&self.state.config).await?;
        let mut cached = self.state.cached_token.lock().await;
        *cached = Some(CachedExternalBearerToken {
            access_token,
            fetched_at: Instant::now(),
        });
        Ok(())
    }
}

#[async_trait]
impl ExternalAuth for BearerTokenRefresher {
    fn auth_mode(&self) -> crate::AuthMode {
        crate::AuthMode::ApiKey
    }

    async fn resolve(&self) -> io::Result<Option<ExternalAuthTokens>> {
        let access_token = self.resolve_access_token().await?;
        Ok(Some(ExternalAuthTokens::access_token_only(access_token)))
    }

    async fn refresh(&self, _context: ExternalAuthRefreshContext) -> io::Result<ExternalAuthTokens> {
        self.refresh_after_unauthorized().await?;
        let access_token = self.resolve_access_token().await?;
        Ok(ExternalAuthTokens::access_token_only(access_token))
    }
}

impl fmt::Debug for BearerTokenRefresher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BearerTokenRefresher").finish_non_exhaustive()
    }
}

struct ExternalBearerAuthState {
    config: ModelProviderAuthInfo,
    cached_token: Mutex<Option<CachedExternalBearerToken>>,
}

impl ExternalBearerAuthState {
    fn new(config: ModelProviderAuthInfo) -> Self {
        Self {
            config,
            cached_token: Mutex::new(None),
        }
    }
}

struct CachedExternalBearerToken {
    access_token: String,
    fetched_at: Instant,
}

async fn run_provider_auth_command(config: &ModelProviderAuthInfo) -> io::Result<String> {
    let program = resolve_provider_auth_program(&config.command, &config.cwd)?;
    let mut command = Command::new(&program);
    command
        .args(&config.args)
        .current_dir(config.cwd.as_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = tokio::time::timeout(config.timeout(), command.output())
        .await
        .map_err(|_| {
            io::Error::other(format!(
                "provider auth command `{}` timed out after {} ms",
                config.command,
                config.timeout_ms.get()
            ))
        })?
        .map_err(|err| {
            io::Error::other(format!(
                "provider auth command `{}` failed to start: {err}",
                config.command
            ))
        })?;

    if !output.status.success() {
        let status = output.status;
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stderr_suffix = if stderr.is_empty() {
            String::new()
        } else {
            format!(": {stderr}")
        };
        return Err(io::Error::other(format!(
            "provider auth command `{}` exited with status {status}{stderr_suffix}",
            config.command
        )));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|_| {
        io::Error::other(format!(
            "provider auth command `{}` wrote non-UTF-8 data to stdout",
            config.command
        ))
    })?;
    let access_token = stdout.trim().to_string();
    if access_token.is_empty() {
        return Err(io::Error::other(format!(
            "provider auth command `{}` produced an empty token",
            config.command
        )));
    }

    Ok(access_token)
}

fn resolve_provider_auth_program(command: &str, cwd: &Path) -> io::Result<PathBuf> {
    let path = Path::new(command);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    if path.components().count() > 1 {
        return Ok(cwd.join(path));
    }

    Ok(PathBuf::from(command))
}
