use crate::pid::Pid;
use crate::{backend, io};
use core::{fmt, hash};

/// `CpuSet` represents a bit-mask of CPUs.
///
/// `CpuSet`s are used by [`sched_setaffinity`] and [`sched_getaffinity`], for
/// example.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/CPU_SET.3.html
/// [`sched_setaffinity`]: crate::thread::sched_setaffinity
/// [`sched_getaffinity`]: crate::thread::sched_getaffinity
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct CpuSet {
    cpu_set: backend::thread::types::RawCpuSet,
}

impl CpuSet {
    /// The maximum number of CPU in `CpuSet`.
    pub const MAX_CPU: usize = backend::thread::types::CPU_SETSIZE;

    /// Create a new and empty `CpuSet`.
    #[inline]
    pub fn new() -> Self {
        Self {
            cpu_set: backend::thread::types::raw_cpu_set_new(),
        }
    }

    /// Test to see if a CPU is in the `CpuSet`.
    ///
    /// `field` is the CPU id to test.
    #[inline]
    pub fn is_set(&self, field: usize) -> bool {
        backend::thread::cpu_set::CPU_ISSET(field, &self.cpu_set)
    }

    /// Add a CPU to `CpuSet`.
    ///
    /// `field` is the CPU id to add.
    #[inline]
    pub fn set(&mut self, field: usize) {
        backend::thread::cpu_set::CPU_SET(field, &mut self.cpu_set)
    }

    /// Remove a CPU from `CpuSet`.
    ///
    /// `field` is the CPU id to remove.
    #[inline]
    pub fn unset(&mut self, field: usize) {
        backend::thread::cpu_set::CPU_CLR(field, &mut self.cpu_set)
    }

    /// Count the number of CPUs set in the `CpuSet`.
    #[cfg(linux_kernel)]
    #[inline]
    pub fn count(&self) -> u32 {
        backend::thread::cpu_set::CPU_COUNT(&self.cpu_set)
    }

    /// Zeroes the `CpuSet`.
    #[inline]
    pub fn clear(&mut self) {
        backend::thread::cpu_set::CPU_ZERO(&mut self.cpu_set)
    }
}

impl Default for CpuSet {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CpuSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CpuSet {{")?;
        let mut first = true;
        for i in 0..Self::MAX_CPU {
            if self.is_set(i) {
                if first {
                    write!(f, " ")?;
                    first = false;
                } else {
                    write!(f, ", ")?;
                }
                write!(f, "cpu{}", i)?;
            }
        }
        write!(f, " }}")
    }
}

impl hash::Hash for CpuSet {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        for i in 0..Self::MAX_CPU {
            self.is_set(i).hash(state);
        }
    }
}

impl Eq for CpuSet {}

impl PartialEq for CpuSet {
    fn eq(&self, other: &Self) -> bool {
        backend::thread::cpu_set::CPU_EQUAL(&self.cpu_set, &other.cpu_set)
    }
}

/// `sched_setaffinity(pid, cpuset)`—Set a thread's CPU affinity mask.
///
/// `pid` is the thread ID to update. If pid is `None`, then the current thread
/// is updated.
///
/// The `CpuSet` argument specifies the set of CPUs on which the thread will be
/// eligible to run.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sched_setaffinity.2.html
#[inline]
pub fn sched_setaffinity(pid: Option<Pid>, cpuset: &CpuSet) -> io::Result<()> {
    backend::thread::syscalls::sched_setaffinity(pid, &cpuset.cpu_set)
}

/// `sched_getaffinity(pid)`—Get a thread's CPU affinity mask.
///
/// `pid` is the thread ID to check. If pid is `None`, then the current thread
/// is checked.
///
/// Returns the set of CPUs on which the thread is eligible to run.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sched_getaffinity.2.html
#[inline]
pub fn sched_getaffinity(pid: Option<Pid>) -> io::Result<CpuSet> {
    let mut cpuset = CpuSet::new();
    backend::thread::syscalls::sched_getaffinity(pid, &mut cpuset.cpu_set).and(Ok(cpuset))
}

/// `sched_getcpu()`—Get the CPU that the current thread is currently on.
///
/// # References
///  - [Linux]
///  - [DragonFly BSD]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/sched_getcpu.3.html
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=sched_getcpu&section=2
// FreeBSD added `sched_getcpu` in 13.0.
#[cfg(any(linux_kernel, target_os = "dragonfly"))]
#[inline]
pub fn sched_getcpu() -> usize {
    backend::thread::syscalls::sched_getcpu()
}
