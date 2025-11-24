//! The `select` function.
//!
//! # Safety
//!
//! `select` is unsafe due to I/O safety.
#![allow(unsafe_code)]

#[cfg(any(linux_like, target_os = "wasi"))]
use crate::backend::c;
use crate::fd::RawFd;
use crate::{backend, io};
#[cfg(any(windows, target_os = "wasi"))]
use core::mem::{align_of, size_of};
#[cfg(any(windows, target_os = "wasi"))]
use core::slice;

pub use crate::timespec::{Nsecs, Secs, Timespec};

/// wasi-libc's `fd_set` type. The libc bindings for it have private fields,
/// so we redeclare it for ourselves so that we can access the fields. They're
/// publicly exposed in wasi-libc.
#[cfg(target_os = "wasi")]
#[repr(C)]
struct FD_SET {
    /// The wasi-libc headers call this `__nfds`.
    fd_count: usize,
    /// The wasi-libc headers call this `__fds`.
    fd_array: [i32; c::FD_SETSIZE],
}

#[cfg(windows)]
use windows_sys::Win32::Networking::WinSock::FD_SET;

/// Storage element type for use with [`select`].
#[cfg(any(
    windows,
    all(
        target_pointer_width = "64",
        any(target_os = "freebsd", target_os = "dragonfly")
    )
))]
#[repr(transparent)]
#[derive(Copy, Clone, Default)]
pub struct FdSetElement(pub(crate) u64);

/// Storage element type for use with [`select`].
#[cfg(linux_like)]
#[repr(transparent)]
#[derive(Copy, Clone, Default)]
pub struct FdSetElement(pub(crate) c::c_ulong);

/// Storage element type for use with [`select`].
#[cfg(not(any(
    linux_like,
    windows,
    target_os = "wasi",
    all(
        target_pointer_width = "64",
        any(target_os = "freebsd", target_os = "dragonfly")
    )
)))]
#[repr(transparent)]
#[derive(Copy, Clone, Default)]
pub struct FdSetElement(pub(crate) u32);

/// Storage element type for use with [`select`].
#[cfg(target_os = "wasi")]
#[repr(transparent)]
#[derive(Copy, Clone, Default)]
pub struct FdSetElement(pub(crate) usize);

/// `select(nfds, readfds, writefds, exceptfds, timeout)`—Wait for events on
/// sets of file descriptors.
///
/// `readfds`, `writefds`, `exceptfds` must point to arrays of `FdSetElement`
/// containing at least `nfds.div_ceil(size_of::<FdSetElement>())` elements.
///
/// This `select` wrapper differs from POSIX in that `nfds` is not limited to
/// `FD_SETSIZE`. Instead of using the fixed-sized `fd_set` type, this function
/// takes raw pointers to arrays of `fd_set_num_elements(max_fd + 1, num_fds)`,
/// where `max_fd` is the maximum value of any fd that will be inserted into
/// the set, and `num_fds` is the maximum number of fds that will be inserted
/// into the set.
///
/// In particular, on Apple platforms, this function behaves as if
/// `_DARWIN_UNLIMITED_SELECT` were predefined.
///
/// On illumos, this function is not defined because the `select` function on
/// this platform always has an `FD_SETSIZE` limitation, following POSIX. This
/// platform's documentation recommends using [`poll`] instead.
///
/// [`fd_set_insert`], [`fd_set_remove`], and [`FdSetIter`] are provided for
/// setting, clearing, and iterating with sets.
///
/// [`poll`]: crate::event::poll()
///
/// # Safety
///
/// All fds in in all the sets must correspond to open file descriptors.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [Winsock]
///  - [glibc]
///
///  [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/select.html
///  [Linux]: https://man7.org/linux/man-pages/man2/select.2.html
///  [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/select.2.html
///  [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=select&sektion=2
///  [NetBSD]: https://man.netbsd.org/select.2
///  [OpenBSD]: https://man.openbsd.org/select.2
///  [DragonFly BSD]: https://man.dragonflybsd.org/?command=select&section=2
///  [Winsock]: https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-select
///  [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Waiting-for-I_002fO.html#index-select
pub unsafe fn select(
    nfds: i32,
    readfds: Option<&mut [FdSetElement]>,
    writefds: Option<&mut [FdSetElement]>,
    exceptfds: Option<&mut [FdSetElement]>,
    timeout: Option<&Timespec>,
) -> io::Result<i32> {
    backend::event::syscalls::select(nfds, readfds, writefds, exceptfds, timeout)
}

