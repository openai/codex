use crate::backend;
use crate::fs::Dev;

/// `makedev(maj, min)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/makedev.3.html
#[inline]
pub fn makedev(maj: u32, min: u32) -> Dev {
    backend::fs::makedev::makedev(maj, min)
}

/// `minor(dev)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/minor.3.html
#[inline]
pub fn minor(dev: Dev) -> u32 {
    backend::fs::makedev::minor(dev)
}

/// `major(dev)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/major.3.html
#[inline]
pub fn major(dev: Dev) -> u32 {
    backend::fs::makedev::major(dev)
}
