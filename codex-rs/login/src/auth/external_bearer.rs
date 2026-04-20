use super::manager::ExternalAuth;
use super::manager::ExternalAuthRefreshContext;
use super::manager::ExternalAuthTokens;
use async_trait::async_trait;
use codex_app_server_protocol::AuthMode;
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
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

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
}

#[async_trait]
impl ExternalAuth for BearerTokenRefresher {
    fn auth_mode(&self) -> AuthMode {
        AuthMode::ApiKey
    }

    async fn resolve(&self) -> io::Result<Option<ExternalAuthTokens>> {
        if let Some(tokens) = self.fresh_cached_tokens().await {
            return Ok(Some(tokens));
        }

        let tokens = {
            let held_refresh_permit = self.acquire_refresh_permit().await?;
            if let Some(tokens) = self.fresh_cached_tokens().await {
                tokens
            } else {
                self.refresh_tokens(&held_refresh_permit).await?
            }
        };
        Ok(Some(tokens))
    }

    async fn refresh(
        &self,
        _context: ExternalAuthRefreshContext,
    ) -> io::Result<ExternalAuthTokens> {
        let tokens = {
            let held_refresh_permit = self.acquire_refresh_permit().await?;
            self.refresh_tokens(&held_refresh_permit).await?
        };
        Ok(tokens)
    }
}

impl BearerTokenRefresher {
    async fn fresh_cached_tokens(&self) -> Option<ExternalAuthTokens> {
        let cached = self.state.cached_token.lock().await;
        let cached_token = cached.as_ref()?;
        let should_use_cached_token = match self.state.config.refresh_interval() {
            Some(refresh_interval) => cached_token.fetched_at.elapsed() < refresh_interval,
            None => true,
        };
        should_use_cached_token
            .then(|| ExternalAuthTokens::access_token_only(cached_token.access_token.clone()))
    }

    async fn refresh_tokens(
        &self,
        _held_refresh_permit: &SemaphorePermit<'_>,
    ) -> io::Result<ExternalAuthTokens> {
        let access_token = run_provider_auth_command(&self.state.config).await?;
        let mut cached = self.state.cached_token.lock().await;
        *cached = Some(CachedExternalBearerToken {
            access_token: access_token.clone(),
            fetched_at: Instant::now(),
        });
        Ok(ExternalAuthTokens::access_token_only(access_token))
    }

    async fn acquire_refresh_permit(&self) -> io::Result<SemaphorePermit<'_>> {
        self.state
            .refresh_lock
            .acquire()
            .await
            .map_err(|_| io::Error::other("external bearer token refresh semaphore closed"))
    }
}

impl fmt::Debug for BearerTokenRefresher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BearerTokenRefresher")
            .finish_non_exhaustive()
    }
}

struct ExternalBearerAuthState {
    config: ModelProviderAuthInfo,
    cached_token: Mutex<Option<CachedExternalBearerToken>>,
    refresh_lock: Semaphore,
}

impl ExternalBearerAuthState {
    fn new(config: ModelProviderAuthInfo) -> Self {
        Self {
            config,
            cached_token: Mutex::new(None),
            refresh_lock: Semaphore::new(/*permits*/ 1),
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
