use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::BufReader;
use tokio::io::ReadBuf;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::sync::OnceCell;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;

use crate::ExecServerClient;
use crate::ExecServerError;
use crate::RemoteExecServerConnectArgs;
use crate::file_system::ExecutorFileSystem;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";
pub const CODEX_EXEC_SERVER_SSH_HOSTS_ENV_VAR: &str = "CODEX_EXEC_SERVER_SSH_HOSTS";
pub const CODEX_EXEC_SERVER_DEFAULT_HOST_ENV_VAR: &str = "CODEX_EXEC_SERVER_DEFAULT_HOST";
pub const LOCAL_HOST_ID: &str = "local";
pub const DEFAULT_HOST_ALIAS: &str = "default";

const SSH_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(20);
const EXEC_SERVER_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const SSH_EXEC_SERVER_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const EXEC_SERVER_CONNECT_RETRY_DELAY: Duration = Duration::from_millis(100);
const SSH_BOOTSTRAP_PWD_MARKER: &str = "__codex_exec_server_pwd__=";
const SSH_BOOTSTRAP_SHELL_MARKER: &str = "__codex_exec_server_shell__=";

/// A host that can provide command execution and filesystem access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostConfig {
    pub id: String,
    pub connection: HostConnection,
}

impl HostConfig {
    pub fn local(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            connection: HostConnection::Local,
        }
    }

    pub fn exec_server_url(id: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            connection: HostConnection::ExecServerUrl { url: url.into() },
        }
    }

    pub fn ssh(id: impl Into<String>, ssh_host: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            connection: HostConnection::Ssh {
                ssh_host: ssh_host.into(),
            },
        }
    }
}

/// How Codex connects to a host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostConnection {
    Local,
    ExecServerUrl { url: String },
    Ssh { ssh_host: String },
}

struct HostEntry {
    config: HostConfig,
    environment: OnceCell<Option<Arc<Environment>>>,
}

impl HostEntry {
    fn new(config: HostConfig) -> Self {
        Self {
            config,
            environment: OnceCell::new(),
        }
    }

    async fn environment(
        &self,
        disabled: bool,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        self.environment
            .get_or_try_init(|| async {
                if disabled {
                    Ok(None)
                } else {
                    Ok(Some(Arc::new(
                        Environment::create_for_host_config(&self.config).await?,
                    )))
                }
            })
            .await
            .map(Option::as_ref)
            .map(std::option::Option::<&Arc<Environment>>::cloned)
    }
}

struct EnvironmentRegistry {
    default_host_id: String,
    disabled: bool,
    hosts: HashMap<String, Arc<HostEntry>>,
}

impl EnvironmentRegistry {
    fn new(exec_server_url: Option<String>, disabled: bool) -> Self {
        let mut hosts = HashMap::new();
        hosts.insert(
            LOCAL_HOST_ID.to_string(),
            Arc::new(HostEntry::new(HostConfig::local(LOCAL_HOST_ID))),
        );

        let default_host_id = if let Some(exec_server_url) = exec_server_url {
            hosts.insert(
                DEFAULT_HOST_ALIAS.to_string(),
                Arc::new(HostEntry::new(HostConfig::exec_server_url(
                    DEFAULT_HOST_ALIAS,
                    exec_server_url,
                ))),
            );
            DEFAULT_HOST_ALIAS.to_string()
        } else {
            LOCAL_HOST_ID.to_string()
        };

        Self {
            default_host_id,
            disabled,
            hosts,
        }
    }
}

/// Lazily creates and caches execution environments for registered hosts.
///
/// The registry lives above any individual thread/session. `current()` keeps
/// existing single-environment behavior, while `current_for_host()` lets a turn
/// route one command to any host already registered with the manager.
pub struct EnvironmentManager {
    registry: RwLock<EnvironmentRegistry>,
}