#[cfg(not(any(windows, target_os = "wasi")))]
const BITS: usize = core::mem::size_of::<FdSetElement>() * 8;

/// Set `fd` in the set pointed to by `fds`.
#[doc(alias = "FD_SET")]
#[inline]
pub fn fd_set_insert(fds: &mut [FdSetElement], fd: RawFd) {
    #[cfg(not(any(windows, target_os = "wasi")))]
    {
        let fd = fd as usize;
        fds[fd / BITS].0 |= 1 << (fd % BITS);
    }

    #[cfg(any(windows, target_os = "wasi"))]
    {
        let set = unsafe { &mut *fds.as_mut_ptr().cast::<FD_SET>() };
        let fd_count = set.fd_count;
        let fd_array = unsafe { slice::from_raw_parts(set.fd_array.as_ptr(), fd_count as usize) };

        if !fd_array.iter().any(|p| *p as RawFd == fd) {
            let fd_array = unsafe {
                slice::from_raw_parts_mut(set.fd_array.as_mut_ptr(), fd_count as usize + 1)
            };
            set.fd_count = fd_count + 1;
            fd_array[fd_count as usize] = fd as _;
        }
    }
}

/// Clear `fd` in the set pointed to by `fds`.
#[doc(alias = "FD_CLR")]
#[inline]
pub fn fd_set_remove(fds: &mut [FdSetElement], fd: RawFd) {
    #[cfg(not(any(windows, target_os = "wasi")))]
    {
        let fd = fd as usize;
        fds[fd / BITS].0 &= !(1 << (fd % BITS));
    }

    #[cfg(any(windows, target_os = "wasi"))]
    {
        let set = unsafe { &mut *fds.as_mut_ptr().cast::<FD_SET>() };
        let fd_count = set.fd_count;
        let fd_array = unsafe { slice::from_raw_parts(set.fd_array.as_ptr(), fd_count as usize) };

        if let Some(pos) = fd_array.iter().position(|p| *p as RawFd == fd) {
            set.fd_count = fd_count - 1;
            set.fd_array[pos] = *set.fd_array.last().unwrap();
        }
    }
}

/// Compute the minimum `nfds` value needed for the set pointed to by
/// `fds`.
#[inline]
pub fn fd_set_bound(fds: &[FdSetElement]) -> RawFd {
    #[cfg(not(any(windows, target_os = "wasi")))]
    {
        if let Some(position) = fds.iter().rposition(|element| element.0 != 0) {
            let element = fds[position].0;
            (position * BITS + (BITS - element.leading_zeros() as usize)) as RawFd
        } else {
            0
        }
    }

    #[cfg(any(windows, target_os = "wasi"))]
    {
        let set = unsafe { &*fds.as_ptr().cast::<FD_SET>() };
        let fd_count = set.fd_count;
        let fd_array = unsafe { slice::from_raw_parts(set.fd_array.as_ptr(), fd_count as usize) };
        let mut max = 0;
        for fd in fd_array {
            if *fd >= max {
                max = *fd + 1;
            }
        }
        max as RawFd
    }
}

/// Compute the number of `FdSetElement`s needed to hold a set which can
/// contain up to `set_count` file descriptors with values less than `nfds`.
#[inline]
pub fn fd_set_num_elements(set_count: usize, nfds: RawFd) -> usize {
    #[cfg(any(windows, target_os = "wasi"))]
    {
        let _ = nfds;

        fd_set_num_elements_for_fd_array(set_count)
    }

    #[cfg(not(any(windows, target_os = "wasi")))]
    {
        let _ = set_count;

        fd_set_num_elements_for_bitvector(nfds)
    }
}

/// `fd_set_num_elements` implementation on platforms with fd array
/// implementations.
#[cfg(any(windows, target_os = "wasi"))]
#[inline]
pub(crate) fn fd_set_num_elements_for_fd_array(set_count: usize) -> usize {
    // Allocate space for an `fd_count` field, plus `set_count` elements
    // for the `fd_array` field.
    div_ceil(
        align_of::<FD_SET>() + set_count * size_of::<RawFd>(),
        size_of::<FdSetElement>(),
    )
}

