//! Configure the process resource limits.
use cfg_if::cfg_if;
use libc::{c_int, c_long, rusage};

use crate::errno::Errno;
use crate::sys::time::TimeVal;
use crate::Result;
pub use libc::rlim_t;
pub use libc::RLIM_INFINITY;
use std::mem;

cfg_if! {
    if #[cfg(any(
        all(target_os = "linux", any(target_env = "gnu", target_env = "uclibc")),
        target_os = "hurd"
    ))]{
        use libc::{__rlimit_resource_t, rlimit};
    } else if #[cfg(any(
        bsd,
        target_os = "android",
        target_os = "aix",
        all(target_os = "linux", not(target_env = "gnu"))
    ))]{
        use libc::rlimit;
    }
}

libc_enum! {
    /// Types of process resources.
    ///
    /// The Resource enum is platform dependent. Check different platform
    /// manuals for more details. Some platform links have been provided for
    /// easier reference (non-exhaustive).
    ///
    /// * [Linux](https://man7.org/linux/man-pages/man2/getrlimit.2.html)
    /// * [FreeBSD](https://www.freebsd.org/cgi/man.cgi?query=setrlimit)
    /// * [NetBSD](https://man.netbsd.org/setrlimit.2)

    // linux-gnu uses u_int as resource enum, which is implemented in libc as
    // well.
    //
    // https://gcc.gnu.org/legacy-ml/gcc/2015-08/msg00441.html
    // https://github.com/rust-lang/libc/blob/master/src/unix/linux_like/linux/gnu/mod.rs
    #[cfg_attr(any(
            all(target_os = "linux", any(target_env = "gnu", target_env = "uclibc")),
            target_os = "hurd"
        ), repr(u32))]
    #[cfg_attr(any(
            bsd,
            target_os = "android",
            target_os = "aix",
            all(target_os = "linux", not(any(target_env = "gnu", target_env = "uclibc")))
        ), repr(i32))]
    #[non_exhaustive]
    pub enum Resource {
        #[cfg(not(any(target_os = "freebsd", netbsdlike)))]
        /// The maximum amount (in bytes) of virtual memory the process is
        /// allowed to map.
        RLIMIT_AS,
        /// The largest size (in bytes) core(5) file that may be created.
        RLIMIT_CORE,
        /// The maximum amount of cpu time (in seconds) to be used by each
        /// process.
        RLIMIT_CPU,
        /// The maximum size (in bytes) of the data segment for a process
        RLIMIT_DATA,
        /// The largest size (in bytes) file that may be created.
        RLIMIT_FSIZE,
        /// The maximum number of open files for this process.
        RLIMIT_NOFILE,
        /// The maximum size (in bytes) of the stack segment for a process.
        RLIMIT_STACK,

        #[cfg(target_os = "freebsd")]
        /// The maximum number of kqueues this user id is allowed to create.
        RLIMIT_KQUEUES,

        #[cfg(linux_android)]
        /// A limit on the combined number of flock locks and fcntl leases that
        /// this process may establish.
        RLIMIT_LOCKS,

        #[cfg(any(linux_android, target_os = "freebsd", netbsdlike))]
        /// The maximum size (in bytes) which a process may lock into memory
        /// using the mlock(2) system call.
        RLIMIT_MEMLOCK,

        #[cfg(linux_android)]
        /// A limit on the number of bytes that can be allocated for POSIX
        /// message queues  for  the  real  user  ID  of  the  calling process.
        RLIMIT_MSGQUEUE,

        #[cfg(linux_android)]
        /// A ceiling to which the process's nice value can be raised using
        /// setpriority or nice.
        RLIMIT_NICE,

        #[cfg(any(
            linux_android,
            target_os = "freebsd",
            netbsdlike,
            target_os = "aix",
        ))]
        /// The maximum number of simultaneous processes for this user id.
        RLIMIT_NPROC,

        #[cfg(target_os = "freebsd")]
        /// The maximum number of pseudo-terminals this user id is allowed to
        /// create.
        RLIMIT_NPTS,

        #[cfg(any(linux_android,
            target_os = "freebsd",
            netbsdlike,
            target_os = "aix",
        ))]
        /// When there is memory pressure and swap is available, prioritize
        /// eviction of a process' resident pages beyond this amount (in bytes).
        RLIMIT_RSS,

        #[cfg(linux_android)]
        /// A ceiling on the real-time priority that may be set for this process
        /// using sched_setscheduler and  sched_set‐ param.
        RLIMIT_RTPRIO,

        #[cfg(any(target_os = "linux"))]
        /// A limit (in microseconds) on the amount of CPU time that a process
        /// scheduled under a real-time scheduling policy may con‐ sume without
        /// making a blocking system call.
        RLIMIT_RTTIME,

        #[cfg(linux_android)]
        /// A limit on the number of signals that may be queued for the real
        /// user ID of the  calling  process.
        RLIMIT_SIGPENDING,

        #[cfg(freebsdlike)]
        /// The maximum size (in bytes) of socket buffer usage for this user.
        RLIMIT_SBSIZE,

        #[cfg(target_os = "freebsd")]
        /// The maximum size (in bytes) of the swap space that may be reserved
        /// or used by all of this user id's processes.
        RLIMIT_SWAP,

        #[cfg(target_os = "freebsd")]
        /// An alias for RLIMIT_AS.
        RLIMIT_VMEM,
    }
}