impl std::fmt::Debug for EnvironmentManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f.debug_struct("EnvironmentManager")
            .field("default_host_id", &registry.default_host_id)
            .field("disabled", &registry.disabled)
            .field("hosts", &registry.hosts.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for EnvironmentManager {
    fn default() -> Self {
        Self::new(/*exec_server_url*/ None)
    }
}

impl EnvironmentManager {
    /// Builds a manager from the raw `CODEX_EXEC_SERVER_URL` value.
    pub fn new(exec_server_url: Option<String>) -> Self {
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        Self {
            registry: RwLock::new(EnvironmentRegistry::new(exec_server_url, disabled)),
        }
    }

    /// Builds a manager from process environment variables.
    pub fn from_env() -> Self {
        let manager = Self::new(std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok());
        if let Ok(ssh_hosts) = std::env::var(CODEX_EXEC_SERVER_SSH_HOSTS_ENV_VAR) {
            for host_config in parse_ssh_hosts_env(&ssh_hosts) {
                if let Err(err) = manager.register_host(host_config) {
                    tracing::warn!(
                        "ignoring invalid {CODEX_EXEC_SERVER_SSH_HOSTS_ENV_VAR} entry: {err}"
                    );
                }
            }
        }
        if let Ok(default_host_id) = std::env::var(CODEX_EXEC_SERVER_DEFAULT_HOST_ENV_VAR)
            && !default_host_id.trim().is_empty()
            && let Err(err) = manager.set_default_host(default_host_id.trim())
        {
            tracing::warn!(
                "ignoring invalid {CODEX_EXEC_SERVER_DEFAULT_HOST_ENV_VAR} value: {err}"
            );
        }
        manager
    }

    /// Builds a manager from the currently selected environment, or from the
    /// disabled mode when no environment is available.
    pub fn from_environment(environment: Option<&Environment>) -> Self {
        match environment {
            Some(environment) => {
                if let Some(ssh_host) = environment
                    .exec_server_url()
                    .and_then(|url| url.strip_prefix("ssh://"))
                    .filter(|host| !host.is_empty())
                {
                    let manager = Self::new(/*exec_server_url*/ None);
                    if manager.register_ssh_host(ssh_host, ssh_host).is_ok()
                        && manager.set_default_host(ssh_host).is_ok()
                    {
                        manager
                    } else {
                        Self::new(environment.exec_server_url().map(str::to_owned))
                    }
                } else {
                    Self::new(environment.exec_server_url().map(str::to_owned))
                }
            }
            None => {
                let manager = Self::new(/*exec_server_url*/ None);
                {
                    let mut registry = manager
                        .registry
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    registry.disabled = true;
                }
                manager
            }
        }
    }

    /// Registers or replaces a host in the shared registry.
    pub fn register_host(&self, host_config: HostConfig) -> Result<(), ExecServerError> {
        let id = normalize_host_id(&host_config.id)?;
        let mut host_config = host_config;
        host_config.id = id.clone();
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry
            .hosts
            .insert(id, Arc::new(HostEntry::new(host_config)));
        Ok(())
    }

    /// Registers an SSH host. The host id defaults to the same value that SSH
    /// resolves via `~/.ssh/config`.
    pub fn register_ssh_host(
        &self,
        id: impl Into<String>,
        ssh_host: impl Into<String>,
    ) -> Result<(), ExecServerError> {
        self.register_host(HostConfig::ssh(id, ssh_host))
    }

    pub fn set_default_host(&self, host_id: &str) -> Result<(), ExecServerError> {
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let resolved = resolve_host_id(&registry, host_id)?;
        if !registry.hosts.contains_key(&resolved) {
            return Err(unknown_host_error(host_id));
        }
        registry.default_host_id = resolved;
        Ok(())
    }

    pub fn default_host_id(&self) -> String {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry.default_host_id.clone()
    }

    pub fn registered_host_ids(&self) -> Vec<String> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut ids = registry.hosts.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        ids
    }

    /// Returns the remote exec-server URL when the default host is URL-backed.
    pub fn exec_server_url(&self) -> Option<String> {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry
            .hosts
            .get(&registry.default_host_id)
            .and_then(|entry| match &entry.config.connection {
                HostConnection::ExecServerUrl { url } => Some(url.clone()),
                _ => None,
            })
    }

    /// Returns true when the default host is configured to use a remote backend.
    pub fn is_remote(&self) -> bool {
        let registry = self
            .registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry
            .hosts
            .get(&registry.default_host_id)
            .is_some_and(|entry| !matches!(entry.config.connection, HostConnection::Local))
    }

    /// Returns the cached default environment, creating it on first access.
    pub async fn current(&self) -> Result<Option<Arc<Environment>>, ExecServerError> {
        self.current_for_host(DEFAULT_HOST_ALIAS).await
    }

    /// Returns the cached environment for a registered host, creating it on
    /// first access. The special host id `default` resolves to the current
    /// registry default.
    pub async fn current_for_host(
        &self,
        host_id: &str,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        let (entry, disabled) = {
            let registry = self
                .registry
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let resolved_host_id = resolve_host_id(&registry, host_id)?;
            let entry = registry
                .hosts
                .get(&resolved_host_id)
                .cloned()
                .ok_or_else(|| unknown_host_error(host_id))?;
            (entry, registry.disabled)
        };
        entry.environment(disabled).await
    }
}

