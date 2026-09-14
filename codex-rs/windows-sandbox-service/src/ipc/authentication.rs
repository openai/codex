//! Authenticates provisioning clients and validates impersonated machine policy.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_windows_sandbox::DirectoryOpenDisposition;
use codex_windows_sandbox::SetupRuntime;
use codex_windows_sandbox::create_directory_guard;
use codex_windows_sandbox::string_from_sid_bytes;
use std::ffi::OsString;
use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::IntoRawHandle;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security as security;
use windows_sys::Win32::Storage::FileSystem as filesystem;
use windows_sys::Win32::System::Pipes as pipes;
use windows_sys::Win32::System::Threading as threading;

use super::home::OwnedHandle;
use super::home::prepare_codex_home;
use super::request::ServiceRequest;

pub(crate) struct ClientIdentity {
    pub(crate) account: String,
    pub(crate) codex_home: PathBuf,
    pub(crate) user_sid: String,
    pub(crate) session_id: u32,
    // The requested route, authenticated against the held client's installed image.
    pub(crate) runtime: SetupRuntime,
    pub(crate) token: OwnedHandle,
    pub(crate) desktop_installation: Option<crate::installation_record::DesktopInstallation>,
    // Pins and guards remain live through registration and the provisioning helper.
    pub(crate) directory_handles: Vec<OwnedHandle>,
}

pub(super) fn authenticate_client(
    pipe: HANDLE,
    authorized_process: &crate::package_identity::AuthorizedClientProcess,
    sandbox_sid: &[u8],
    request: &ServiceRequest,
) -> Result<(ClientIdentity, Result<()>)> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let identity = authenticate_impersonated_client(
                    pipe,
                    authorized_process,
                    sandbox_sid,
                    request,
                )?;
                let policy_result = match request {
                    // Registration does not provision resources or change sandbox policy.
                    ServiceRequest::RegisterInstallation { .. } => Ok(()),
                    ServiceRequest::ProvisionSandbox(request) => {
                        crate::machine_policy::validate_provisioning_settings(
                            &identity.codex_home,
                            &request.settings,
                            &request.listeners,
                            identity.token.0,
                        )
                    }
                };
                Ok((identity, policy_result))
            })
            .join()
            .map_err(|_| anyhow::anyhow!("provisioning client authentication thread panicked"))?
    })
}

