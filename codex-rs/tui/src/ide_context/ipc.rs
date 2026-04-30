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

// The desktop IPC client gives requests 5 seconds to complete. Match that prompt-time budget here:
// fetching IDE context includes router discovery and extension event-loop work, so a shorter TUI
// deadline can incorrectly skip context even though the IDE answers normally.
const IDE_CONTEXT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(any(unix, windows))]
const MAX_IPC_FRAME_BYTES: usize = 256 * 1024 * 1024;
#[cfg(any(unix, windows))]
const TUI_SOURCE_CLIENT_ID: &str = "codex-tui";
#[cfg(any(unix, windows))]
const OPEN_IDE_HINT: &str =
    "Open this project in VS Code or Cursor with the Codex extension active.";
#[cfg(any(unix, windows))]
const IDE_DID_NOT_PROVIDE_CONTEXT_HINT: &str = "The IDE extension did not provide context.";
#[cfg(any(unix, windows))]
const KEEP_TRYING_HINT: &str = "Codex will keep trying on future messages.";

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
    #[cfg(any(unix, windows))]
    pub(crate) fn user_facing_hint(&self) -> String {
        match self {
            IdeContextError::Connect(_) => OPEN_IDE_HINT.to_string(),
            IdeContextError::RequestFailed(error) if error == "no-client-found" => {
                OPEN_IDE_HINT.to_string()
            }
            IdeContextError::RequestFailed(_) => {
                format!("{IDE_DID_NOT_PROVIDE_CONTEXT_HINT} Try /ide again.")
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
            IdeContextError::Connect(_) => OPEN_IDE_HINT.to_string(),
            IdeContextError::RequestFailed(error) if error == "no-client-found" => {
                OPEN_IDE_HINT.to_string()
            }
            IdeContextError::Read(error) if error.kind() == std::io::ErrorKind::TimedOut => {
                "Codex timed out waiting for IDE context. It will keep trying on future messages."
                    .to_string()
            }
            IdeContextError::RequestFailed(error) if error == "client-disconnected" => {
                hint_with_retry("The IDE connection changed while Codex was requesting context.")
            }
            IdeContextError::RequestFailed(error) if error == "request-timeout" => {
                hint_with_retry("The IDE extension did not answer in time.")
            }
            IdeContextError::RequestFailed(error) if error == "request-version-mismatch" => {
                "The connected IDE extension is not compatible with this IDE context request."
                    .to_string()
            }
            IdeContextError::RequestFailed(error) if error == "no-handler-for-request" => {
                "The connected IDE client does not support IDE context requests.".to_string()
            }
            IdeContextError::Send(_) => {
                hint_with_retry("Codex lost the IDE connection while requesting context.")
            }
            IdeContextError::InvalidResponse(_) => {
                hint_with_retry("Codex received an unexpected IDE context response.")
            }
            IdeContextError::RequestFailed(_) => hint_with_retry(IDE_DID_NOT_PROVIDE_CONTEXT_HINT),
            IdeContextError::Read(_) => hint_with_retry("Codex could not read IDE context."),
        }
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn user_facing_hint(&self) -> String {
        self.to_string()
    }

    #[cfg(not(any(unix, windows)))]
    pub(crate) fn prompt_skip_hint(&self) -> String {
        self.to_string()
    }
}

#[cfg(any(unix, windows))]
fn hint_with_retry(message: &str) -> String {
    format!("{message} {KEEP_TRYING_HINT}")
}

#[cfg(unix)]
type IdeContextStream = UnixDeadlineStream;

#[cfg(windows)]
type IdeContextStream = super::windows_pipe::WindowsPipeStream;

#[cfg(any(unix, windows))]
pub(crate) fn fetch_ide_context(workspace_root: &Path) -> Result<IdeContext, IdeContextError> {
    fetch_ide_context_from_socket(
        default_ipc_socket_path(),
        workspace_root,
        IDE_CONTEXT_REQUEST_TIMEOUT,
    )
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn fetch_ide_context(_workspace_root: &Path) -> Result<IdeContext, IdeContextError> {
    Err(IdeContextError::UnsupportedPlatform)
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

#[cfg(any(unix, windows))]
fn fetch_ide_context_from_socket(
    socket_path: PathBuf,
    workspace_root: &Path,
    timeout: Duration,
) -> Result<IdeContext, IdeContextError> {
    let deadline = Instant::now() + timeout;
    let mut stream = connect_stream(socket_path, deadline)?;
    fetch_ide_context_from_stream(&mut stream, workspace_root, deadline)
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
        let stream = connect_unix_stream_before_deadline(&socket_path, deadline)?;
        validate_unix_peer_owner(&stream)?;
        Ok(Self::new(stream, deadline))
    }

    fn new(stream: std::os::unix::net::UnixStream, deadline: Instant) -> Self {
        Self { stream, deadline }
    }

    fn set_deadline(&mut self, deadline: Instant) {
        self.deadline = deadline;
    }

    fn wait_for_ready(&self, events: libc::c_short) -> std::io::Result<()> {
        use std::os::fd::AsRawFd;

        wait_for_fd_ready(self.stream.as_raw_fd(), events, self.deadline)
    }
}

#[cfg(unix)]
fn connect_unix_stream_before_deadline(
    socket_path: &Path,
    deadline: Instant,
) -> std::io::Result<std::os::unix::net::UnixStream> {
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;
    use std::os::fd::IntoRawFd;
    use std::os::fd::OwnedFd;

    validate_unix_socket_path(socket_path)?;
    let (addr, addr_len) = unix_socket_addr(socket_path)?;
    let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    set_fd_close_on_exec(fd.as_raw_fd())?;
    set_fd_nonblocking(fd.as_raw_fd())?;

    let result = unsafe {
        libc::connect(
            fd.as_raw_fd(),
            &addr as *const libc::sockaddr_un as *const libc::sockaddr,
            addr_len,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if !is_in_progress_connect_error(&error) {
            return Err(error);
        }

        wait_for_fd_ready(fd.as_raw_fd(), libc::POLLOUT, deadline)?;
        let socket_error = socket_error(fd.as_raw_fd())?;
        if socket_error != 0 {
            return Err(std::io::Error::from_raw_os_error(socket_error));
        }
    }

    Ok(unsafe { std::os::unix::net::UnixStream::from_raw_fd(fd.into_raw_fd()) })
}

#[cfg(unix)]
fn unix_socket_addr(socket_path: &Path) -> std::io::Result<(libc::sockaddr_un, libc::socklen_t)> {
    use std::os::unix::ffi::OsStrExt;

    let path_bytes = socket_path.as_os_str().as_bytes();
    if path_bytes.contains(&0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "IDE context Unix socket path contains a nul byte",
        ));
    }

    let mut addr = unsafe { std::mem::zeroed::<libc::sockaddr_un>() };
    if path_bytes.len() >= addr.sun_path.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "IDE context Unix socket path is too long",
        ));
    }

    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in addr.sun_path.iter_mut().zip(path_bytes) {
        *slot = *byte as libc::c_char;
    }

    let addr_len =
        std::mem::size_of::<libc::sockaddr_un>() - addr.sun_path.len() + path_bytes.len() + 1;
    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        addr.sun_len = u8::try_from(addr_len).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "IDE context Unix socket address is too long",
            )
        })?;
    }

    let addr_len = libc::socklen_t::try_from(addr_len).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "IDE context Unix socket address is too long",
        )
    })?;
    Ok((addr, addr_len))
}

