//! Resume gated registration refresh only after an authenticated service restart.
//! Only response-side pipe disconnections qualify, never failed authentication,
//! failed request writes, explicit replies, or protocol errors.

use std::fs::File;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use windows_sys::Win32::Foundation::ERROR_BROKEN_PIPE;
use windows_sys::Win32::Foundation::ERROR_NO_DATA;
use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Services::SERVICE_START_PENDING;

use crate::FramedProvisioningMessage;
use crate::SandboxProvisioningResponse;

// Registering the service-bearing package can restart the service once for each
// of the two managed accounts. Keep both retries inside the original deadline.
const MAX_SERVICE_RESTARTS: usize = 2;
const RECONNECT_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn send(
    mut pipe: File,
    request: &FramedProvisioningMessage,
    deadline: Instant,
) -> anyhow::Result<SandboxProvisioningResponse> {
    let mut restarts_remaining = MAX_SERVICE_RESTARTS;
    loop {
        remaining_time(deadline)?;
        let server_pid = super::verify_server(pipe.as_raw_handle() as HANDLE)
            .context("authenticate provisioning pipe server")?;
        crate::write_provisioning_frame(&mut pipe, request)
            .context("send sandbox provisioning request")?;
        let response = super::read_response(&mut pipe, deadline);
        if !take_restart(&response, &mut restarts_remaining, deadline) {
            return response;
        }
        drop(pipe);
        pipe = connect_after_restart(server_pid, deadline)?;
    }
}

fn connect_after_restart(previous_pid: u32, deadline: Instant) -> anyhow::Result<File> {
    loop {
        remaining_time(deadline)?;
        if let Some(pipe) = super::connect(deadline)?
            && super::pipe_server_process_id(pipe.as_raw_handle() as HANDLE)? != previous_pid
            // The service creates its listener before reporting RUNNING.
            && super::query_service_status()?.dwCurrentState != SERVICE_START_PENDING
        {
            // The caller authenticates this new PID against the running SCM
            // service before writing. A same-PID reuse is conservatively refused.
            return Ok(pipe);
        }
        std::thread::sleep(remaining_time(deadline)?.min(RECONNECT_POLL_INTERVAL));
    }
}

fn remaining_time(deadline: Instant) -> io::Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "timed out waiting for restarted sandbox provisioning service",
        ));
    }
    Ok(remaining)
}

fn take_restart(
    response: &anyhow::Result<SandboxProvisioningResponse>,
    restarts_remaining: &mut usize,
    deadline: Instant,
) -> bool {
    let Err(error) = response else {
        return false;
    };
    if *restarts_remaining == 0
        || Instant::now() >= deadline
        || !error.downcast_ref::<io::Error>().is_some_and(|error| {
            matches!(
                error.kind(),
                io::ErrorKind::UnexpectedEof | io::ErrorKind::BrokenPipe
            ) || matches!(
                error.raw_os_error(),
                Some(code)
                    if code == ERROR_BROKEN_PIPE as i32
                        || code == ERROR_NO_DATA as i32
                        || code == ERROR_PIPE_NOT_CONNECTED as i32
            )
        })
    {
        return false;
    }
    *restarts_remaining -= 1;
    true
}

#[cfg(test)]
#[path = "refresh_retry_tests.rs"]
mod tests;