fn authenticate_impersonated_client(
    pipe: HANDLE,
    authorized_process: &crate::package_identity::AuthorizedClientProcess,
    sandbox_sid: &[u8],
    request: &ServiceRequest,
) -> Result<ClientIdentity> {
    // Pin the group generation through identity capture.
    // Release this lock before the provisioning worker checks machine policy.
    let _setup_lock = codex_windows_sandbox::acquire_sandbox_setup_lock(/*timeout_ms*/ 5_000)?;
    anyhow::ensure!(
        codex_windows_sandbox::resolve_sid(codex_windows_sandbox::SANDBOX_USERS_GROUP)
            .is_ok_and(|current| current == sandbox_sid),
        codex_windows_sandbox::SANDBOX_GROUP_CHANGED
    );
    if unsafe { pipes::ImpersonateNamedPipeClient(pipe) } == 0 {
        return Err(std::io::Error::last_os_error()).context("impersonate provisioning client");
    }
    let mut raw_token = 0;
    if unsafe {
        threading::OpenThreadToken(
            threading::GetCurrentThread(),
            security::TOKEN_QUERY | security::TOKEN_IMPERSONATE,
            1,
            &mut raw_token,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("open provisioning client token");
    }
    let token = OwnedHandle(raw_token);
    crate::package_identity::authorize_client(authorized_process, token.0)
        .context("authorize the packaged Codex provisioning client")?;
    let mut session = 0_u32;
    let mut returned = 0_u32;
    if unsafe {
        security::GetTokenInformation(
            token.0,
            security::TokenSessionId,
            (&raw mut session).cast(),
            size_of::<u32>() as u32,
            &mut returned,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("read provisioning client session");
    }
    if session == 0 {
        bail!("non-interactive service accounts cannot request provisioning");
    }

    let mut is_sandbox_member = 0;
    if unsafe {
        security::CheckTokenMembership(
            token.0,
            sandbox_sid.as_ptr() as *mut c_void,
            &mut is_sandbox_member,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("check sandbox group membership");
    }
    if is_sandbox_member != 0 {
        bail!("sandbox accounts cannot request provisioning");
    }

    let user = unsafe { codex_windows_sandbox::get_user_sid_bytes(token.0) }
        .context("read provisioning client identity")?;
    let user_sid = string_from_sid_bytes(&user).map_err(anyhow::Error::msg)?;
    let account = unsafe { codex_windows_sandbox::account_name_from_sid(user.as_ptr() as _) }
        .context("resolve provisioning client account")?;
    let runtime = crate::package_identity::authorize_setup_runtime(authorized_process, request)?;
    let requested_home = match request {
        ServiceRequest::RegisterInstallation { codex_home } => codex_home,
        ServiceRequest::ProvisionSandbox(request) => &request.codex_home,
    };
    let (codex_home, handles) = match request {
        ServiceRequest::ProvisionSandbox(request) => prepare_codex_home(
            requested_home,
            runtime,
            if request.refresh_only {
                DirectoryOpenDisposition::OpenExisting
            } else {
                DirectoryOpenDisposition::OpenOrCreate
            },
        )?,
        ServiceRequest::RegisterInstallation { .. } => {
            // Registration must not create the sandbox directories or change their ACLs.
            codex_windows_sandbox::validate_local_directory_path(requested_home)?;
            let mut handles = Vec::new();
            super::home::pin_existing_ancestors(requested_home, &mut handles)?;
            // Uninstall removes sandbox files as SYSTEM; read access cannot grant that authority.
            let home = super::home::pin_directory(
                requested_home,
                filesystem::FILE_ADD_FILE
                    | filesystem::FILE_ADD_SUBDIRECTORY
                    | filesystem::WRITE_DAC,
                DirectoryOpenDisposition::OpenExisting,
            )?;
            // Bind the guard to the authorized directory before resolving its pathname.
            let guard = create_directory_guard(unsafe { BorrowedHandle::borrow_raw(home.0 as _) })?;
            drop(super::home::pin_directory(
                requested_home,
                filesystem::FILE_READ_ATTRIBUTES,
                DirectoryOpenDisposition::OpenExisting,
            )?);
            // Preserve the authorized directory's literal name (including trailing dots).
            let mut buffer = vec![0_u16; 260];
            let path = loop {
                let length = unsafe {
                    filesystem::GetFinalPathNameByHandleW(
                        home.0,
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                        filesystem::FILE_NAME_NORMALIZED | filesystem::VOLUME_NAME_DOS,
                    )
                };
                if length == 0 {
                    return Err(std::io::Error::last_os_error())
                        .context("resolve authorized desktop home");
                }
                if (length as usize) < buffer.len() {
                    break PathBuf::from(OsString::from_wide(&buffer[..length as usize]));
                }
                buffer.resize(length as usize, /*value*/ 0);
            };
            handles.push(home);
            handles.push(OwnedHandle(guard.into_raw_handle() as HANDLE));
            (path, handles)
        }
    };
    let desktop_installation =
        crate::installation_record::read_desktop_installation(&codex_home, token.0)
            .inspect_err(|_| {
                crate::service::log_error(
                    crate::service::EVENT_SERVICE_FAILED,
                    "unable to read desktop directory ownership; preserving desktop directories",
                );
            })
            .ok();
    Ok(ClientIdentity {
        account,
        codex_home,
        user_sid,
        session_id: session,
        runtime,
        token,
        desktop_installation,
        directory_handles: handles,
    })
}
