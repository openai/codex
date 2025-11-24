//! Execution scheduling
//!
//! See Also
//! [sched.h](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sched.h.html)
use crate::{Errno, Result};

#[cfg(linux_android)]
pub use self::sched_linux_like::*;

#[cfg(linux_android)]
mod sched_linux_like {
    use crate::errno::Errno;
    use crate::unistd::Pid;
    use crate::Result;
    use libc::{self, c_int, c_void};
    use std::mem;
    use std::option::Option;
    use std::os::unix::io::{AsFd, AsRawFd};

    // For some functions taking with a parameter of type CloneFlags,
    // only a subset of these flags have an effect.
    libc_bitflags! {
        /// Options for use with [`clone`]
        pub struct CloneFlags: c_int {
            /// The calling process and the child process run in the same
            /// memory space.
            CLONE_VM;
            /// The caller and the child process share the same  filesystem
            /// information.
            CLONE_FS;
            /// The calling process and the child process share the same file
            /// descriptor table.
            CLONE_FILES;
            /// The calling process and the child process share the same table
            /// of signal handlers.
            CLONE_SIGHAND;
            /// If the calling process is being traced, then trace the child
            /// also.
            CLONE_PTRACE;
            /// The execution of the calling process is suspended until the
            /// child releases its virtual memory resources via a call to
            /// execve(2) or _exit(2) (as with vfork(2)).
            CLONE_VFORK;
            /// The parent of the new child  (as returned by getppid(2))
            /// will be the same as that of the calling process.
            CLONE_PARENT;
            /// The child is placed in the same thread group as the calling
            /// process.
            CLONE_THREAD;
            /// The cloned child is started in a new mount namespace.
            CLONE_NEWNS;
            /// The child and the calling process share a single list of System
            /// V semaphore adjustment values
            CLONE_SYSVSEM;
            // Not supported by Nix due to lack of varargs support in Rust FFI
            // CLONE_SETTLS;
            // Not supported by Nix due to lack of varargs support in Rust FFI
            // CLONE_PARENT_SETTID;
            // Not supported by Nix due to lack of varargs support in Rust FFI
            // CLONE_CHILD_CLEARTID;
            /// Unused since Linux 2.6.2
            #[deprecated(since = "0.23.0", note = "Deprecated by Linux 2.6.2")]
            CLONE_DETACHED;
            /// A tracing process cannot force `CLONE_PTRACE` on this child
            /// process.
            CLONE_UNTRACED;
            // Not supported by Nix due to lack of varargs support in Rust FFI
            // CLONE_CHILD_SETTID;
            /// Create the process in a new cgroup namespace.
            CLONE_NEWCGROUP;
            /// Create the process in a new UTS namespace.
            CLONE_NEWUTS;
            /// Create the process in a new IPC namespace.
            CLONE_NEWIPC;
            /// Create the process in a new user namespace.
            CLONE_NEWUSER;
            /// Create the process in a new PID namespace.
            CLONE_NEWPID;
            /// Create the process in a new network namespace.
            CLONE_NEWNET;
            /// The new process shares an I/O context with the calling process.
            CLONE_IO;
        }
    }

    /// Type for the function executed by [`clone`].
    pub type CloneCb<'a> = Box<dyn FnMut() -> isize + 'a>;

    /// `clone` create a child process
    /// ([`clone(2)`](https://man7.org/linux/man-pages/man2/clone.2.html))
    ///
    /// `stack` is a reference to an array which will hold the stack of the new
    /// process.  Unlike when calling `clone(2)` from C, the provided stack
    /// address need not be the highest address of the region.  Nix will take
    /// care of that requirement.  The user only needs to provide a reference to
    /// a normally allocated buffer.
    ///
    /// # Safety
    ///
    /// Because `clone` creates a child process with its stack located in
    /// `stack` without specifying the size of the stack, special care must be
    /// taken to ensure that the child process does not overflow the provided
    /// stack space.
    ///
    /// See [`fork`](crate::unistd::fork) for additional safety concerns related
    /// to executing child processes.
    pub unsafe fn clone(
        mut cb: CloneCb,
        stack: &mut [u8],
        flags: CloneFlags,
        signal: Option<c_int>,
    ) -> Result<Pid> {
        extern "C" fn callback(data: *mut CloneCb) -> c_int {
            let cb: &mut CloneCb = unsafe { &mut *data };
            (*cb)() as c_int
        }

        let combined = flags.bits() | signal.unwrap_or(0);
        let res = unsafe {
            let ptr = stack.as_mut_ptr().add(stack.len());
            let ptr_aligned = ptr.sub(ptr as usize % 16);
            libc::clone(
                mem::transmute(
                    callback
                        as extern "C" fn(*mut Box<dyn FnMut() -> isize>) -> i32,
                ),
                ptr_aligned as *mut c_void,
                combined,
                &mut cb as *mut _ as *mut c_void,
            )
        };

        Errno::result(res).map(Pid::from_raw)
    }

    /// disassociate parts of the process execution context
    ///
    /// See also [unshare(2)](https://man7.org/linux/man-pages/man2/unshare.2.html)
    pub fn unshare(flags: CloneFlags) -> Result<()> {
        let res = unsafe { libc::unshare(flags.bits()) };

        Errno::result(res).map(drop)
    }

    /// reassociate thread with a namespace
    ///
    /// See also [setns(2)](https://man7.org/linux/man-pages/man2/setns.2.html)
    pub fn setns<Fd: AsFd>(fd: Fd, nstype: CloneFlags) -> Result<()> {
        let res = unsafe { libc::setns(fd.as_fd().as_raw_fd(), nstype.bits()) };

        Errno::result(res).map(drop)
    }
}

