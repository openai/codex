#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
use crate::{backend, io, path};

/// `pivot_root(new_root, put_old)`—Change the root mount.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/pivot_root.2.html
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
#[inline]
pub fn pivot_root<P: path::Arg, Q: path::Arg>(new_root: P, put_old: Q) -> io::Result<()> {
    new_root.into_with_c_str(|new_root| {
        put_old.into_with_c_str(|put_old| backend::process::syscalls::pivot_root(new_root, put_old))
    })
}
