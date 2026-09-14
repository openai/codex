//! Authenticated local IPC for the Windows sandbox provisioning service.
//! Expected service limitations defer provisioning to the client's elevated helper;
//! authentication and policy rejections remain errors.
//! Shutdown wakeups are retried until the listener connects or stops.

mod authentication;
mod home;
mod listener;
mod request;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
pub(crate) use authentication::ClientIdentity;
use authentication::authenticate_client;
use codex_windows_sandbox::FramedProvisioningMessage;
use codex_windows_sandbox::PROVISIONING_PROTOCOL_VERSION;
use codex_windows_sandbox::ProvisioningMessage;
use codex_windows_sandbox::SandboxProvisioningResponse;
use codex_windows_sandbox::to_wide;
use codex_windows_sandbox::write_provisioning_frame;
pub(crate) use home::OwnedHandle;
pub(crate) use home::pin_directory;
pub(crate) use home::pin_existing_ancestors;
#[cfg(test)]
use request::ProvisioningRequest;
pub(crate) use request::ServiceRequest;
use request::validate_request;
use std::mem::size_of;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem as filesystem;
use windows_sys::Win32::System::Pipes as pipes;

use crate::installation_record::InstallationRecord;

pub(crate) const PIPE_NAME: &str = codex_windows_sandbox::SANDBOX_PROVISIONING_PIPE_NAME;

const MAX_REQUEST_BYTES: usize = 4096;
const MAX_RESPONSE_MESSAGE_BYTES: usize = 512;
const REQUEST_IDLE_TIMEOUT: Duration = Duration::from_secs(5);
const PIPE_USER_ACCESS: &str = "0x0012019b";

