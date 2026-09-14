//! Restores owner-scoped uninstall notifications and ties file cleanup to pinned directories.

use std::cell::RefCell;
use std::io;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::IntoRawHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_windows_sandbox::DirectoryOpenDisposition;
use codex_windows_sandbox::create_directory_guard;
use codex_windows_sandbox::string_from_sid_bytes;
use windows::ApplicationModel::Package;
use windows::ApplicationModel::PackageCatalog;
use windows::ApplicationModel::PackageUninstallingEventArgs;
use windows::Foundation::EventRegistrationToken;
use windows::Foundation::TypedEventHandler;
use windows::Win32::System::WinRT::RO_INIT_MULTITHREADED;
use windows::Win32::System::WinRT::RoInitialize;
use windows::Win32::System::WinRT::RoUninitialize;
use windows::core::HSTRING;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security as security;
use windows_sys::Win32::Storage::FileSystem as filesystem;
use windows_sys::Win32::System::RemoteDesktop::WTS_CURRENT_SERVER_HANDLE;
use windows_sys::Win32::System::RemoteDesktop::WTSEnumerateSessionsW;
use windows_sys::Win32::System::RemoteDesktop::WTSFreeMemory;
use windows_sys::Win32::System::RemoteDesktop::WTSQueryUserToken;

use crate::installation_record::InstallationRecord;
use crate::ipc::OwnedHandle;

mod cleanup;

struct UserInstallation {
    codex_home: Option<PathBuf>,
    // Ancestors, home, then a guard that prevents in-place junction conversion.
    directory_handles: Vec<OwnedHandle>,
    record: InstallationRecord,
    user_token: OwnedHandle,
    catalog: PackageCatalog,
    token: EventRegistrationToken,
}

pub(crate) struct PackageLifecycle {
    package_name: HSTRING,
    uninstalling: Arc<AtomicBool>,
    installation: RefCell<Option<UserInstallation>>,
}