/// Get the current processes resource limits
///
/// The special value [`RLIM_INFINITY`] indicates that no limit will be
/// enforced.
///
/// # Parameters
///
/// * `resource`: The [`Resource`] that we want to get the limits of.
///
/// # Examples
///
/// ```
/// # use nix::sys::resource::{getrlimit, Resource};
///
/// let (soft_limit, hard_limit) = getrlimit(Resource::RLIMIT_NOFILE).unwrap();
/// println!("current soft_limit: {}", soft_limit);
/// println!("current hard_limit: {}", hard_limit);
/// ```
///
/// # References
///
/// [getrlimit(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/getrlimit.html#tag_16_215)
///
/// [`Resource`]: enum.Resource.html
pub fn getrlimit(resource: Resource) -> Result<(rlim_t, rlim_t)> {
    let mut old_rlim = mem::MaybeUninit::<rlimit>::uninit();

    cfg_if! {
        if #[cfg(any(
            all(target_os = "linux", any(target_env = "gnu", target_env = "uclibc")),
            target_os = "hurd"
        ))] {
            let res = unsafe { libc::getrlimit(resource as __rlimit_resource_t, old_rlim.as_mut_ptr()) };
        } else {
            let res = unsafe { libc::getrlimit(resource as c_int, old_rlim.as_mut_ptr()) };
        }
    }

    Errno::result(res).map(|_| {
        let rlimit { rlim_cur, rlim_max } = unsafe { old_rlim.assume_init() };
        (rlim_cur, rlim_max)
    })
}

/// Set the current processes resource limits
///
/// # Parameters
///
/// * `resource`: The [`Resource`] that we want to set the limits of.
/// * `soft_limit`: The value that the kernel enforces for the corresponding
///   resource.
/// * `hard_limit`: The ceiling for the soft limit. Must be lower or equal to
///   the current hard limit for non-root users.
///
/// The special value [`RLIM_INFINITY`] indicates that no limit will be
/// enforced.
///
/// # Examples
///
/// ```
/// # use nix::sys::resource::{setrlimit, Resource};
///
/// let soft_limit = 512;
/// let hard_limit = 1024;
/// setrlimit(Resource::RLIMIT_NOFILE, soft_limit, hard_limit).unwrap();
/// ```
///
/// # References
///
/// [setrlimit(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/getrlimit.html#tag_16_215)
///
/// [`Resource`]: enum.Resource.html
///
/// Note: `setrlimit` provides a safe wrapper to libc's `setrlimit`.
pub fn setrlimit(
    resource: Resource,
    soft_limit: rlim_t,
    hard_limit: rlim_t,
) -> Result<()> {
    let new_rlim = rlimit {
        rlim_cur: soft_limit,
        rlim_max: hard_limit,
    };
    cfg_if! {
        if #[cfg(any(
            all(target_os = "linux", any(target_env = "gnu", target_env = "uclibc")),
            target_os = "hurd",
        ))]{
            let res = unsafe { libc::setrlimit(resource as __rlimit_resource_t, &new_rlim as *const rlimit) };
        }else{
            let res = unsafe { libc::setrlimit(resource as c_int, &new_rlim as *const rlimit) };
        }
    }

    Errno::result(res).map(drop)
}

libc_enum! {
    /// Whose resource usage should be returned by [`getrusage`].
    #[repr(i32)]
    #[non_exhaustive]
    pub enum UsageWho {
        /// Resource usage for the current process.
        RUSAGE_SELF,

        /// Resource usage for all the children that have terminated and been waited for.
        RUSAGE_CHILDREN,

        #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
        /// Resource usage for the calling thread.
        RUSAGE_THREAD,
    }
}