/// `fd_set_num_elements` implementation on platforms with bitvector
/// implementations.
#[cfg(not(any(windows, target_os = "wasi")))]
#[inline]
pub(crate) fn fd_set_num_elements_for_bitvector(nfds: RawFd) -> usize {
    // Allocate space for a dense bitvector for `nfds` bits.
    let nfds = nfds as usize;
    div_ceil(nfds, BITS)
}

fn div_ceil(lhs: usize, rhs: usize) -> usize {
    let d = lhs / rhs;
    let r = lhs % rhs;
    if r > 0 {
        d + 1
    } else {
        d
    }
}

/// An iterator over the fds in a set.
#[doc(alias = "FD_ISSET")]
#[cfg(not(any(windows, target_os = "wasi")))]
pub struct FdSetIter<'a> {
    current: RawFd,
    fds: &'a [FdSetElement],
}

/// An iterator over the fds in a set.
#[doc(alias = "FD_ISSET")]
#[cfg(any(windows, target_os = "wasi"))]
pub struct FdSetIter<'a> {
    current: usize,
    fds: &'a [FdSetElement],
}

impl<'a> FdSetIter<'a> {
    /// Construct a `FdSetIter` for the given set.
    pub fn new(fds: &'a [FdSetElement]) -> Self {
        Self { current: 0, fds }
    }
}

#[cfg(not(any(windows, target_os = "wasi")))]
impl<'a> Iterator for FdSetIter<'a> {
    type Item = RawFd;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(element) = self.fds.get(self.current as usize / BITS) {
            // Test whether the current element has more bits set.
            let shifted = element.0 >> ((self.current as usize % BITS) as u32);
            if shifted != 0 {
                let fd = self.current + shifted.trailing_zeros() as RawFd;
                self.current = fd + 1;
                return Some(fd);
            }

            // Search through the array for the next element with bits set.
            if let Some(index) = self.fds[(self.current as usize / BITS) + 1..]
                .iter()
                .position(|element| element.0 != 0)
            {
                let index = index + (self.current as usize / BITS) + 1;
                let element = self.fds[index].0;
                let fd = (index * BITS) as RawFd + element.trailing_zeros() as RawFd;
                self.current = fd + 1;
                return Some(fd);
            }
        }
        None
    }
}

#[cfg(any(windows, target_os = "wasi"))]
impl<'a> Iterator for FdSetIter<'a> {
    type Item = RawFd;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;

        let set = unsafe { &*self.fds.as_ptr().cast::<FD_SET>() };
        let fd_count = set.fd_count;
        let fd_array = unsafe { slice::from_raw_parts(set.fd_array.as_ptr(), fd_count as usize) };

        if current == fd_count as usize {
            return None;
        }
        let fd = fd_array[current as usize];
        self.current = current + 1;
        Some(fd as RawFd)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::mem::{align_of, size_of};

    #[test]
    #[cfg(any(windows, target_os = "wasi"))]
    fn layouts() {
        // The `FdSetElement` array should be suitably aligned.
        assert_eq!(align_of::<FdSetElement>(), align_of::<FD_SET>());

        // The layout of `FD_SET` should match our layout of a set of the same
        // size.
        assert_eq!(
            fd_set_num_elements_for_fd_array(
                memoffset::span_of!(FD_SET, fd_array).len() / size_of::<RawFd>()
            ) * size_of::<FdSetElement>(),
            size_of::<FD_SET>()
        );
    }

    #[test]
    #[cfg(any(bsd, linux_kernel))]
    fn layouts() {
        use crate::backend::c;

        // The `FdSetElement` array should be suitably aligned.
        assert_eq!(align_of::<FdSetElement>(), align_of::<c::fd_set>());

        // The layout of `fd_set` should match our layout of a set of the same
        // size.
        assert_eq!(
            fd_set_num_elements_for_bitvector(c::FD_SETSIZE as RawFd) * size_of::<FdSetElement>(),
            size_of::<c::fd_set>()
        );
    }
}