/// Concrete execution/filesystem environment selected for a host.
///
/// This bundles the selected backend together with the corresponding remote
/// client, if any. SSH-backed environments also retain the child processes that
/// keep the remote exec-server SSH process alive.
#[derive(Clone)]
pub struct Environment {
    exec_server_url: Option<String>,
    remote_exec_server_client: Option<ExecServerClient>,
    exec_backend: Arc<dyn ExecBackend>,
    default_cwd: Option<PathBuf>,
    default_shell: Option<String>,
    _ssh_exec_server: Option<Arc<SshExecServer>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            exec_server_url: None,
            remote_exec_server_client: None,
            exec_backend: Arc::new(LocalProcess::default()),
            default_cwd: None,
            default_shell: None,
            _ssh_exec_server: None,
        }
    }
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field("exec_server_url", &self.exec_server_url)
            .field("default_cwd", &self.default_cwd)
            .field("default_shell", &self.default_shell)
            .finish_non_exhaustive()
    }
}

impl Environment {
    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value.
    pub async fn create(exec_server_url: Option<String>) -> Result<Self, ExecServerError> {
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        if disabled {
            return Err(ExecServerError::Protocol(
                "disabled mode does not create an Environment".to_string(),
            ));
        }

        match exec_server_url {
            Some(exec_server_url) => {
                Self::create_remote(
                    exec_server_url,
                    /*default_cwd*/ None,
                    /*default_shell*/ None,
                    /*ssh_exec_server*/ None,
                    "codex-environment",
                )
                .await
            }
            None => Ok(Self::default()),
        }
    }

    async fn create_for_host_config(host_config: &HostConfig) -> Result<Self, ExecServerError> {
        match &host_config.connection {
            HostConnection::Local => Self::create(/*exec_server_url*/ None).await,
            HostConnection::ExecServerUrl { url } => {
                Self::create_remote(
                    url.clone(),
                    /*default_cwd*/ None,
                    /*default_shell*/ None,
                    /*ssh_exec_server*/ None,
                    format!("codex-environment-{}", host_config.id),
                )
                .await
            }
            HostConnection::Ssh { ssh_host } => {
                let bootstrap = SshExecServer::connect(ssh_host).await?;
                Self::create_remote_from_client(
                    format!("ssh://{ssh_host}"),
                    bootstrap.client,
                    bootstrap.default_cwd.clone(),
                    bootstrap.default_shell.clone(),
                    Some(Arc::clone(&bootstrap.session)),
                )
            }
        }
    }

    async fn create_remote(
        exec_server_url: String,
        default_cwd: Option<PathBuf>,
        default_shell: Option<String>,
        ssh_exec_server: Option<Arc<SshExecServer>>,
        client_name: impl Into<String>,
    ) -> Result<Self, ExecServerError> {
        let remote_exec_server_client =
            connect_websocket_with_retry(exec_server_url.clone(), client_name.into()).await?;
        let exec_backend: Arc<dyn ExecBackend> =
            Arc::new(RemoteProcess::new(remote_exec_server_client.clone()));

        Ok(Self {
            exec_server_url: Some(exec_server_url),
            remote_exec_server_client: Some(remote_exec_server_client),
            exec_backend,
            default_cwd,
            default_shell,
            _ssh_exec_server: ssh_exec_server,
        })
    }

