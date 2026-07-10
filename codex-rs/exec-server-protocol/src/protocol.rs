use std::collections::HashMap;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_file_system::FileSystemSandboxContext;
pub use codex_file_system::WalkOptions;
pub use codex_file_system::WalkOutcome;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_network_proxy::NetworkDecision;
use codex_network_proxy::NetworkPolicyRequest;
use codex_network_proxy::RemoteNetworkProxyLaunchConfig;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_shell_command::shell_detect::DetectedShell;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;

use crate::ProcessId;

pub const INITIALIZE_METHOD: &str = "initialize";
pub const INITIALIZED_METHOD: &str = "initialized";
pub const EXEC_METHOD: &str = "process/start";
pub const EXEC_READ_METHOD: &str = "process/read";
pub const EXEC_WRITE_METHOD: &str = "process/write";
pub const EXEC_SIGNAL_METHOD: &str = "process/signal";
pub const EXEC_TERMINATE_METHOD: &str = "process/terminate";
pub const EXEC_OUTPUT_DELTA_METHOD: &str = "process/output";
pub const EXEC_EXITED_METHOD: &str = "process/exited";
pub const EXEC_CLOSED_METHOD: &str = "process/closed";
pub const NETWORK_POLICY_REQUEST_METHOD: &str = "network/policyRequest";
pub const NETWORK_POLICY_DECISION_METHOD: &str = "network/policyDecision";
pub const NETWORK_POLICY_DECISION_TIMEOUT: Duration = Duration::from_secs(30);
pub const ENVIRONMENT_INFO_METHOD: &str = "environment/info";
pub const FS_READ_FILE_METHOD: &str = "fs/readFile";
pub const FS_OPEN_METHOD: &str = "fs/open";
pub const FS_READ_BLOCK_METHOD: &str = "fs/readBlock";
pub const FS_CLOSE_METHOD: &str = "fs/close";
pub const FS_WRITE_FILE_METHOD: &str = "fs/writeFile";
pub const FS_CREATE_DIRECTORY_METHOD: &str = "fs/createDirectory";
pub const FS_GET_METADATA_METHOD: &str = "fs/getMetadata";
pub const FS_CANONICALIZE_METHOD: &str = "fs/canonicalize";
pub const FS_READ_DIRECTORY_METHOD: &str = "fs/readDirectory";
pub const FS_WALK_METHOD: &str = "fs/walk";
pub const FS_REMOVE_METHOD: &str = "fs/remove";
pub const FS_COPY_METHOD: &str = "fs/copy";
/// JSON-RPC request method for executor-side HTTP requests.
pub const HTTP_REQUEST_METHOD: &str = "http/request";
/// JSON-RPC notification method for streamed executor HTTP response bodies.
pub const HTTP_REQUEST_BODY_DELTA_METHOD: &str = "http/request/bodyDelta";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ByteChunk(#[serde(with = "base64_bytes")] pub Vec<u8>);

impl ByteChunk {
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for ByteChunk {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client_name: String,
    #[serde(default)]
    pub resume_session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub session_id: String,
}

/// Information about an execution/filesystem environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentInfo {
    pub shell: ShellInfo,
    /// Working directory inherited by the exec-server process.
    #[serde(default)]
    pub cwd: Option<PathUri>,
}

impl EnvironmentInfo {
    /// Returns information about the current local exec-server process.
    pub fn local() -> Self {
        Self {
            shell: codex_shell_command::shell_detect::default_user_shell().into(),
            cwd: std::env::current_dir()
                .ok()
                .and_then(|cwd| PathUri::from_host_native_path(cwd).ok()),
        }
    }
}

/// Shell detected for an execution/filesystem environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellInfo {
    /// Stable shell name, for example `zsh`, `bash`, `powershell`, `sh`, or `cmd`.
    pub name: String,
    /// Target-native shell executable path or command name. Fallbacks such as `cmd.exe` need not
    /// be absolute, so this is not a [`PathUri`].
    pub path: String,
}

