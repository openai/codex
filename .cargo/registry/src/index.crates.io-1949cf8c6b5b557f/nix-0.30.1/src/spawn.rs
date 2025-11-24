//! Safe wrappers around posix_spawn* functions found in the libc "spawn.h" header.

use std::{ffi::CStr, mem, os::fd::RawFd};

#[cfg(any(feature = "fs", feature = "term"))]
use crate::fcntl::OFlag;
#[cfg(feature = "signal")]
use crate::sys::signal::SigSet;
#[cfg(feature = "fs")]
use crate::sys::stat::Mode;
use crate::{errno::Errno, unistd::Pid, NixPath, Result};

/// A spawn attributes object. See [posix_spawnattr_t](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_init.html).
#[repr(transparent)]
#[derive(Debug)]
pub struct PosixSpawnAttr {
    attr: libc::posix_spawnattr_t,
}

impl PosixSpawnAttr {
    /// Initialize the spawn attributes object. See
    /// [posix_spawnattr_init](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_init.html).
    #[doc(alias("posix_spawnattr_init"))]
    pub fn init() -> Result<PosixSpawnAttr> {
        let mut attr = mem::MaybeUninit::uninit();
        let res = unsafe { libc::posix_spawnattr_init(attr.as_mut_ptr()) };

        Errno::result(res)?;

        let attr = unsafe { attr.assume_init() };
        Ok(PosixSpawnAttr { attr })
    }

    /// Reinitialize the spawn attributes object.
    /// This is a wrapper around
    /// [posix_spawnattr_destroy](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_destroy.html)
    /// followed by
    /// [posix_spawnattr_init](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_init.html).
    #[doc(alias("posix_spawnattr_destroy"))]
    pub fn reinit(mut self) -> Result<PosixSpawnAttr> {
        let res = unsafe {
            libc::posix_spawnattr_destroy(
                &mut self.attr as *mut libc::posix_spawnattr_t,
            )
        };
        Errno::result(res)?;

        let res = unsafe {
            libc::posix_spawnattr_init(
                &mut self.attr as *mut libc::posix_spawnattr_t,
            )
        };
        Errno::result(res)?;

        Ok(self)
    }

    /// Set spawn flags. See
    /// [posix_spawnattr_setflags](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_setflags.html).
    #[doc(alias("posix_spawnattr_setflags"))]
    pub fn set_flags(&mut self, flags: PosixSpawnFlags) -> Result<()> {
        let res = unsafe {
            libc::posix_spawnattr_setflags(
                &mut self.attr as *mut libc::posix_spawnattr_t,
                flags.bits() as libc::c_short,
            )
        };
        Errno::result(res)?;

        Ok(())
    }

    /// Get spawn flags. See
    /// [posix_spawnattr_getflags](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_getflags.html).
    #[doc(alias("posix_spawnattr_getflags"))]
    pub fn flags(&self) -> Result<PosixSpawnFlags> {
        let mut flags: libc::c_short = 0;
        let res = unsafe {
            libc::posix_spawnattr_getflags(
                &self.attr as *const libc::posix_spawnattr_t,
                &mut flags,
            )
        };
        Errno::result(res)?;

        Ok(PosixSpawnFlags::from_bits_truncate(flags.into()))
    }

    /// Set spawn pgroup. See
    /// [posix_spawnattr_setpgroup](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_setpgroup.html).
    #[doc(alias("posix_spawnattr_setpgroup"))]
    pub fn set_pgroup(&mut self, pgroup: Pid) -> Result<()> {
        let res = unsafe {
            libc::posix_spawnattr_setpgroup(
                &mut self.attr as *mut libc::posix_spawnattr_t,
                pgroup.as_raw(),
            )
        };
        Errno::result(res)?;

        Ok(())
    }

    /// Get spawn pgroup. See
    /// [posix_spawnattr_getpgroup](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_getpgroup.html).
    #[doc(alias("posix_spawnattr_getpgroup"))]
    pub fn pgroup(&self) -> Result<Pid> {
        let mut pid: libc::pid_t = 0;

        let res = unsafe {
            libc::posix_spawnattr_getpgroup(
                &self.attr as *const libc::posix_spawnattr_t,
                &mut pid,
            )
        };
        Errno::result(res)?;

        Ok(Pid::from_raw(pid))
    }