impl PackageLifecycle {
    pub(crate) fn new(uninstalling: Arc<AtomicBool>) -> Result<Self> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .context("initialize the sandbox service Windows Runtime apartment")?;
        Ok(Self {
            package_name: Package::Current()?.Id()?.FullName()?,
            uninstalling,
            installation: RefCell::default(),
        })
    }

    pub(crate) fn register_authenticated_user(
        &self,
        mut record: InstallationRecord,
        user_token: OwnedHandle,
    ) -> Result<InstallationRecord> {
        let mut active = self.installation.borrow_mut();
        let previous = match active.as_ref() {
            Some(installation) => Some(installation.record.clone()),
            None => crate::installation_record::load()?,
        };
        if let Some(previous) = previous {
            // A missing watcher does not retire its owner; other clients can use elevated setup.
            ensure!(
                previous.user_sid == record.user_sid && previous.codex_home == record.codex_home,
                crate::ipc::ServiceUnavailable(
                    "installation is already registered to a different owner or home"
                )
            );
            record.desktop_installation = previous
                .desktop_installation
                .or(record.desktop_installation);
        }
        crate::installation_record::save(&record)?;
        if let Some(installation) = active.as_mut()
            && installation.codex_home.is_some()
        {
            // A restored watcher must immediately use newly registered desktop ownership.
            installation.record = record.clone();
            return Ok(record);
        }

        let saved_record = record.clone();
        with_owner_impersonation(user_token.0, || {
            let mut directory_handles = Vec::new();
            let codex_home = match crate::ipc::pin_existing_ancestors(
                &record.codex_home,
                &mut directory_handles,
            )
            .and_then(|()| {
                let home = directory_handles
                    .last()
                    .context("pin the registered home")?;
                let guard =
                    create_directory_guard(unsafe { BorrowedHandle::borrow_raw(home.0 as _) })?;
                // Reject a conversion that happened before the handle-relative guard was created.
                drop(crate::ipc::pin_directory(
                    &record.codex_home,
                    filesystem::FILE_READ_ATTRIBUTES,
                    DirectoryOpenDisposition::OpenExisting,
                )?);
                directory_handles.push(OwnedHandle(guard.into_raw_handle() as HANDLE));
                Ok(())
            }) {
                Ok(()) => Some(record.codex_home.clone()),
                Err(error) => {
                    directory_handles.clear();
                    crate::service::log_error(
                        crate::service::EVENT_SERVICE_FAILED,
                        &format!(
                            "skipping sandbox file cleanup because the home could not be pinned: {error:#}"
                        ),
                    );
                    None
                }
            };
            if let Some(installation) = active.as_mut() {
                installation.codex_home = codex_home;
                installation.directory_handles = directory_handles;
                installation.record = record;
                return Ok(());
            }
            let catalog = PackageCatalog::OpenForCurrentUser().context(
                crate::ipc::ServiceUnavailable("open the authenticated user's package catalog"),
            )?;
            let package_name = self.package_name.clone();
            let uninstalling = Arc::clone(&self.uninstalling);
            let token = catalog
                .PackageUninstalling(&TypedEventHandler::<
                    PackageCatalog,
                    PackageUninstallingEventArgs,
                >::new(move |_, event| {
                    if let Some(event) = event
                        && event.Package()?.Id()?.FullName()? == package_name
                    {
                        uninstalling.store(!event.IsComplete()?, Ordering::Release);
                    }
                    Ok(())
                }))
                .context(crate::ipc::ServiceUnavailable(
                    "subscribe to authenticated package uninstall notifications",
                ))?;
            active.replace(UserInstallation {
                codex_home,
                directory_handles,
                record,
                user_token,
                catalog,
                token,
            });
            Ok(())
        })?;
        Ok(saved_record)
    }

    pub(crate) fn restore_logged_in_owner(&self, recorded_session_id: u32) -> Result<()> {
        let Err(error) = self.restore_authenticated_user(recorded_session_id) else {
            return Ok(());
        };

        // Session IDs can change while the service is stopped; the owner SID is durable.
        let mut sessions = std::ptr::null_mut();
        let mut count = 0;
        if unsafe {
            WTSEnumerateSessionsW(
                WTS_CURRENT_SERVER_HANDLE,
                /*reserved*/ 0,
                /*version*/ 1,
                &mut sessions,
                &mut count,
            )
        } == 0
        {
            return Err(io::Error::last_os_error()).context("list current Windows sessions");
        }
        let restored = (0..count).any(|index| {
            let session_id = unsafe { (*sessions.add(index as usize)).SessionId };
            session_id != recorded_session_id && self.restore_authenticated_user(session_id).is_ok()
        });
        unsafe { WTSFreeMemory(sessions.cast()) };
        if restored { Ok(()) } else { Err(error) }
    }

    pub(crate) fn restore_authenticated_user(&self, session_id: u32) -> Result<()> {
        if self.installation.borrow().is_some() {
            return Ok(());
        }
        let Some(record) = crate::installation_record::load()? else {
            return Ok(());
        };

        let mut raw_token = 0;
        if unsafe { WTSQueryUserToken(session_id, &mut raw_token) } == 0 {
            return Err(io::Error::last_os_error()).context("open the logged-in user's token");
        }
        let token = crate::ipc::OwnedHandle(raw_token);
        let user = crate::package_identity::token_user(token.0)?;
        let sid = unsafe { std::ptr::read_unaligned(user.as_ptr().cast::<security::TOKEN_USER>()) }
            .User
            .Sid;
        let sid_length = unsafe { security::GetLengthSid(sid) };
        if sid_length == 0 {
            return Err(io::Error::last_os_error()).context("read the logged-in user's SID");
        }
        let user_sid = string_from_sid_bytes(unsafe {
            std::slice::from_raw_parts(sid.cast::<u8>(), sid_length as usize)
        })
        .map_err(anyhow::Error::msg)?;
        ensure!(
            user_sid == record.user_sid,
            "logged-in user does not match the recorded sandbox owner"
        );

        self.register_authenticated_user(
            InstallationRecord {
                session_id,
                ..record
            },
            token,
        )
        .map(|_| ())
    }

    pub(crate) fn clean_up(&self) -> Result<()> {
        let mut installation = self.installation.borrow_mut();
        let installation = installation
            .as_mut()
            .context("the authenticated package installation was not recorded")?;
        cleanup::clean_up(installation)
    }
}

fn with_owner_impersonation(
    user_token: HANDLE,
    operation: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if unsafe { security::ImpersonateLoggedOnUser(user_token) } == 0 {
        return Err(io::Error::last_os_error()).context("impersonate the sandbox owner");
    }
    let result = operation();
    if unsafe { security::RevertToSelf() } == 0 {
        crate::service::log_error(
            crate::service::EVENT_SERVICE_FAILED,
            &format!(
                "unable to revert sandbox-owner impersonation: {}",
                io::Error::last_os_error()
            ),
        );
        // Continuing as the user would make later machine cleanup unsafe.
        std::process::abort();
    }
    result
}

impl Drop for PackageLifecycle {
    fn drop(&mut self) {
        if let Some(installation) = self.installation.get_mut().take() {
            let _ = installation
                .catalog
                .RemovePackageUninstalling(installation.token);
        }
        unsafe { RoUninitialize() };
    }
}
