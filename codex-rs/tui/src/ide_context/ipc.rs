//! Private transport for fetching IDE context for TUI `/ide` support.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

#[cfg(any(unix, windows))]
use serde_json::Value;
#[cfg(any(unix, windows, test))]
use serde_json::json;
use thiserror::Error;

use super::IdeContext;

// The desktop integration can take several seconds to determine whether an IDE can answer an
// initial probe. Keep this long enough that transient local read timeouts do not prevent enabling
// the feature, but use a much shorter budget on the prompt-submit path below.
const IDE_CONTEXT_PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const IDE_CONTEXT_PROMPT_TIMEOUT: Duration = Duration::from_millis(500);
// Prompt rendering applies its own smaller cap to selected text before injection.
#[cfg(any(unix, windows))]
const MAX_IPC_FRAME_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Error)]
pub(crate) enum IdeContextError {
    #[cfg(any(unix, windows))]
    #[error("failed to connect to IDE context provider: {0}")]
    Connect(std::io::Error),
    #[cfg(any(unix, windows))]
    #[error("failed to request IDE context: {0}")]
    Send(std::io::Error),
    #[cfg(any(unix, windows))]
    #[error("failed to read IDE context: {0}")]
    Read(std::io::Error),
    #[cfg(any(unix, windows))]
    #[error("invalid IDE context response: {0}")]
    InvalidResponse(String),
    #[cfg(any(unix, windows))]
    #[error("IDE context response exceeded maximum size")]
    ResponseTooLarge,
    #[cfg(any(unix, windows))]
    #[error("IDE context request failed")]
    RequestFailed(String),
    #[cfg(not(any(unix, windows)))]
    #[error("IDE context is not supported on this platform")]
    UnsupportedPlatform,
}

impl IdeContextError {
    /// Returns true for short-lived states that can appear just after the TUI disconnects.
    #[cfg(any(unix, windows))]
    pub(crate) fn is_retryable_after_recent_toggle(&self) -> bool {
        match self {
            IdeContextError::RequestFailed(error) => {
                matches!(error.as_str(), "no-client-found" | "client-disconnected")
            }
            IdeContextError::Read(error) => error.kind() == std::io::ErrorKind::WouldBlock,
            IdeContextError::Connect(_)
            | IdeContextError::Send(_)
            | IdeContextError::InvalidResponse(_)
            | IdeContextError::ResponseTooLarge => false,
        }
    }

    #[cfg(any(unix, windows))]
    pub(crate) fn user_facing_hint(&self) -> String {
        match self {
            IdeContextError::Connect(_) => {
                "Open this project in VS Code or Cursor with the Codex extension active."
                    .to_string()
            }
            IdeContextError::RequestFailed(error) if error == "no-client-found" => {
                "Open this project in VS Code or Cursor with the Codex extension active."
                    .to_string()
            }
            IdeContextError::RequestFailed(_) => {
                "The IDE extension did not provide context. Try /ide again.".to_string()
            }
            IdeContextError::ResponseTooLarge => {
                "The selected IDE context is too large. Clear any large selection in your IDE and try /ide again.".to_string()
            }
            IdeContextError::Send(_) => {
                "Codex could not request IDE context. Try /ide again.".to_string()
            }
            IdeContextError::Read(_) | IdeContextError::InvalidResponse(_) => {
                "Codex could not read IDE context. Try /ide again.".to_string()
            }
        }
    }

    #[cfg(any(unix, windows))]
    pub(crate) fn prompt_skip_hint(&self) -> String {
        match self {
            IdeContextError::ResponseTooLarge => {
                "The selected IDE context is too large. Clear any large selection in your IDE."
                    .to_string()
            }
            IdeContextError::Connect(_) => {
                "Open this project in VS Code or Cursor with the Codex extension active."
                    .to_string()
            }
            IdeContextError::RequestFailed(error) if error == "no-client-found" => {
                "Open this project in VS Code or Cursor with the Codex extension active."
                    .to_string()
            }
            IdeContextError::Send(_)
            | IdeContextError::Read(_)
            | IdeContextError::InvalidResponse(_)
            | IdeContextError::RequestFailed(_) => {
                "Codex will keep trying on future messages.".to_string()
            }
        }
    }