/// Output of `getrusage` with information about resource usage. Some of the fields
/// may be unused in some platforms, and will be always zeroed out. See their manuals
/// for details.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Usage(rusage);

impl AsRef<rusage> for Usage {
    fn as_ref(&self) -> &rusage {
        &self.0
    }
}

impl AsMut<rusage> for Usage {
    fn as_mut(&mut self) -> &mut rusage {
        &mut self.0
    }
}

impl Usage {
    /// Total amount of time spent executing in user mode.
    pub fn user_time(&self) -> TimeVal {
        TimeVal::from(self.0.ru_utime)
    }

    /// Total amount of time spent executing in kernel mode.
    pub fn system_time(&self) -> TimeVal {
        TimeVal::from(self.0.ru_stime)
    }

    /// The resident set size at its peak, in kilobytes.
    pub fn max_rss(&self) -> c_long {
        self.0.ru_maxrss
    }

    /// Integral value expressed in kilobytes times ticks of execution indicating
    /// the amount of text memory shared with other processes.
    pub fn shared_integral(&self) -> c_long {
        self.0.ru_ixrss
    }

    /// Integral value expressed in kilobytes times ticks of execution indicating
    /// the amount of unshared memory used by data.
    pub fn unshared_data_integral(&self) -> c_long {
        self.0.ru_idrss
    }

    /// Integral value expressed in kilobytes times ticks of execution indicating
    /// the amount of unshared memory used for stack space.
    pub fn unshared_stack_integral(&self) -> c_long {
        self.0.ru_isrss
    }

    /// Number of page faults that were served without resorting to I/O, with pages
    /// that have been allocated previously by the kernel.
    pub fn minor_page_faults(&self) -> c_long {
        self.0.ru_minflt
    }

    /// Number of page faults that were served through I/O (i.e. swap).
    pub fn major_page_faults(&self) -> c_long {
        self.0.ru_majflt
    }

    /// Number of times all of the memory was fully swapped out.
    pub fn full_swaps(&self) -> c_long {
        self.0.ru_nswap
    }

    /// Number of times a read was done from a block device.
    pub fn block_reads(&self) -> c_long {
        self.0.ru_inblock
    }

    /// Number of times a write was done to a block device.
    pub fn block_writes(&self) -> c_long {
        self.0.ru_oublock
    }

    /// Number of IPC messages sent.
    pub fn ipc_sends(&self) -> c_long {
        self.0.ru_msgsnd
    }

    /// Number of IPC messages received.
    pub fn ipc_receives(&self) -> c_long {
        self.0.ru_msgrcv
    }

    /// Number of signals received.
    pub fn signals(&self) -> c_long {
        self.0.ru_nsignals
    }

    /// Number of times a context switch was voluntarily invoked.
    pub fn voluntary_context_switches(&self) -> c_long {
        self.0.ru_nvcsw
    }

    /// Number of times a context switch was imposed by the kernel (usually due to
    /// time slice expiring or preemption by a higher priority process).
    pub fn involuntary_context_switches(&self) -> c_long {
        self.0.ru_nivcsw
    }
}

/// Get usage information for a process, its children or the current thread
///
/// Real time information can be obtained for either the current process or (in some
/// systems) thread, but information about children processes is only provided for
/// those that have terminated and been waited for (see [`super::wait::wait`]).
///
/// Some information may be missing depending on the platform, and the way information
/// is provided for children may also vary. Check the manuals for details.
///
/// # References
///
/// * [getrusage(2)](https://pubs.opengroup.org/onlinepubs/009696699/functions/getrusage.html)
/// * [Linux](https://man7.org/linux/man-pages/man2/getrusage.2.html)
/// * [FreeBSD](https://www.freebsd.org/cgi/man.cgi?query=getrusage)
/// * [NetBSD](https://man.netbsd.org/getrusage.2)
/// * [MacOS](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getrusage.2.html)
///
/// [`UsageWho`]: enum.UsageWho.html
///
/// Note: `getrusage` provides a safe wrapper to libc's [`libc::getrusage`].
pub fn getrusage(who: UsageWho) -> Result<Usage> {
    unsafe {
        let mut rusage = mem::MaybeUninit::<rusage>::uninit();
        let res = libc::getrusage(who as c_int, rusage.as_mut_ptr());
        Errno::result(res).map(|_| Usage(rusage.assume_init()))
    }
}