#[cfg(unix)]
fn set_fd_close_on_exec(fd: libc::c_int) -> std::io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
    if result < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(unix)]
fn set_fd_nonblocking(fd: libc::c_int) -> std::io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result < 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(unix)]
fn is_in_progress_connect_error(error: &std::io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(code)
            if code == libc::EINPROGRESS
                || code == libc::EALREADY
                || code == libc::EWOULDBLOCK
                || code == libc::EINTR
    )
}

#[cfg(unix)]
fn socket_error(fd: libc::c_int) -> std::io::Result<libc::c_int> {
    let mut socket_error = 0;
    let mut socket_error_len = libc::socklen_t::try_from(std::mem::size_of::<libc::c_int>())
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid socket error length",
            )
        })?;
    let result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_ERROR,
            &mut socket_error as *mut _ as *mut libc::c_void,
            &mut socket_error_len,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(socket_error)
}

#[cfg(unix)]
fn remaining_timeout(deadline: Instant) -> std::io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(deadline_timeout_io_error)
}

#[cfg(unix)]
fn remaining_timeout_ms(deadline: Instant) -> std::io::Result<libc::c_int> {
    let millis = remaining_timeout(deadline)?.as_millis().max(1);
    Ok(libc::c_int::try_from(millis).unwrap_or(libc::c_int::MAX))
}

