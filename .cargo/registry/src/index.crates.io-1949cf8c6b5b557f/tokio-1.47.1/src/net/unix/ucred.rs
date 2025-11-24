use crate::net::unix;

/// Credentials of a process.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct UCred {
    /// PID (process ID) of the process.
    pid: Option<unix::pid_t>,
    /// UID (user ID) of the process.
    uid: unix::uid_t,
    /// GID (group ID) of the process.
    gid: unix::gid_t,
}

impl UCred {
    /// Gets UID (user ID) of the process.
    pub fn uid(&self) -> unix::uid_t {
        self.uid
    }

    /// Gets GID (group ID) of the process.
    pub fn gid(&self) -> unix::gid_t {
        self.gid
    }

    /// Gets PID (process ID) of the process.
    ///
    /// This is only implemented under Linux, Android, iOS, macOS, Solaris and
    /// Illumos. On other platforms this will always return `None`.
    pub fn pid(&self) -> Option<unix::pid_t> {
        self.pid
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "redox",
    target_os = "android",
    target_os = "openbsd",
    target_os = "haiku",
    target_os = "cygwin"
))]
pub(crate) use self::impl_linux::get_peer_cred;

#[cfg(any(target_os = "netbsd", target_os = "nto"))]
pub(crate) use self::impl_netbsd::get_peer_cred;

#[cfg(any(target_os = "dragonfly", target_os = "freebsd"))]
pub(crate) use self::impl_bsd::get_peer_cred;

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
pub(crate) use self::impl_macos::get_peer_cred;

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
pub(crate) use self::impl_solaris::get_peer_cred;

#[cfg(target_os = "aix")]
pub(crate) use self::impl_aix::get_peer_cred;

#[cfg(any(target_os = "espidf", target_os = "vita"))]
pub(crate) use self::impl_noproc::get_peer_cred;

#[cfg(any(
    target_os = "linux",
    target_os = "redox",
    target_os = "android",
    target_os = "openbsd",
    target_os = "haiku",
    target_os = "cygwin"
))]
pub(crate) mod impl_linux {
    use crate::net::unix::{self, UnixStream};

    use libc::{c_void, getsockopt, socklen_t, SOL_SOCKET, SO_PEERCRED};
    use std::{io, mem};

