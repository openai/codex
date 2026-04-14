#[path = "file_system.rs"]
mod file_system;
#[path = "process_id.rs"]
mod process_id;
#[path = "protocol.rs"]
mod protocol;

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::OnceCell;
use tokio::sync::watch;

pub use codex_app_server_protocol::FsCopyParams;
pub use codex_app_server_protocol::FsCopyResponse;
pub use codex_app_server_protocol::FsCreateDirectoryParams;
pub use codex_app_server_protocol::FsCreateDirectoryResponse;
pub use codex_app_server_protocol::FsGetMetadataParams;
pub use codex_app_server_protocol::FsGetMetadataResponse;
pub use codex_app_server_protocol::FsReadDirectoryParams;
pub use codex_app_server_protocol::FsReadDirectoryResponse;
pub use codex_app_server_protocol::FsReadFileParams;
pub use codex_app_server_protocol::FsReadFileResponse;
pub use codex_app_server_protocol::FsRemoveParams;
pub use codex_app_server_protocol::FsRemoveResponse;
pub use codex_app_server_protocol::FsWriteFileParams;
pub use codex_app_server_protocol::FsWriteFileResponse;
pub use file_system::CopyOptions;
pub use file_system::CreateDirectoryOptions;
pub use file_system::ExecutorFileSystem;
pub use file_system::FileMetadata;
pub use file_system::FileSystemResult;
pub use file_system::ReadDirectoryEntry;
pub use file_system::RemoveOptions;
pub use process_id::ProcessId;
pub use protocol::ExecClosedNotification;
pub use protocol::ExecExitedNotification;
pub use protocol::ExecOutputDeltaNotification;
pub use protocol::ExecOutputStream;
pub use protocol::ExecParams;
pub use protocol::ExecResponse;
pub use protocol::InitializeParams;
pub use protocol::InitializeResponse;
pub use protocol::ReadParams;
pub use protocol::ReadResponse;
pub use protocol::TerminateParams;
pub use protocol::TerminateResponse;
pub use protocol::WriteParams;
pub use protocol::WriteResponse;
pub use protocol::WriteStatus;

pub struct StartedExecProcess {
    pub process: Arc<dyn ExecProcess>,
}

#[async_trait]
pub trait ExecProcess: Send + Sync {
    fn process_id(&self) -> &ProcessId;

    fn subscribe_wake(&self) -> watch::Receiver<u64>;

    async fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError>;

    async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError>;

    async fn terminate(&self) -> Result<(), ExecServerError>;
}

#[async_trait]
pub trait ExecBackend: Send + Sync {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError>;
}

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";
pub const DEFAULT_LISTEN_URL: &str = "ws://127.0.0.1:0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecServerClientConnectOptions {
    pub client_name: String,
    pub initialize_timeout: std::time::Duration,
}