#[cfg(any(linux_android, freebsdlike))]
pub use self::sched_affinity::*;

#[cfg(any(linux_android, freebsdlike))]
mod sched_affinity {
    use crate::errno::Errno;
    use crate::unistd::Pid;
    use crate::Result;
    use std::mem;

    /// CpuSet represent a bit-mask of CPUs.
    /// CpuSets are used by sched_setaffinity and
    /// sched_getaffinity for example.
    ///
    /// This is a wrapper around `libc::cpu_set_t`.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct CpuSet {
        #[cfg(not(target_os = "freebsd"))]
        cpu_set: libc::cpu_set_t,
        #[cfg(target_os = "freebsd")]
        cpu_set: libc::cpuset_t,
    }

    impl CpuSet {
        /// Create a new and empty CpuSet.
        pub fn new() -> CpuSet {
            CpuSet {
                cpu_set: unsafe { mem::zeroed() },
            }
        }

        /// Test to see if a CPU is in the CpuSet.
        /// `field` is the CPU id to test
        pub fn is_set(&self, field: usize) -> Result<bool> {
            if field >= CpuSet::count() {
                Err(Errno::EINVAL)
            } else {
                Ok(unsafe { libc::CPU_ISSET(field, &self.cpu_set) })
            }
        }

        /// Add a CPU to CpuSet.
        /// `field` is the CPU id to add
        pub fn set(&mut self, field: usize) -> Result<()> {
            if field >= CpuSet::count() {
                Err(Errno::EINVAL)
            } else {
                unsafe {
                    libc::CPU_SET(field, &mut self.cpu_set);
                }
                Ok(())
            }
        }

        /// Remove a CPU from CpuSet.
        /// `field` is the CPU id to remove
        pub fn unset(&mut self, field: usize) -> Result<()> {
            if field >= CpuSet::count() {
                Err(Errno::EINVAL)
            } else {
                unsafe {
                    libc::CPU_CLR(field, &mut self.cpu_set);
                }
                Ok(())
            }
        }

        /// Return the maximum number of CPU in CpuSet
        pub const fn count() -> usize {
            #[cfg(not(target_os = "freebsd"))]
            let bytes = mem::size_of::<libc::cpu_set_t>();
            #[cfg(target_os = "freebsd")]
            let bytes = mem::size_of::<libc::cpuset_t>();

            8 * bytes
        }
    }

    impl Default for CpuSet {
        fn default() -> Self {
            Self::new()
        }
    }

    /// `sched_setaffinity` set a thread's CPU affinity mask
    /// ([`sched_setaffinity(2)`](https://man7.org/linux/man-pages/man2/sched_setaffinity.2.html))
    ///
    /// `pid` is the thread ID to update.
    /// If pid is zero, then the calling thread is updated.
    ///
    /// The `cpuset` argument specifies the set of CPUs on which the thread
    /// will be eligible to run.
    ///
    /// # Example
    ///
    /// Binding the current thread to CPU 0 can be done as follows:
    ///
    /// ```rust,no_run
    /// use nix::sched::{CpuSet, sched_setaffinity};
    /// use nix::unistd::Pid;
    ///
    /// let mut cpu_set = CpuSet::new();
    /// cpu_set.set(0).unwrap();
    /// sched_setaffinity(Pid::from_raw(0), &cpu_set).unwrap();
    /// ```
    pub fn sched_setaffinity(pid: Pid, cpuset: &CpuSet) -> Result<()> {
        let res = unsafe {
            libc::sched_setaffinity(
                pid.into(),
                mem::size_of::<CpuSet>() as libc::size_t,
                &cpuset.cpu_set,
            )
        };

        Errno::result(res).map(drop)
    }

    /// `sched_getaffinity` get a thread's CPU affinity mask
    /// ([`sched_getaffinity(2)`](https://man7.org/linux/man-pages/man2/sched_getaffinity.2.html))
    ///
    /// `pid` is the thread ID to check.
    /// If pid is zero, then the calling thread is checked.
    ///
    /// Returned `cpuset` is the set of CPUs on which the thread
    /// is eligible to run.
    ///
    /// # Example
    ///
    /// Checking if the current thread can run on CPU 0 can be done as follows:
    ///
    /// ```rust,no_run
    /// use nix::sched::sched_getaffinity;
    /// use nix::unistd::Pid;
    ///
    /// let cpu_set = sched_getaffinity(Pid::from_raw(0)).unwrap();
    /// if cpu_set.is_set(0).unwrap() {
    ///     println!("Current thread can run on CPU 0");
    /// }
    /// ```
    pub fn sched_getaffinity(pid: Pid) -> Result<CpuSet> {
        let mut cpuset = CpuSet::new();
        let res = unsafe {
            libc::sched_getaffinity(
                pid.into(),
                mem::size_of::<CpuSet>() as libc::size_t,
                &mut cpuset.cpu_set,
            )
        };

        Errno::result(res).and(Ok(cpuset))
    }

    /// Determines the CPU on which the calling thread is running.
    pub fn sched_getcpu() -> Result<usize> {
        let res = unsafe { libc::sched_getcpu() };

        Errno::result(res).map(|int| int as usize)
    }
}

/// Explicitly yield the processor to other threads.
///
/// [Further reading](https://pubs.opengroup.org/onlinepubs/9699919799/functions/sched_yield.html)
pub fn sched_yield() -> Result<()> {
    let res = unsafe { libc::sched_yield() };

    Errno::result(res).map(drop)
}
