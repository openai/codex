#[cfg(feature = "set")]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
#[cfg(feature = "set")]
use std::os::windows::ffi::OsStrExt;
use std::os::windows::ffi::OsStringExt;
use std::ptr;

mod bindings;
use bindings::{ComputerNamePhysicalDnsHostname, GetComputerNameExW, PWSTR};

pub fn get() -> io::Result<OsString> {
    let mut size = 0;
    unsafe {
        // Don't care much about the result here,
        // it is guaranteed to return an error,
        // since we passed the NULL pointer as a buffer
        let result =
            GetComputerNameExW(ComputerNamePhysicalDnsHostname, ptr::null_mut(), &mut size);
        debug_assert_eq!(result, 0);
    };

    let mut buffer = Vec::with_capacity(size as usize);

    let result = unsafe {
        GetComputerNameExW(
            ComputerNamePhysicalDnsHostname,
            PWSTR::from(buffer.as_mut_ptr()),
            &mut size,
        )
    };

    match result {
        0 => Err(io::Error::last_os_error()),
        _ => {
            unsafe {
                buffer.set_len(size as usize);
            }

            Ok(OsString::from_wide(&buffer))
        }
    }
}

#[cfg(feature = "set")]
pub fn set(hostname: &OsStr) -> io::Result<()> {
    use bindings::{SetComputerNameExW, PCWSTR};

    let mut buffer = hostname.encode_wide().collect::<Vec<_>>();
    buffer.push(0x00); // Appending the null terminator

    let result = unsafe {
        SetComputerNameExW(
            ComputerNamePhysicalDnsHostname,
            PCWSTR::from(buffer.as_ptr()),
        )
    };

    match result {
        0 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}