#[cfg(unix)]
fn wait_for_fd_ready(
    fd: libc::c_int,
    events: libc::c_short,
    deadline: Instant,
) -> std::io::Result<()> {
    loop {
        // Keep deadline handling in user space. Some macOS Unix socket environments reject
        // SO_RCVTIMEO/SO_SNDTIMEO, but poll works consistently for our request-scoped timeout.
        let mut poll_fd = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll_fd, 1, remaining_timeout_ms(deadline)?) };
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
fn answer_unsupported_request<T: std::io::Write + ?Sized>(
    stream: &mut T,
    message: &Value,
) -> Result<(), IdeContextError> {
    if let Some(inbound_request_id) = message.get("requestId").and_then(Value::as_str) {
        let response = json!({
            "type": "response",
            "requestId": inbound_request_id,
            "resultType": "error",
            "error": "no-handler-for-request",
        });
        write_frame(stream, &response).map_err(IdeContextError::Send)?;
    }
    Ok(())
}

#[cfg(any(unix, windows))]
fn fetch_ide_context_from_stream(
    stream: &mut IdeContextStream,
    workspace_root: &Path,
    deadline: Instant,
) -> Result<IdeContext, IdeContextError> {
    let request_id = uuid::Uuid::new_v4().to_string();
    write_ide_context_request(stream, &request_id, workspace_root)
        .map_err(IdeContextError::Send)?;
    let response = read_response_frame(stream, &request_id, deadline)?;
    extract_ide_context(response)
}

#[cfg(any(unix, windows))]
fn write_ide_context_request<T: std::io::Write + ?Sized>(
    stream: &mut T,
    request_id: &str,
    workspace_root: &Path,
) -> std::io::Result<()> {
    let ide_context_request = json!({
        "type": "request",
        "requestId": request_id,
        "sourceClientId": TUI_SOURCE_CLIENT_ID,
        "version": 0,
        "method": "ide-context",
        "params": {
            "workspaceRoot": workspace_root.to_string_lossy(),
        },
    });
    write_frame(stream, &ide_context_request)
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
fn read_response_frame(
    stream: &mut IdeContextStream,
    request_id: &str,
    deadline: Instant,
) -> Result<Value, IdeContextError> {
    loop {
        ensure_deadline_not_expired(deadline)?;
        stream.set_deadline(deadline);
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
            Some("client-discovery-response") => {}
            Some("request") => {
                answer_unsupported_request(stream, &message)?;
            }
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[cfg(unix)]
    use pretty_assertions::assert_eq;

    #[cfg(unix)]
    fn test_deadline() -> Instant {
        Instant::now() + Duration::from_secs(1)
    }

    #[cfg(unix)]
    fn write_ide_context_response(
        stream: &mut impl std::io::Write,
        request_id: &str,
        active_selection_content: &str,
    ) {
        if let Err(err) = write_frame(
            stream,
            &json!({
                "type": "response",
                "requestId": request_id,
                "resultType": "success",
                "method": "ide-context",
                "handledByClientId": "vscode-client",
                "result": {
                    "type": "broadcast",
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
        ) {
            panic!("write ide-context response failed: {err}");
        }
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
    fn fetch_ide_context_uses_unregistered_request_route() {
        use std::os::unix::net::UnixListener;
        use std::thread;

        let tempdir = tempfile::tempdir().expect("tempdir");
        let socket_path = tempdir.path().join("codex-ipc.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind socket");

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");

            let ide_context = read_frame(&mut stream, test_deadline()).expect("read ide-context");
            assert_eq!(
                ide_context.get("method").and_then(Value::as_str),
                Some("ide-context")
            );
            assert_eq!(
                ide_context.get("sourceClientId").and_then(Value::as_str),
                Some(TUI_SOURCE_CLIENT_ID)
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
                    "type": "request",
                    "requestId": "inbound-request",
                    "sourceClientId": "vscode-client",
                    "version": 0,
                    "method": "unknown-method",
                    "params": {}
                }),
            )
            .expect("write inbound request before ide-context response");
            let inbound_response = read_frame(&mut stream, test_deadline())
                .expect("read inbound request response before ide-context response");
            assert_eq!(
                inbound_response,
                json!({
                    "type": "response",
                    "requestId": "inbound-request",
                    "resultType": "error",
                    "error": "no-handler-for-request"
                })
            );

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

            write_frame(
                &mut stream,
                &json!({
                    "type": "broadcast",
                    "method": "thread-stream-state-changed",
                    "params": "x".repeat(2 * 1024 * 1024),
                }),
            )
            .expect("write large broadcast");
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
}