    #[cfg(target_os = "openbsd")]
    use libc::sockpeercred as ucred;
    #[cfg(any(
        target_os = "linux",
        target_os = "redox",
        target_os = "android",
        target_os = "haiku",
        target_os = "cygwin"
    ))]
    use libc::ucred;

    pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<super::UCred> {
        use std::os::unix::io::AsRawFd;

        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut ucred = ucred {
                pid: 0,
                uid: 0,
                gid: 0,
            };

            let ucred_size = mem::size_of::<ucred>();

            // These paranoid checks should be optimized-out
            assert!(mem::size_of::<u32>() <= mem::size_of::<usize>());
            assert!(ucred_size <= u32::MAX as usize);

            let mut ucred_size = ucred_size as socklen_t;

            let ret = getsockopt(
                raw_fd,
                SOL_SOCKET,
                SO_PEERCRED,
                &mut ucred as *mut ucred as *mut c_void,
                &mut ucred_size,
            );
            if ret == 0 && ucred_size as usize == mem::size_of::<ucred>() {
                Ok(super::UCred {
                    uid: ucred.uid as unix::uid_t,
                    gid: ucred.gid as unix::gid_t,
                    pid: Some(ucred.pid as unix::pid_t),
                })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(any(target_os = "netbsd", target_os = "nto"))]
pub(crate) mod impl_netbsd {
    use crate::net::unix::{self, UnixStream};

    use libc::{c_void, getsockopt, socklen_t, unpcbid, LOCAL_PEEREID, SOL_SOCKET};
    use std::io;
    use std::mem::size_of;
    use std::os::unix::io::AsRawFd;

    pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<super::UCred> {
        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut unpcbid = unpcbid {
                unp_pid: 0,
                unp_euid: 0,
                unp_egid: 0,
            };

            let unpcbid_size = size_of::<unpcbid>();
            let mut unpcbid_size = unpcbid_size as socklen_t;

            let ret = getsockopt(
                raw_fd,
                SOL_SOCKET,
                LOCAL_PEEREID,
                &mut unpcbid as *mut unpcbid as *mut c_void,
                &mut unpcbid_size,
            );
            if ret == 0 && unpcbid_size as usize == size_of::<unpcbid>() {
                Ok(super::UCred {
                    uid: unpcbid.unp_euid as unix::uid_t,
                    gid: unpcbid.unp_egid as unix::gid_t,
                    pid: Some(unpcbid.unp_pid as unix::pid_t),
                })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(any(target_os = "dragonfly", target_os = "freebsd"))]
pub(crate) mod impl_bsd {
    use crate::net::unix::{self, UnixStream};

    use libc::getpeereid;
    use std::io;
    use std::mem::MaybeUninit;
    use std::os::unix::io::AsRawFd;

    pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<super::UCred> {
        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut uid = MaybeUninit::uninit();
            let mut gid = MaybeUninit::uninit();

            let ret = getpeereid(raw_fd, uid.as_mut_ptr(), gid.as_mut_ptr());

            if ret == 0 {
                Ok(super::UCred {
                    uid: uid.assume_init() as unix::uid_t,
                    gid: gid.assume_init() as unix::gid_t,
                    pid: None,
                })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
pub(crate) mod impl_macos {
    use crate::net::unix::{self, UnixStream};

    use libc::{c_void, getpeereid, getsockopt, pid_t, LOCAL_PEEREPID, SOL_LOCAL};
    use std::io;
    use std::mem::size_of;
    use std::mem::MaybeUninit;
    use std::os::unix::io::AsRawFd;

    pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<super::UCred> {
        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut uid = MaybeUninit::uninit();
            let mut gid = MaybeUninit::uninit();
            let mut pid: MaybeUninit<pid_t> = MaybeUninit::uninit();
            let mut pid_size: MaybeUninit<u32> = MaybeUninit::new(size_of::<pid_t>() as u32);

            if getsockopt(
                raw_fd,
                SOL_LOCAL,
                LOCAL_PEEREPID,
                pid.as_mut_ptr() as *mut c_void,
                pid_size.as_mut_ptr(),
            ) != 0
            {
                return Err(io::Error::last_os_error());
            }

            assert!(pid_size.assume_init() == (size_of::<pid_t>() as u32));

            let ret = getpeereid(raw_fd, uid.as_mut_ptr(), gid.as_mut_ptr());

            if ret == 0 {
                Ok(super::UCred {
                    uid: uid.assume_init() as unix::uid_t,
                    gid: gid.assume_init() as unix::gid_t,
                    pid: Some(pid.assume_init() as unix::pid_t),
                })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
pub(crate) mod impl_solaris {
    use crate::net::unix::{self, UnixStream};
    use std::io;
    use std::os::unix::io::AsRawFd;
    use std::ptr;

    pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<super::UCred> {
        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut cred = ptr::null_mut();
            let ret = libc::getpeerucred(raw_fd, &mut cred);

            if ret == 0 {
                let uid = libc::ucred_geteuid(cred);
                let gid = libc::ucred_getegid(cred);
                let pid = libc::ucred_getpid(cred);

                libc::ucred_free(cred);

                Ok(super::UCred {
                    uid: uid as unix::uid_t,
                    gid: gid as unix::gid_t,
                    pid: Some(pid as unix::pid_t),
                })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(target_os = "aix")]
pub(crate) mod impl_aix {
    use crate::net::unix::UnixStream;
    use std::io;
    use std::os::unix::io::AsRawFd;

    pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<super::UCred> {
        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut uid = std::mem::MaybeUninit::uninit();
            let mut gid = std::mem::MaybeUninit::uninit();

            let ret = libc::getpeereid(raw_fd, uid.as_mut_ptr(), gid.as_mut_ptr());

            if ret == 0 {
                Ok(super::UCred {
                    uid: uid.assume_init(),
                    gid: gid.assume_init(),
                    pid: None,
                })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(any(target_os = "espidf", target_os = "vita"))]
pub(crate) mod impl_noproc {
    use crate::net::unix::UnixStream;
    use std::io;

    pub(crate) fn get_peer_cred(_sock: &UnixStream) -> io::Result<super::UCred> {
        Ok(super::UCred {
            uid: 0,
            gid: 0,
            pid: None,
        })
    }
}
