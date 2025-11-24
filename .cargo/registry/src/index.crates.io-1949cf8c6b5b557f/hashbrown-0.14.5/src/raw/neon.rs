use super::bitmask::BitMask;
use super::EMPTY;
use core::arch::aarch64 as neon;
use core::mem;
use core::num::NonZeroU64;

pub(crate) type BitMaskWord = u64;
pub(crate) type NonZeroBitMaskWord = NonZeroU64;
pub(crate) const BITMASK_STRIDE: usize = 8;
pub(crate) const BITMASK_MASK: BitMaskWord = !0;
pub(crate) const BITMASK_ITER_MASK: BitMaskWord = 0x8080_8080_8080_8080;

/// Abstraction over a group of control bytes which can be scanned in
/// parallel.
///
/// This implementation uses a 64-bit NEON value.
#[derive(Copy, Clone)]
pub(crate) struct Group(neon::uint8x8_t);

#[allow(clippy::use_self)]
impl Group {
    /// Number of bytes in the group.
    pub(crate) const WIDTH: usize = mem::size_of::<Self>();

    /// Returns a full group of empty bytes, suitable for use as the initial
    /// value for an empty hash table.
    ///
    /// This is guaranteed to be aligned to the group size.
    #[inline]
    pub(crate) const fn static_empty() -> &'static [u8; Group::WIDTH] {
        #[repr(C)]
        struct AlignedBytes {
            _align: [Group; 0],
            bytes: [u8; Group::WIDTH],
        }
        const ALIGNED_BYTES: AlignedBytes = AlignedBytes {
            _align: [],
            bytes: [EMPTY; Group::WIDTH],
        };
        &ALIGNED_BYTES.bytes
    }

    /// Loads a group of bytes starting at the given address.
    #[inline]
    #[allow(clippy::cast_ptr_alignment)] // unaligned load
    pub(crate) unsafe fn load(ptr: *const u8) -> Self {
        Group(neon::vld1_u8(ptr))
    }

    /// Loads a group of bytes starting at the given address, which must be
    /// aligned to `mem::align_of::<Group>()`.
    #[inline]
    #[allow(clippy::cast_ptr_alignment)]
    pub(crate) unsafe fn load_aligned(ptr: *const u8) -> Self {
        // FIXME: use align_offset once it stabilizes
        debug_assert_eq!(ptr as usize & (mem::align_of::<Self>() - 1), 0);
        Group(neon::vld1_u8(ptr))
    }

    /// Stores the group of bytes to the given address, which must be
    /// aligned to `mem::align_of::<Group>()`.
    #[inline]
    #[allow(clippy::cast_ptr_alignment)]
    pub(crate) unsafe fn store_aligned(self, ptr: *mut u8) {
        // FIXME: use align_offset once it stabilizes
        debug_assert_eq!(ptr as usize & (mem::align_of::<Self>() - 1), 0);
        neon::vst1_u8(ptr, self.0);
    }

    /// Returns a `BitMask` indicating all bytes in the group which *may*
    /// have the given value.
    #[inline]
    pub(crate) fn match_byte(self, byte: u8) -> BitMask {
        unsafe {
            let cmp = neon::vceq_u8(self.0, neon::vdup_n_u8(byte));
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
        }
    }

    /// Returns a `BitMask` indicating all bytes in the group which are
    /// `EMPTY`.
    #[inline]
    pub(crate) fn match_empty(self) -> BitMask {
        self.match_byte(EMPTY)
    }

    /// Returns a `BitMask` indicating all bytes in the group which are
    /// `EMPTY` or `DELETED`.
    #[inline]
    pub(crate) fn match_empty_or_deleted(self) -> BitMask {
        unsafe {
            let cmp = neon::vcltz_s8(neon::vreinterpret_s8_u8(self.0));
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
        }
    }

    /// Returns a `BitMask` indicating all bytes in the group which are full.
    #[inline]
    pub(crate) fn match_full(self) -> BitMask {
        unsafe {
            let cmp = neon::vcgez_s8(neon::vreinterpret_s8_u8(self.0));
            BitMask(neon::vget_lane_u64(neon::vreinterpret_u64_u8(cmp), 0))
        }
    }

    /// Performs the following transformation on all bytes in the group:
    /// - `EMPTY => EMPTY`
    /// - `DELETED => EMPTY`
    /// - `FULL => DELETED`
    #[inline]
    pub(crate) fn convert_special_to_empty_and_full_to_deleted(self) -> Self {
        // Map high_bit = 1 (EMPTY or DELETED) to 1111_1111
        // and high_bit = 0 (FULL) to 1000_0000
        //
        // Here's this logic expanded to concrete values:
        //   let special = 0 > byte = 1111_1111 (true) or 0000_0000 (false)
        //   1111_1111 | 1000_0000 = 1111_1111
        //   0000_0000 | 1000_0000 = 1000_0000
        unsafe {
            let special = neon::vcltz_s8(neon::vreinterpret_s8_u8(self.0));
            Group(neon::vorr_u8(special, neon::vdup_n_u8(0x80)))
        }
    }
}