    #[cfg(any(unix, windows))]
    pub(crate) fn should_reset_client(&self) -> bool {
        match self {
            IdeContextError::Connect(_)
            | IdeContextError::Send(_)
            | IdeContextError::Read(_)
            | IdeContextError::InvalidResponse(_)
            | IdeContextError::ResponseTooLarge => true,
            IdeContextError::RequestFailed(_) => false,
        }
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn is_retryable_after_recent_toggle(&self) -> bool {
        false
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn user_facing_hint(&self) -> String {
        self.to_string()
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn prompt_skip_hint(&self) -> String {
        self.to_string()
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn should_reset_client(&self) -> bool {
        false
    }
}

/// Persistent IPC client used while TUI `/ide` mode is enabled.
///
/// The initial connection and initialize handshake happen once on `/ide on`, and each user turn
/// asks for a fresh IDE context snapshot over the same route with a short prompt-time deadline.
#[cfg(any(unix, windows))]
pub(crate) struct IdeContextClient {
    stream: IdeContextStream,
    client_id: String,
}

#[cfg(unix)]
type IdeContextStream = UnixDeadlineStream;

#[cfg(windows)]
type IdeContextStream = super::windows_pipe::WindowsPipeStream;

#[cfg(any(unix, windows))]
impl IdeContextClient {
    pub(crate) fn connect() -> Result<Self, IdeContextError> {
        Self::connect_to_socket(default_ipc_socket_path(), IDE_CONTEXT_PROBE_TIMEOUT)
    }

    pub(crate) fn connect_for_prompt() -> Result<Self, IdeContextError> {
        Self::connect_to_socket(default_ipc_socket_path(), IDE_CONTEXT_PROMPT_TIMEOUT)
    }

    pub(crate) fn fetch_ide_context(
        &mut self,
        workspace_root: &Path,
    ) -> Result<IdeContext, IdeContextError> {
        self.fetch_ide_context_with_timeout(workspace_root, IDE_CONTEXT_PROBE_TIMEOUT)
    }

    pub(crate) fn fetch_ide_context_for_prompt(
        &mut self,
        workspace_root: &Path,
    ) -> Result<IdeContext, IdeContextError> {
        self.fetch_ide_context_with_timeout(workspace_root, IDE_CONTEXT_PROMPT_TIMEOUT)
    }

    fn connect_to_socket(socket_path: PathBuf, timeout: Duration) -> Result<Self, IdeContextError> {
        let deadline = Instant::now() + timeout;
        Self::connect_to_socket_before_deadline(socket_path, deadline)
    }

    fn connect_to_socket_before_deadline(
        socket_path: PathBuf,
        deadline: Instant,
    ) -> Result<Self, IdeContextError> {
        let mut stream = connect_stream(socket_path, deadline)?;
        let client_id = initialize_client(&mut stream, deadline)?;
        Ok(Self { stream, client_id })
    }

    fn fetch_ide_context_with_timeout(
        &mut self,
        workspace_root: &Path,
        timeout: Duration,
    ) -> Result<IdeContext, IdeContextError> {
        let deadline = Instant::now() + timeout;
        self.fetch_ide_context_before_deadline(workspace_root, deadline)
    }

    fn fetch_ide_context_before_deadline(
        &mut self,
        workspace_root: &Path,
        deadline: Instant,
    ) -> Result<IdeContext, IdeContextError> {
        self.stream.set_deadline(deadline);
        fetch_ide_context_with_client_id(
            &mut self.stream,
            &self.client_id,
            workspace_root,
            deadline,
        )
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) struct IdeContextClient;

#[cfg(not(any(unix, windows)))]
impl IdeContextClient {
    pub(crate) fn connect() -> Result<Self, IdeContextError> {
        Err(IdeContextError::UnsupportedPlatform)
    }

    pub(crate) fn connect_for_prompt() -> Result<Self, IdeContextError> {
        Err(IdeContextError::UnsupportedPlatform)
    }

    pub(crate) fn fetch_ide_context(
        &mut self,
        _workspace_root: &Path,
    ) -> Result<IdeContext, IdeContextError> {
        Err(IdeContextError::UnsupportedPlatform)
    }

    pub(crate) fn fetch_ide_context_for_prompt(
        &mut self,
        _workspace_root: &Path,
    ) -> Result<IdeContext, IdeContextError> {
        Err(IdeContextError::UnsupportedPlatform)
    }
}

#[cfg(unix)]
fn default_ipc_socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir()
        .join("codex-ipc")
        .join(format!("ipc-{uid}.sock"))
}

#[cfg(windows)]
fn default_ipc_socket_path() -> PathBuf {
    PathBuf::from(r"\\.\pipe\codex-ipc")
}

#[cfg(not(any(unix, windows)))]
fn default_ipc_socket_path() -> PathBuf {
    PathBuf::new()
}

#[cfg(all(test, unix))]
fn fetch_ide_context_from_socket(
    socket_path: PathBuf,
    workspace_root: &Path,
    timeout: Duration,
) -> Result<IdeContext, IdeContextError> {
    let deadline = Instant::now() + timeout;
    let mut client = IdeContextClient::connect_to_socket_before_deadline(socket_path, deadline)?;
    client.fetch_ide_context_before_deadline(workspace_root, deadline)
}

#[cfg(unix)]
fn connect_stream(
    socket_path: PathBuf,
    deadline: Instant,
) -> Result<IdeContextStream, IdeContextError> {
    UnixDeadlineStream::connect(socket_path, deadline).map_err(IdeContextError::Connect)
}

#[cfg(unix)]
struct UnixDeadlineStream {
    stream: std::os::unix::net::UnixStream,
    deadline: Instant,
}

#[cfg(unix)]
impl UnixDeadlineStream {
    fn connect(socket_path: PathBuf, deadline: Instant) -> std::io::Result<Self> {
        validate_unix_socket_path(&socket_path)?;
        let stream = std::os::unix::net::UnixStream::connect(socket_path)?;
        validate_unix_peer_owner(&stream)?;
        stream.set_nonblocking(true)?;
        Ok(Self::new(stream, deadline))
    }

    fn new(stream: std::os::unix::net::UnixStream, deadline: Instant) -> Self {
        Self { stream, deadline }
    }

    fn set_deadline(&mut self, deadline: Instant) {
        self.deadline = deadline;
    }

    fn remaining_timeout(&self) -> std::io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(deadline_timeout_io_error)
    }

    fn remaining_timeout_ms(&self) -> std::io::Result<libc::c_int> {
        let millis = self.remaining_timeout()?.as_millis().max(1);
        Ok(libc::c_int::try_from(millis).unwrap_or(libc::c_int::MAX))
    }

    fn wait_for_ready(&self, events: libc::c_short) -> std::io::Result<()> {
        use std::os::fd::AsRawFd;

        loop {
            // Keep deadline handling in user space. Some macOS Unix socket environments reject
            // SO_RCVTIMEO/SO_SNDTIMEO, but poll works consistently for our request-scoped timeout.
            let mut poll_fd = libc::pollfd {
                fd: self.stream.as_raw_fd(),
                events,
                revents: 0,
            };
            let result = unsafe { libc::poll(&mut poll_fd, 1, self.remaining_timeout_ms()?) };
            if result == 0 {
                return Err(deadline_timeout_io_error());
            }
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if poll_fd.revents & libc::POLLNVAL != 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid IDE context Unix socket",
                ));
            }
            if poll_fd.revents & (events | libc::POLLERR | libc::POLLHUP) != 0 {
                return Ok(());
            }
        }
    }
}

