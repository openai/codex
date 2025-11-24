//! Interfaces for managing memory-backed files.

use cfg_if::cfg_if;
use std::os::unix::io::{FromRawFd, OwnedFd, RawFd};

use crate::errno::Errno;
use crate::{NixPath, Result};

libc_bitflags!(
    /// Options that change the behavior of [`memfd_create`].
    pub struct MFdFlags: libc::c_uint {
        /// Set the close-on-exec ([`FD_CLOEXEC`]) flag on the new file descriptor.
        ///
        /// By default, the new file descriptor is set to remain open across an [`execve`]
        /// (the `FD_CLOEXEC` flag is initially disabled). This flag can be used to change
        /// this default. The file offset is set to the beginning of the file (see [`lseek`]).
        ///
        /// See also the description of the `O_CLOEXEC` flag in [`open(2)`].
        ///
        /// [`execve`]: crate::unistd::execve
        /// [`lseek`]: crate::unistd::lseek
        /// [`FD_CLOEXEC`]: crate::fcntl::FdFlag::FD_CLOEXEC
        /// [`open(2)`]: https://man7.org/linux/man-pages/man2/open.2.html
        MFD_CLOEXEC;
        /// Allow sealing operations on this file.
        ///
        /// See also the file sealing notes given in [`memfd_create(2)`].
        ///
        /// [`memfd_create(2)`]: https://man7.org/linux/man-pages/man2/memfd_create.2.html
        MFD_ALLOW_SEALING;
        /// Anonymous file will be created using huge pages. It should be safe now to
        /// combine with [`MFD_ALLOW_SEALING`] too.
        /// However, despite its presence, on FreeBSD it is unimplemented for now (ENOSYS).
        ///
        /// See also the hugetlb filesystem in [`memfd_create(2)`].
        ///
        /// [`memfd_create(2)`]: https://man7.org/linux/man-pages/man2/memfd_create.2.html
        #[cfg(linux_android)]
        MFD_HUGETLB;
        /// Shift to get the huge page size.
        #[cfg(target_env = "ohos")]
        MFD_HUGE_SHIFT;
        /// Mask to get the huge page size.
        #[cfg(target_env = "ohos")]
        MFD_HUGE_MASK;
        /// hugetlb size of 64KB.
        #[cfg(target_env = "ohos")]
        MFD_HUGE_64KB;
        /// hugetlb size of 512KB.
        #[cfg(target_env = "ohos")]
        MFD_HUGE_512KB;
        /// Following are to be used with [`MFD_HUGETLB`], indicating the desired hugetlb size.
        ///
        /// See also the hugetlb filesystem in [`memfd_create(2)`].
        ///
        /// [`memfd_create(2)`]: https://man7.org/linux/man-pages/man2/memfd_create.2.html
        #[cfg(linux_android)]
        MFD_HUGE_1MB;
        /// hugetlb size of 2MB.
        #[cfg(linux_android)]
        MFD_HUGE_2MB;
        /// hugetlb size of 8MB.
        #[cfg(linux_android)]
        MFD_HUGE_8MB;
        /// hugetlb size of 16MB.
        #[cfg(linux_android)]
        MFD_HUGE_16MB;
        /// hugetlb size of 32MB.
        #[cfg(linux_android)]
        MFD_HUGE_32MB;
        /// hugetlb size of 256MB.
        #[cfg(linux_android)]
        MFD_HUGE_256MB;
        /// hugetlb size of 512MB.
        #[cfg(linux_android)]
        MFD_HUGE_512MB;
        /// hugetlb size of 1GB.
        #[cfg(linux_android)]
        MFD_HUGE_1GB;
        /// hugetlb size of 2GB.
        #[cfg(linux_android)]
        MFD_HUGE_2GB;
        /// hugetlb size of 16GB.
        #[cfg(linux_android)]
        MFD_HUGE_16GB;
    }
);

#[deprecated(since = "0.30.0", note = "Use `MFdFlags instead`")]
/// The deprecated MemFdCreateFlag type alias
pub type MemFdCreateFlag = MFdFlags;

/// Creates an anonymous file that lives in memory, and return a file-descriptor to it.
///
/// The file behaves like a regular file, and so can be modified, truncated, memory-mapped, and so on.
/// However, unlike a regular file, it lives in RAM and has a volatile backing storage.
///
/// For more information, see [`memfd_create(2)`].
///
/// [`memfd_create(2)`]: https://man7.org/linux/man-pages/man2/memfd_create.2.html
#[inline] // Delays codegen, preventing linker errors with dylibs and --no-allow-shlib-undefined
pub fn memfd_create<P: NixPath + ?Sized>(
    name: &P,
    flags: MFdFlags,
) -> Result<OwnedFd> {
    let res = name.with_nix_path(|cstr| {
        unsafe {
            cfg_if! {
            if #[cfg(all(
                // Android does not have a memfd_create symbol
                not(target_os = "android"),
                any(
                    target_os = "freebsd",
                    // If the OS is Linux, gnu/musl/ohos expose a memfd_create symbol but not uclibc
                    target_env = "gnu",
                    target_env = "musl",
                    target_env = "ohos"
                )))]
            {
                libc::memfd_create(cstr.as_ptr(), flags.bits())
            } else {
                libc::syscall(libc::SYS_memfd_create, cstr.as_ptr(), flags.bits())
            }
        }
        }
    })?;

    Errno::result(res).map(|r| unsafe { OwnedFd::from_raw_fd(r as RawFd) })
}
