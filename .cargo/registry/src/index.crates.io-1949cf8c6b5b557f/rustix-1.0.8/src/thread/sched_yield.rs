use crate::backend;

/// `sched_yield()`—Hints to the OS that other processes should run.
///
/// This function always succeeds.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sched_yield.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sched_yield.2.html
#[inline]
pub fn sched_yield() {
    backend::thread::syscalls::sched_yield()
}