#[cfg(unix)]
impl std::io::Read for UnixDeadlineStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            self.wait_for_ready(libc::POLLIN)?;
            match self.stream.read(buf) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }
}

#[cfg(unix)]
impl std::io::Write for UnixDeadlineStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            self.wait_for_ready(libc::POLLOUT)?;
            match self.stream.write(buf) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.wait_for_ready(libc::POLLOUT)?;
        self.stream.flush()
    }
}

#[cfg(unix)]
fn validate_unix_socket_path(socket_path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;

    let uid = unsafe { libc::getuid() };
    let parent = socket_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "IDE context socket has no parent directory",
        )
    })?;
    let parent_metadata = std::fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir() || parent_metadata.uid() != uid {
        return Err(permission_denied_io_error(
            "IDE context socket directory is not owned by the current user",
        ));
    }
    if parent_metadata.permissions().mode() & 0o022 != 0 {
        return Err(permission_denied_io_error(
            "IDE context socket directory is writable by other users",
        ));
    }

    let socket_metadata = std::fs::symlink_metadata(socket_path)?;
    if !socket_metadata.file_type().is_socket() || socket_metadata.uid() != uid {
        return Err(permission_denied_io_error(
            "IDE context socket is not owned by the current user",
        ));
    }

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn validate_unix_peer_owner(stream: &std::os::unix::net::UnixStream) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    let mut credentials = unsafe { std::mem::zeroed::<libc::ucred>() };
    let mut credentials_len: libc::socklen_t =
        std::mem::size_of::<libc::ucred>().try_into().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid peer credential length",
            )
        })?;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut credentials as *mut _ as *mut libc::c_void,
            &mut credentials_len,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }

    ensure_peer_uid_matches_current_user(credentials.uid)
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn validate_unix_peer_owner(stream: &std::os::unix::net::UnixStream) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;

    let mut peer_uid: libc::uid_t = 0;
    let mut peer_gid: libc::gid_t = 0;
    let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut peer_uid, &mut peer_gid) };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }

    ensure_peer_uid_matches_current_user(peer_uid)
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn validate_unix_peer_owner(_stream: &std::os::unix::net::UnixStream) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn ensure_peer_uid_matches_current_user(peer_uid: libc::uid_t) -> std::io::Result<()> {
    if peer_uid != unsafe { libc::getuid() } {
        return Err(permission_denied_io_error(
            "IDE context provider is not owned by the current user",
        ));
    }

    Ok(())
}

