use crate::backend::c;
use crate::ffi;
use bitflags::bitflags;
use core::marker::PhantomData;

bitflags! {
    /// `O_*` constants for use with [`pipe_with`].
    ///
    /// [`pipe_with`]: crate::pipe::pipe_with
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct PipeFlags: ffi::c_uint {
        /// `O_CLOEXEC`
        const CLOEXEC = linux_raw_sys::general::O_CLOEXEC;
        /// `O_DIRECT`
        const DIRECT = linux_raw_sys::general::O_DIRECT;
        /// `O_NONBLOCK`
        const NONBLOCK = linux_raw_sys::general::O_NONBLOCK;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// `SPLICE_F_*` constants for use with [`splice`], [`vmsplice`], and
    /// [`tee`].
    ///
    /// [`splice`]: crate::pipe::splice
    /// [`vmsplice`]: crate::pipe::splice
    /// [`tee`]: crate::pipe::tee
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct SpliceFlags: ffi::c_uint {
        /// `SPLICE_F_MOVE`
        const MOVE = linux_raw_sys::general::SPLICE_F_MOVE;
        /// `SPLICE_F_NONBLOCK`
        const NONBLOCK = linux_raw_sys::general::SPLICE_F_NONBLOCK;
        /// `SPLICE_F_MORE`
        const MORE = linux_raw_sys::general::SPLICE_F_MORE;
        /// `SPLICE_F_GIFT`
        const GIFT = linux_raw_sys::general::SPLICE_F_GIFT;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// A buffer type for use with [`vmsplice`].
///
/// It is guaranteed to be ABI compatible with the iovec type on Unix platforms
/// and `WSABUF` on Windows. Unlike `IoSlice` and `IoSliceMut` it is
/// semantically like a raw pointer, and therefore can be shared or mutated as
/// needed.
///
/// [`vmsplice`]: crate::pipe::vmsplice
#[repr(transparent)]
pub struct IoSliceRaw<'a> {
    _buf: c::iovec,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> IoSliceRaw<'a> {
    /// Creates a new `IoSlice` wrapping a byte slice.
    pub fn from_slice(buf: &'a [u8]) -> Self {
        IoSliceRaw {
            _buf: c::iovec {
                iov_base: (buf.as_ptr() as *mut u8).cast::<ffi::c_void>(),
                iov_len: buf.len() as _,
            },
            _lifetime: PhantomData,
        }
    }

    /// Creates a new `IoSlice` wrapping a mutable byte slice.
    pub fn from_slice_mut(buf: &'a mut [u8]) -> Self {
        IoSliceRaw {
            _buf: c::iovec {
                iov_base: buf.as_mut_ptr().cast::<ffi::c_void>(),
                iov_len: buf.len() as _,
            },
            _lifetime: PhantomData,
        }
    }
}
