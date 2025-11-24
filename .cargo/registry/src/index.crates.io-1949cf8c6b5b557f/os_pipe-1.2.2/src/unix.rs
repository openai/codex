use crate::PipeReader;
use crate::PipeWriter;
use libc::c_int;
use std::fs::File;
use std::io;
use std::os::unix::prelude::*;

// We need to atomically create pipes and set the CLOEXEC flag on them. This is
// done with the pipe2() API. However, macOS doesn't support pipe2. There, all
// we can do is call pipe() followed by fcntl(), and hope that no other threads
// fork() in between. The following code is copied from the nix crate, where it
// works but is deprecated.
#[cfg(not(any(
    target_os = "aix",
    target_os = "ios",
    target_os = "visionos",
    target_os = "macos",
    target_os = "haiku"
)))]
fn pipe2_cloexec() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds: [c_int; 2] = [0; 2];
    let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }
    unsafe { Ok((OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1]))) }
}

#[cfg(any(
    target_os = "aix",
    target_os = "ios",
    target_os = "visionos",
    target_os = "macos",
    target_os = "haiku"
))]
fn pipe2_cloexec() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds: [c_int; 2] = [0; 2];
    let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }
    // Wrap the fds immediately, so that we'll drop them and close them in the unlikely event that
    // any of the following fcntls fails.
    let owned_fds = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    let res = unsafe { libc::fcntl(fds[0], libc::F_SETFD, libc::FD_CLOEXEC) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }
    let res = unsafe { libc::fcntl(fds[1], libc::F_SETFD, libc::FD_CLOEXEC) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(owned_fds)
}

pub(crate) fn pipe() -> io::Result<(PipeReader, PipeWriter)> {
    let (read_fd, write_fd) = pipe2_cloexec()?;
    Ok((read_fd.into(), write_fd.into()))
}

pub(crate) fn dup(handle: impl AsFd) -> io::Result<OwnedFd> {
    handle.as_fd().try_clone_to_owned()
}

impl IntoRawFd for PipeReader {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

impl AsRawFd for PipeReader {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl FromRawFd for PipeReader {
    unsafe fn from_raw_fd(fd: RawFd) -> PipeReader {
        unsafe { PipeReader(File::from_raw_fd(fd)) }
    }
}

impl From<PipeReader> for OwnedFd {
    fn from(pr: PipeReader) -> Self {
        pr.0.into()
    }
}

impl AsFd for PipeReader {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl From<OwnedFd> for PipeReader {
    fn from(fd: OwnedFd) -> Self {
        PipeReader(fd.into())
    }
}

impl IntoRawFd for PipeWriter {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

impl AsRawFd for PipeWriter {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl FromRawFd for PipeWriter {
    unsafe fn from_raw_fd(fd: RawFd) -> PipeWriter {
        unsafe { PipeWriter(File::from_raw_fd(fd)) }
    }
}

impl From<PipeWriter> for OwnedFd {
    fn from(pw: PipeWriter) -> Self {
        pw.0.into()
    }
}

impl AsFd for PipeWriter {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl From<OwnedFd> for PipeWriter {
    fn from(fd: OwnedFd) -> Self {
        PipeWriter(fd.into())
    }
}
