//! Binds provisioning requests to the packaged Codex client and its Windows user.

use std::io;
#[cfg(debug_assertions)]
use std::sync::atomic::AtomicBool;
#[cfg(debug_assertions)]
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security as security;
use windows_sys::Win32::System::Pipes;
use windows_sys::Win32::System::Threading;

mod registered;

pub(crate) use registered::authorize_setup_runtime;
#[cfg(test)]
pub(crate) use registered::require_runtime_package_family;
#[cfg(debug_assertions)]
static FOREGROUND_MODE: AtomicBool = AtomicBool::new(false);

struct OwnedHandle(HANDLE);

pub(crate) struct AuthorizedClientProcess {
    handle: OwnedHandle,
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.0 != 0 && self.0 != foundation::INVALID_HANDLE_VALUE {
            unsafe { foundation::CloseHandle(self.0) };
        }
    }
}

#[cfg(debug_assertions)]
pub(crate) fn enable_foreground_mode() {
    FOREGROUND_MODE.store(true, Ordering::Release);
}

pub(crate) fn authorize_client_process(pipe: HANDLE) -> Result<AuthorizedClientProcess> {
    let mut process_id = 0;
    if unsafe { Pipes::GetNamedPipeClientProcessId(pipe, &mut process_id) } == 0 || process_id == 0
    {
        return Err(io::Error::last_os_error()).context("identify the provisioning client process");
    }

    let process = unsafe {
        Threading::OpenProcess(Threading::PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id)
    };
    if process == 0 {
        return Err(io::Error::last_os_error()).context("open the provisioning client process");
    }
    let process = OwnedHandle(process);

    let client_family = unsafe { codex_windows_sandbox::process_package_family(process.0) }
        .context("read provisioning client package identity")?;
    let service_family =
        unsafe { codex_windows_sandbox::process_package_family(Threading::GetCurrentProcess()) }
            .context("read provisioning service package identity")?;
    match client_family {
        Some(client_family) => match service_family.as_deref() {
            Some(service_family) if client_family != service_family => {
                bail!("provisioning client package does not match the service package")
            }
            Some(_) => {}
            #[cfg(debug_assertions)]
            None if FOREGROUND_MODE.load(Ordering::Acquire)
                && is_known_codex_package_family(&client_family) => {}
            #[cfg(debug_assertions)]
            None if FOREGROUND_MODE.load(Ordering::Acquire) => {
                bail!("provisioning client does not belong to a trusted Codex package family")
            }
            None => bail!("provisioning service has no package identity"),
        },
        None if service_family.is_some() => {}
        #[cfg(debug_assertions)]
        None if FOREGROUND_MODE.load(Ordering::Acquire) => {}
        None => bail!("provisioning clients must run with an installed Codex package identity"),
    }

    Ok(AuthorizedClientProcess { handle: process })
}

pub(crate) fn authorize_client(
    process: &AuthorizedClientProcess,
    client_token: HANDLE,
) -> Result<()> {
    let mut process_token = 0;
    if unsafe {
        Threading::OpenProcessToken(process.handle.0, security::TOKEN_QUERY, &mut process_token)
    } == 0
    {
        return Err(io::Error::last_os_error())
            .context("open the provisioning client process token");
    }
    let process_token = OwnedHandle(process_token);
    let process_user = unsafe { codex_windows_sandbox::get_user_sid_bytes(process_token.0) }
        .context("read the client process user")?;
    let impersonated_user = unsafe { codex_windows_sandbox::get_user_sid_bytes(client_token) }
        .context("read the impersonated client user")?;
    if process_user != impersonated_user {
        bail!("provisioning client process does not belong to the impersonated user");
    }

    Ok(())
}

#[cfg(debug_assertions)]
fn is_known_codex_package_family(package_family: &str) -> bool {
    matches!(
        package_family,
        "OpenAI.Codex_3k8sg7r9htsxt"
            | "OpenAI.CodexAlpha_3k8sg7r9htsxt"
            | "OpenAI.CodexBeta_3k8sg7r9htsxt"
            | "OpenAI.CodexNightly_3k8sg7r9htsxt"
    )
}
