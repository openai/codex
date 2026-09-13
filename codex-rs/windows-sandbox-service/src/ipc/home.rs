//! Validates and pins local Codex home and sandbox directories, keeping sandbox
//! leaves nonempty so they cannot become junctions during provisioning.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_windows_sandbox::DirectoryOpenDisposition;
use codex_windows_sandbox::create_directory_guard;
use codex_windows_sandbox::open_directory_no_reparse;
use codex_windows_sandbox::to_wide;
use codex_windows_sandbox::validate_local_directory_path;
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::IntoRawHandle;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem as filesystem;

use super::ServiceUnavailable;

const DRIVE_FIXED: u32 = 3;

pub(crate) struct OwnedHandle(pub(crate) HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.0 != 0 && self.0 != foundation::INVALID_HANDLE_VALUE {
            unsafe { foundation::CloseHandle(self.0) };
        }
    }
}

pub(super) fn prepare_codex_home(requested: &Path) -> Result<(PathBuf, Vec<OwnedHandle>)> {
    let mut handles = Vec::new();
    validate_local_directory_path(requested)?;
    let requested_root = requested
        .ancestors()
        .last()
        .context("find the root of the requested Codex home")?;
    if unsafe { filesystem::GetDriveTypeW(to_wide(requested_root.as_os_str()).as_ptr()) }
        != DRIVE_FIXED
    {
        return Err(ServiceUnavailable("Codex home must be located on a fixed local drive").into());
    }
    let parent = requested
        .parent()
        .context("Codex home must have an existing parent directory")?;
    pin_existing_ancestors(parent, &mut handles)?;
    handles.push(pin_directory(
        requested,
        filesystem::FILE_READ_ATTRIBUTES,
        DirectoryOpenDisposition::OpenOrCreate,
    )?);
    let home = requested
        .canonicalize()
        .with_context(|| format!("canonicalize Codex home {}", requested.display()))?;
    validate_local_directory_path(&home)?;
    let root = home
        .ancestors()
        .last()
        .context("find the root of the requested Codex home")?;
    if unsafe { filesystem::GetDriveTypeW(to_wide(root.as_os_str()).as_ptr()) } != DRIVE_FIXED {
        return Err(ServiceUnavailable("Codex home must be located on a fixed local drive").into());
    }
    if home != requested {
        pin_existing_ancestors(&home, &mut handles)?;
    }
    let sandbox_bin = home.join(".sandbox-bin");
    for (index, directory) in [
        home.as_path(),
        &home.join(".sandbox"),
        &home.join(".sandbox-secrets"),
        &sandbox_bin,
    ]
    .into_iter()
    .enumerate()
    {
        let mut access = filesystem::FILE_READ_ATTRIBUTES
            | filesystem::FILE_ADD_FILE
            | filesystem::FILE_ADD_SUBDIRECTORY;
        if index != 0 {
            access |= filesystem::WRITE_DAC;
        }
        let handle = pin_directory(directory, access, DirectoryOpenDisposition::OpenOrCreate)
            .map_err(|error| {
                // Older elevated installs omitted WRITE_DAC; let the interactive helper repair it.
                if directory == sandbox_bin.as_path()
                    && error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                        error.raw_os_error() == Some(foundation::ERROR_ACCESS_DENIED as i32)
                    })
                {
                    error.context(ServiceUnavailable("sandbox binary directory needs elevated repair"))
                } else {
                    error
                }
            })
            .with_context(|| {
            if index == 0 {
                format!(
                    "requesting user must be permitted to write Codex home {}",
                    directory.display()
                )
            } else {
                format!(
                    "requesting user must be permitted to write and modify sandbox directory ACL {}",
                    directory.display()
                )
            }
        })?;
        if index != 0 {
            // The handle is owned here and remains live until the guard is installed.
            let directory_handle = unsafe { BorrowedHandle::borrow_raw(handle.0 as _) };
            let guard = create_directory_guard(directory_handle)?;
            // Reject conversion before guard creation; after creation the retained
            // file prevents this directory from becoming empty and being converted.
            drop(pin_directory(
                directory,
                filesystem::FILE_READ_ATTRIBUTES,
                DirectoryOpenDisposition::OpenExisting,
            )?);
            handles.push(OwnedHandle(guard.into_raw_handle() as HANDLE));
        }
        handles.push(handle);
        if index == 0 {
            continue;
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = entry.path().symlink_metadata()?;
            if metadata.file_attributes() & filesystem::FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                bail!("refusing reparse point {}", entry.path().display());
            }
        }
    }

    // Pinned parent handles prevent directory replacement, but existing helper file
    // opens remain pathname-based; hardlink/leaf-file races need a follow-up redesign.
    Ok((home, handles))
}

pub(crate) fn pin_existing_ancestors(path: &Path, handles: &mut Vec<OwnedHandle>) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if !matches!(component, Component::Prefix(_)) && current.is_absolute() {
            handles.push(pin_directory(
                &current,
                filesystem::FILE_READ_ATTRIBUTES,
                DirectoryOpenDisposition::OpenExisting,
            )?);
        }
    }
    Ok(())
}

pub(crate) fn pin_directory(
    path: &Path,
    access: u32,
    disposition: DirectoryOpenDisposition,
) -> Result<OwnedHandle> {
    let handle = open_directory_no_reparse(
        path,
        // Metadata-only access does not enforce no-delete sharing.
        access | filesystem::FILE_TRAVERSE,
        filesystem::FILE_SHARE_READ | filesystem::FILE_SHARE_WRITE,
        disposition,
    )
    .with_context(|| format!("pin {}", path.display()))?;
    Ok(OwnedHandle(handle.into_raw_handle() as HANDLE))
}
