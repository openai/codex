//! Provisions an authenticated client's sandbox through its selected setup path.

mod registered;

use std::os::windows::fs::MetadataExt;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::IntoRawHandle;
use std::sync::atomic::AtomicBool;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_windows_sandbox::SandboxProvisioningResponse;
use codex_windows_sandbox::SetupRuntime;
use codex_windows_sandbox::run_elevated_provisioning_setup_with_retained_handles;
use windows_sys::Win32::Storage::FileSystem as filesystem;

use crate::installation_record::InstallationRecord;
use crate::ipc::ClientIdentity;
use crate::ipc::OwnedHandle;
use crate::ipc::ServiceRequest;

// The lifecycle callback validates ownership and returns the canonical saved record.
fn register_owner(
    identity: &ClientIdentity,
    on_authenticated_user: &dyn Fn(
        InstallationRecord,
        OwnedHandle,
        codex_windows_sandbox::SetupRuntime,
    ) -> Result<InstallationRecord>,
) -> Result<InstallationRecord> {
    let token = unsafe { BorrowedHandle::borrow_raw(identity.token.0 as _) }
        .try_clone_to_owned()
        .context("retain authenticated uninstall owner")?;
    on_authenticated_user(
        InstallationRecord {
            codex_home: identity.codex_home.clone(),
            user_sid: identity.user_sid.clone(),
            session_id: identity.session_id,
            desktop_installation: identity.desktop_installation.clone(),
            runtime: None,
        },
        OwnedHandle(token.into_raw_handle() as _),
        identity.runtime,
    )
}

fn setup_is_complete(
    identity: &ClientIdentity,
    settings: &codex_windows_sandbox::WindowsSandboxProvisioningSettings,
) -> Result<bool> {
    crate::package_lifecycle::with_owner_impersonation(identity.token.0, || {
        Ok(
            codex_windows_sandbox::sandbox_setup_is_complete_with_settings(
                &identity.codex_home,
                settings,
            ),
        )
    })
}

pub(crate) fn run(
    identity: ClientIdentity,
    request: ServiceRequest,
    sandbox_sid: &[u8],
    shutdown: &AtomicBool,
    on_authenticated_user: &dyn Fn(
        InstallationRecord,
        OwnedHandle,
        codex_windows_sandbox::SetupRuntime,
    ) -> Result<InstallationRecord>,
) -> Result<SandboxProvisioningResponse> {
    let request = match request {
        ServiceRequest::RegisterInstallation { .. } => {
            let installation = InstallationRecord {
                codex_home: identity.codex_home,
                user_sid: identity.user_sid,
                session_id: identity.session_id,
                desktop_installation: identity.desktop_installation,
                runtime: None,
            };
            on_authenticated_user(installation, identity.token, identity.runtime)?;
            return Ok(SandboxProvisioningResponse::Ok);
        }
        ServiceRequest::ProvisionSandbox(request) => request,
    };
    if identity.runtime == SetupRuntime::Registered {
        return registered::run(
            &identity,
            request,
            sandbox_sid,
            shutdown,
            on_authenticated_user,
        );
    }
    let settings = request.settings;
    register_owner(&identity, on_authenticated_user)?;
    if setup_is_complete(&identity, &settings)? {
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
        settings,
        identity.runtime,
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
