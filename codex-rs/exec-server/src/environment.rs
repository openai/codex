use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::sync::OnceCell;

use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecServerClient;
use crate::ExecServerError;
use crate::ReadDirectoryEntry;
use crate::RemoteExecServerConnectArgs;
use crate::RemoveOptions;
use crate::StartedExecProcess;
use crate::file_system::ExecutorFileSystem;
use crate::file_system::FileMetadata;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;
use crate::protocol::ExecParams;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

/// Describes where execution and filesystem operations for a session come from.
///
/// `CODEX_EXEC_SERVER_URL=none` maps to [`EnvironmentMode::Disabled`] so callers
/// can distinguish "intentionally unavailable" from "use the local executor".
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EnvironmentMode {
    /// Run against the local process and filesystem implementations.
    Local,
    /// Run against a remote exec-server endpoint.
    Remote { exec_server_url: String },
    /// Disable executor-backed capabilities for this session entirely.
    Disabled,
}

/// Feature-style view of what a selected environment supports.
///
/// Tool building and runtime guards should prefer these booleans over
/// re-interpreting environment URLs so future environment modes can evolve
/// without touching every call site.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct EnvironmentCapabilities {
    exec_enabled: bool,
    filesystem_enabled: bool,
}

impl EnvironmentCapabilities {
    /// Creates a capability set for a concrete environment mode.
    pub fn new(exec_enabled: bool, filesystem_enabled: bool) -> Self {
        Self {
            exec_enabled,
            filesystem_enabled,
        }
    }

    /// Returns whether process execution should be exposed.
    pub fn exec_enabled(self) -> bool {
        self.exec_enabled
    }

    /// Returns whether filesystem-backed tools should be exposed.
    pub fn filesystem_enabled(self) -> bool {
        self.filesystem_enabled
    }
}

impl EnvironmentMode {
    /// Returns the remote exec-server URL when this mode is remote.
    pub fn exec_server_url(&self) -> Option<&str> {
        match self {
            Self::Local | Self::Disabled => None,
            Self::Remote { exec_server_url } => Some(exec_server_url.as_str()),
        }
    }

    /// Returns whether this mode uses a remote exec-server.
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }

    /// Returns whether this mode disables environment-backed APIs.
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    /// Returns the tool/runtime capabilities implied by this mode.
    pub fn capabilities(&self) -> EnvironmentCapabilities {
        match self {
            Self::Local | Self::Remote { .. } => EnvironmentCapabilities::new(
                /*exec_enabled*/ true, /*filesystem_enabled*/ true,
            ),
            Self::Disabled => EnvironmentCapabilities::new(
                /*exec_enabled*/ false, /*filesystem_enabled*/ false,
            ),
        }
    }

    fn from_exec_server_url(exec_server_url: Option<String>) -> Self {
        match exec_server_url.as_deref().map(str::trim) {
            None | Some("") => Self::Local,
            Some(url) if url.eq_ignore_ascii_case("none") => Self::Disabled,
            Some(url) => Self::Remote {
                exec_server_url: url.to_string(),
            },
        }
    }
}

/// Provides access to the exec backend for a selected environment.
///
/// Implementations are expected to return the backend that matches the current
/// environment mode, including disabled backends that reject execution.
pub trait ExecutorEnvironment: Send + Sync {
    fn get_exec_backend(&self) -> Arc<dyn ExecBackend>;
}

/// Lazily creates and caches the active environment for a session.
///
/// The manager keeps the session's environment mode stable so subagents and
/// follow-up turns preserve explicit `Disabled` semantics.
#[derive(Debug)]
pub struct EnvironmentManager {
    mode: EnvironmentMode,
    current_environment: OnceCell<Arc<Environment>>,
}

impl Default for EnvironmentManager {
    fn default() -> Self {
        Self::new(/*exec_server_url*/ None)
    }
}

impl EnvironmentManager {
    /// Builds a manager from the raw `CODEX_EXEC_SERVER_URL` value.
    pub fn new(exec_server_url: Option<String>) -> Self {
        Self::from_mode(EnvironmentMode::from_exec_server_url(exec_server_url))
    }

    /// Builds a manager from an already-parsed environment mode.
    pub fn from_mode(mode: EnvironmentMode) -> Self {
        Self {
            mode,
            current_environment: OnceCell::new(),
        }
    }

