//! IOCP-backed MCP pipes with private, preconnected child endpoints.

use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::ptr;

use codex_utils_pty::ChildStdin;
use codex_utils_pty::Command;
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::net::windows::named_pipe::PipeMode;
use tokio::net::windows::named_pipe::ServerOptions;
use uuid::Uuid;
use windows_sys::Win32::Foundation::GENERIC_WRITE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::Authorization::SDDL_REVISION_1;
use windows_sys::Win32::Security::PSECURITY_DESCRIPTOR;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::SECURITY_IDENTIFICATION;
use windows_sys::Win32::Storage::FileSystem::SECURITY_SQOS_PRESENT;

pub(super) struct Stdio {
    pub(super) stdin: NamedPipeServer,
    pub(super) stdout: NamedPipeServer,
    pub(super) stderr: NamedPipeServer,
}

impl Stdio {
    pub(super) fn configure(command: &mut Command) -> io::Result<Self> {
        let security = PipeSecurity::new()?;
        let (stdin, child_stdin) = connected_pipe(Direction::ChildReads, &security)?;
        let (stdout, child_stdout) = connected_pipe(Direction::ChildWrites, &security)?;
        let (stderr, child_stderr) = connected_pipe(Direction::ChildWrites, &security)?;
        command
            .stdin(ChildStdin::File(child_stdin.into()))
            .stdout_handle(child_stdout.into())
            .stderr_handle(child_stderr.into());
        Ok(Self {
            stdin,
            stdout,
            stderr,
        })
    }
}

enum Direction {
    ChildReads,
    ChildWrites,
}

fn connected_pipe(
    direction: Direction,
    security: &PipeSecurity,
) -> io::Result<(NamedPipeServer, File)> {
    let name = format!(r"\\.\pipe\codex-mcp-{}", Uuid::new_v4().simple());
    let mut options = ServerOptions::new();
    options
        .pipe_mode(PipeMode::Byte)
        .first_pipe_instance(true)
        .max_instances(1)
        .reject_remote_clients(true);
    let mut client = OpenOptions::new();
    client.security_qos_flags(SECURITY_IDENTIFICATION | SECURITY_SQOS_PRESENT);
    match direction {
        Direction::ChildReads => {
            options.access_inbound(false).access_outbound(true);
            client.read(true);
        }
        Direction::ChildWrites => {
            options.access_inbound(true).access_outbound(false);
            // Helpers can query the blocking mode of their stdout and stderr pipes.
            client.access_mode(GENERIC_WRITE | FILE_READ_ATTRIBUTES);
        }
    }
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).map_err(io::Error::other)?,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    // SAFETY: attributes and its owned descriptor stay live through CreateNamedPipeW.
    // The kernel copies the descriptor; the returned pipe owns its separate handle.
    let server = unsafe {
        options.create_with_security_attributes_raw(&name, ptr::from_mut(&mut attributes).cast())
    }?;
    let child = client.open(&name)?;
    // The child endpoint is already connected. Complete registration on the
    // launcher's blocking thread before the child is allowed to execute.
    tokio::runtime::Handle::current().block_on(server.connect())?;
    Ok((server, child))
}

struct PipeSecurity(PSECURITY_DESCRIPTOR);

impl PipeSecurity {
    fn new() -> io::Result<Self> {
        // Only the pipe owner and SYSTEM can open the single local instance.
        let sddl: Vec<u16> = "D:P(A;;GA;;;SY)(A;;GA;;;OW)"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut descriptor = ptr::null_mut();
        // SAFETY: sddl is NUL-terminated and descriptor is a valid writable out
        // pointer. On success Windows transfers a LocalFree-owned allocation.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }
}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner of the successful SDDL conversion's
        // allocation; no pipe creation call retains a pointer to it.
        if !unsafe { LocalFree(self.0) }.is_null() {
            tracing::warn!(
                "Failed to release MCP pipe security descriptor: {}",
                io::Error::last_os_error()
            );
        }
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
