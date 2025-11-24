use super::bitmask::BitMask;
use super::EMPTY;
use core::{mem, ptr};

// Use the native word size as the group size. Using a 64-bit group size on
// a 32-bit architecture will just end up being more expensive because
// shifts and multiplies will need to be emulated.
#[cfg(any(
    target_pointer_width = "64",
    target_arch = "aarch64",
    target_arch = "x86_64",
    target_arch = "wasm32",
))]
type GroupWord = u64;
#[cfg(all(
    target_pointer_width = "32",
    not(target_arch = "aarch64"),
    not(target_arch = "x86_64"),
    not(target_arch = "wasm32"),
))]
type GroupWord = u32;

pub type BitMaskWord = GroupWord;
pub const BITMASK_STRIDE: usize = 8;
// We only care about the highest bit of each byte for the mask.
#[allow(clippy::cast_possible_truncation, clippy::unnecessary_cast)]
pub const BITMASK_MASK: BitMaskWord = 0x8080_8080_8080_8080_u64 as GroupWord;

/// Helper function to replicate a byte across a `GroupWord`.
#[inline]
fn repeat(byte: u8) -> GroupWord {
    GroupWord::from_ne_bytes([byte; Group::WIDTH])
}

/// Abstraction over a group of control bytes which can be scanned in
/// parallel.
///
/// This implementation uses a word-sized integer.
#[derive(Copy, Clone)]
pub struct Group(GroupWord);

// We perform all operations in the native endianness, and convert to
// little-endian just before creating a BitMask. The can potentially
// enable the compiler to eliminate unnecessary byte swaps if we are
// only checking whether a BitMask is empty.
#[allow(clippy::use_self)]
impl Group {
    /// Number of bytes in the group.
    pub const WIDTH: usize = mem::size_of::<Self>();

    /// Returns a full group of empty bytes, suitable for use as the initial
    /// value for an empty hash table.
    ///
    /// This is guaranteed to be aligned to the group size.
    #[inline]
    pub const fn static_empty() -> &'static [u8; Group::WIDTH] {
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
    pub unsafe fn load(ptr: *const u8) -> Self {
        Group(ptr::read_unaligned(ptr.cast()))
    }

    /// Loads a group of bytes starting at the given address, which must be
    /// aligned to `mem::align_of::<Group>()`.
    #[inline]
    #[allow(clippy::cast_ptr_alignment)]
    pub unsafe fn load_aligned(ptr: *const u8) -> Self {
        // FIXME: use align_offset once it stabilizes
        debug_assert_eq!(ptr as usize & (mem::align_of::<Self>() - 1), 0);
        Group(ptr::read(ptr.cast()))
    }

    /// Stores the group of bytes to the given address, which must be
    /// aligned to `mem::align_of::<Group>()`.
    #[inline]
    #[allow(clippy::cast_ptr_alignment)]
    pub unsafe fn store_aligned(self, ptr: *mut u8) {
        // FIXME: use align_offset once it stabilizes
        debug_assert_eq!(ptr as usize & (mem::align_of::<Self>() - 1), 0);
        ptr::write(ptr.cast(), self.0);
    }

    /// Returns a `BitMask` indicating all bytes in the group which *may*
    /// have the given value.
    ///
    /// This function may return a false positive in certain cases where
    /// the byte in the group differs from the searched value only in its
    /// lowest bit. This is fine because:
    /// - This never happens for `EMPTY` and `DELETED`, only full entries.
    /// - The check for key equality will catch these.
    /// - This only happens if there is at least 1 true match.
    /// - The chance of this happening is very low (< 1% chance per byte).
    #[inline]
    pub fn match_byte(self, byte: u8) -> BitMask {
        // This algorithm is derived from
        // https://graphics.stanford.edu/~seander/bithacks.html##ValueInWord
        let cmp = self.0 ^ repeat(byte);
        BitMask((cmp.wrapping_sub(repeat(0x01)) & !cmp & repeat(0x80)).to_le())
    }

    /// Returns a `BitMask` indicating all bytes in the group which are
    /// `EMPTY`.
    #[inline]
    pub fn match_empty(self) -> BitMask {
        // If the high bit is set, then the byte must be either:
        // 1111_1111 (EMPTY) or 1000_0000 (DELETED).
        // So we can just check if the top two bits are 1 by ANDing them.
        BitMask((self.0 & (self.0 << 1) & repeat(0x80)).to_le())
    }

    /// Returns a `BitMask` indicating all bytes in the group which are
    /// `EMPTY` or `DELETED`.
    #[inline]
    pub fn match_empty_or_deleted(self) -> BitMask {
        // A byte is EMPTY or DELETED iff the high bit is set
        BitMask((self.0 & repeat(0x80)).to_le())
    }

    /// Returns a `BitMask` indicating all bytes in the group which are full.
    #[inline]
    pub fn match_full(self) -> BitMask {
        self.match_empty_or_deleted().invert()
    }

    /// Performs the following transformation on all bytes in the group:
    /// - `EMPTY => EMPTY`
    /// - `DELETED => EMPTY`
    /// - `FULL => DELETED`
    #[inline]
    pub fn convert_special_to_empty_and_full_to_deleted(self) -> Self {
        // Map high_bit = 1 (EMPTY or DELETED) to 1111_1111
        // and high_bit = 0 (FULL) to 1000_0000
        //
        // Here's this logic expanded to concrete values:
        //   let full = 1000_0000 (true) or 0000_0000 (false)
        //   !1000_0000 + 1 = 0111_1111 + 1 = 1000_0000 (no carry)
        //   !0000_0000 + 0 = 1111_1111 + 0 = 1111_1111 (no carry)
        let full = !self.0 & repeat(0x80);
        Group(!full + (full >> 7))
    }
}
