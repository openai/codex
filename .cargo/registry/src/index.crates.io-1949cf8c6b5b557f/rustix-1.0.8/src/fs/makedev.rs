use crate::backend;
use crate::fs::Dev;

/// `makedev(maj, min)`—Compute a device ID from a given major and minor ID.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/makedev.3.html
#[inline]
pub fn makedev(maj: u32, min: u32) -> Dev {
    backend::fs::makedev::makedev(maj, min)
}

/// `minor(dev)`—Compute the minor ID of a given device ID.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/minor.3.html
#[inline]
pub fn minor(dev: Dev) -> u32 {
    backend::fs::makedev::minor(dev)
}

/// `major(dev)`—Compute the major ID of a given device ID.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/major.3.html
#[inline]
pub fn major(dev: Dev) -> u32 {
    backend::fs::makedev::major(dev)
}