impl Default for ExecServerClientConnectOptions {
    fn default() -> Self {
        Self {
            client_name: "codex-core".to_string(),
            initialize_timeout: std::time::Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExecServerConnectArgs {
    pub websocket_url: String,
    pub client_name: String,
    pub connect_timeout: std::time::Duration,
    pub initialize_timeout: std::time::Duration,
}

impl RemoteExecServerConnectArgs {
    pub fn new(websocket_url: String, client_name: String) -> Self {
        Self {
            websocket_url,
            client_name,
            connect_timeout: std::time::Duration::from_secs(10),
            initialize_timeout: std::time::Duration::from_secs(10),
        }
    }
}

impl From<RemoteExecServerConnectArgs> for ExecServerClientConnectOptions {
    fn from(value: RemoteExecServerConnectArgs) -> Self {
        Self {
            client_name: value.client_name,
            initialize_timeout: value.initialize_timeout,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecServerError {
    #[error("exec-server is unavailable on wasm32")]
    Unsupported,
    #[error("exec-server transport closed")]
    Closed,
    #[error("failed to serialize or deserialize exec-server JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("exec-server protocol error: {0}")]
    Protocol(String),
    #[error("exec-server rejected request ({code}): {message}")]
    Server { code: i64, message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ExecServerListenUrlParseError {
    #[error("exec-server listen URLs are unavailable on wasm32: {0}")]
    UnsupportedListenUrl(String),
}

#[derive(Clone, Default)]
pub struct ExecServerClient;

impl ExecServerClient {
    pub async fn connect_websocket(
        _args: RemoteExecServerConnectArgs,
    ) -> Result<Self, ExecServerError> {
        Err(ExecServerError::Unsupported)
    }

    pub async fn initialize(
        &self,
        _options: ExecServerClientConnectOptions,
    ) -> Result<InitializeResponse, ExecServerError> {
        Err(ExecServerError::Unsupported)
    }

    pub async fn exec(&self, _params: ExecParams) -> Result<ExecResponse, ExecServerError> {
        Err(ExecServerError::Unsupported)
    }

    pub async fn read(&self, _params: ReadParams) -> Result<ReadResponse, ExecServerError> {
        Err(ExecServerError::Unsupported)
    }

    pub async fn write(
        &self,
        _process_id: &ProcessId,
        _chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        Err(ExecServerError::Unsupported)
    }

    pub async fn terminate(
        &self,
        _process_id: &ProcessId,
    ) -> Result<TerminateResponse, ExecServerError> {
        Err(ExecServerError::Unsupported)
    }
}

pub trait ExecutorEnvironment: Send + Sync {
    fn get_exec_backend(&self) -> Arc<dyn ExecBackend>;
}

#[derive(Default)]
pub struct EnvironmentManager {
    exec_server_url: Option<String>,
    current_environment: OnceCell<Arc<Environment>>,
}

impl EnvironmentManager {
    pub fn new(exec_server_url: Option<String>) -> Self {
        Self {
            exec_server_url: normalize_exec_server_url(exec_server_url),
            current_environment: OnceCell::new(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok())
    }

    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub async fn current(&self) -> Result<Arc<Environment>, ExecServerError> {
        self.current_environment
            .get_or_init(|| async { Arc::new(Environment::default()) })
            .await;
        self.current_environment
            .get()
            .cloned()
            .ok_or(ExecServerError::Unsupported)
    }
}

#[derive(Clone)]
pub struct Environment {
    exec_server_url: Option<String>,
    exec_backend: Arc<dyn ExecBackend>,
    filesystem: Arc<dyn ExecutorFileSystem>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            exec_server_url: None,
            exec_backend: Arc::new(NoopExecBackend),
            filesystem: Arc::new(UnsupportedFileSystem),
        }
    }
}

impl Environment {
    pub async fn create(exec_server_url: Option<String>) -> Result<Self, ExecServerError> {
        Ok(Self {
            exec_server_url: normalize_exec_server_url(exec_server_url),
            ..Self::default()
        })
    }

    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        Arc::clone(&self.filesystem)
    }
}

impl ExecutorEnvironment for Environment {
    fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }
}

#[derive(Debug, Default)]
struct NoopExecBackend;

#[async_trait]
impl ExecBackend for NoopExecBackend {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError> {
        Ok(StartedExecProcess {
            process: Arc::new(NoopExecProcess::new(params.process_id)),
        })
    }
}

#[derive(Debug)]
struct NoopExecProcess {
    process_id: ProcessId,
    wake_tx: watch::Sender<u64>,
}

impl NoopExecProcess {
    fn new(process_id: ProcessId) -> Self {
        let (wake_tx, _wake_rx) = watch::channel(0);
        Self {
            process_id,
            wake_tx,
        }
    }
}

#[async_trait]
impl ExecProcess for NoopExecProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    fn subscribe_wake(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    async fn read(
        &self,
        _after_seq: Option<u64>,
        _max_bytes: Option<usize>,
        _wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError> {
        Ok(ReadResponse {
            chunks: Vec::new(),
            next_seq: 0,
            exited: true,
            exit_code: Some(1),
            closed: true,
            failure: Some("exec-server is unavailable on wasm32".to_string()),
        })
    }

    async fn write(&self, _chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError> {
        Ok(WriteResponse {
            status: WriteStatus::UnknownProcess,
        })
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct UnsupportedFileSystem;

#[async_trait]
impl ExecutorFileSystem for UnsupportedFileSystem {
    async fn read_file(
        &self,
        _path: &codex_utils_absolute_path::AbsolutePathBuf,
    ) -> FileSystemResult<Vec<u8>> {
        Err(unsupported_io_error())
    }

    async fn write_file(
        &self,
        _path: &codex_utils_absolute_path::AbsolutePathBuf,
        _contents: Vec<u8>,
    ) -> FileSystemResult<()> {
        Err(unsupported_io_error())
    }

    async fn create_directory(
        &self,
        _path: &codex_utils_absolute_path::AbsolutePathBuf,
        _options: CreateDirectoryOptions,
    ) -> FileSystemResult<()> {
        Err(unsupported_io_error())
    }

    async fn get_metadata(
        &self,
        _path: &codex_utils_absolute_path::AbsolutePathBuf,
    ) -> FileSystemResult<FileMetadata> {
        Err(unsupported_io_error())
    }

    async fn read_directory(
        &self,
        _path: &codex_utils_absolute_path::AbsolutePathBuf,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        Err(unsupported_io_error())
    }

    async fn remove(
        &self,
        _path: &codex_utils_absolute_path::AbsolutePathBuf,
        _options: RemoveOptions,
    ) -> FileSystemResult<()> {
        Err(unsupported_io_error())
    }

    async fn copy(
        &self,
        _source_path: &codex_utils_absolute_path::AbsolutePathBuf,
        _destination_path: &codex_utils_absolute_path::AbsolutePathBuf,
        _options: CopyOptions,
    ) -> FileSystemResult<()> {
        Err(unsupported_io_error())
    }
}

pub async fn run_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Err(Box::new(ExecServerError::Unsupported))
}

pub async fn run_main_with_listen_url(
    _listen_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Err(Box::new(ExecServerError::Unsupported))
}

fn normalize_exec_server_url(exec_server_url: Option<String>) -> Option<String> {
    exec_server_url.and_then(|url| {
        let url = url.trim();
        (!url.is_empty()).then(|| url.to_string())
    })
}

fn unsupported_io_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "exec-server filesystem is unavailable on wasm32",
    )
}
