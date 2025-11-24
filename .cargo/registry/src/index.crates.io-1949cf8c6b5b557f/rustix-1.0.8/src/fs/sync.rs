use crate::backend;

/// `sync`—Flush cached filesystem data for all filesystems.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/sync.html
/// [Linux]: https://man7.org/linux/man-pages/man2/sync.2.html
#[inline]
pub fn sync() {
    backend::fs::syscalls::sync();
}