impl From<DetectedShell> for ShellInfo {
    fn from(shell: DetectedShell) -> Self {
        Self {
            name: shell.name().to_string(),
            path: shell.shell_path.to_string_lossy().into_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecParams {
    /// Client-chosen logical process handle scoped to this connection/session.
    /// This is a protocol key, not an OS pid.
    pub process_id: ProcessId,
    pub argv: Vec<String>,
    /// Working directory URI, interpreted using the exec-server host's path rules at launch time.
    pub cwd: PathUri,
    #[serde(default)]
    pub env_policy: Option<ExecEnvPolicy>,
    pub env: HashMap<String, String>,
    pub tty: bool,
    /// Keep non-tty stdin writable through `process/write`.
    #[serde(default)]
    pub pipe_stdin: bool,
    /// Optional process-visible argv0 override. Values such as `codex-linux-sandbox` are command
    /// names rather than paths, so this is not a [`PathUri`].
    pub arg0: Option<String>,
    /// Portable sandbox intent. Concrete wrapper argv is resolved by the exec-server.
    #[serde(default)]
    pub sandbox: Option<FileSystemSandboxContext>,
    /// Managed-network intent. This serializes to the legacy `enforceManagedNetwork`,
    /// `managedNetwork`, and `networkProxy` fields, but Rust callers cannot set those
    /// independently.
    #[serde(flatten)]
    pub managed_network: ExecManagedNetwork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecManagedNetwork {
    mode: ExecManagedNetworkMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExecManagedNetworkMode {
    Disabled,
    EnforceWithoutProxy,
    ExistingProxy(ManagedNetworkSandboxContext),
    LaunchProxy(Box<RemoteNetworkProxyLaunchConfig>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecManagedNetworkWire {
    #[serde(default)]
    enforce_managed_network: bool,
    #[serde(default)]
    managed_network: Option<ManagedNetworkSandboxContext>,
    #[serde(default)]
    network_proxy: Option<RemoteNetworkProxyLaunchConfig>,
}

impl ExecManagedNetwork {
    pub fn disabled() -> Self {
        Self {
            mode: ExecManagedNetworkMode::Disabled,
        }
    }

    pub fn enforce_without_proxy() -> Self {
        Self {
            mode: ExecManagedNetworkMode::EnforceWithoutProxy,
        }
    }

    pub fn existing_proxy(managed_network: ManagedNetworkSandboxContext) -> Self {
        Self {
            mode: ExecManagedNetworkMode::ExistingProxy(managed_network),
        }
    }

    pub fn launch_proxy(network_proxy: RemoteNetworkProxyLaunchConfig) -> Self {
        Self {
            mode: ExecManagedNetworkMode::LaunchProxy(Box::new(network_proxy)),
        }
    }

    pub fn from_parts(
        enforce_managed_network: bool,
        managed_network: Option<ManagedNetworkSandboxContext>,
        network_proxy: Option<RemoteNetworkProxyLaunchConfig>,
    ) -> Result<Self, String> {
        Self::try_from(ExecManagedNetworkWire {
            enforce_managed_network,
            managed_network,
            network_proxy,
        })
    }

    pub fn enforce(&self) -> bool {
        !matches!(self.mode, ExecManagedNetworkMode::Disabled)
    }

    pub fn sandbox_context(&self) -> Option<&ManagedNetworkSandboxContext> {
        match &self.mode {
            ExecManagedNetworkMode::Disabled
            | ExecManagedNetworkMode::EnforceWithoutProxy
            | ExecManagedNetworkMode::LaunchProxy(_) => None,
            ExecManagedNetworkMode::ExistingProxy(managed_network) => Some(managed_network),
        }
    }

    pub fn launch_config(&self) -> Option<&RemoteNetworkProxyLaunchConfig> {
        match &self.mode {
            ExecManagedNetworkMode::Disabled
            | ExecManagedNetworkMode::EnforceWithoutProxy
            | ExecManagedNetworkMode::ExistingProxy(_) => None,
            ExecManagedNetworkMode::LaunchProxy(network_proxy) => Some(network_proxy.as_ref()),
        }
    }
}

impl Default for ExecManagedNetwork {
    fn default() -> Self {
        Self::disabled()
    }
}

impl TryFrom<ExecManagedNetworkWire> for ExecManagedNetwork {
    type Error = String;

    fn try_from(wire: ExecManagedNetworkWire) -> Result<Self, Self::Error> {
        match (wire.enforce_managed_network, wire.managed_network, wire.network_proxy) {
            (_, Some(_), Some(_)) => Err(
                "`managedNetwork` sandbox facts and `networkProxy` launch config are mutually exclusive"
                    .to_string(),
            ),
            (_, None, Some(network_proxy)) => Ok(Self::launch_proxy(network_proxy)),
            (_, Some(managed_network), None) => Ok(Self::existing_proxy(managed_network)),
            (true, None, None) => Ok(Self::enforce_without_proxy()),
            (false, None, None) => Ok(Self::disabled()),
        }
    }
}

impl From<&ExecManagedNetwork> for ExecManagedNetworkWire {
    fn from(managed_network: &ExecManagedNetwork) -> Self {
        match &managed_network.mode {
            ExecManagedNetworkMode::Disabled => Self {
                enforce_managed_network: false,
                managed_network: None,
                network_proxy: None,
            },
            ExecManagedNetworkMode::EnforceWithoutProxy => Self {
                enforce_managed_network: true,
                managed_network: None,
                network_proxy: None,
            },
            ExecManagedNetworkMode::ExistingProxy(managed_network) => Self {
                enforce_managed_network: true,
                managed_network: Some(managed_network.clone()),
                network_proxy: None,
            },
            ExecManagedNetworkMode::LaunchProxy(network_proxy) => Self {
                enforce_managed_network: true,
                managed_network: None,
                network_proxy: Some((**network_proxy).clone()),
            },
        }
    }
}

impl Serialize for ExecManagedNetwork {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ExecManagedNetworkWire::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExecManagedNetwork {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ExecManagedNetworkWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecEnvPolicy {
    pub inherit: ShellEnvironmentPolicyInherit,
    pub ignore_default_excludes: bool,
    pub exclude: Vec<String>,
    pub r#set: HashMap<String, String>,
    pub include_only: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResponse {
    pub process_id: ProcessId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyRequestNotification {
    pub request_id: String,
    pub process_id: ProcessId,
    pub request: NetworkPolicyRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyDecisionNotification {
    pub request_id: String,
    pub process_id: ProcessId,
    pub decision: NetworkDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadParams {
    pub process_id: ProcessId,
    pub after_seq: Option<u64>,
    pub max_bytes: Option<usize>,
    pub wait_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOutputChunk {
    pub seq: u64,
    pub stream: ExecOutputStream,
    pub chunk: ByteChunk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResponse {
    pub chunks: Vec<ProcessOutputChunk>,
    pub next_seq: u64,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub closed: bool,
    pub failure: Option<String>,
    /// Whether the executor classified the process failure as a sandbox denial.
    #[serde(default)]
    pub sandbox_denied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteParams {
    pub process_id: ProcessId,
    pub chunk: ByteChunk,
    pub write_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WriteStatus {
    Accepted,
    UnknownProcess,
    StdinClosed,
    Starting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResponse {
    pub status: WriteStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProcessSignal {
    Interrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalParams {
    pub process_id: ProcessId,
    pub signal: ProcessSignal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminateParams {
    pub process_id: ProcessId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminateResponse {
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadFileParams {
    pub path: PathUri,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadFileResponse {
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsOpenParams {
    pub handle_id: String,
    pub path: PathUri,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsOpenResponse {
    pub handle_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadBlockParams {
    pub handle_id: String,
    pub offset: u64,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadBlockResponse {
    pub chunk: ByteChunk,
    pub eof: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCloseParams {
    pub handle_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCloseResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteFileParams {
    pub path: PathUri,
    pub data_base64: String,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteFileResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCreateDirectoryParams {
    pub path: PathUri,
    pub recursive: Option<bool>,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCreateDirectoryResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsGetMetadataParams {
    pub path: PathUri,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsGetMetadataResponse {
    pub is_directory: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub created_at_ms: i64,
    pub modified_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCanonicalizeParams {
    pub path: PathUri,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCanonicalizeResponse {
    pub path: PathUri,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryParams {
    pub path: PathUri,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryEntry {
    pub file_name: String,
    pub is_directory: bool,
    pub is_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadDirectoryResponse {
    pub entries: Vec<FsReadDirectoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWalkParams {
    pub path: PathUri,
    pub options: WalkOptions,
    pub sandbox: Option<FileSystemSandboxContext>,
}

pub type FsWalkResponse = WalkOutcome;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsRemoveParams {
    pub path: PathUri,
    pub recursive: Option<bool>,
    pub force: Option<bool>,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsRemoveResponse {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCopyParams {
    pub source_path: PathUri,
    pub destination_path: PathUri,
    pub recursive: bool,
    pub sandbox: Option<FileSystemSandboxContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCopyResponse {}

/// HTTP header represented in the executor protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpHeader {
    /// Header name as it appears on the HTTP wire.
    pub name: String,
    /// Header value after UTF-8 conversion.
    pub value: String,
}

/// Redirect behavior for an executor-side HTTP request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HttpRedirectPolicy {
    /// Follow redirects using the HTTP client's normal limits.
    #[default]
    Follow,
    /// Return the redirect response without following its location.
    Stop,
}

/// Executor-side HTTP request envelope.
///
/// This intentionally stays transport-shaped rather than MCP-shaped so callers
/// can use it for Streamable HTTP, OAuth discovery, and future executor-owned
/// HTTP probes without introducing one protocol method per higher-level use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestParams {
    /// HTTP method, for example `GET`, `POST`, or `DELETE`.
    pub method: String,
    /// Absolute `http://` or `https://` URL.
    pub url: String,
    /// Ordered request headers. Repeated header names are preserved.
    #[serde(default)]
    pub headers: Vec<HttpHeader>,
    /// Optional request body bytes.
    #[serde(default, rename = "bodyBase64")]
    pub body: Option<ByteChunk>,
    /// Request timeout in milliseconds.
    ///
    /// Omitted or `null` disables the timeout. A number applies that exact
    /// millisecond deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Whether the executor should follow HTTP redirects.
    #[serde(default)]
    pub redirect_policy: HttpRedirectPolicy,
    /// Caller-chosen stream id for `http/request/bodyDelta` notifications.
    ///
    /// The id must remain unique on a connection until the terminal body delta
    /// arrives, even if the caller stops reading the stream earlier. Buffered
    /// requests still send an id so callers can keep one consistent request
    /// envelope shape.
    pub request_id: String,
    /// Return after response headers and stream the response body as deltas.
    #[serde(default)]
    pub stream_response: bool,
}

/// HTTP response envelope returned from an executor `http/request` call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestResponse {
    /// Numeric HTTP response status code.
    pub status: u16,
    /// Ordered response headers. Repeated header names are preserved.
    pub headers: Vec<HttpHeader>,
    /// Buffered response body bytes. Empty when `streamResponse` is true.
    #[serde(rename = "bodyBase64")]
    pub body: ByteChunk,
}

/// Ordered response-body frame for `streamResponse` HTTP requests.
///
/// Headers are returned in the `http/request` response so the caller can choose
/// a parser immediately; body bytes then arrive on this notification stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestBodyDeltaNotification {
    /// Request id from the streamed `http/request` call.
    pub request_id: String,
    /// Monotonic one-based body frame sequence number.
    pub seq: u64,
    /// Response-body bytes carried by this frame.
    #[serde(rename = "deltaBase64")]
    pub delta: ByteChunk,
    /// Marks response-body EOF. No later deltas are expected for this request.
    #[serde(default)]
    pub done: bool,
    /// Terminal stream error. Set only on the final notification.
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecOutputStream {
    Stdout,
    Stderr,
    Pty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecOutputDeltaNotification {
    pub process_id: ProcessId,
    pub seq: u64,
    pub stream: ExecOutputStream,
    pub chunk: ByteChunk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecExitedNotification {
    pub process_id: ProcessId,
    pub seq: u64,
    pub exit_code: i32,
    #[serde(default)]
    pub sandbox_denied: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecClosedNotification {
    pub process_id: ProcessId,
    pub seq: u64,
}

mod base64_bytes {
    use super::BASE64_STANDARD;
    use base64::Engine as _;
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serializer;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64_STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        BASE64_STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::EnvironmentInfo;
    use super::ExecExitedNotification;
    use super::ExecManagedNetwork;
    use super::ExecParams;
    use super::FsReadFileParams;
    use super::HttpRequestParams;
    use super::ProcessId;
    use super::ShellInfo;
    use codex_file_system::FileSystemSandboxContext;
    use codex_network_proxy::ManagedNetworkSandboxContext;
    use codex_network_proxy::NetworkProxyAuditMetadata;
    use codex_network_proxy::NetworkProxyConfig;
    use codex_network_proxy::RemoteNetworkProxyConfig;
    use codex_network_proxy::RemoteNetworkProxyLaunchConfig;
    use codex_protocol::models::PermissionProfile;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    #[test]
    fn exec_params_serializes_proxy_launch_as_legacy_wire_fields() {
        let cwd =
            PathUri::from_host_native_path(std::env::current_dir().expect("current directory"))
                .expect("cwd URI");
        let params = ExecParams {
            process_id: ProcessId::from("managed-network"),
            argv: vec!["true".to_string()],
            cwd,
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            managed_network: ExecManagedNetwork::launch_proxy(
                RemoteNetworkProxyLaunchConfig::new(
                    RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig::default())
                        .expect("supported remote config"),
                )
                .with_audit_metadata(NetworkProxyAuditMetadata {
                    conversation_id: Some("conversation-1".to_string()),
                    ..NetworkProxyAuditMetadata::default()
                })
                .for_execution("remote".to_string(), "execution-1".to_string()),
            ),
        };

        let mut serialized = serde_json::to_value(&params).expect("serialize exec params");
        assert_eq!(serialized["enforceManagedNetwork"], serde_json::json!(true));
        assert_eq!(serialized["managedNetwork"], serde_json::json!(null));
        assert_eq!(
            serialized["networkProxy"]["auditMetadata"]["conversationId"],
            "conversation-1"
        );
        let round_trip: ExecParams =
            serde_json::from_value(serialized.clone()).expect("deserialize exec params");
        assert_eq!(round_trip, params);

        serialized
            .as_object_mut()
            .expect("exec params object")
            .remove("networkProxy");
        let legacy: ExecParams =
            serde_json::from_value(serialized).expect("deserialize legacy exec params");
        assert_eq!(
            legacy.managed_network,
            ExecManagedNetwork::enforce_without_proxy()
        );
    }

    #[test]
    fn exec_params_serializes_existing_proxy_sandbox_facts_as_legacy_wire_fields() {
        let cwd =
            PathUri::from_host_native_path(std::env::current_dir().expect("current directory"))
                .expect("cwd URI");
        let sandbox_context = ManagedNetworkSandboxContext {
            loopback_ports: vec![43123, 48081],
            allow_local_binding: false,
        };
        let params = ExecParams {
            process_id: ProcessId::from("managed-network"),
            argv: vec!["true".to_string()],
            cwd,
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            managed_network: ExecManagedNetwork::existing_proxy(sandbox_context.clone()),
        };

        let serialized = serde_json::to_value(&params).expect("serialize exec params");
        assert_eq!(
            serialized["managedNetwork"],
            serde_json::json!({
                "loopbackPorts": [43123, 48081],
                "allowLocalBinding": false,
            })
        );
        assert_eq!(serialized["networkProxy"], serde_json::json!(null));
        let round_trip: ExecParams =
            serde_json::from_value(serialized).expect("deserialize exec params");
        assert_eq!(round_trip, params);

        let legacy = ExecManagedNetwork::from_parts(
            /*enforce_managed_network*/ false,
            Some(sandbox_context),
            /*network_proxy*/ None,
        )
        .expect("legacy managedNetwork facts imply enforcement");
        assert_eq!(legacy.enforce(), true);
    }

    #[test]
    fn exec_params_defaults_missing_managed_network_fields_to_disabled() {
        let cwd =
            PathUri::from_host_native_path(std::env::current_dir().expect("current directory"))
                .expect("cwd URI");
        let params: ExecParams = serde_json::from_value(serde_json::json!({
            "processId": "legacy-without-managed-network",
            "argv": ["true"],
            "cwd": cwd,
            "env": {},
            "tty": false,
            "arg0": null,
            "sandbox": null
        }))
        .expect("deserialize legacy exec params");

        assert_eq!(params.managed_network, ExecManagedNetwork::disabled());
    }

    #[test]
    fn exec_params_rejects_both_proxy_launch_and_existing_proxy_facts() {
        let cwd =
            PathUri::from_host_native_path(std::env::current_dir().expect("current directory"))
                .expect("cwd URI");
        let launch = RemoteNetworkProxyLaunchConfig::new(
            RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig::default())
                .expect("supported remote config"),
        )
        .for_execution("remote".to_string(), "execution-1".to_string());
        let result = serde_json::from_value::<ExecParams>(serde_json::json!({
            "processId": "invalid-managed-network",
            "argv": ["true"],
            "cwd": cwd,
            "env": {},
            "tty": false,
            "arg0": null,
            "sandbox": null,
            "enforceManagedNetwork": true,
            "managedNetwork": {
                "loopbackPorts": [43123],
                "allowLocalBinding": false,
            },
            "networkProxy": launch
        }));

        assert!(result.is_err());
    }

    #[test]
    fn environment_info_accepts_legacy_response_without_cwd() {
        let info: EnvironmentInfo = serde_json::from_value(serde_json::json!({
            "shell": { "name": "zsh", "path": "/bin/zsh" }
        }))
        .expect("legacy environment info should deserialize");

        assert_eq!(
            info,
            EnvironmentInfo {
                shell: ShellInfo {
                    name: "zsh".to_string(),
                    path: "/bin/zsh".to_string(),
                },
                cwd: None,
            }
        );
    }

    #[test]
    fn filesystem_protocol_rejects_native_absolute_paths() {
        let native_path = std::env::current_dir()
            .expect("current directory")
            .join("native-file.txt");
        let native_cwd = std::env::current_dir().expect("current directory");

        serde_json::from_value::<FsReadFileParams>(serde_json::json!({
            "path": native_path.to_string_lossy(),
            "sandbox": null,
        }))
        .expect_err("native absolute path should not deserialize as a URI");

        let sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
            PermissionProfile::default(),
            PathUri::from_host_native_path(&native_cwd).expect("cwd URI"),
        );
        let mut native_path_sandbox =
            serde_json::to_value(sandbox).expect("sandbox should serialize");
        native_path_sandbox["cwd"] = serde_json::json!(native_cwd.to_string_lossy());

        serde_json::from_value::<FsReadFileParams>(serde_json::json!({
            "path": PathUri::from_host_native_path(native_path)
                .expect("path URI")
                .to_string(),
            "sandbox": native_path_sandbox,
        }))
        .expect_err("native absolute sandbox cwd should not deserialize as a URI");
    }

    #[test]
    fn http_request_timeout_treats_omitted_and_null_as_no_timeout() {
        let omitted: HttpRequestParams = serde_json::from_value(serde_json::json!({
            "method": "GET",
            "url": "https://example.test",
            "requestId": "req-omitted-timeout",
        }))
        .expect("omitted timeout should deserialize");
        let null_timeout: HttpRequestParams = serde_json::from_value(serde_json::json!({
            "method": "GET",
            "url": "https://example.test",
            "requestId": "req-null-timeout",
            "timeoutMs": null,
        }))
        .expect("null timeout should deserialize");
        let explicit_timeout: HttpRequestParams = serde_json::from_value(serde_json::json!({
            "method": "GET",
            "url": "https://example.test",
            "requestId": "req-explicit-timeout",
            "timeoutMs": 1234,
        }))
        .expect("numeric timeout should deserialize");

        assert_eq!(
            (omitted.request_id.as_str(), omitted.timeout_ms),
            ("req-omitted-timeout", None)
        );
        assert_eq!(
            (null_timeout.request_id.as_str(), null_timeout.timeout_ms),
            ("req-null-timeout", None)
        );
        assert_eq!(
            (
                explicit_timeout.request_id.as_str(),
                explicit_timeout.timeout_ms
            ),
            ("req-explicit-timeout", Some(1234))
        );
    }

    #[test]
    fn exited_notification_accepts_legacy_payload_without_sandbox_denied() {
        let notification: ExecExitedNotification = serde_json::from_value(serde_json::json!({
            "processId": "proc-1",
            "seq": 3,
            "exitCode": 1,
        }))
        .expect("legacy exited notification should deserialize");

        assert_eq!(notification.sandbox_denied, None);
    }
}