/// The service cannot complete this request; the interactive setup helper may still work.
#[derive(Debug)]
pub(crate) struct ServiceUnavailable(pub(crate) &'static str);

impl std::fmt::Display for ServiceUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for ServiceUnavailable {}

#[derive(Debug, Eq, PartialEq)]
enum PipeConnection {
    Connected,
    Disconnected,
}

pub(crate) fn run(
    shutdown: Arc<AtomicBool>,
    on_ready: impl FnOnce() -> Result<()>,
    register_installation: impl Fn(InstallationRecord, OwnedHandle) -> Result<InstallationRecord>,
    on_session_change: impl Fn() -> Result<()>,
) -> Result<()> {
    let listener = listener::ProvisioningListener::open()?;
    on_ready().context("publish provisioning listener readiness")?;

    while !shutdown.load(Ordering::Acquire) {
        let connection = accept_pipe_connection(listener.pipe.0)?;
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        // Session-change wakeups close the pipe immediately and can arrive disconnected.
        on_session_change().context("restore the signed-in user's uninstall listener")?;
        if connection == PipeConnection::Disconnected {
            continue;
        }

        let authorized_process =
            match crate::package_identity::authorize_client_process(listener.pipe.0) {
                Ok(process) => process,
                Err(_) => {
                    unsafe { pipes::DisconnectNamedPipe(listener.pipe.0) };
                    continue;
                }
            };
        let result = handle_request(
            listener.pipe.0,
            &authorized_process,
            &listener.sandbox_sid,
            &shutdown,
            &register_installation,
        );
        let response = match result {
            Ok(response) => response,
            Err(error) if error.is::<ServiceUnavailable>() => {
                eprintln!("sandbox provisioning service unavailable: {error:#}");
                SandboxProvisioningResponse::Unavailable
            }
            Err(error) => {
                eprintln!("sandbox provisioning request failed: {error}");
                SandboxProvisioningResponse::Error {
                    message: response_error_message(&error),
                }
            }
        };
        let response = FramedProvisioningMessage {
            version: PROVISIONING_PROTOCOL_VERSION,
            message: ProvisioningMessage::ProvisionSandboxResponse { payload: response },
        };
        write_response(&listener.pipe, &response, &shutdown)?;
        unsafe { pipes::DisconnectNamedPipe(listener.pipe.0) };
    }
    Ok(())
}

/// Sends one frame, then waits briefly for the client to close before disconnecting.
fn write_response(
    pipe: &OwnedHandle,
    response: &FramedProvisioningMessage,
    shutdown: &AtomicBool,
) -> Result<()> {
    let mut frame = Vec::new();
    write_provisioning_frame(&mut frame, response)
        .context("serialize sandbox provisioning response")?;
    let mut written = 0;
    let sent = unsafe {
        filesystem::WriteFile(
            pipe.0,
            frame.as_ptr(),
            frame.len() as u32,
            &mut written,
            ptr::null_mut(),
        )
    };
    if sent != 0 {
        let deadline = Instant::now() + Duration::from_secs(1);
        while !shutdown.load(Ordering::Acquire) && Instant::now() < deadline {
            if unsafe {
                pipes::PeekNamedPipe(
                    pipe.0,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            } == 0
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}

fn response_error_message(error: &anyhow::Error) -> String {
    let mut message = String::new();
    for character in error.to_string().chars() {
        let character = if character.is_control() {
            ' '
        } else {
            character
        };
        if message.len() + character.len_utf8() > MAX_RESPONSE_MESSAGE_BYTES {
            break;
        }
        message.push(character);
    }
    message
}

fn accept_pipe_connection(pipe: HANDLE) -> Result<PipeConnection> {
    if unsafe { pipes::ConnectNamedPipe(pipe, ptr::null_mut()) } != 0 {
        return Ok(PipeConnection::Connected);
    }

    let error = unsafe { foundation::GetLastError() };
    match error {
        foundation::ERROR_PIPE_CONNECTED => Ok(PipeConnection::Connected),
        foundation::ERROR_NO_DATA | foundation::ERROR_BROKEN_PIPE => {
            if unsafe { pipes::DisconnectNamedPipe(pipe) } == 0 {
                let reset_error = std::io::Error::last_os_error();
                if reset_error.raw_os_error() != Some(foundation::ERROR_PIPE_NOT_CONNECTED as i32) {
                    return Err(reset_error).context("reset disconnected provisioning client");
                }
            }
            Ok(PipeConnection::Disconnected)
        }
        _ => Err(std::io::Error::from_raw_os_error(error as i32))
            .context("accept provisioning client"),
    }
}

pub(crate) fn wake(pipe_name: &str, is_stopped: impl Fn() -> bool) {
    let pipe_name = to_wide(pipe_name);
    while !is_stopped() {
        let handle = unsafe {
            filesystem::CreateFileW(
                pipe_name.as_ptr(),
                foundation::GENERIC_WRITE,
                /*dwsharemode*/ 0,
                ptr::null(),
                filesystem::OPEN_EXISTING,
                /*dwflagsandattributes*/ 0,
                /*htemplatefile*/ 0,
            )
        };
        if handle != foundation::INVALID_HANDLE_VALUE {
            unsafe { foundation::CloseHandle(handle) };
            return;
        }
        // A disconnected instance cannot accept the wakeup until ConnectNamedPipe runs.
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn pipe_security_descriptor(sandbox_sid: &str) -> String {
    format!("D:P(D;;GA;;;{sandbox_sid})(A;;GA;;;SY)(A;;GA;;;BA)(A;;{PIPE_USER_ACCESS};;;IU)")
}

fn handle_request(
    pipe: HANDLE,
    authorized_process: &crate::package_identity::AuthorizedClientProcess,
    sandbox_sid: &[u8],
    shutdown: &AtomicBool,
    register_installation: &dyn Fn(InstallationRecord, OwnedHandle) -> Result<InstallationRecord>,
) -> Result<SandboxProvisioningResponse> {
    let deadline = Instant::now() + REQUEST_IDLE_TIMEOUT;
    let mut request = [0_u8; MAX_REQUEST_BYTES];
    let mut request_length = 0;
    loop {
        if shutdown.load(Ordering::Acquire) {
            bail!("service is stopping");
        }
        if Instant::now() >= deadline {
            bail!("provisioning request timed out");
        }
        let mut available = 0;
        if unsafe {
            pipes::PeekNamedPipe(
                pipe,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                &mut available,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error()).context("inspect provisioning request");
        }
        if available as usize > MAX_REQUEST_BYTES - request_length {
            bail!("provisioning request exceeds size limit");
        }
        if available != 0 {
            let mut read = 0;
            if unsafe {
                filesystem::ReadFile(
                    pipe,
                    request[request_length..].as_mut_ptr(),
                    available,
                    &mut read,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error()).context("read provisioning request");
            }
            if read == 0 {
                bail!("provisioning client sent an empty request");
            }
            request_length += read as usize;
            let received = &request[..request_length];
            if received.len() >= size_of::<u32>() {
                let payload_length =
                    u32::from_le_bytes([received[0], received[1], received[2], received[3]])
                        as usize;
                if payload_length > MAX_REQUEST_BYTES - size_of::<u32>() {
                    bail!("provisioning request exceeds size limit");
                }
                let frame_length = size_of::<u32>() + payload_length;
                if request_length > frame_length {
                    bail!("provisioning requests must contain exactly one IPC frame");
                }
                if request_length == frame_length {
                    break;
                }
            }
            continue;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    let request = validate_request(&request[..request_length])?;
    let (identity, policy_result) =
        authenticate_client(pipe, authorized_process, sandbox_sid, &request)?;
    if let Err(error) = policy_result {
        if is_config_parse_error(&error) {
            return Ok(SandboxProvisioningResponse::Unavailable);
        }
        crate::service::log_error(
            crate::service::EVENT_REQUEST_REJECTED,
            &format!("Codex sandbox provisioning was rejected by administrator policy: {error}"),
        );
        return Err(error)
            .context("requested sandbox settings violate administrator-controlled machine policy");
    }
    crate::provisioning::run(identity, request, register_installation)
}

fn is_config_parse_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.is::<toml::de::Error>()
            || cause
                .downcast_ref::<std::io::Error>()
                .and_then(std::io::Error::get_ref)
                .is_some_and(<dyn std::error::Error + Send + Sync>::is::<toml::de::Error>)
    })
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod tests;