    fn create_remote_from_client(
        exec_server_url: String,
        remote_exec_server_client: ExecServerClient,
        default_cwd: Option<PathBuf>,
        default_shell: Option<String>,
        ssh_exec_server: Option<Arc<SshExecServer>>,
    ) -> Result<Self, ExecServerError> {
        let exec_backend: Arc<dyn ExecBackend> =
            Arc::new(RemoteProcess::new(remote_exec_server_client.clone()));

        Ok(Self {
            exec_server_url: Some(exec_server_url),
            remote_exec_server_client: Some(remote_exec_server_client),
            exec_backend,
            default_cwd,
            default_shell,
            _ssh_exec_server: ssh_exec_server,
        })
    }

    pub fn is_remote(&self) -> bool {
        self.exec_server_url.is_some()
    }

    /// Returns the remote exec-server URL when this environment is remote.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub fn default_cwd(&self) -> Option<&Path> {
        self.default_cwd.as_deref()
    }

    pub fn default_shell(&self) -> Option<&str> {
        self.default_shell.as_deref()
    }

    pub fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        match self.remote_exec_server_client.clone() {
            Some(client) => Arc::new(RemoteFileSystem::new(client)),
            None => Arc::new(LocalFileSystem),
        }
    }
}

struct SshExecServerBootstrap {
    client: ExecServerClient,
    default_cwd: Option<PathBuf>,
    default_shell: Option<String>,
    session: Arc<SshExecServer>,
}

struct SshExecServer {
    child: Mutex<Option<Child>>,
}

impl std::fmt::Debug for SshExecServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshExecServer").finish_non_exhaustive()
    }
}

impl SshExecServer {
    async fn connect(ssh_host: &str) -> Result<SshExecServerBootstrap, ExecServerError> {
        let metadata = read_ssh_metadata(ssh_host).await?;
        let mut child = Command::new("ssh")
            .arg(ssh_host)
            .arg(build_ssh_websocket_proxy_command())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ExecServerError::Spawn)?;

        let stdin = child.stdin.take().ok_or_else(|| {
            ExecServerError::Protocol("ssh exec-server proxy stdin was not piped".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ExecServerError::Protocol("ssh exec-server proxy stdout was not piped".to_string())
        })?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(drain_reader(stderr));
        }

        let session = Arc::new(Self {
            child: Mutex::new(Some(child)),
        });
        let client = timeout(
            SSH_EXEC_SERVER_CONNECT_TIMEOUT,
            ExecServerClient::connect_websocket_stream(
                "ws://127.0.0.1/".to_string(),
                ChildWebSocketStream { stdin, stdout },
                crate::ExecServerClientConnectOptions {
                    client_name: format!("codex-environment-ssh-{ssh_host}"),
                    initialize_timeout: SSH_EXEC_SERVER_CONNECT_TIMEOUT,
                    resume_session_id: None,
                },
            ),
        )
        .await
        .map_err(|_| ExecServerError::WebSocketConnectTimeout {
            url: format!("ssh://{ssh_host}"),
            timeout: SSH_EXEC_SERVER_CONNECT_TIMEOUT,
        })??;

        Ok(SshExecServerBootstrap {
            client,
            default_cwd: metadata.default_cwd,
            default_shell: metadata.default_shell,
            session,
        })
    }
}

impl Drop for SshExecServer {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.start_kill();
        }
    }
}

struct ChildWebSocketStream {
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl AsyncRead for ChildWebSocketStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ChildWebSocketStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        Pin::new(&mut this.stdin).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.stdin).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.stdin).poll_shutdown(cx)
    }
}

struct SshMetadata {
    default_cwd: Option<PathBuf>,
    default_shell: Option<String>,
}

