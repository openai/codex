//! Queries held process package identities without trusting routing hints or environment state.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use std::io;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::Packaging::Appx::GetPackageFamilyName;

/// Reads a process's OS package family. Absence is not authorization.
///
/// # Safety
/// The caller must keep the process handle valid throughout the query.
pub unsafe fn process_package_family(process: HANDLE) -> Result<Option<String>> {
    let mut length = 0;
    let status = unsafe { GetPackageFamilyName(process, &mut length, std::ptr::null_mut()) };
    if status == foundation::APPMODEL_ERROR_NO_PACKAGE {
        return Ok(None);
    }
    if status != foundation::ERROR_INSUFFICIENT_BUFFER {
        return Err(io::Error::from_raw_os_error(status as i32))
            .context("query the process package family");
    }
    if length == 0 || length > 256 {
        bail!("the process package family has an invalid length");
    }

    let mut buffer = vec![0_u16; length as usize];
    let status = unsafe { GetPackageFamilyName(process, &mut length, buffer.as_mut_ptr()) };
    if status != foundation::ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32))
            .context("read the process package family");
    }

    let value = buffer
        .get(..length as usize)
        .context("the package-family API returned an invalid length")?;
    let Some((&0, value)) = value.split_last() else {
        bail!("the process package family is not null-terminated");
    };
    if value.is_empty() || value.contains(&0) {
        bail!("the process package family is malformed");
    }
    Ok(Some(
        String::from_utf16(value).context("the package family contains invalid UTF-16")?,
    ))
}