    feature! {
    #![feature = "signal"]
    /// Set spawn sigdefault. See
    /// [posix_spawnattr_setsigdefault](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_setsigdefault.html).
    #[doc(alias("posix_spawnattr_setsigdefault"))]
    pub fn set_sigdefault(&mut self, sigdefault: &SigSet) -> Result<()> {
        let res = unsafe {
            libc::posix_spawnattr_setsigdefault(
                &mut self.attr as *mut libc::posix_spawnattr_t,
                sigdefault.as_ref(),
            )
        };
        Errno::result(res)?;

        Ok(())
    }

    /// Get spawn sigdefault. See
    /// [posix_spawnattr_getsigdefault](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_getsigdefault.html).
    #[doc(alias("posix_spawnattr_getsigdefault"))]
    pub fn sigdefault(&self) -> Result<SigSet> {
        let mut sigset = mem::MaybeUninit::uninit();

        let res = unsafe {
            libc::posix_spawnattr_getsigdefault(
                &self.attr as *const libc::posix_spawnattr_t,
                sigset.as_mut_ptr(),
            )
        };
        Errno::result(res)?;

        let sigdefault =
            unsafe { SigSet::from_sigset_t_unchecked(sigset.assume_init()) };
        Ok(sigdefault)
    }

    /// Set spawn sigmask. See
    /// [posix_spawnattr_setsigmask](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_setsigmask.html).
    #[doc(alias("posix_spawnattr_setsigmask"))]
    pub fn set_sigmask(&mut self, sigdefault: &SigSet) -> Result<()> {
        let res = unsafe {
            libc::posix_spawnattr_setsigmask(
                &mut self.attr as *mut libc::posix_spawnattr_t,
                sigdefault.as_ref(),
            )
        };
        Errno::result(res)?;

        Ok(())
    }

    /// Get spawn sigmask. See
    /// [posix_spawnattr_getsigmask](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_getsigmask.html).
    #[doc(alias("posix_spawnattr_getsigmask"))]
    pub fn sigmask(&self) -> Result<SigSet> {
        let mut sigset = mem::MaybeUninit::uninit();

        let res = unsafe {
            libc::posix_spawnattr_getsigmask(
                &self.attr as *const libc::posix_spawnattr_t,
                sigset.as_mut_ptr(),
            )
        };
        Errno::result(res)?;

        let sigdefault =
            unsafe { SigSet::from_sigset_t_unchecked(sigset.assume_init()) };
        Ok(sigdefault)
    }
    }
}

impl Drop for PosixSpawnAttr {
    fn drop(&mut self) {
        unsafe {
            libc::posix_spawnattr_destroy(
                &mut self.attr as *mut libc::posix_spawnattr_t,
            );
        }
    }
}

libc_bitflags!(
    /// Process attributes to be changed in the new process image when invoking [`posix_spawn`]
    /// or [`posix_spawnp`]. See
    /// [posix_spawn](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn.html).
    pub struct PosixSpawnFlags: libc::c_int {
        /// Reset effective user ID of the child process to parent's real user ID.
        POSIX_SPAWN_RESETIDS;
        /// Put the child in a process group specified by the spawn-pgroup attribute. See
        /// [posix_spawnattr_setpgroup](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_setpgroup.html).
        POSIX_SPAWN_SETPGROUP;
        /// Force set signals to default signal handling in child process. See
        /// [posix_spawnattr_setsigdefault](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_setsigdefault.html).
        #[cfg(feature = "signal")]
        POSIX_SPAWN_SETSIGDEF;
        /// Set signal mask of child process. See
        /// [posix_spawnattr_setsigmask](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnattr_setsigmask.html).
        #[cfg(feature = "signal")]
        POSIX_SPAWN_SETSIGMASK;
        // TODO: Add support for the following two flags whenever support for
        // https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sched.h.html
        // is added to nix.
        // POSIX_SPAWN_SETSCHEDPARAM;
        // POSIX_SPAWN_SETSCHEDULER;
    }
);

/// A spawn file actions object. See [posix_spawn_file_actions_t](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn_file_actions_addclose.html).
#[repr(transparent)]
#[derive(Debug)]
pub struct PosixSpawnFileActions {
    fa: libc::posix_spawn_file_actions_t,
}

impl PosixSpawnFileActions {
    /// Initialize the spawn file actions object. See
    /// [posix_spawn_file_actions_init](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn_file_actions_init.html).
    #[doc(alias("posix_spawn_file_actions_init"))]
    pub fn init() -> Result<PosixSpawnFileActions> {
        let mut actions = mem::MaybeUninit::uninit();
        let res = unsafe {
            libc::posix_spawn_file_actions_init(actions.as_mut_ptr())
        };
        Errno::result(res)?;
        Ok(unsafe {
            PosixSpawnFileActions {
                fa: actions.assume_init(),
            }
        })
    }

    /// Reinitialize the spawn file actions object.
    /// This is a wrapper around
    /// [posix_spawn_file_actions_destroy](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn_file_actions_destroy.html).
    /// followed by
    /// [posix_spawn_file_actions_init](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn_file_actions_init.html).
    #[doc(alias("posix_spawn_file_actions_destroy"))]
    pub fn reinit(mut self) -> Result<PosixSpawnFileActions> {
        let res = unsafe {
            libc::posix_spawn_file_actions_destroy(
                &mut self.fa as *mut libc::posix_spawn_file_actions_t,
            )
        };
        Errno::result(res)?;

        let res = unsafe {
            libc::posix_spawn_file_actions_init(
                &mut self.fa as *mut libc::posix_spawn_file_actions_t,
            )
        };
        Errno::result(res)?;

        Ok(self)
    }