#[cfg(windows)]
fn connect_stream(
    socket_path: PathBuf,
    deadline: Instant,
) -> Result<IdeContextStream, IdeContextError> {
    super::windows_pipe::WindowsPipeStream::connect(socket_path, deadline)
        .map_err(IdeContextError::Connect)
}

#[cfg(any(unix, windows))]
fn initialize_client<T: std::io::Read + std::io::Write + ?Sized>(
    stream: &mut T,
    deadline: Instant,
) -> Result<String, IdeContextError> {
    let initialize_request_id = uuid::Uuid::new_v4().to_string();
    let initialize_request = json!({
        "type": "request",
        "requestId": initialize_request_id.clone(),
        "sourceClientId": "initializing-client",
        "version": 0,
        "method": "initialize",
        "params": {
            // Match the desktop client type so the current IDE extension can handle us unchanged.
            "clientType": "desktop",
        },
    });
    write_frame(stream, &initialize_request).map_err(IdeContextError::Send)?;
    let initialize_response = read_response_frame(stream, &initialize_request_id, deadline)?;
    extract_client_id(&initialize_response)
}

#[cfg(any(unix, windows))]
fn fetch_ide_context_with_client_id<T: std::io::Read + std::io::Write + ?Sized>(
    stream: &mut T,
    client_id: &str,
    workspace_root: &Path,
    deadline: Instant,
) -> Result<IdeContext, IdeContextError> {
    let ide_context_request_id = uuid::Uuid::new_v4().to_string();
    let ide_context_request = json!({
        "type": "request",
        "requestId": ide_context_request_id.clone(),
        "sourceClientId": client_id,
        "version": 0,
        "method": "ide-context",
        "params": {
            "workspaceRoot": workspace_root.to_string_lossy(),
        },
    });
    write_frame(stream, &ide_context_request).map_err(IdeContextError::Send)?;
    let ide_context_response = read_response_frame(stream, &ide_context_request_id, deadline)?;
    extract_ide_context(ide_context_response)
}

