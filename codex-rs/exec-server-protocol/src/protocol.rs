use std::collections::HashMap;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_file_system::FileSystemSandboxContext;
pub use codex_file_system::WalkOptions;
pub use codex_file_system::WalkOutcome;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_shell_command::shell_detect::DetectedShell;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;

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
    /// Whether the eventual executor-side sandbox must enforce managed networking.
    #[serde(default)]
    pub enforce_managed_network: bool,
    /// Optional details for enforcing managed networking without a live proxy object.
    ///
    /// When `enforce_managed_network` is true and these details are absent, the executor must
    /// continue to fail closed. This preserves compatibility with older clients.
    #[serde(default)]
    pub managed_network: Option<ManagedNetworkSandboxContext>,
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
    use super::ExecParams;
    use super::FsReadFileParams;
    use super::HttpRequestParams;
    use super::ProcessId;
    use super::ShellInfo;
    use codex_file_system::FileSystemSandboxContext;
    use codex_network_proxy::ManagedNetworkSandboxContext;
    use codex_protocol::models::PermissionProfile;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    #[test]
    fn exec_params_managed_network_context_round_trips_and_defaults_for_legacy_peers() {
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
            enforce_managed_network: true,
            managed_network: Some(ManagedNetworkSandboxContext {
                loopback_ports: vec![43123, 48081],
                allow_local_binding: false,
            }),
        };

        let mut serialized = serde_json::to_value(&params).expect("serialize exec params");
        assert_eq!(
            serialized["managedNetwork"],
            serde_json::json!({
                "loopbackPorts": [43123, 48081],
                "allowLocalBinding": false,
            })
        );
        let round_trip: ExecParams =
            serde_json::from_value(serialized.clone()).expect("deserialize exec params");
        assert_eq!(round_trip, params);

        serialized
            .as_object_mut()
            .expect("exec params object")
            .remove("managedNetwork");
        let legacy: ExecParams =
            serde_json::from_value(serialized).expect("deserialize legacy exec params");
        assert!(legacy.enforce_managed_network);
        assert_eq!(legacy.managed_network, None);
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
