//! Runs provisioning after IPC has authenticated the owner and checked machine policy.
//! The authenticated token and directory pins retain their existing helper lifetimes.

use std::os::windows::fs::MetadataExt;
use std::os::windows::io::BorrowedHandle;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_windows_sandbox::SandboxProvisioningResponse;
use codex_windows_sandbox::run_elevated_provisioning_setup_with_retained_handles;
use codex_windows_sandbox::sandbox_setup_is_complete_with_settings;
use windows_sys::Win32::Storage::FileSystem as filesystem;

use crate::installation_record::InstallationRecord;
use crate::ipc::ClientIdentity;
use crate::ipc::OwnedHandle;
use crate::ipc::ServiceRequest;

pub(crate) fn run(
    identity: ClientIdentity,
    request: ServiceRequest,
    register_installation: &dyn Fn(InstallationRecord, OwnedHandle) -> Result<InstallationRecord>,
) -> Result<SandboxProvisioningResponse> {
    // A policy-rejected request must not choose the uninstall owner. Use the
    // token already authenticated above instead of impersonating the pipe again.
    register_installation(
        InstallationRecord {
            codex_home: identity.codex_home.clone(),
            user_sid: identity.user_sid,
            session_id: identity.session_id,
            desktop_installation: identity.desktop_installation,
        },
        identity.token,
    )?;
    let request = match request {
        ServiceRequest::RegisterInstallation { .. } => {
            return Ok(SandboxProvisioningResponse::Ok);
        }
        ServiceRequest::ProvisionSandbox(request) => request,
    };
    if sandbox_setup_is_complete_with_settings(&identity.codex_home, &request.settings) {
        return Ok(SandboxProvisioningResponse::Ok);
    }
    let helper = std::env::current_exe()
        .context("locate the provisioning service executable")?
        .with_file_name("codex-windows-sandbox-setup.exe");
    let helper_metadata = helper
        .symlink_metadata()
        .with_context(|| format!("inspect packaged setup helper {}", helper.display()))?;
    if !helper_metadata.is_file()
        || helper_metadata.file_attributes() & filesystem::FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        bail!(
            "refusing invalid packaged setup helper {}",
            helper.display()
        );
    }
    let retained_handles = identity
        .directory_handles
        .iter()
        // The identity owns these handles through the synchronous helper launch and wait.
        .map(|handle| unsafe { BorrowedHandle::borrow_raw(handle.0 as _) })
        .collect::<Vec<_>>();
    match run_elevated_provisioning_setup_with_retained_handles(
        &identity.codex_home,
        &identity.account,
        request.settings,
        &retained_handles,
    ) {
        Ok(()) => {
            crate::service::log_information(
                crate::service::EVENT_PROVISIONING_SUCCEEDED,
                "Codex sandbox provisioning completed successfully.",
            );
            Ok(SandboxProvisioningResponse::Ok)
        }
        Err(error) => {
            crate::service::log_error(
                crate::service::EVENT_PROVISIONING_FAILED,
                &format!("Codex sandbox provisioning failed: {error}"),
            );
            Err(error).context("sandbox provisioning failed")
        }
    }
}
