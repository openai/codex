//! Admits registered Core only from the service's installed package version and
//! prevents legacy setup from replacing accounts owned by a registered runtime.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_windows_sandbox::SetupRuntime;
use windows_sys::Win32::System::Threading;

use super::AuthorizedClientProcess;
use crate::ipc::ServiceRequest;

pub(crate) fn authorize_setup_runtime(
    process: &AuthorizedClientProcess,
    request: &ServiceRequest,
) -> Result<SetupRuntime> {
    match request {
        ServiceRequest::ProvisionSandbox(request) if request.registered_core => {
            authorize_runtime_client_process(process)?;
            Ok(SetupRuntime::Registered)
        }
        ServiceRequest::ProvisionSandbox(_) => {
            anyhow::ensure!(
                crate::installation_record::load_runtime()?.is_none(),
                "registered Core owns these sandbox accounts; legacy provisioning is not permitted"
            );
            Ok(SetupRuntime::Legacy)
        }
        ServiceRequest::RegisterInstallation { .. } => Ok(SetupRuntime::Legacy),
    }
}

fn authorize_runtime_client_process(process: &AuthorizedClientProcess) -> Result<()> {
    let client_family = unsafe { codex_windows_sandbox::process_package_family(process.handle.0) }
        .context("read runtime registration client package family")?;
    let service_family =
        unsafe { codex_windows_sandbox::process_package_family(Threading::GetCurrentProcess()) }
            .context("read runtime registration service package family")?;
    require_runtime_package_family(client_family.as_deref(), service_family.as_deref())?;
    let mut image = [0u16; 32768];
    let mut length = image.len() as u32;
    if unsafe {
        Threading::QueryFullProcessImageNameW(process.handle.0, 0, image.as_mut_ptr(), &mut length)
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("read provisioning client image");
    }
    let image = std::path::PathBuf::from(String::from_utf16(&image[..length as usize])?);
    let image = image.canonicalize()?;
    let service = std::env::current_exe()?.canonicalize()?;
    let directory = service.parent().context("service image has no directory")?;
    anyhow::ensure!(
        ["codex.exe", "codex-code-mode-host.exe"]
            .iter()
            .any(|name| {
                image
                    .as_os_str()
                    .eq_ignore_ascii_case(directory.join(name).as_os_str())
            }),
        "registered Core caller is not from this installed service version"
    );
    Ok(())
}

pub(crate) fn require_runtime_package_family(
    client: Option<&str>,
    service: Option<&str>,
) -> Result<()> {
    match (client, service) {
        (Some(client), Some(service)) if client == service => Ok(()),
        _ => bail!(
            "runtime registration requires a client in the service's installed package family"
        ),
    }
}
