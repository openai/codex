//! Create master and slave virtual pseudo-terminals (PTYs)

pub use libc::pid_t as SessionId;
pub use libc::winsize as Winsize;

use std::ffi::CStr;
use std::io;
#[cfg(not(target_os = "aix"))]
use std::mem;
use std::os::unix::prelude::*;

use crate::errno::Errno;
#[cfg(not(target_os = "aix"))]
use crate::sys::termios::Termios;
#[cfg(all(feature = "process", not(target_os = "aix")))]
use crate::unistd::Pid;
use crate::{fcntl, unistd, Result};

/// Representation of a master/slave pty pair
///
/// This is returned by [`openpty`].
#[derive(Debug)]
pub struct OpenptyResult {
    /// The master port in a virtual pty pair
    pub master: OwnedFd,
    /// The slave port in a virtual pty pair
    pub slave: OwnedFd,
}

feature! {
#![feature = "process"]
/// A successful result of [`forkpty()`].
#[derive(Debug)]
pub enum ForkptyResult {
    /// This is the parent process of the underlying fork.
    Parent {
        /// The PID of the fork's child process
        child: Pid,
        /// A file descriptor referring to master side of the pseudoterminal of
        /// the child process.
        master: OwnedFd,
    },
    /// This is the child process of the underlying fork.
    Child,
}
}

/// Representation of the Master device in a master/slave pty pair
///
/// While this datatype is a thin wrapper around `OwnedFd`, it enforces that the available PTY
/// functions are given the correct file descriptor.
#[derive(Debug)]
pub struct PtyMaster(OwnedFd);

impl AsRawFd for PtyMaster {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl AsFd for PtyMaster {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl IntoRawFd for PtyMaster {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.0;
        fd.into_raw_fd()
    }
}

impl io::Read for PtyMaster {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unistd::read(self.0.as_raw_fd(), buf).map_err(io::Error::from)
    }
}

impl io::Write for PtyMaster {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unistd::write(&self.0, buf).map_err(io::Error::from)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl io::Read for &PtyMaster {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unistd::read(self.0.as_raw_fd(), buf).map_err(io::Error::from)
    }
}

