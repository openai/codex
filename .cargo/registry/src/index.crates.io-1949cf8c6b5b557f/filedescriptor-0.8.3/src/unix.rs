use crate::{
    AsRawFileDescriptor, AsRawSocketDescriptor, Error, FileDescriptor, FromRawFileDescriptor,
    FromRawSocketDescriptor, IntoRawFileDescriptor, IntoRawSocketDescriptor, OwnedHandle, Pipe,
    Result, StdioDescriptor,
};
use std::os::unix::prelude::*;

pub(crate) type HandleType = ();

/// `RawFileDescriptor` is a platform independent type alias for the
/// underlying platform file descriptor type.  It is primarily useful
/// for avoiding using `cfg` blocks in platform independent code.
pub type RawFileDescriptor = RawFd;

/// `SocketDescriptor` is a platform independent type alias for the
/// underlying platform socket descriptor type.  It is primarily useful
/// for avoiding using `cfg` blocks in platform independent code.
pub type SocketDescriptor = RawFd;

impl<T: AsRawFd> AsRawFileDescriptor for T {
    fn as_raw_file_descriptor(&self) -> RawFileDescriptor {
        self.as_raw_fd()
    }
}

impl<T: IntoRawFd> IntoRawFileDescriptor for T {
    fn into_raw_file_descriptor(self) -> RawFileDescriptor {
        self.into_raw_fd()
    }
}

impl<T: FromRawFd> FromRawFileDescriptor for T {
    unsafe fn from_raw_file_descriptor(fd: RawFileDescriptor) -> Self {
        Self::from_raw_fd(fd)
    }
}

impl<T: AsRawFd> AsRawSocketDescriptor for T {
    fn as_socket_descriptor(&self) -> SocketDescriptor {
        self.as_raw_fd()
    }
}

impl<T: IntoRawFd> IntoRawSocketDescriptor for T {
    fn into_socket_descriptor(self) -> SocketDescriptor {
        self.into_raw_fd()
    }
}

impl<T: FromRawFd> FromRawSocketDescriptor for T {
    unsafe fn from_socket_descriptor(fd: SocketDescriptor) -> Self {
        Self::from_raw_fd(fd)
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.handle);
        }
    }
}

impl std::os::fd::AsFd for OwnedHandle {
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        unsafe { std::os::fd::BorrowedFd::borrow_raw(self.handle) }
    }
}

impl AsRawFd for OwnedHandle {
    fn as_raw_fd(&self) -> RawFd {
        self.handle
    }
}

impl IntoRawFd for OwnedHandle {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.handle;
        std::mem::forget(self);
        fd
    }
}

impl FromRawFd for OwnedHandle {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self {
            handle: fd,
            handle_type: (),
        }
    }
}

