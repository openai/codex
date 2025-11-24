//! Feature tests for OS functionality
pub use self::os::*;

#[cfg(any(linux_android, target_os = "emscripten"))]
mod os {
    use crate::sys::utsname::uname;
    use crate::Result;
    use std::os::unix::ffi::OsStrExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Features:
    // * atomic cloexec on socket: 2.6.27
    // * pipe2: 2.6.27
    // * accept4: 2.6.28

    static VERS_UNKNOWN: usize = 1;
    static VERS_2_6_18: usize = 2;
    static VERS_2_6_27: usize = 3;
    static VERS_2_6_28: usize = 4;
    static VERS_3: usize = 5;

    #[inline]
    fn digit(dst: &mut usize, b: u8) {
        *dst *= 10;
        *dst += (b - b'0') as usize;
    }

    fn parse_kernel_version() -> Result<usize> {
        let u = uname()?;

        let mut curr: usize = 0;
        let mut major: usize = 0;
        let mut minor: usize = 0;
        let mut patch: usize = 0;

        for &b in u.release().as_bytes() {
            if curr >= 3 {
                break;
            }

            match b {
                b'.' | b'-' => {
                    curr += 1;
                }
                b'0'..=b'9' => match curr {
                    0 => digit(&mut major, b),
                    1 => digit(&mut minor, b),
                    _ => digit(&mut patch, b),
                },
                _ => break,
            }
        }

        Ok(if major >= 3 {
            VERS_3
        } else if major >= 2 {
            if minor >= 7 {
                VERS_UNKNOWN
            } else if minor >= 6 {
                if patch >= 28 {
                    VERS_2_6_28
                } else if patch >= 27 {
                    VERS_2_6_27
                } else {
                    VERS_2_6_18
                }
            } else {
                VERS_UNKNOWN
            }
        } else {
            VERS_UNKNOWN
        })
    }

    fn kernel_version() -> Result<usize> {
        static KERNEL_VERS: AtomicUsize = AtomicUsize::new(0);
        let mut kernel_vers = KERNEL_VERS.load(Ordering::Relaxed);

        if kernel_vers == 0 {
            kernel_vers = parse_kernel_version()?;
            KERNEL_VERS.store(kernel_vers, Ordering::Relaxed);
        }

        Ok(kernel_vers)
    }

    /// Check if the OS supports atomic close-on-exec for sockets
    pub fn socket_atomic_cloexec() -> bool {
        kernel_version()
            .map(|version| version >= VERS_2_6_27)
            .unwrap_or(false)
    }

    #[test]
    fn test_parsing_kernel_version() {
        assert!(kernel_version().unwrap() > 0);
    }
}

#[cfg(any(
        freebsdlike,                // FreeBSD since 10.0 DragonFlyBSD since ???
        netbsdlike,                 // NetBSD since 6.0 OpenBSD since 5.7
        target_os = "hurd",         // Since glibc 2.28
        target_os = "illumos",      // Since ???
        target_os = "redox",        // Since 1-july-2020
        target_os = "cygwin",
))]
mod os {
    /// Check if the OS supports atomic close-on-exec for sockets
    pub const fn socket_atomic_cloexec() -> bool {
        true
    }
}

#[cfg(any(
    target_os = "aix",
    apple_targets,
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "solaris"
))]
mod os {
    /// Check if the OS supports atomic close-on-exec for sockets
    pub const fn socket_atomic_cloexec() -> bool {
        false
    }
}