async fn drain_reader<R>(reader: R)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    drain_buf_reader(BufReader::new(reader)).await;
}

async fn drain_buf_reader<R>(mut reader: BufReader<R>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
        line.clear();
    }
}

async fn read_ssh_metadata(ssh_host: &str) -> Result<SshMetadata, ExecServerError> {
    let output = timeout(
        SSH_BOOTSTRAP_TIMEOUT,
        Command::new("ssh")
            .arg(ssh_host)
            .arg(build_ssh_metadata_command())
            .output(),
    )
    .await
    .map_err(|_| {
        ExecServerError::Protocol(format!(
            "timed out waiting for ssh metadata after {SSH_BOOTSTRAP_TIMEOUT:?}"
        ))
    })?
    .map_err(ExecServerError::Spawn)?;

    if !output.status.success() {
        return Err(ExecServerError::Protocol(format!(
            "ssh metadata command failed with status {}",
            output.status
        )));
    }

    let mut default_cwd = None;
    let mut default_shell = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix(SSH_BOOTSTRAP_PWD_MARKER) {
            if !value.is_empty() {
                default_cwd = Some(PathBuf::from(value));
            }
        } else if let Some(value) = line.strip_prefix(SSH_BOOTSTRAP_SHELL_MARKER)
            && !value.is_empty()
        {
            default_shell = Some(value.to_string());
        }
    }

    Ok(SshMetadata {
        default_cwd,
        default_shell,
    })
}

fn build_ssh_metadata_command() -> String {
    let script = format!(
        "printf '{SSH_BOOTSTRAP_PWD_MARKER}%s\\n' \"$PWD\"; printf '{SSH_BOOTSTRAP_SHELL_MARKER}%s\\n' \"${{SHELL:-}}\""
    );
    let quoted_script = shell_single_quote(&script);
    format!(
        "if command -v zsh >/dev/null 2>&1; then exec zsh -lc {quoted_script}; fi; if command -v bash >/dev/null 2>&1; then exec bash -lc {quoted_script}; fi; exec sh -lc {quoted_script}"
    )
}

