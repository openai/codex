//! Owns a provisioning pipe and the sandbox group generation in its deny rule.
//! Replaces stale listeners only after a response, closing the old pipe before creating another.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_windows_sandbox::ensure_sandbox_users_group;
use codex_windows_sandbox::string_from_sid_bytes;
use codex_windows_sandbox::to_wide;
use std::mem::size_of;
use std::ptr;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::Security as security;
use windows_sys::Win32::Security::Authorization as authorization;
use windows_sys::Win32::Storage::FileSystem as filesystem;
use windows_sys::Win32::System::Pipes as pipes;

use super::MAX_REQUEST_BYTES;
use super::OwnedHandle;
use super::pipe_security_descriptor;

pub(super) struct ProvisioningListener {
    pub(super) pipe: OwnedHandle,
    pub(super) sandbox_sid: Vec<u8>,
    _descriptor: SecurityDescriptor,
}

impl ProvisioningListener {
    pub(super) fn open() -> Result<Self> {
        let pipe_name = codex_windows_sandbox::windows_sandbox_service_pipe_name()?;
        let _setup_lock =
            codex_windows_sandbox::acquire_sandbox_setup_lock(/*timeout_ms*/ 5_000)?;
        if crate::installation_record::load()?
            .and_then(|record| record.runtime)
            .is_some_and(|runtime| runtime.retiring.is_some())
        {
            bail!("interrupted sandbox cleanup requires repair");
        }
        let sandbox_sid = ensure_sandbox_users_group()?;
        let (descriptor, pipe) = create_provisioning_pipe(&pipe_name, &sandbox_sid)?;
        Ok(Self {
            pipe,
            sandbox_sid,
            _descriptor: descriptor,
        })
    }

    pub(super) fn refresh(self) -> Result<Self> {
        let current_group = {
            let _setup_lock =
                codex_windows_sandbox::acquire_sandbox_setup_lock(/*timeout_ms*/ 5_000)?;
            codex_windows_sandbox::resolve_sid(codex_windows_sandbox::SANDBOX_USERS_GROUP)
        };
        if current_group.is_ok_and(|current| current == self.sandbox_sid) {
            unsafe { pipes::DisconnectNamedPipe(self.pipe.0) };
            return Ok(self);
        }
        // A stale deny SID was refused before dispatch. Close this first-instance
        // pipe before rebuilding its descriptor for the current group generation.
        drop(self);
        Self::open()
    }
}

struct SecurityDescriptor(security::PSECURITY_DESCRIPTOR);

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe { foundation::LocalFree(self.0 as foundation::HLOCAL) };
    }
}

/// Preserves the existing listener and descriptor lifetimes.
fn create_provisioning_pipe(
    pipe_name: &str,
    sandbox_sid: &[u8],
) -> Result<(SecurityDescriptor, OwnedHandle)> {
    let sid_string = string_from_sid_bytes(sandbox_sid).map_err(anyhow::Error::msg)?;
    let sddl = pipe_security_descriptor(&sid_string);
    let mut descriptor: security::PSECURITY_DESCRIPTOR = ptr::null_mut();
    if unsafe {
        authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW(
            to_wide(sddl).as_ptr(),
            authorization::SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("create provisioning pipe DACL");
    }
    let descriptor = SecurityDescriptor(descriptor);
    let attributes = security::SECURITY_ATTRIBUTES {
        nLength: size_of::<security::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };

    let pipe = unsafe {
        pipes::CreateNamedPipeW(
            to_wide(pipe_name).as_ptr(),
            filesystem::PIPE_ACCESS_DUPLEX | filesystem::FILE_FLAG_FIRST_PIPE_INSTANCE,
            pipes::PIPE_TYPE_BYTE
                | pipes::PIPE_READMODE_BYTE
                | pipes::PIPE_WAIT
                | pipes::PIPE_REJECT_REMOTE_CLIENTS,
            1,
            1024,
            MAX_REQUEST_BYTES as u32,
            0,
            &attributes,
        )
    };
    if pipe == foundation::INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error()).context("create provisioning pipe");
    }
    Ok((descriptor, OwnedHandle(pipe)))
}
