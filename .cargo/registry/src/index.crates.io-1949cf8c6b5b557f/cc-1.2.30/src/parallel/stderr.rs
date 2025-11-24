#![cfg_attr(target_family = "wasm", allow(unused))]
/// Helpers functions for [`ChildStderr`].
use std::{convert::TryInto, process::ChildStderr};

use crate::{Error, ErrorKind};

#[cfg(all(not(unix), not(windows), not(target_family = "wasm")))]
compile_error!("Only unix and windows support non-blocking pipes! For other OSes, disable the parallel feature.");

#[cfg(unix)]
fn get_flags(fd: std::os::unix::io::RawFd) -> Result<i32, Error> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags == -1 {
        Err(Error::new(
            ErrorKind::IOError,
            format!(
                "Failed to get flags for pipe {}: {}",
                fd,
                std::io::Error::last_os_error()
            ),
        ))
    } else {
        Ok(flags)
    }
}

#[cfg(unix)]
fn set_flags(fd: std::os::unix::io::RawFd, flags: std::os::raw::c_int) -> Result<(), Error> {
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags) } == -1 {
        Err(Error::new(
            ErrorKind::IOError,
            format!(
                "Failed to set flags for pipe {}: {}",
                fd,
                std::io::Error::last_os_error()
            ),
        ))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
pub fn set_non_blocking(pipe: &impl std::os::unix::io::AsRawFd) -> Result<(), Error> {
    // On Unix, switch the pipe to non-blocking mode.
    // On Windows, we have a different way to be non-blocking.
    let fd = pipe.as_raw_fd();

    let flags = get_flags(fd)?;
    set_flags(fd, flags | libc::O_NONBLOCK)
}

pub fn bytes_available(stderr: &mut ChildStderr) -> Result<usize, Error> {
    let mut bytes_available = 0;
    #[cfg(windows)]
    {
        use crate::windows::windows_sys::PeekNamedPipe;
        use std::os::windows::io::AsRawHandle;
        use std::ptr::null_mut;
        if unsafe {
            PeekNamedPipe(
                stderr.as_raw_handle(),
                null_mut(),
                0,
                null_mut(),
                &mut bytes_available,
                null_mut(),
            )
        } == 0
        {
            return Err(Error::new(
                ErrorKind::IOError,
                format!(
                    "PeekNamedPipe failed with {}",
                    std::io::Error::last_os_error()
                ),
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        if unsafe { libc::ioctl(stderr.as_raw_fd(), libc::FIONREAD, &mut bytes_available) } != 0 {
            return Err(Error::new(
                ErrorKind::IOError,
                format!("ioctl failed with {}", std::io::Error::last_os_error()),
            ));
        }
    }
    Ok(bytes_available.try_into().unwrap())
}