    /// Add a [dup2](https://pubs.opengroup.org/onlinepubs/9699919799/functions/dup2.html) action. See
    /// [posix_spawn_file_actions_adddup2](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn_file_actions_adddup2.html).
    #[doc(alias("posix_spawn_file_actions_adddup2"))]
    pub fn add_dup2(&mut self, fd: RawFd, newfd: RawFd) -> Result<()> {
        let res = unsafe {
            libc::posix_spawn_file_actions_adddup2(
                &mut self.fa as *mut libc::posix_spawn_file_actions_t,
                fd,
                newfd,
            )
        };
        Errno::result(res)?;

        Ok(())
    }

    feature! {
    #![all(feature = "fs", feature = "term")]
    /// Add an open action. See
    /// [posix_spawn_file_actions_addopen](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn_file_actions_addopen.html).
    #[doc(alias("posix_spawn_file_actions_addopen"))]
    pub fn add_open<P: ?Sized + NixPath>(
        &mut self,
        fd: RawFd,
        path: &P,
        oflag: OFlag,
        mode: Mode,
    ) -> Result<()> {
        let res = path.with_nix_path(|cstr| unsafe {
            libc::posix_spawn_file_actions_addopen(
                &mut self.fa as *mut libc::posix_spawn_file_actions_t,
                fd,
                cstr.as_ptr(),
                oflag.bits(),
                mode.bits(),
            )
        })?;
        Errno::result(res)?;

        Ok(())
    }
    }

    /// Add a close action. See
    /// [posix_spawn_file_actions_addclose](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn_file_actions_addclose.html).
    #[doc(alias("posix_spawn_file_actions_addclose"))]
    pub fn add_close(&mut self, fd: RawFd) -> Result<()> {
        let res = unsafe {
            libc::posix_spawn_file_actions_addclose(
                &mut self.fa as *mut libc::posix_spawn_file_actions_t,
                fd,
            )
        };
        Errno::result(res)?;

        Ok(())
    }
}

impl Drop for PosixSpawnFileActions {
    fn drop(&mut self) {
        unsafe {
            libc::posix_spawn_file_actions_destroy(
                &mut self.fa as *mut libc::posix_spawn_file_actions_t,
            );
        }
    }
}

// The POSIX standard requires those `args` and `envp` to be of type `*const *mut [c_char]`,
// but implementations won't modify them, making the `mut` type redundant. Considering this,
// Nix does not expose this mutability, but we have to change the interface when calling the
// underlying libc interfaces , this helper function does the conversion job.
//
// SAFETY:
// It is safe to add the mutability in types as implementations won't mutable them.
unsafe fn to_exec_array<S: AsRef<CStr>>(args: &[S]) -> Vec<*mut libc::c_char> {
    let mut v: Vec<*mut libc::c_char> = args
        .iter()
        .map(|s| s.as_ref().as_ptr().cast_mut())
        .collect();
    v.push(std::ptr::null_mut());
    v
}

/// Create a new child process from the specified process image. See
/// [posix_spawn](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn.html).
pub fn posix_spawn<P, SA, SE>(
    path: &P,
    file_actions: &PosixSpawnFileActions,
    attr: &PosixSpawnAttr,
    args: &[SA],
    envp: &[SE],
) -> Result<Pid>
where
    P: NixPath + ?Sized,
    SA: AsRef<CStr>,
    SE: AsRef<CStr>,
{
    let mut pid = 0;

    let ret = unsafe {
        let args_p = to_exec_array(args);
        let env_p = to_exec_array(envp);

        path.with_nix_path(|c_str| {
            libc::posix_spawn(
                &mut pid as *mut libc::pid_t,
                c_str.as_ptr(),
                &file_actions.fa as *const libc::posix_spawn_file_actions_t,
                &attr.attr as *const libc::posix_spawnattr_t,
                args_p.as_ptr(),
                env_p.as_ptr(),
            )
        })?
    };

    if ret != 0 {
        return Err(Errno::from_raw(ret));
    }

    Ok(Pid::from_raw(pid))
}

/// Create a new child process from the specified process image. See
/// [posix_spawnp](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawnp.html).
pub fn posix_spawnp<SA: AsRef<CStr>, SE: AsRef<CStr>>(
    path: &CStr,
    file_actions: &PosixSpawnFileActions,
    attr: &PosixSpawnAttr,
    args: &[SA],
    envp: &[SE],
) -> Result<Pid> {
    let mut pid = 0;

    let ret = unsafe {
        let args_p = to_exec_array(args);
        let env_p = to_exec_array(envp);

        libc::posix_spawnp(
            &mut pid as *mut libc::pid_t,
            path.as_ptr(),
            &file_actions.fa as *const libc::posix_spawn_file_actions_t,
            &attr.attr as *const libc::posix_spawnattr_t,
            args_p.as_ptr(),
            env_p.as_ptr(),
        )
    };

    if ret != 0 {
        return Err(Errno::from_raw(ret));
    }

    Ok(Pid::from_raw(pid))
}