#[cfg(any(unix, windows))]
fn write_frame<T: std::io::Write + ?Sized>(stream: &mut T, message: &Value) -> std::io::Result<()> {
    let payload = serde_json::to_vec(message).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid IDE context JSON message: {err}"),
        )
    })?;
    let payload_len = u32::try_from(payload.len()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "IDE context payload exceeds u32 length",
        )
    })?;
    stream.write_all(&payload_len.to_le_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()
}

#[cfg(any(unix, windows))]
fn read_frame<T: std::io::Read + ?Sized>(
    stream: &mut T,
    deadline: Instant,
) -> Result<Value, IdeContextError> {
    let mut len_bytes = [0_u8; 4];
    read_exact_before_deadline(stream, &mut len_bytes, deadline)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    if len > MAX_IPC_FRAME_BYTES {
        return Err(IdeContextError::ResponseTooLarge);
    }

    let mut payload = vec![0_u8; len];
    read_exact_before_deadline(stream, &mut payload, deadline)?;
    serde_json::from_slice(&payload)
        .map_err(|err| IdeContextError::InvalidResponse(format!("invalid JSON payload: {err}")))
}

#[cfg(any(unix, windows))]
fn read_exact_before_deadline<T: std::io::Read + ?Sized>(
    stream: &mut T,
    buf: &mut [u8],
    deadline: Instant,
) -> Result<(), IdeContextError> {
    // std::io::Read::read_exact has no way to observe our request deadline between partial reads.
    // Keep the frame header and payload under the same budget as the surrounding response wait.
    let mut read_so_far = 0;
    while read_so_far < buf.len() {
        ensure_deadline_not_expired(deadline)?;
        match stream.read(&mut buf[read_so_far..]) {
            Ok(0) => {
                return Err(IdeContextError::Read(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "failed to fill whole IDE context frame",
                )));
            }
            Ok(bytes_read) => {
                read_so_far += bytes_read;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(IdeContextError::Read(error)),
        }
    }

    ensure_deadline_not_expired(deadline)
}

#[cfg(any(unix, windows))]
fn read_response_frame<T: std::io::Read + std::io::Write + ?Sized>(
    stream: &mut T,
    request_id: &str,
    deadline: Instant,
) -> Result<Value, IdeContextError> {
    loop {
        ensure_deadline_not_expired(deadline)?;
        let message = read_frame(stream, deadline)?;
        match message.get("type").and_then(Value::as_str) {
            Some("response") => {
                if message.get("requestId").and_then(Value::as_str) == Some(request_id) {
                    return Ok(message);
                }
            }
            Some("broadcast") => {}
            Some("client-discovery-request") => {
                if let Some(discovery_request_id) = message.get("requestId").and_then(Value::as_str)
                {
                    let response = json!({
                        "type": "client-discovery-response",
                        "requestId": discovery_request_id,
                        "response": {
                            "canHandle": false,
                        },
                    });
                    write_frame(stream, &response).map_err(IdeContextError::Send)?;
                }
            }
            Some("client-discovery-response") | Some("request") => {}
            Some(other) => {
                return Err(IdeContextError::InvalidResponse(format!(
                    "unexpected IDE context message type: {other}"
                )));
            }
            None => {
                return Err(IdeContextError::InvalidResponse(
                    "IDE context message did not include a type".to_string(),
                ));
            }
        }
    }
}

#[cfg(any(unix, windows))]
fn ensure_deadline_not_expired(deadline: Instant) -> Result<(), IdeContextError> {
    if Instant::now() >= deadline {
        return Err(timeout_error());
    }

    Ok(())
}

#[cfg(any(unix, windows))]
fn timeout_error() -> IdeContextError {
    IdeContextError::Read(deadline_timeout_io_error())
}

#[cfg(any(unix, windows))]
fn deadline_timeout_io_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "timed out waiting for IDE context",
    )
}

#[cfg(unix)]
fn permission_denied_io_error(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, message)
}