    /// Builds a manager from process environment variables.
    pub fn from_env() -> Self {
        Self::new(std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok())
    }

    /// Returns the stable mode for this manager.
    pub fn mode(&self) -> &EnvironmentMode {
        &self.mode
    }

    /// Returns the remote exec-server URL when one is configured.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.mode.exec_server_url()
    }

    /// Returns the cached environment, creating it on first access.
    pub async fn current(&self) -> Result<Arc<Environment>, ExecServerError> {
        self.current_environment
            .get_or_try_init(|| async {
                Ok(Arc::new(
                    Environment::create_for_mode(self.mode.clone()).await?,
                ))
            })
            .await
            .map(Arc::clone)
    }
}

/// Concrete execution/filesystem environment selected for a session.
///
/// This bundles the chosen mode together with the corresponding exec backend
/// and remote client, if any.
#[derive(Clone)]
pub struct Environment {
    mode: EnvironmentMode,
    remote_exec_server_client: Option<ExecServerClient>,
    exec_backend: Arc<dyn ExecBackend>,
}

impl Default for Environment {
    fn default() -> Self {
        let local_process = LocalProcess::default();
        if let Err(err) = local_process.initialize() {
            panic!("default local process initialization should succeed: {err:?}");
        }
        if let Err(err) = local_process.initialized() {
            panic!("default local process should accept initialized notification: {err}");
        }

        Self {
            mode: EnvironmentMode::Local,
            remote_exec_server_client: None,
            exec_backend: Arc::new(local_process),
        }
    }
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl Environment {
    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value.
    pub async fn create(exec_server_url: Option<String>) -> Result<Self, ExecServerError> {
        Self::create_for_mode(EnvironmentMode::from_exec_server_url(exec_server_url)).await
    }

    /// Builds an environment for an explicit mode.
    pub async fn create_for_mode(mode: EnvironmentMode) -> Result<Self, ExecServerError> {
        let remote_exec_server_client = if let EnvironmentMode::Remote { exec_server_url } = &mode {
            Some(
                ExecServerClient::connect_websocket(RemoteExecServerConnectArgs {
                    websocket_url: exec_server_url.clone(),
                    client_name: "codex-environment".to_string(),
                    connect_timeout: std::time::Duration::from_secs(5),
                    initialize_timeout: std::time::Duration::from_secs(5),
                })
                .await?,
            )
        } else {
            None
        };

        let exec_backend: Arc<dyn ExecBackend> = match (&mode, remote_exec_server_client.clone()) {
            (EnvironmentMode::Remote { .. }, Some(client)) => Arc::new(RemoteProcess::new(client)),
            (EnvironmentMode::Remote { .. }, None) => {
                return Err(ExecServerError::Protocol(
                    "remote mode should have an exec-server client".to_string(),
                ));
            }
            EnvironmentMode::Local => {
                let local_process = LocalProcess::default();
                local_process
                    .initialize()
                    .map_err(|err| ExecServerError::Protocol(err.message))?;
                local_process
                    .initialized()
                    .map_err(ExecServerError::Protocol)?;
                Arc::new(local_process)
            }
            EnvironmentMode::Disabled => Arc::new(DisabledExecBackend),
        };

        Ok(Self {
            mode,
            remote_exec_server_client,
            exec_backend,
        })
    }

    /// Returns the selected mode for this environment.
    pub fn mode(&self) -> &EnvironmentMode {
        &self.mode
    }

    /// Returns the capabilities exposed by this environment.
    pub fn capabilities(&self) -> EnvironmentCapabilities {
        self.mode.capabilities()
    }

    /// Returns whether process execution is available.
    pub fn exec_enabled(&self) -> bool {
        self.capabilities().exec_enabled()
    }

    /// Returns whether filesystem-backed operations are available.
    pub fn filesystem_enabled(&self) -> bool {
        self.capabilities().filesystem_enabled()
    }

    /// Returns the remote exec-server URL when this environment is remote.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.mode.exec_server_url()
    }

    pub fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        match (&self.mode, self.remote_exec_server_client.clone()) {
            (EnvironmentMode::Remote { .. }, Some(client)) => {
                Arc::new(RemoteFileSystem::new(client))
            }
            (EnvironmentMode::Remote { .. }, None) => {
                panic!("remote mode should have an exec-server client")
            }
            (EnvironmentMode::Local, _) => Arc::new(LocalFileSystem),
            (EnvironmentMode::Disabled, _) => Arc::new(DisabledFileSystem),
        }
    }
}