impl OwnedHandle {
    /// Helper function to set the close-on-exec flag for a raw descriptor
    fn cloexec(&mut self) -> Result<()> {
        let flags = unsafe { libc::fcntl(self.handle, libc::F_GETFD) };
        if flags == -1 {
            return Err(Error::Fcntl(std::io::Error::last_os_error()));
        }
        let result = unsafe { libc::fcntl(self.handle, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
        if result == -1 {
            Err(Error::Cloexec(std::io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    fn non_atomic_dup(fd: RawFd) -> Result<Self> {
        let duped = unsafe { libc::dup(fd) };
        if duped == -1 {
            Err(Error::Dup {
                fd: fd.into(),
                source: std::io::Error::last_os_error(),
            })
        } else {
            let mut owned = OwnedHandle {
                handle: duped,
                handle_type: (),
            };
            owned.cloexec()?;
            Ok(owned)
        }
    }

    fn non_atomic_dup2(fd: RawFd, dest_fd: RawFd) -> Result<Self> {
        let duped = unsafe { libc::dup2(fd, dest_fd) };
        if duped == -1 {
            Err(Error::Dup2 {
                src_fd: fd.into(),
                dest_fd: dest_fd.into(),
                source: std::io::Error::last_os_error(),
            })
        } else {
            let mut owned = OwnedHandle {
                handle: duped,
                handle_type: (),
            };
            owned.cloexec()?;
            Ok(owned)
        }
    }

    #[inline]
    pub(crate) fn dup_impl<F: AsRawFileDescriptor>(
        fd: &F,
        handle_type: HandleType,
    ) -> Result<Self> {
        let fd = fd.as_raw_file_descriptor();
        let duped = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
        if duped == -1 {
            let err = std::io::Error::last_os_error();
            if let Some(libc::EINVAL) = err.raw_os_error() {
                // We may be running on eg: WSL or an old kernel that
                // doesn't support F_DUPFD_CLOEXEC; fall back.
                Self::non_atomic_dup(fd)
            } else {
                Err(Error::Dup {
                    fd: fd.into(),
                    source: err,
                })
            }
        } else {
            Ok(OwnedHandle {
                handle: duped,
                handle_type,
            })
        }
    }

    #[inline]
    pub(crate) unsafe fn dup2_impl<F: AsRawFileDescriptor>(fd: &F, dest_fd: RawFd) -> Result<Self> {
        let fd = fd.as_raw_file_descriptor();

        #[cfg(not(target_os = "linux"))]
        return Self::non_atomic_dup2(fd, dest_fd);

        #[cfg(target_os = "linux")]
        {
            let duped = libc::dup3(fd, dest_fd, libc::O_CLOEXEC);

            if duped == -1 {
                let err = std::io::Error::last_os_error();
                if let Some(libc::EINVAL) = err.raw_os_error() {
                    // We may be running on eg: WSL or an old kernel that
                    // doesn't support O_CLOEXEC; fall back.
                    Self::non_atomic_dup2(fd, dest_fd)
                } else {
                    Err(Error::Dup2 {
                        src_fd: fd.into(),
                        dest_fd: dest_fd.into(),
                        source: err,
                    })
                }
            } else {
                Ok(OwnedHandle {
                    handle: duped,
                    handle_type: (),
                })
            }
        }
    }

    pub(crate) fn probe_handle_type(_handle: RawFileDescriptor) -> HandleType {
        ()
    }
}

impl std::io::Read for FileDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let size = unsafe { libc::read(self.handle.handle, buf.as_mut_ptr() as *mut _, buf.len()) };
        if size == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
}

impl std::io::Write for FileDescriptor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let size = unsafe { libc::write(self.handle.handle, buf.as_ptr() as *const _, buf.len()) };
        if size == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(size as usize)
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::os::fd::AsFd for FileDescriptor {
    fn as_fd(&self) -> std::os::fd::BorrowedFd {
        self.handle.as_fd()
    }
}

impl AsRawFd for FileDescriptor {
    fn as_raw_fd(&self) -> RawFd {
        self.handle.as_raw_fd()
    }
}

impl IntoRawFd for FileDescriptor {
    fn into_raw_fd(self) -> RawFd {
        self.handle.into_raw_fd()
    }
}

impl FromRawFd for FileDescriptor {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self {
            handle: OwnedHandle::from_raw_fd(fd),
        }
    }
}

impl FileDescriptor {
    #[inline]
    pub(crate) fn as_stdio_impl(&self) -> Result<std::process::Stdio> {
        let duped = OwnedHandle::dup(self)?;
        let fd = duped.into_raw_fd();
        let stdio = unsafe { std::process::Stdio::from_raw_fd(fd) };
        Ok(stdio)
    }

    #[inline]
    pub(crate) fn as_file_impl(&self) -> Result<std::fs::File> {
        let duped = OwnedHandle::dup(self)?;
        let fd = duped.into_raw_fd();
        let stdio = unsafe { std::fs::File::from_raw_fd(fd) };
        Ok(stdio)
    }

    #[inline]
    pub(crate) fn set_non_blocking_impl(&mut self, non_blocking: bool) -> Result<()> {
        let on = if non_blocking { 1 } else { 0 };
        let res = unsafe { libc::ioctl(self.handle.as_raw_file_descriptor(), libc::FIONBIO, &on) };
        if res != 0 {
            Err(Error::FionBio(std::io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    /// Attempt to duplicate the underlying handle from an object that is
    /// representable as the system `RawFileDescriptor` type and assign it to
    /// a destination file descriptor. It then returns a `FileDescriptor`
    /// wrapped around the duplicate.  Since the duplication requires kernel
    /// resources that may not be available, this is a potentially fallible operation.
    /// The returned handle has a separate lifetime from the source, but
    /// references the same object at the kernel level.
    pub unsafe fn dup2<F: AsRawFileDescriptor>(f: &F, dest_fd: RawFd) -> Result<Self> {
        OwnedHandle::dup2_impl(f, dest_fd).map(|handle| Self { handle })
    }

    /// Helper function to unset the close-on-exec flag for a raw descriptor
    fn no_cloexec(fd: RawFd) -> Result<()> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if flags == -1 {
            return Err(Error::Fcntl(std::io::Error::last_os_error()));
        }
        let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) };
        if result == -1 {
            Err(Error::Cloexec(std::io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    pub(crate) fn redirect_stdio_impl<F: AsRawFileDescriptor>(
        f: &F,
        stdio: StdioDescriptor,
    ) -> Result<Self> {
        let std_descriptor = match stdio {
            StdioDescriptor::Stdin => libc::STDIN_FILENO,
            StdioDescriptor::Stdout => libc::STDOUT_FILENO,
            StdioDescriptor::Stderr => libc::STDERR_FILENO,
        };

        let std_original = FileDescriptor::dup(&std_descriptor)?;
        // Assign f into std_descriptor, then convert to an fd so that
        // we don't close it when the returned FileDescriptor is dropped.
        // Then we discard/ignore the fd because it is nominally owned by
        // the stdio machinery for the process
        let _ = unsafe { FileDescriptor::dup2(f, std_descriptor) }?.into_raw_fd();
        Self::no_cloexec(std_descriptor)?;

        Ok(std_original)
    }
}

impl Pipe {
    #[cfg(target_os = "linux")]
    pub fn new() -> Result<Pipe> {
        let mut fds = [-1i32; 2];
        let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        if res == -1 {
            Err(Error::Pipe(std::io::Error::last_os_error()))
        } else {
            let read = FileDescriptor {
                handle: OwnedHandle {
                    handle: fds[0],
                    handle_type: (),
                },
            };
            let write = FileDescriptor {
                handle: OwnedHandle {
                    handle: fds[1],
                    handle_type: (),
                },
            };
            Ok(Pipe { read, write })
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Result<Pipe> {
        let mut fds = [-1i32; 2];
        let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if res == -1 {
            Err(Error::Pipe(std::io::Error::last_os_error()))
        } else {
            let mut read = FileDescriptor {
                handle: OwnedHandle {
                    handle: fds[0],
                    handle_type: (),
                },
            };
            let mut write = FileDescriptor {
                handle: OwnedHandle {
                    handle: fds[1],
                    handle_type: (),
                },
            };
            read.handle.cloexec()?;
            write.handle.cloexec()?;
            Ok(Pipe { read, write })
        }
    }
}

#[cfg(target_os = "linux")]
#[doc(hidden)]
pub fn socketpair_impl() -> Result<(FileDescriptor, FileDescriptor)> {
    let mut fds = [-1i32; 2];
    let res = unsafe {
        libc::socketpair(
            libc::PF_LOCAL,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    if res == -1 {
        Err(Error::Socketpair(std::io::Error::last_os_error()))
    } else {
        let read = FileDescriptor {
            handle: OwnedHandle {
                handle: fds[0],
                handle_type: (),
            },
        };
        let write = FileDescriptor {
            handle: OwnedHandle {
                handle: fds[1],
                handle_type: (),
            },
        };
        Ok((read, write))
    }
}

#[cfg(not(target_os = "linux"))]
#[doc(hidden)]
pub fn socketpair_impl() -> Result<(FileDescriptor, FileDescriptor)> {
    let mut fds = [-1i32; 2];
    let res = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    if res == -1 {
        Err(Error::Socketpair(std::io::Error::last_os_error()))
    } else {
        let mut read = FileDescriptor {
            handle: OwnedHandle {
                handle: fds[0],
                handle_type: (),
            },
        };
        let mut write = FileDescriptor {
            handle: OwnedHandle {
                handle: fds[1],
                handle_type: (),
            },
        };
        read.handle.cloexec()?;
        write.handle.cloexec()?;
        Ok((read, write))
    }
}

pub use libc::{pollfd, POLLERR, POLLHUP, POLLIN, POLLOUT};
use std::time::Duration;

#[cfg(not(target_os = "macos"))]
#[doc(hidden)]
pub fn poll_impl(pfd: &mut [pollfd], duration: Option<Duration>) -> Result<usize> {
    let poll_result = unsafe {
        libc::poll(
            pfd.as_mut_ptr(),
            pfd.len() as _,
            duration
                .map(|wait| wait.as_millis() as libc::c_int)
                .unwrap_or(-1),
        )
    };
    if poll_result < 0 {
        Err(Error::Poll(std::io::Error::last_os_error()))
    } else {
        Ok(poll_result as usize)
    }
}

// macOS has a broken poll(2) implementation, so we introduce a layer to deal with that here
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use libc::{fd_set, timeval, FD_ISSET, FD_SET, FD_SETSIZE, FD_ZERO, POLLERR, POLLIN, POLLOUT};
    use std::os::unix::io::RawFd;

    struct FdSet {
        set: fd_set,
    }

    #[inline]
    fn check_fd(fd: RawFd) -> Result<()> {
        if fd < 0 {
            return Err(Error::IllegalFdValue(fd.into()));
        }
        if fd as usize >= FD_SETSIZE {
            return Err(Error::FdValueOutsideFdSetSize(fd.into()));
        }
        Ok(())
    }

    impl FdSet {
        pub fn new() -> Self {
            unsafe {
                let mut set = std::mem::MaybeUninit::uninit();
                FD_ZERO(set.as_mut_ptr());
                Self {
                    set: set.assume_init(),
                }
            }
        }

        pub fn add(&mut self, fd: RawFd) -> Result<()> {
            check_fd(fd)?;
            unsafe {
                FD_SET(fd, &mut self.set);
            }
            Ok(())
        }

        pub fn contains(&mut self, fd: RawFd) -> bool {
            check_fd(fd).unwrap();
            unsafe { FD_ISSET(fd, &mut self.set) }
        }
    }

    fn materialize(set: &mut Option<FdSet>) -> &mut FdSet {
        set.get_or_insert_with(FdSet::new)
    }

    fn set_ptr(set: &mut Option<FdSet>) -> *mut fd_set {
        set.as_mut()
            .map(|s| &mut s.set as *mut _)
            .unwrap_or_else(std::ptr::null_mut)
    }

    fn is_set(set: &mut Option<FdSet>, fd: RawFd) -> bool {
        set.as_mut().map(|s| s.contains(fd)).unwrap_or(false)
    }

    pub fn poll_impl(pfd: &mut [pollfd], duration: Option<Duration>) -> Result<usize> {
        let mut read_set = None;
        let mut write_set = None;
        let mut exception_set = None;
        let mut nfds = 0;

        for item in pfd.iter_mut() {
            item.revents = 0;

            nfds = nfds.max(item.fd);

            if item.events & POLLIN != 0 {
                materialize(&mut read_set).add(item.fd)?;
            }
            if item.events & POLLOUT != 0 {
                materialize(&mut write_set).add(item.fd)?;
            }
            materialize(&mut exception_set).add(item.fd)?;
        }

        let mut timeout = duration.map(|d| timeval {
            tv_sec: d.as_secs() as _,
            tv_usec: d.subsec_micros() as _,
        });

        let res = unsafe {
            libc::select(
                nfds + 1,
                set_ptr(&mut read_set),
                set_ptr(&mut write_set),
                set_ptr(&mut exception_set),
                timeout
                    .as_mut()
                    .map(|t| t as *mut _)
                    .unwrap_or_else(std::ptr::null_mut),
            )
        };

        if res < 0 {
            Err(std::io::Error::last_os_error().into())
        } else {
            for item in pfd.iter_mut() {
                if is_set(&mut read_set, item.fd) {
                    item.revents |= POLLIN;
                }
                if is_set(&mut write_set, item.fd) {
                    item.revents |= POLLOUT;
                }
                if is_set(&mut exception_set, item.fd) {
                    item.revents |= POLLERR;
                }
            }

            Ok(res as usize)
        }
    }
}

#[cfg(target_os = "macos")]
#[doc(hidden)]
pub use macos::poll_impl;