#[cfg(any(unix, windows))]
fn extract_client_id(response: &Value) -> Result<String, IdeContextError> {
    ensure_success_response(response)?;
    response
        .get("result")
        .and_then(|result| result.get("clientId"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            IdeContextError::InvalidResponse(
                "initialize response did not include result.clientId".to_string(),
            )
        })
}

#[cfg(any(unix, windows))]
fn extract_ide_context(response: Value) -> Result<IdeContext, IdeContextError> {
    ensure_success_response(&response)?;
    let ide_context = response
        .get("result")
        .and_then(|result| result.get("ideContext"))
        .cloned()
        .ok_or_else(|| {
            IdeContextError::InvalidResponse(
                "ide-context response did not include result.ideContext".to_string(),
            )
        })?;
    serde_json::from_value(ide_context)
        .map_err(|err| IdeContextError::InvalidResponse(err.to_string()))
}

#[cfg(any(unix, windows))]
fn ensure_success_response(response: &Value) -> Result<(), IdeContextError> {
    match response.get("resultType").and_then(Value::as_str) {
        Some("success") => Ok(()),
        Some("error") => Err(IdeContextError::RequestFailed(
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
                .to_string(),
        )),
        _ => Err(IdeContextError::InvalidResponse(
            "response did not include a success or error resultType".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use pretty_assertions::assert_eq;

    #[cfg(unix)]
    fn test_deadline() -> Instant {
        Instant::now() + Duration::from_secs(1)
    }

    #[cfg(unix)]
    fn write_initialize_response(stream: &mut impl std::io::Write, request_id: &str) {
        write_frame(
            stream,
            &json!({
                "type": "response",
                "requestId": request_id,
                "resultType": "success",
                "method": "initialize",
                "handledByClientId": "server",
                "result": {
                    "clientId": "rust-client"
                }
            }),
        )
        .expect("write initialize response");
    }

    #[cfg(unix)]
    fn write_ide_context_response(
        stream: &mut impl std::io::Write,
        request_id: &str,
        active_selection_content: &str,
    ) {
        write_frame(
            stream,
            &json!({
                "type": "response",
                "requestId": request_id,
                "resultType": "success",
                "method": "ide-context",
                "handledByClientId": "vscode-client",
                "result": {
                    "ideContext": {
                        "activeFile": {
                            "label": "lib.rs",
                            "path": "src/lib.rs",
                            "fsPath": "/repo/src/lib.rs",
                            "selection": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 3 }
                            },
                            "activeSelectionContent": active_selection_content,
                            "selections": []
                        },
                        "openTabs": []
                    }
                }
            }),
        )
        .expect("write ide-context response");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn retryable_after_recent_toggle_covers_transient_errors() {
        assert!(
            IdeContextError::RequestFailed("no-client-found".to_string())
                .is_retryable_after_recent_toggle()
        );
        assert!(
            IdeContextError::RequestFailed("client-disconnected".to_string())
                .is_retryable_after_recent_toggle()
        );
        assert!(
            !IdeContextError::RequestFailed("request-timeout".to_string())
                .is_retryable_after_recent_toggle()
        );
        assert!(
            IdeContextError::Read(std::io::Error::from(std::io::ErrorKind::WouldBlock))
                .is_retryable_after_recent_toggle()
        );
        assert!(
            !IdeContextError::Read(std::io::Error::from(std::io::ErrorKind::TimedOut))
                .is_retryable_after_recent_toggle()
        );
        assert!(
            !IdeContextError::RequestFailed("other-error".to_string())
                .is_retryable_after_recent_toggle()
        );
        assert!(
            !IdeContextError::InvalidResponse("bad payload".to_string())
                .is_retryable_after_recent_toggle()
        );
        assert!(!IdeContextError::ResponseTooLarge.is_retryable_after_recent_toggle());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn read_response_frame_respects_expired_deadline() {
        let mut stream = std::io::Cursor::new(Vec::new());
        write_frame(
            &mut stream,
            &json!({
                "type": "broadcast",
                "method": "client-status-changed",
                "sourceClientId": "vscode-client",
                "version": 0,
                "params": {
                    "clientId": "vscode-client",
                    "clientType": "vscode",
                    "status": "connected"
                }
            }),
        )
        .expect("write broadcast frame");
        stream.set_position(0);

        let err = read_response_frame(&mut stream, "missing-request", Instant::now())
            .expect_err("expired deadline should fail before reading");

        assert!(matches!(
            err,
            IdeContextError::Read(error) if error.kind() == std::io::ErrorKind::TimedOut
        ));
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn read_frame_respects_deadline_while_reading_payload() {
        struct SlowPayloadReader {
            header: [u8; 4],
            header_sent: bool,
            payload: Vec<u8>,
        }

        impl std::io::Read for SlowPayloadReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if !self.header_sent {
                    self.header_sent = true;
                    buf[..self.header.len()].copy_from_slice(&self.header);
                    return Ok(self.header.len());
                }

                std::thread::sleep(Duration::from_millis(20));
                let bytes_to_copy = self.payload.len().min(buf.len());
                buf[..bytes_to_copy].copy_from_slice(&self.payload[..bytes_to_copy]);
                self.payload.drain(..bytes_to_copy);
                Ok(bytes_to_copy)
            }
        }

        let payload = br#"{"type":"response"}"#.to_vec();
        let mut stream = SlowPayloadReader {
            header: u32::try_from(payload.len())
                .expect("payload length fits u32")
                .to_le_bytes(),
            header_sent: false,
            payload,
        };

        let err = read_frame(&mut stream, Instant::now() + Duration::from_millis(1))
            .expect_err("expired deadline should fail while reading payload");

        assert!(matches!(
            err,
            IdeContextError::Read(error) if error.kind() == std::io::ErrorKind::TimedOut
        ));
    }

    #[cfg(unix)]
    #[test]
    fn unix_deadline_stream_uses_remaining_deadline_for_blocking_reads() {
        use std::os::unix::net::UnixStream;

        let (client, _server) = UnixStream::pair().expect("create unix stream pair");
        let mut stream =
            UnixDeadlineStream::new(client, Instant::now() + Duration::from_millis(50));
        let start = Instant::now();
        let mut buf = [0_u8; 1];

        let err = std::io::Read::read(&mut stream, &mut buf)
            .expect_err("read should time out at the request deadline");

        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn validate_unix_socket_path_rejects_unsafe_parent_directory() {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;

        let tempdir = tempfile::tempdir().expect("tempdir");
        std::fs::set_permissions(tempdir.path(), std::fs::Permissions::from_mode(0o777))
            .expect("set unsafe permissions");
        let socket_path = tempdir.path().join("codex-ipc.sock");
        let _listener = UnixListener::bind(&socket_path).expect("bind socket");

        let err = validate_unix_socket_path(&socket_path)
            .expect_err("world-writable parent directory should be rejected");

        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[cfg(unix)]
    #[test]
    fn fetch_ide_context_handles_interleaved_messages() {
        use std::os::unix::net::UnixListener;
        use std::thread;

        let tempdir = tempfile::tempdir().expect("tempdir");
        let socket_path = tempdir.path().join("codex-ipc.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind socket");

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");

            let initialize = read_frame(&mut stream, test_deadline()).expect("read initialize");
            assert_eq!(
                initialize.get("method").and_then(Value::as_str),
                Some("initialize")
            );
            assert_eq!(
                initialize
                    .get("params")
                    .and_then(|params| params.get("clientType"))
                    .and_then(Value::as_str),
                Some("desktop")
            );
            let initialize_request_id = initialize
                .get("requestId")
                .and_then(Value::as_str)
                .expect("initialize request id");
            write_initialize_response(&mut stream, initialize_request_id);

            let ide_context = read_frame(&mut stream, test_deadline()).expect("read ide-context");
            assert_eq!(
                ide_context.get("method").and_then(Value::as_str),
                Some("ide-context")
            );
            assert_eq!(
                ide_context.get("sourceClientId").and_then(Value::as_str),
                Some("rust-client")
            );
            assert_eq!(
                ide_context
                    .get("params")
                    .and_then(|params| params.get("workspaceRoot"))
                    .and_then(Value::as_str),
                Some("/repo")
            );
            let ide_context_request_id = ide_context
                .get("requestId")
                .and_then(Value::as_str)
                .expect("ide-context request id");
            write_frame(
                &mut stream,
                &json!({
                    "type": "broadcast",
                    "method": "client-status-changed",
                    "sourceClientId": "vscode-client",
                    "version": 0,
                    "params": {
                        "clientId": "vscode-client",
                        "clientType": "vscode",
                        "status": "connected"
                    }
                }),
            )
            .expect("write broadcast before ide-context response");

            write_frame(
                &mut stream,
                &json!({
                    "type": "client-discovery-request",
                    "requestId": "discovery-request",
                    "request": ide_context.clone(),
                }),
            )
            .expect("write client discovery request");
            let discovery_response =
                read_frame(&mut stream, test_deadline()).expect("read client discovery response");
            assert_eq!(
                discovery_response.get("type").and_then(Value::as_str),
                Some("client-discovery-response")
            );
            assert_eq!(
                discovery_response.get("requestId").and_then(Value::as_str),
                Some("discovery-request")
            );
            assert_eq!(
                discovery_response
                    .get("response")
                    .and_then(|response| response.get("canHandle"))
                    .and_then(Value::as_bool),
                Some(false)
            );

            write_ide_context_response(&mut stream, ide_context_request_id, "use");
        });

        let context =
            fetch_ide_context_from_socket(socket_path, Path::new("/repo"), Duration::from_secs(1))
                .expect("fetch ide context");

        server.join().expect("server joins");
        assert_eq!(
            context
                .active_file
                .as_ref()
                .map(|file| file.active_selection_content.as_str()),
            Some("use")
        );
    }

    #[cfg(unix)]
    #[test]
    fn ide_context_client_reuses_initialized_connection_for_prompt_requests() {
        use std::os::unix::net::UnixListener;
        use std::thread;

        let tempdir = tempfile::tempdir().expect("tempdir");
        let socket_path = tempdir.path().join("codex-ipc.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind socket");

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");

            let initialize = read_frame(&mut stream, test_deadline()).expect("read initialize");
            let initialize_request_id = initialize
                .get("requestId")
                .and_then(Value::as_str)
                .expect("initialize request id");
            write_initialize_response(&mut stream, initialize_request_id);

            for active_selection_content in ["first", "second"] {
                let ide_context =
                    read_frame(&mut stream, test_deadline()).expect("read ide-context");
                assert_eq!(
                    ide_context.get("method").and_then(Value::as_str),
                    Some("ide-context")
                );
                assert_eq!(
                    ide_context.get("sourceClientId").and_then(Value::as_str),
                    Some("rust-client")
                );
                assert_eq!(
                    ide_context
                        .get("params")
                        .and_then(|params| params.get("workspaceRoot"))
                        .and_then(Value::as_str),
                    Some("/repo")
                );
                let ide_context_request_id = ide_context
                    .get("requestId")
                    .and_then(Value::as_str)
                    .expect("ide-context request id");
                write_ide_context_response(
                    &mut stream,
                    ide_context_request_id,
                    active_selection_content,
                );
            }
        });

        let mut client = IdeContextClient::connect_to_socket(socket_path, Duration::from_secs(1))
            .expect("connect IDE context client");
        let first = client
            .fetch_ide_context(Path::new("/repo"))
            .expect("fetch first IDE context");
        let second = client
            .fetch_ide_context_for_prompt(Path::new("/repo"))
            .expect("fetch second IDE context");

        server.join().expect("server joins");
        assert_eq!(
            [
                first
                    .active_file
                    .as_ref()
                    .map(|file| file.active_selection_content.as_str()),
                second
                    .active_file
                    .as_ref()
                    .map(|file| file.active_selection_content.as_str()),
            ],
            [Some("first"), Some("second")]
        );
    }
}