impl io::Write for &PtyMaster {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unistd::write(&self.0, buf).map_err(io::Error::from)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Grant access to a slave pseudoterminal (see
/// [`grantpt(3)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/grantpt.html))
///
/// `grantpt()` changes the mode and owner of the slave pseudoterminal device corresponding to the
/// master pseudoterminal referred to by `fd`. This is a necessary step towards opening the slave.
#[inline]
pub fn grantpt(fd: &PtyMaster) -> Result<()> {
    if unsafe { libc::grantpt(fd.as_raw_fd()) } < 0 {
        return Err(Errno::last());
    }

    Ok(())
}

/// Open a pseudoterminal device (see
/// [`posix_openpt(3)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_openpt.html))
///
/// `posix_openpt()` returns a file descriptor to an existing unused pseudoterminal master device.
///
/// # Examples
///
/// A common use case with this function is to open both a master and slave PTY pair. This can be
/// done as follows:
///
/// ```
/// use std::path::Path;
/// use nix::fcntl::{OFlag, open};
/// use nix::pty::{grantpt, posix_openpt, ptsname, unlockpt};
/// use nix::sys::stat::Mode;
///
/// # #[allow(dead_code)]
/// # fn run() -> nix::Result<()> {
/// // Open a new PTY master
/// let master_fd = posix_openpt(OFlag::O_RDWR)?;
///
/// // Allow a slave to be generated for it
/// grantpt(&master_fd)?;
/// unlockpt(&master_fd)?;
///
/// // Get the name of the slave
/// let slave_name = unsafe { ptsname(&master_fd) }?;
///
/// // Try to open the slave
/// let _slave_fd = open(Path::new(&slave_name), OFlag::O_RDWR, Mode::empty())?;
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn posix_openpt(flags: fcntl::OFlag) -> Result<PtyMaster> {
    let fd = unsafe { libc::posix_openpt(flags.bits()) };

    if fd < 0 {
        return Err(Errno::last());
    }

    Ok(PtyMaster(unsafe { OwnedFd::from_raw_fd(fd) }))
}

/// Get the name of the slave pseudoterminal (see
/// [`ptsname(3)`](https://man7.org/linux/man-pages/man3/ptsname.3.html))
///
/// `ptsname()` returns the name of the slave pseudoterminal device corresponding to the master
/// referred to by `fd`.
///
/// This value is useful for opening the slave pty once the master has already been opened with
/// `posix_openpt()`.
///
/// # Safety
///
/// `ptsname()` mutates global variables and is *not* threadsafe.
/// Mutating global variables is always considered `unsafe` by Rust and this
/// function is marked as `unsafe` to reflect that.
///
/// For a threadsafe and non-`unsafe` alternative on Linux, see `ptsname_r()`.
#[inline]
pub unsafe fn ptsname(fd: &PtyMaster) -> Result<String> {
    let name_ptr = unsafe { libc::ptsname(fd.as_raw_fd()) };
    if name_ptr.is_null() {
        return Err(Errno::last());
    }

    let name = unsafe { CStr::from_ptr(name_ptr) };
    Ok(name.to_string_lossy().into_owned())
}

/// Get the name of the slave pseudoterminal (see
/// [`ptsname(3)`](https://man7.org/linux/man-pages/man3/ptsname.3.html))
///
/// `ptsname_r()` returns the name of the slave pseudoterminal device corresponding to the master
/// referred to by `fd`. This is the threadsafe version of `ptsname()`, but it is not part of the
/// POSIX standard and is instead a Linux-specific extension.
///
/// This value is useful for opening the slave ptty once the master has already been opened with
/// `posix_openpt()`.
#[cfg(linux_android)]
#[inline]
pub fn ptsname_r(fd: &PtyMaster) -> Result<String> {
    let mut name_buf = Vec::<libc::c_char>::with_capacity(64);
    let name_buf_ptr = name_buf.as_mut_ptr();
    let cname = unsafe {
        let cap = name_buf.capacity();
        if libc::ptsname_r(fd.as_raw_fd(), name_buf_ptr, cap) != 0 {
            return Err(crate::Error::last());
        }
        CStr::from_ptr(name_buf.as_ptr())
    };

    let name = cname.to_string_lossy().into_owned();
    Ok(name)
}

/// Unlock a pseudoterminal master/slave pseudoterminal pair (see
/// [`unlockpt(3)`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/unlockpt.html))
///
/// `unlockpt()` unlocks the slave pseudoterminal device corresponding to the master pseudoterminal
/// referred to by `fd`. This must be called before trying to open the slave side of a
/// pseudoterminal.
#[inline]
pub fn unlockpt(fd: &PtyMaster) -> Result<()> {
    if unsafe { libc::unlockpt(fd.as_raw_fd()) } < 0 {
        return Err(Errno::last());
    }

    Ok(())
}

/// Create a new pseudoterminal, returning the slave and master file descriptors
/// in `OpenptyResult`
/// (see [`openpty`](https://man7.org/linux/man-pages/man3/openpty.3.html)).
///
/// If `winsize` is not `None`, the window size of the slave will be set to
/// the values in `winsize`. If `termios` is not `None`, the pseudoterminal's
/// terminal settings of the slave will be set to the values in `termios`.
#[inline]
#[cfg(not(target_os = "aix"))]
pub fn openpty<
    'a,
    'b,
    T: Into<Option<&'a Winsize>>,
    U: Into<Option<&'b Termios>>,
>(
    winsize: T,
    termios: U,
) -> Result<OpenptyResult> {
    use std::ptr;

    let mut slave = mem::MaybeUninit::<libc::c_int>::uninit();
    let mut master = mem::MaybeUninit::<libc::c_int>::uninit();
    let ret = {
        match (termios.into(), winsize.into()) {
            (Some(termios), Some(winsize)) => {
                let inner_termios = termios.get_libc_termios();
                unsafe {
                    libc::openpty(
                        master.as_mut_ptr(),
                        slave.as_mut_ptr(),
                        ptr::null_mut(),
                        &*inner_termios as *const libc::termios as *mut _,
                        winsize as *const Winsize as *mut _,
                    )
                }
            }
            (None, Some(winsize)) => unsafe {
                libc::openpty(
                    master.as_mut_ptr(),
                    slave.as_mut_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    winsize as *const Winsize as *mut _,
                )
            },
            (Some(termios), None) => {
                let inner_termios = termios.get_libc_termios();
                unsafe {
                    libc::openpty(
                        master.as_mut_ptr(),
                        slave.as_mut_ptr(),
                        ptr::null_mut(),
                        &*inner_termios as *const libc::termios as *mut _,
                        ptr::null_mut(),
                    )
                }
            }
            (None, None) => unsafe {
                libc::openpty(
                    master.as_mut_ptr(),
                    slave.as_mut_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
        }
    };

    Errno::result(ret)?;

    unsafe {
        Ok(OpenptyResult {
            master: OwnedFd::from_raw_fd(master.assume_init()),
            slave: OwnedFd::from_raw_fd(slave.assume_init()),
        })
    }
}

feature! {
#![feature = "process"]
/// Create a new process operating in a pseudoterminal.
///
/// If `winsize` is not `None`, the window size of the slave will be set to
/// the values in `winsize`. If `termios` is not `None`, the pseudoterminal's
/// terminal settings of the slave will be set to the values in `termios`.
///
/// # Safety
///
/// In a multithreaded program, only [async-signal-safe] functions like `pause`
/// and `_exit` may be called by the child (the parent isn't restricted). Note
/// that memory allocation may **not** be async-signal-safe and thus must be
/// prevented.
///
/// Those functions are only a small subset of your operating system's API, so
/// special care must be taken to only invoke code you can control and audit.
///
/// [async-signal-safe]: https://man7.org/linux/man-pages/man7/signal-safety.7.html
///
/// # Reference
///
/// * [FreeBSD](https://man.freebsd.org/cgi/man.cgi?query=forkpty)
/// * [Linux](https://man7.org/linux/man-pages/man3/forkpty.3.html)
#[cfg(not(target_os = "aix"))]
pub unsafe fn forkpty<'a, 'b, T: Into<Option<&'a Winsize>>, U: Into<Option<&'b Termios>>>(
    winsize: T,
    termios: U,
) -> Result<ForkptyResult> {
    use std::ptr;

    let mut master = mem::MaybeUninit::<libc::c_int>::uninit();

    let term = match termios.into() {
        Some(termios) => {
            let inner_termios = termios.get_libc_termios();
            &*inner_termios as *const libc::termios as *mut _
        },
        None => ptr::null_mut(),
    };

    let win = winsize
        .into()
        .map(|ws| ws as *const Winsize as *mut _)
        .unwrap_or(ptr::null_mut());

    let res = unsafe { libc::forkpty(master.as_mut_ptr(), ptr::null_mut(), term, win) };

    let success_ret = Errno::result(res)?;
    let forkpty_result = match success_ret {
        // In the child process
        0 => ForkptyResult::Child,
        // In the parent process
        child_pid => {
            // SAFETY:
            // 1. The master buffer is guaranteed to be initialized in the parent process
            // 2. OwnedFd::from_raw_fd won't panic as the fd is a valid file descriptor
            let master = unsafe { OwnedFd::from_raw_fd( master.assume_init() ) };
            ForkptyResult::Parent {
                    master,
                    child: Pid::from_raw(child_pid),
            }
        }
    };

    Ok(forkpty_result)
}
}