#[derive(Debug)]
struct DisabledExecBackend;

#[async_trait]
impl ExecBackend for DisabledExecBackend {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError> {
        Err(ExecServerError::Protocol(format!(
            "environment is disabled; cannot start process `{}`",
            params.process_id
        )))
    }
}

#[derive(Debug)]
struct DisabledFileSystem;

#[async_trait]
impl ExecutorFileSystem for DisabledFileSystem {
    async fn read_file(&self, path: &AbsolutePathBuf) -> io::Result<Vec<u8>> {
        Err(disabled_filesystem_error(path))
    }

    async fn write_file(&self, path: &AbsolutePathBuf, _contents: Vec<u8>) -> io::Result<()> {
        Err(disabled_filesystem_error(path))
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        _options: CreateDirectoryOptions,
    ) -> io::Result<()> {
        Err(disabled_filesystem_error(path))
    }

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> io::Result<FileMetadata> {
        Err(disabled_filesystem_error(path))
    }

    async fn read_directory(&self, path: &AbsolutePathBuf) -> io::Result<Vec<ReadDirectoryEntry>> {
        Err(disabled_filesystem_error(path))
    }

    async fn remove(&self, path: &AbsolutePathBuf, _options: RemoveOptions) -> io::Result<()> {
        Err(disabled_filesystem_error(path))
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        _destination_path: &AbsolutePathBuf,
        _options: CopyOptions,
    ) -> io::Result<()> {
        Err(disabled_filesystem_error(source_path))
    }
}

fn disabled_filesystem_error(path: &AbsolutePathBuf) -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        format!(
            "environment is disabled; filesystem access is unavailable for `{}`",
            path.display()
        ),
    )
}

impl ExecutorEnvironment for Environment {
    fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;

    use super::Environment;
    use super::EnvironmentManager;
    use super::EnvironmentMode;
    use crate::ProcessId;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn create_local_environment_does_not_connect() {
        let environment = Environment::create_for_mode(EnvironmentMode::Local)
            .await
            .expect("create environment");

        assert_eq!(environment.exec_server_url(), None);
        assert!(environment.remote_exec_server_client.is_none());
    }

    #[test]
    fn environment_manager_normalizes_empty_url() {
        let manager = EnvironmentManager::new(Some(String::new()));

        assert_eq!(manager.mode(), &EnvironmentMode::Local);
        assert_eq!(manager.exec_server_url(), None);
    }

    #[test]
    fn environment_manager_treats_none_value_as_disabled() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert_eq!(manager.mode(), &EnvironmentMode::Disabled);
        assert_eq!(manager.exec_server_url(), None);
    }

    #[test]
    fn disabled_mode_capabilities_are_off() {
        let capabilities = EnvironmentMode::Disabled.capabilities();

        assert!(!capabilities.exec_enabled());
        assert!(!capabilities.filesystem_enabled());
    }

    #[tokio::test]
    async fn environment_manager_current_caches_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager.current().await.expect("get current environment");
        let second = manager.current().await.expect("get current environment");

        assert!(Arc::ptr_eq(&first, &second));
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
    async fn disabled_environment_rejects_exec_and_filesystem_access() {
        let environment = Environment::create_for_mode(EnvironmentMode::Disabled)
            .await
            .expect("create disabled environment");

        let exec_error = match environment
            .get_exec_backend()
            .start(crate::ExecParams {
                process_id: ProcessId::from("disabled-proc"),
                argv: vec!["true".to_string()],
                cwd: std::env::current_dir().expect("read current dir"),
                env: Default::default(),
                tty: false,
                arg0: None,
            })
            .await
        {
            Ok(_) => panic!("disabled environment should reject exec"),
            Err(err) => err,
        };
        assert_eq!(
            exec_error.to_string(),
            "exec-server protocol error: environment is disabled; cannot start process `disabled-proc`"
        );

        let path =
            codex_utils_absolute_path::AbsolutePathBuf::try_from(std::env::temp_dir().as_path())
                .expect("temp dir");
        let fs_error = environment
            .get_filesystem()
            .get_metadata(&path)
            .await
            .expect_err("disabled environment should reject filesystem access");
        assert_eq!(fs_error.kind(), io::ErrorKind::Unsupported);
    }
}