fn build_ssh_websocket_proxy_command() -> String {
    let script = r#"out="${TMPDIR:-/tmp}/codex-exec-server.$$.out"; err="${TMPDIR:-/tmp}/codex-exec-server.$$.err"; rm -f "$out" "$err"; cleanup() { if [ -n "${pid:-}" ]; then kill "$pid" >/dev/null 2>&1 || true; fi; rm -f "$out" "$err"; }; trap cleanup EXIT HUP INT TERM; codex exec-server --listen ws://127.0.0.1:0 >"$out" 2>"$err" & pid=$!; i=0; port=""; while [ "$i" -lt 100 ]; do if ! kill -0 "$pid" >/dev/null 2>&1; then cat "$err" >&2; exit 1; fi; port="$(sed -n 's#^ws://127\.0\.0\.1:\([0-9][0-9]*\)$#\1#p' "$out" | tail -1)"; if [ -n "$port" ]; then break; fi; i=$((i+1)); sleep 0.1; done; if [ -z "$port" ]; then echo "timed out waiting for codex exec-server URL" >&2; cat "$err" >&2; exit 1; fi; nc 127.0.0.1 "$port"; status=$?; cleanup; exit "$status""#;
    let quoted_script = shell_single_quote(script);
    format!(
        "if command -v zsh >/dev/null 2>&1; then exec zsh -lc {quoted_script}; fi; if command -v bash >/dev/null 2>&1; then exec bash -lc {quoted_script}; fi; exec sh -lc {quoted_script}"
    )
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

async fn connect_websocket_with_retry(
    exec_server_url: String,
    client_name: String,
) -> Result<ExecServerClient, ExecServerError> {
    let deadline = Instant::now() + EXEC_SERVER_CONNECT_TIMEOUT;
    loop {
        match ExecServerClient::connect_websocket(RemoteExecServerConnectArgs {
            websocket_url: exec_server_url.clone(),
            client_name: client_name.clone(),
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: EXEC_SERVER_CONNECT_TIMEOUT,
            resume_session_id: None,
        })
        .await
        {
            Ok(client) => return Ok(client),
            Err(_err) if Instant::now() < deadline => {
                sleep(EXEC_SERVER_CONNECT_RETRY_DELAY).await;
            }
            Err(err) => return Err(err),
        }
    }
}

fn normalize_exec_server_url(exec_server_url: Option<String>) -> (Option<String>, bool) {
    match exec_server_url.as_deref().map(str::trim) {
        None | Some("") => (None, false),
        Some(url) if url.eq_ignore_ascii_case("none") => (None, true),
        Some(url) => (Some(url.to_string()), false),
    }
}

fn normalize_host_id(host_id: &str) -> Result<String, ExecServerError> {
    let host_id = host_id.trim();
    if host_id.is_empty() {
        return Err(ExecServerError::Protocol(
            "host id cannot be empty".to_string(),
        ));
    }
    Ok(host_id.to_string())
}

fn resolve_host_id(
    registry: &EnvironmentRegistry,
    host_id: &str,
) -> Result<String, ExecServerError> {
    let host_id = normalize_host_id(host_id)?;
    if host_id == DEFAULT_HOST_ALIAS {
        Ok(registry.default_host_id.clone())
    } else {
        Ok(host_id)
    }
}

fn unknown_host_error(host_id: &str) -> ExecServerError {
    ExecServerError::Protocol(format!(
        "unknown exec host `{host_id}`; registered hosts are configured through `{CODEX_EXEC_SERVER_SSH_HOSTS_ENV_VAR}`"
    ))
}

fn parse_ssh_hosts_env(value: &str) -> Vec<HostConfig> {
    value
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                return None;
            }
            let (id, ssh_host) = entry
                .split_once('=')
                .or_else(|| entry.split_once(':'))
                .unwrap_or((entry, entry));
            let id = id.trim();
            let ssh_host = ssh_host.trim();
            if id.is_empty() || ssh_host.is_empty() {
                None
            } else {
                Some(HostConfig::ssh(id, ssh_host))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::CODEX_EXEC_SERVER_DEFAULT_HOST_ENV_VAR;
    use super::CODEX_EXEC_SERVER_SSH_HOSTS_ENV_VAR;
    use super::DEFAULT_HOST_ALIAS;
    use super::Environment;
    use super::EnvironmentManager;
    use super::HostConfig;
    use super::LOCAL_HOST_ID;
    use super::parse_ssh_hosts_env;
    use crate::ProcessId;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn create_local_environment_does_not_connect() {
        let environment = Environment::create(/*exec_server_url*/ None)
            .await
            .expect("create environment");

        assert_eq!(environment.exec_server_url(), None);
        assert!(environment.remote_exec_server_client.is_none());
    }

    #[test]
    fn environment_manager_normalizes_empty_url() {
        let manager = EnvironmentManager::new(Some(String::new()));

        assert_eq!(manager.exec_server_url(), None);
        assert!(!manager.is_remote());
        assert_eq!(manager.default_host_id(), LOCAL_HOST_ID);
    }

    #[test]
    fn environment_manager_treats_none_value_as_disabled() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert_eq!(manager.exec_server_url(), None);
        assert!(!manager.is_remote());
    }

    #[test]
    fn environment_manager_reports_remote_url() {
        let manager = EnvironmentManager::new(Some("ws://127.0.0.1:8765".to_string()));

        assert!(manager.is_remote());
        assert_eq!(
            manager.exec_server_url().as_deref(),
            Some("ws://127.0.0.1:8765")
        );
        assert_eq!(manager.default_host_id(), DEFAULT_HOST_ALIAS);
    }

    #[test]
    fn environment_manager_registers_ssh_hosts() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        manager
            .register_ssh_host("host-a", "host-a.example")
            .expect("register ssh host");
        manager
            .set_default_host("host-a")
            .expect("set default host");

        assert_eq!(manager.default_host_id(), "host-a");
        assert_eq!(
            manager.registered_host_ids(),
            vec!["host-a".to_string(), LOCAL_HOST_ID.to_string()]
        );
        assert!(manager.is_remote());
        assert_eq!(manager.exec_server_url(), None);
    }

    #[test]
    fn parse_ssh_hosts_env_accepts_aliases() {
        assert_eq!(
            parse_ssh_hosts_env("host-a, build=build-host, staging:staging-host"),
            vec![
                HostConfig::ssh("host-a", "host-a"),
                HostConfig::ssh("build", "build-host"),
                HostConfig::ssh("staging", "staging-host"),
            ]
        );
    }

    #[tokio::test]
    async fn environment_manager_current_caches_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager.current().await.expect("get current environment");
        let second = manager.current().await.expect("get current environment");

        let first = first.expect("local environment");
        let second = second.expect("local environment");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn environment_manager_current_for_host_caches_per_host() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let default = manager
            .current_for_host(DEFAULT_HOST_ALIAS)
            .await
            .expect("get default environment")
            .expect("default environment");
        let local = manager
            .current_for_host(LOCAL_HOST_ID)
            .await
            .expect("get local environment")
            .expect("local environment");

        assert!(Arc::ptr_eq(&default, &local));
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_current_environment() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert!(
            manager
                .current()
                .await
                .expect("get current environment")
                .is_none()
        );
    }

    #[tokio::test]
    async fn default_environment_has_ready_local_executor() {
        let environment = Environment::default();

        let response = environment
            .get_exec_backend()
            .start(crate::ExecParams {
                process_id: ProcessId::from("default-env-proc"),
                argv: vec!["true".to_string()],
                cwd: std::env::current_dir().expect("read current dir"),
                env: Default::default(),
                tty: false,
                arg0: None,
            })
            .await
            .expect("start process");

        assert_eq!(response.process.process_id().as_str(), "default-env-proc");
    }

    #[tokio::test]
    #[ignore]
    async fn ssh_environment_manager_bootstraps_configured_host() {
        let Ok(host) = std::env::var("CODEX_TEST_SSH_HOST") else {
            eprintln!("set CODEX_TEST_SSH_HOST to an SSH config host to run this smoke test");
            return;
        };
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);
        manager
            .register_ssh_host(&host, &host)
            .expect("register ssh host");
        manager.set_default_host(&host).expect("set default host");

        let environment = manager
            .current()
            .await
            .expect("bootstrap ssh environment")
            .expect("ssh environment");
        let cwd = environment
            .default_cwd()
            .expect("ssh bootstrap should report cwd")
            .to_path_buf();
        let response = environment
            .get_exec_backend()
            .start(crate::ExecParams {
                process_id: ProcessId::from("ssh-env-proc"),
                argv: vec![
                    environment.default_shell().unwrap_or("/bin/sh").to_string(),
                    "-lc".to_string(),
                    "printf remote:%s \"$(uname -s)\"".to_string(),
                ],
                cwd,
                env: Default::default(),
                tty: false,
                arg0: None,
            })
            .await
            .expect("start remote process");

        assert_eq!(response.process.process_id().as_str(), "ssh-env-proc");
        let output = response
            .process
            .read(
                /*after_seq*/ None,
                /*max_bytes*/ Some(1024),
                /*wait_ms*/ Some(5000),
            )
            .await
            .expect("read remote process");
        let text = output
            .chunks
            .iter()
            .map(|chunk| String::from_utf8_lossy(&chunk.chunk.0).to_string())
            .collect::<String>();
        assert!(
            text.starts_with("remote:"),
            "expected remote uname output, got {text:?}"
        );
    }

    #[test]
    fn env_var_names_are_stable() {
        assert_eq!(
            CODEX_EXEC_SERVER_SSH_HOSTS_ENV_VAR,
            "CODEX_EXEC_SERVER_SSH_HOSTS"
        );
        assert_eq!(
            CODEX_EXEC_SERVER_DEFAULT_HOST_ENV_VAR,
            "CODEX_EXEC_SERVER_DEFAULT_HOST"
        );
    }
}
