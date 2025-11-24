// NOTE: The descriptions for each of the vector methods on the traits below
// are pretty inscrutable. For this reason, there are tests for every method
// on for every trait impl below. If you're confused about what an op does,
// consult its test. (They probably should be doc tests, but I couldn't figure
// out how to write them in a non-annoying way.)

use core::{
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
};

/// A trait for describing vector operations used by vectorized searchers.
///
/// The trait is highly constrained to low level vector operations needed for
/// the specific algorithms used in this crate. In general, it was invented
/// mostly to be generic over x86's __m128i and __m256i types. At time of
/// writing, it also supports wasm and aarch64 128-bit vector types as well.
///
/// # Safety
///
/// All methods are not safe since they are intended to be implemented using
/// vendor intrinsics, which are also not safe. Callers must ensure that
/// the appropriate target features are enabled in the calling function,
/// and that the current CPU supports them. All implementations should
/// avoid marking the routines with `#[target_feature]` and instead mark
/// them as `#[inline(always)]` to ensure they get appropriately inlined.
/// (`inline(always)` cannot be used with target_feature.)
pub(crate) trait Vector:
    Copy + Debug + Send + Sync + UnwindSafe + RefUnwindSafe
{
    /// The number of bits in the vector.
    const BITS: usize;
    /// The number of bytes in the vector. That is, this is the size of the
    /// vector in memory.
    const BYTES: usize;

    /// Create a vector with 8-bit lanes with the given byte repeated into each
    /// lane.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn splat(byte: u8) -> Self;

    /// Read a vector-size number of bytes from the given pointer. The pointer
    /// does not need to be aligned.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    ///
    /// Callers must guarantee that at least `BYTES` bytes are readable from
    /// `data`.
    unsafe fn load_unaligned(data: *const u8) -> Self;

    /// Returns true if and only if this vector has zero in all of its lanes.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn is_zero(self) -> bool;

    /// Do an 8-bit pairwise equality check. If lane `i` is equal in this
    /// vector and the one given, then lane `i` in the resulting vector is set
    /// to `0xFF`. Otherwise, it is set to `0x00`.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn cmpeq(self, vector2: Self) -> Self;

    /// Perform a bitwise 'and' of this vector and the one given and return
    /// the result.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn and(self, vector2: Self) -> Self;

    /// Perform a bitwise 'or' of this vector and the one given and return
    /// the result.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[allow(dead_code)] // unused, but useful enough to keep around?
    unsafe fn or(self, vector2: Self) -> Self;

    /// Shift each 8-bit lane in this vector to the right by the number of
    /// bits indictated by the `BITS` type parameter.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn shift_8bit_lane_right<const BITS: i32>(self) -> Self;

    /// Shift this vector to the left by one byte and shift the most
    /// significant byte of `vector2` into the least significant position of
    /// this vector.
    ///
    /// Stated differently, this behaves as if `self` and `vector2` were
    /// concatenated into a `2 * Self::BITS` temporary buffer and then shifted
    /// right by `Self::BYTES - 1` bytes.
    ///
    /// With respect to the Teddy algorithm, `vector2` is usually a previous
    /// `Self::BYTES` chunk from the haystack and `self` is the chunk
    /// immediately following it. This permits combining the last two bytes
    /// from the previous chunk (`vector2`) with the first `Self::BYTES - 1`
    /// bytes from the current chunk. This permits aligning the result of
    /// various shuffles so that they can be and-ed together and a possible
    /// candidate discovered.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn shift_in_one_byte(self, vector2: Self) -> Self;

    /// Shift this vector to the left by two bytes and shift the two most
    /// significant bytes of `vector2` into the least significant position of
    /// this vector.
    ///
    /// Stated differently, this behaves as if `self` and `vector2` were
    /// concatenated into a `2 * Self::BITS` temporary buffer and then shifted
    /// right by `Self::BYTES - 2` bytes.
    ///
    /// With respect to the Teddy algorithm, `vector2` is usually a previous
    /// `Self::BYTES` chunk from the haystack and `self` is the chunk
    /// immediately following it. This permits combining the last two bytes
    /// from the previous chunk (`vector2`) with the first `Self::BYTES - 2`
    /// bytes from the current chunk. This permits aligning the result of
    /// various shuffles so that they can be and-ed together and a possible
    /// candidate discovered.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn shift_in_two_bytes(self, vector2: Self) -> Self;

    /// Shift this vector to the left by three bytes and shift the three most
    /// significant bytes of `vector2` into the least significant position of
    /// this vector.
    ///
    /// Stated differently, this behaves as if `self` and `vector2` were
    /// concatenated into a `2 * Self::BITS` temporary buffer and then shifted
    /// right by `Self::BYTES - 3` bytes.
    ///
    /// With respect to the Teddy algorithm, `vector2` is usually a previous
    /// `Self::BYTES` chunk from the haystack and `self` is the chunk
    /// immediately following it. This permits combining the last three bytes
    /// from the previous chunk (`vector2`) with the first `Self::BYTES - 3`
    /// bytes from the current chunk. This permits aligning the result of
    /// various shuffles so that they can be and-ed together and a possible
    /// candidate discovered.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn shift_in_three_bytes(self, vector2: Self) -> Self;

    /// Shuffles the bytes in this vector according to the indices in each of
    /// the corresponding lanes in `indices`.
    ///
    /// If `i` is the index of corresponding lanes, `A` is this vector, `B` is
    /// indices and `C` is the resulting vector, then `C = A[B[i]]`.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn shuffle_bytes(self, indices: Self) -> Self;

    /// Call the provided function for each 64-bit lane in this vector. The
    /// given function is provided the lane index and lane value as a `u64`.
    ///
    /// If `f` returns `Some`, then iteration over the lanes is stopped and the
    /// value is returned. Otherwise, this returns `None`.
    ///
    /// # Notes
    ///
    /// Conceptually it would be nice if we could have a
    /// `unpack64(self) -> [u64; BITS / 64]` method, but defining that is
    /// tricky given Rust's [current support for const generics][support].
    /// And even if we could, it would be tricky to write generic code over
    /// it. (Not impossible. We could introduce another layer that requires
    /// `AsRef<[u64]>` or something.)
    ///
    /// [support]: https://github.com/rust-lang/rust/issues/60551
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn for_each_64bit_lane<T>(
        self,
        f: impl FnMut(usize, u64) -> Option<T>,
    ) -> Option<T>;
}

/// This trait extends the `Vector` trait with additional operations to support
/// Fat Teddy.
///
/// Fat Teddy uses 16 buckets instead of 8, but reads half as many bytes (as
/// the vector size) instead of the full size of a vector per iteration. For
/// example, when using a 256-bit vector, Slim Teddy reads 32 bytes at a timr
/// but Fat Teddy reads 16 bytes at a time.
///
/// Fat Teddy is useful when searching for a large number of literals.
/// The extra number of buckets spreads the literals out more and reduces
/// verification time.
///
/// Currently we only implement this for AVX on x86_64. It would be nice to
/// implement this for SSE on x86_64 and NEON on aarch64, with the latter two
/// only reading 8 bytes at a time. It's not clear how well it would work, but
/// there are some tricky things to figure out in terms of implementation. The
/// `half_shift_in_{one,two,three}_bytes` methods in particular are probably
/// the trickiest of the bunch. For AVX2, these are implemented by taking
/// advantage of the fact that `_mm256_alignr_epi8` operates on each 128-bit
/// half instead of the full 256-bit vector. (Where as `_mm_alignr_epi8`
/// operates on the full 128-bit vector and not on each 64-bit half.) I didn't
/// do a careful survey of NEON to see if it could easily support these
/// operations.
pub(crate) trait FatVector: Vector {
    type Half: Vector;

    /// Read a half-vector-size number of bytes from the given pointer, and
    /// broadcast it across both halfs of a full vector. The pointer does not
    /// need to be aligned.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    ///
    /// Callers must guarantee that at least `Self::HALF::BYTES` bytes are
    /// readable from `data`.
    unsafe fn load_half_unaligned(data: *const u8) -> Self;

    /// Like `Vector::shift_in_one_byte`, except this is done for each half
    /// of the vector instead.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn half_shift_in_one_byte(self, vector2: Self) -> Self;

    /// Like `Vector::shift_in_two_bytes`, except this is done for each half
    /// of the vector instead.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn half_shift_in_two_bytes(self, vector2: Self) -> Self;

    /// Like `Vector::shift_in_two_bytes`, except this is done for each half
    /// of the vector instead.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn half_shift_in_three_bytes(self, vector2: Self) -> Self;

    /// Swap the 128-bit lanes in this vector.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn swap_halves(self) -> Self;

    /// Unpack and interleave the 8-bit lanes from the low 128 bits of each
    /// vector and return the result.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn interleave_low_8bit_lanes(self, vector2: Self) -> Self;

    /// Unpack and interleave the 8-bit lanes from the high 128 bits of each
    /// vector and return the result.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn interleave_high_8bit_lanes(self, vector2: Self) -> Self;

    /// Call the provided function for each 64-bit lane in the lower half
    /// of this vector and then in the other vector. The given function is
    /// provided the lane index and lane value as a `u64`. (The high 128-bits
    /// of each vector are ignored.)
    ///
    /// If `f` returns `Some`, then iteration over the lanes is stopped and the
    /// value is returned. Otherwise, this returns `None`.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    unsafe fn for_each_low_64bit_lane<T>(
        self,
        vector2: Self,
        f: impl FnMut(usize, u64) -> Option<T>,
    ) -> Option<T>;
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod x86_64_ssse3 {
    use core::arch::x86_64::*;

    use crate::util::int::{I32, I8};

    use super::Vector;

    impl Vector for __m128i {
        const BITS: usize = 128;
        const BYTES: usize = 16;

        #[inline(always)]
        unsafe fn splat(byte: u8) -> __m128i {
            _mm_set1_epi8(i8::from_bits(byte))
        }

        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> __m128i {
            _mm_loadu_si128(data.cast::<__m128i>())
        }

        #[inline(always)]
        unsafe fn is_zero(self) -> bool {
            let cmp = self.cmpeq(Self::splat(0));
            _mm_movemask_epi8(cmp).to_bits() == 0xFFFF
        }

        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> __m128i {
            _mm_cmpeq_epi8(self, vector2)
        }

        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> __m128i {
            _mm_and_si128(self, vector2)
        }

        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> __m128i {
            _mm_or_si128(self, vector2)
        }

        #[inline(always)]
        unsafe fn shift_8bit_lane_right<const BITS: i32>(self) -> Self {
            // Apparently there is no _mm_srli_epi8, so we emulate it by
            // shifting 16-bit integers and masking out the high nybble of each
            // 8-bit lane (since that nybble will contain bits from the low
            // nybble of the previous lane).
            let lomask = Self::splat(0xF);
            _mm_srli_epi16(self, BITS).and(lomask)
        }

        #[inline(always)]
        unsafe fn shift_in_one_byte(self, vector2: Self) -> Self {
            _mm_alignr_epi8(self, vector2, 15)
        }

        #[inline(always)]
        unsafe fn shift_in_two_bytes(self, vector2: Self) -> Self {
            _mm_alignr_epi8(self, vector2, 14)
        }

        #[inline(always)]
        unsafe fn shift_in_three_bytes(self, vector2: Self) -> Self {
            _mm_alignr_epi8(self, vector2, 13)
        }

        #[inline(always)]
        unsafe fn shuffle_bytes(self, indices: Self) -> Self {
            _mm_shuffle_epi8(self, indices)
        }

        #[inline(always)]
        unsafe fn for_each_64bit_lane<T>(
            self,
            mut f: impl FnMut(usize, u64) -> Option<T>,
        ) -> Option<T> {
            // We could just use _mm_extract_epi64 here, but that requires
            // SSE 4.1. It isn't necessarily a problem to just require SSE 4.1,
            // but everything else works with SSSE3 so we stick to that subset.
            let lanes: [u64; 2] = core::mem::transmute(self);
            if let Some(t) = f(0, lanes[0]) {
                return Some(t);
            }
            if let Some(t) = f(1, lanes[1]) {
                return Some(t);
            }
            None
        }
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod x86_64_avx2 {
    use core::arch::x86_64::*;

    use crate::util::int::{I32, I64, I8};

    use super::{FatVector, Vector};

    impl Vector for __m256i {
        const BITS: usize = 256;
        const BYTES: usize = 32;

        #[inline(always)]
        unsafe fn splat(byte: u8) -> __m256i {
            _mm256_set1_epi8(i8::from_bits(byte))
        }

        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> __m256i {
            _mm256_loadu_si256(data.cast::<__m256i>())
        }

        #[inline(always)]
        unsafe fn is_zero(self) -> bool {
            let cmp = self.cmpeq(Self::splat(0));
            _mm256_movemask_epi8(cmp).to_bits() == 0xFFFFFFFF
        }

        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> __m256i {
            _mm256_cmpeq_epi8(self, vector2)
        }

        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> __m256i {
            _mm256_and_si256(self, vector2)
        }

        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> __m256i {
            _mm256_or_si256(self, vector2)
        }

        #[inline(always)]
        unsafe fn shift_8bit_lane_right<const BITS: i32>(self) -> Self {
            let lomask = Self::splat(0xF);
            _mm256_srli_epi16(self, BITS).and(lomask)
        }

        #[inline(always)]
        unsafe fn shift_in_one_byte(self, vector2: Self) -> Self {
            // Credit goes to jneem for figuring this out:
            // https://github.com/jneem/teddy/blob/9ab5e899ad6ef6911aecd3cf1033f1abe6e1f66c/src/x86/teddy_simd.rs#L145-L184
            //
            // TL;DR avx2's PALIGNR instruction is actually just two 128-bit
            // PALIGNR instructions, which is not what we want, so we need to
            // do some extra shuffling.
            let v = _mm256_permute2x128_si256(vector2, self, 0x21);
            _mm256_alignr_epi8(self, v, 15)
        }

        #[inline(always)]
        unsafe fn shift_in_two_bytes(self, vector2: Self) -> Self {
            // Credit goes to jneem for figuring this out:
            // https://github.com/jneem/teddy/blob/9ab5e899ad6ef6911aecd3cf1033f1abe6e1f66c/src/x86/teddy_simd.rs#L145-L184
            //
            // TL;DR avx2's PALIGNR instruction is actually just two 128-bit
            // PALIGNR instructions, which is not what we want, so we need to
            // do some extra shuffling.
            let v = _mm256_permute2x128_si256(vector2, self, 0x21);
            _mm256_alignr_epi8(self, v, 14)
        }

        #[inline(always)]
        unsafe fn shift_in_three_bytes(self, vector2: Self) -> Self {
            // Credit goes to jneem for figuring this out:
            // https://github.com/jneem/teddy/blob/9ab5e899ad6ef6911aecd3cf1033f1abe6e1f66c/src/x86/teddy_simd.rs#L145-L184
            //
            // TL;DR avx2's PALIGNR instruction is actually just two 128-bit
            // PALIGNR instructions, which is not what we want, so we need to
            // do some extra shuffling.
            let v = _mm256_permute2x128_si256(vector2, self, 0x21);
            _mm256_alignr_epi8(self, v, 13)
        }

        #[inline(always)]
        unsafe fn shuffle_bytes(self, indices: Self) -> Self {
            _mm256_shuffle_epi8(self, indices)
        }

        #[inline(always)]
        unsafe fn for_each_64bit_lane<T>(
            self,
            mut f: impl FnMut(usize, u64) -> Option<T>,
        ) -> Option<T> {
            // NOTE: At one point in the past, I used transmute to this to
            // get a [u64; 4], but it turned out to lead to worse codegen IIRC.
            // I've tried it more recently, and it looks like that's no longer
            // the case. But since there's no difference, we stick with the
            // slightly more complicated but transmute-free version.
            let lane = _mm256_extract_epi64(self, 0).to_bits();
            if let Some(t) = f(0, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(self, 1).to_bits();
            if let Some(t) = f(1, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(self, 2).to_bits();
            if let Some(t) = f(2, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(self, 3).to_bits();
            if let Some(t) = f(3, lane) {
                return Some(t);
            }
            None
        }
    }

    impl FatVector for __m256i {
        type Half = __m128i;

        #[inline(always)]
        unsafe fn load_half_unaligned(data: *const u8) -> Self {
            let half = Self::Half::load_unaligned(data);
            _mm256_broadcastsi128_si256(half)
        }

        #[inline(always)]
        unsafe fn half_shift_in_one_byte(self, vector2: Self) -> Self {
            _mm256_alignr_epi8(self, vector2, 15)
        }

        #[inline(always)]
        unsafe fn half_shift_in_two_bytes(self, vector2: Self) -> Self {
            _mm256_alignr_epi8(self, vector2, 14)
        }

        #[inline(always)]
        unsafe fn half_shift_in_three_bytes(self, vector2: Self) -> Self {
            _mm256_alignr_epi8(self, vector2, 13)
        }

        #[inline(always)]
        unsafe fn swap_halves(self) -> Self {
            _mm256_permute4x64_epi64(self, 0x4E)
        }

        #[inline(always)]
        unsafe fn interleave_low_8bit_lanes(self, vector2: Self) -> Self {
            _mm256_unpacklo_epi8(self, vector2)
        }

        #[inline(always)]
        unsafe fn interleave_high_8bit_lanes(self, vector2: Self) -> Self {
            _mm256_unpackhi_epi8(self, vector2)
        }

        #[inline(always)]
        unsafe fn for_each_low_64bit_lane<T>(
            self,
            vector2: Self,
            mut f: impl FnMut(usize, u64) -> Option<T>,
        ) -> Option<T> {
            let lane = _mm256_extract_epi64(self, 0).to_bits();
            if let Some(t) = f(0, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(self, 1).to_bits();
            if let Some(t) = f(1, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(vector2, 0).to_bits();
            if let Some(t) = f(2, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(vector2, 1).to_bits();
            if let Some(t) = f(3, lane) {
                return Some(t);
            }
            None
        }
    }
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
mod aarch64_neon {
    use core::arch::aarch64::*;

    use super::Vector;

    impl Vector for uint8x16_t {
        const BITS: usize = 128;
        const BYTES: usize = 16;

        #[inline(always)]
        unsafe fn splat(byte: u8) -> uint8x16_t {
            vdupq_n_u8(byte)
        }

        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> uint8x16_t {
            vld1q_u8(data)
        }

        #[inline(always)]
        unsafe fn is_zero(self) -> bool {
            // Could also use vmaxvq_u8.
            // ... I tried that and couldn't observe any meaningful difference
            // in benchmarks.
            let maxes = vreinterpretq_u64_u8(vpmaxq_u8(self, self));
            vgetq_lane_u64(maxes, 0) == 0
        }

        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> uint8x16_t {
            vceqq_u8(self, vector2)
        }

        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> uint8x16_t {
            vandq_u8(self, vector2)
        }

        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> uint8x16_t {
            vorrq_u8(self, vector2)
        }

        #[inline(always)]
        unsafe fn shift_8bit_lane_right<const BITS: i32>(self) -> Self {
            debug_assert!(BITS <= 7);
            vshrq_n_u8(self, BITS)
        }

        #[inline(always)]
        unsafe fn shift_in_one_byte(self, vector2: Self) -> Self {
            vextq_u8(vector2, self, 15)
        }

        #[inline(always)]
        unsafe fn shift_in_two_bytes(self, vector2: Self) -> Self {
            vextq_u8(vector2, self, 14)
        }

        #[inline(always)]
        unsafe fn shift_in_three_bytes(self, vector2: Self) -> Self {
            vextq_u8(vector2, self, 13)
        }

        #[inline(always)]
        unsafe fn shuffle_bytes(self, indices: Self) -> Self {
            vqtbl1q_u8(self, indices)
        }

        #[inline(always)]
        unsafe fn for_each_64bit_lane<T>(
            self,
            mut f: impl FnMut(usize, u64) -> Option<T>,
        ) -> Option<T> {
            let this = vreinterpretq_u64_u8(self);
            let lane = vgetq_lane_u64(this, 0);
            if let Some(t) = f(0, lane) {
                return Some(t);
            }
            let lane = vgetq_lane_u64(this, 1);
            if let Some(t) = f(1, lane) {
                return Some(t);
            }
            None
        }
    }
}

#[cfg(all(test, target_arch = "x86_64", target_feature = "sse2"))]
mod tests_x86_64_ssse3 {
    use core::arch::x86_64::*;

    use crate::util::int::{I32, U32};

    use super::*;

    fn is_runnable() -> bool {
        std::is_x86_feature_detected!("ssse3")
    }

    #[target_feature(enable = "ssse3")]
    unsafe fn load(lanes: [u8; 16]) -> __m128i {
        __m128i::load_unaligned(&lanes as *const u8)
    }

    #[target_feature(enable = "ssse3")]
    unsafe fn unload(v: __m128i) -> [u8; 16] {
        [
            _mm_extract_epi8(v, 0).to_bits().low_u8(),
            _mm_extract_epi8(v, 1).to_bits().low_u8(),
            _mm_extract_epi8(v, 2).to_bits().low_u8(),
            _mm_extract_epi8(v, 3).to_bits().low_u8(),
            _mm_extract_epi8(v, 4).to_bits().low_u8(),
            _mm_extract_epi8(v, 5).to_bits().low_u8(),
            _mm_extract_epi8(v, 6).to_bits().low_u8(),
            _mm_extract_epi8(v, 7).to_bits().low_u8(),
            _mm_extract_epi8(v, 8).to_bits().low_u8(),
            _mm_extract_epi8(v, 9).to_bits().low_u8(),
            _mm_extract_epi8(v, 10).to_bits().low_u8(),
            _mm_extract_epi8(v, 11).to_bits().low_u8(),
            _mm_extract_epi8(v, 12).to_bits().low_u8(),
            _mm_extract_epi8(v, 13).to_bits().low_u8(),
            _mm_extract_epi8(v, 14).to_bits().low_u8(),
            _mm_extract_epi8(v, 15).to_bits().low_u8(),
        ]
    }

    #[test]
    fn vector_splat() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v = __m128i::splat(0xAF);
            assert_eq!(
                unload(v),
                [
                    0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF,
                    0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF
                ]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_is_zero() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v = load([0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert!(!v.is_zero());
            let v = load([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert!(v.is_zero());
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_cmpeq() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 1]);
            let v2 =
                load([16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
            assert_eq!(
                unload(v1.cmpeq(v2)),
                [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_and() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v1 =
                load([0, 0, 0, 0, 0, 0b1001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            let v2 =
                load([0, 0, 0, 0, 0, 0b1010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                unload(v1.and(v2)),
                [0, 0, 0, 0, 0, 0b1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_or() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v1 =
                load([0, 0, 0, 0, 0, 0b1001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            let v2 =
                load([0, 0, 0, 0, 0, 0b1010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                unload(v1.or(v2)),
                [0, 0, 0, 0, 0, 0b1011, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_8bit_lane_right() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v = load([
                0, 0, 0, 0, 0b1011, 0b0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            assert_eq!(
                unload(v.shift_8bit_lane_right::<2>()),
                [0, 0, 0, 0, 0b0010, 0b0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_one_byte() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 = load([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.shift_in_one_byte(v2)),
                [32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_two_bytes() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 = load([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.shift_in_two_bytes(v2)),
                [31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_three_bytes() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 = load([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.shift_in_three_bytes(v2)),
                [30, 31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shuffle_bytes() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 =
                load([0, 0, 0, 0, 4, 4, 4, 4, 8, 8, 8, 8, 12, 12, 12, 12]);
            assert_eq!(
                unload(v1.shuffle_bytes(v2)),
                [1, 1, 1, 1, 5, 5, 5, 5, 9, 9, 9, 9, 13, 13, 13, 13],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_for_each_64bit_lane() {
        #[target_feature(enable = "ssse3")]
        unsafe fn test() {
            let v = load([
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
                0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            ]);
            let mut lanes = [0u64; 2];
            v.for_each_64bit_lane(|i, lane| {
                lanes[i] = lane;
                None::<()>
            });
            assert_eq!(lanes, [0x0807060504030201, 0x100F0E0D0C0B0A09],);
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }
}

#[cfg(all(test, target_arch = "x86_64", target_feature = "sse2"))]
mod tests_x86_64_avx2 {
    use core::arch::x86_64::*;

    use crate::util::int::{I32, U32};

    use super::*;

    fn is_runnable() -> bool {
        std::is_x86_feature_detected!("avx2")
    }

    #[target_feature(enable = "avx2")]
    unsafe fn load(lanes: [u8; 32]) -> __m256i {
        __m256i::load_unaligned(&lanes as *const u8)
    }

    #[target_feature(enable = "avx2")]
    unsafe fn load_half(lanes: [u8; 16]) -> __m256i {
        __m256i::load_half_unaligned(&lanes as *const u8)
    }

    #[target_feature(enable = "avx2")]
    unsafe fn unload(v: __m256i) -> [u8; 32] {
        [
            _mm256_extract_epi8(v, 0).to_bits().low_u8(),
            _mm256_extract_epi8(v, 1).to_bits().low_u8(),
            _mm256_extract_epi8(v, 2).to_bits().low_u8(),
            _mm256_extract_epi8(v, 3).to_bits().low_u8(),
            _mm256_extract_epi8(v, 4).to_bits().low_u8(),
            _mm256_extract_epi8(v, 5).to_bits().low_u8(),
            _mm256_extract_epi8(v, 6).to_bits().low_u8(),
            _mm256_extract_epi8(v, 7).to_bits().low_u8(),
            _mm256_extract_epi8(v, 8).to_bits().low_u8(),
            _mm256_extract_epi8(v, 9).to_bits().low_u8(),
            _mm256_extract_epi8(v, 10).to_bits().low_u8(),
            _mm256_extract_epi8(v, 11).to_bits().low_u8(),
            _mm256_extract_epi8(v, 12).to_bits().low_u8(),
            _mm256_extract_epi8(v, 13).to_bits().low_u8(),
            _mm256_extract_epi8(v, 14).to_bits().low_u8(),
            _mm256_extract_epi8(v, 15).to_bits().low_u8(),
            _mm256_extract_epi8(v, 16).to_bits().low_u8(),
            _mm256_extract_epi8(v, 17).to_bits().low_u8(),
            _mm256_extract_epi8(v, 18).to_bits().low_u8(),
            _mm256_extract_epi8(v, 19).to_bits().low_u8(),
            _mm256_extract_epi8(v, 20).to_bits().low_u8(),
            _mm256_extract_epi8(v, 21).to_bits().low_u8(),
            _mm256_extract_epi8(v, 22).to_bits().low_u8(),
            _mm256_extract_epi8(v, 23).to_bits().low_u8(),
            _mm256_extract_epi8(v, 24).to_bits().low_u8(),
            _mm256_extract_epi8(v, 25).to_bits().low_u8(),
            _mm256_extract_epi8(v, 26).to_bits().low_u8(),
            _mm256_extract_epi8(v, 27).to_bits().low_u8(),
            _mm256_extract_epi8(v, 28).to_bits().low_u8(),
            _mm256_extract_epi8(v, 29).to_bits().low_u8(),
            _mm256_extract_epi8(v, 30).to_bits().low_u8(),
            _mm256_extract_epi8(v, 31).to_bits().low_u8(),
        ]
    }

    #[test]
    fn vector_splat() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v = __m256i::splat(0xAF);
            assert_eq!(
                unload(v),
                [
                    0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF,
                    0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF,
                    0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF,
                    0xAF, 0xAF, 0xAF, 0xAF, 0xAF,
                ]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_is_zero() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v = load([
                0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            assert!(!v.is_zero());
            let v = load([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            assert!(v.is_zero());
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_cmpeq() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 1,
            ]);
            let v2 = load([
                32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18,
                17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1,
            ]);
            assert_eq!(
                unload(v1.cmpeq(v2)),
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF
                ]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_and() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                0, 0, 0, 0, 0, 0b1001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            let v2 = load([
                0, 0, 0, 0, 0, 0b1010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            assert_eq!(
                unload(v1.and(v2)),
                [
                    0, 0, 0, 0, 0, 0b1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_or() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                0, 0, 0, 0, 0, 0b1001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            let v2 = load([
                0, 0, 0, 0, 0, 0b1010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            assert_eq!(
                unload(v1.or(v2)),
                [
                    0, 0, 0, 0, 0, 0b1011, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_8bit_lane_right() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v = load([
                0, 0, 0, 0, 0b1011, 0b0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            assert_eq!(
                unload(v.shift_8bit_lane_right::<2>()),
                [
                    0, 0, 0, 0, 0b0010, 0b0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_one_byte() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            let v2 = load([
                33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
                48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
                63, 64,
            ]);
            assert_eq!(
                unload(v1.shift_in_one_byte(v2)),
                [
                    64, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
                    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
                    31,
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_two_bytes() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            let v2 = load([
                33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
                48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
                63, 64,
            ]);
            assert_eq!(
                unload(v1.shift_in_two_bytes(v2)),
                [
                    63, 64, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
                    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
                    30,
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_three_bytes() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            let v2 = load([
                33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
                48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
                63, 64,
            ]);
            assert_eq!(
                unload(v1.shift_in_three_bytes(v2)),
                [
                    62, 63, 64, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,
                    15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
                    29,
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shuffle_bytes() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            let v2 = load([
                0, 0, 0, 0, 4, 4, 4, 4, 8, 8, 8, 8, 12, 12, 12, 12, 16, 16,
                16, 16, 20, 20, 20, 20, 24, 24, 24, 24, 28, 28, 28, 28,
            ]);
            assert_eq!(
                unload(v1.shuffle_bytes(v2)),
                [
                    1, 1, 1, 1, 5, 5, 5, 5, 9, 9, 9, 9, 13, 13, 13, 13, 17,
                    17, 17, 17, 21, 21, 21, 21, 25, 25, 25, 25, 29, 29, 29,
                    29
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn vector_for_each_64bit_lane() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v = load([
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
                0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14,
                0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E,
                0x1F, 0x20,
            ]);
            let mut lanes = [0u64; 4];
            v.for_each_64bit_lane(|i, lane| {
                lanes[i] = lane;
                None::<()>
            });
            assert_eq!(
                lanes,
                [
                    0x0807060504030201,
                    0x100F0E0D0C0B0A09,
                    0x1817161514131211,
                    0x201F1E1D1C1B1A19
                ]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn fat_vector_half_shift_in_one_byte() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load_half([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
            ]);
            let v2 = load_half([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.half_shift_in_one_byte(v2)),
                [
                    32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 32,
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn fat_vector_half_shift_in_two_bytes() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load_half([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
            ]);
            let v2 = load_half([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.half_shift_in_two_bytes(v2)),
                [
                    31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 31,
                    32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn fat_vector_half_shift_in_three_bytes() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load_half([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
            ]);
            let v2 = load_half([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.half_shift_in_three_bytes(v2)),
                [
                    30, 31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 30,
                    31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn fat_vector_swap_halves() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v = load([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v.swap_halves()),
                [
                    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
                    31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
                    16,
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn fat_vector_interleave_low_8bit_lanes() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            let v2 = load([
                33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
                48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
                63, 64,
            ]);
            assert_eq!(
                unload(v1.interleave_low_8bit_lanes(v2)),
                [
                    1, 33, 2, 34, 3, 35, 4, 36, 5, 37, 6, 38, 7, 39, 8, 40,
                    17, 49, 18, 50, 19, 51, 20, 52, 21, 53, 22, 54, 23, 55,
                    24, 56,
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn fat_vector_interleave_high_8bit_lanes() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            let v2 = load([
                33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
                48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
                63, 64,
            ]);
            assert_eq!(
                unload(v1.interleave_high_8bit_lanes(v2)),
                [
                    9, 41, 10, 42, 11, 43, 12, 44, 13, 45, 14, 46, 15, 47, 16,
                    48, 25, 57, 26, 58, 27, 59, 28, 60, 29, 61, 30, 62, 31,
                    63, 32, 64,
                ],
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }

    #[test]
    fn fat_vector_for_each_low_64bit_lane() {
        #[target_feature(enable = "avx2")]
        unsafe fn test() {
            let v1 = load([
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
                0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14,
                0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E,
                0x1F, 0x20,
            ]);
            let v2 = load([
                0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A,
                0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34,
                0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E,
                0x3F, 0x40,
            ]);
            let mut lanes = [0u64; 4];
            v1.for_each_low_64bit_lane(v2, |i, lane| {
                lanes[i] = lane;
                None::<()>
            });
            assert_eq!(
                lanes,
                [
                    0x0807060504030201,
                    0x100F0E0D0C0B0A09,
                    0x2827262524232221,
                    0x302F2E2D2C2B2A29
                ]
            );
        }
        if !is_runnable() {
            return;
        }
        unsafe { test() }
    }
}

#[cfg(all(test, target_arch = "aarch64", target_feature = "neon"))]
mod tests_aarch64_neon {
    use core::arch::aarch64::*;

    use super::*;

    #[target_feature(enable = "neon")]
    unsafe fn load(lanes: [u8; 16]) -> uint8x16_t {
        uint8x16_t::load_unaligned(&lanes as *const u8)
    }

    #[target_feature(enable = "neon")]
    unsafe fn unload(v: uint8x16_t) -> [u8; 16] {
        [
            vgetq_lane_u8(v, 0),
            vgetq_lane_u8(v, 1),
            vgetq_lane_u8(v, 2),
            vgetq_lane_u8(v, 3),
            vgetq_lane_u8(v, 4),
            vgetq_lane_u8(v, 5),
            vgetq_lane_u8(v, 6),
            vgetq_lane_u8(v, 7),
            vgetq_lane_u8(v, 8),
            vgetq_lane_u8(v, 9),
            vgetq_lane_u8(v, 10),
            vgetq_lane_u8(v, 11),
            vgetq_lane_u8(v, 12),
            vgetq_lane_u8(v, 13),
            vgetq_lane_u8(v, 14),
            vgetq_lane_u8(v, 15),
        ]
    }

    // Example functions. These don't test the Vector traits, but rather,
    // specific NEON instructions. They are basically little experiments I
    // wrote to figure out what an instruction does since their descriptions
    // are so dense. I decided to keep the experiments around as example tests
    // in case there' useful.

    #[test]
    fn example_vmaxvq_u8_non_zero() {
        #[target_feature(enable = "neon")]
        unsafe fn example() {
            let v = load([0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(vmaxvq_u8(v), 1);
        }
        unsafe { example() }
    }

    #[test]
    fn example_vmaxvq_u8_zero() {
        #[target_feature(enable = "neon")]
        unsafe fn example() {
            let v = load([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(vmaxvq_u8(v), 0);
        }
        unsafe { example() }
    }

    #[test]
    fn example_vpmaxq_u8_non_zero() {
        #[target_feature(enable = "neon")]
        unsafe fn example() {
            let v = load([0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            let r = vpmaxq_u8(v, v);
            assert_eq!(
                unload(r),
                [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0]
            );
        }
        unsafe { example() }
    }

    #[test]
    fn example_vpmaxq_u8_self() {
        #[target_feature(enable = "neon")]
        unsafe fn example() {
            let v =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let r = vpmaxq_u8(v, v);
            assert_eq!(
                unload(r),
                [2, 4, 6, 8, 10, 12, 14, 16, 2, 4, 6, 8, 10, 12, 14, 16]
            );
        }
        unsafe { example() }
    }

    #[test]
    fn example_vpmaxq_u8_other() {
        #[target_feature(enable = "neon")]
        unsafe fn example() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 = load([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            let r = vpmaxq_u8(v1, v2);
            assert_eq!(
                unload(r),
                [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32]
            );
        }
        unsafe { example() }
    }

    // Now we test the actual methods on the Vector trait.

    #[test]
    fn vector_splat() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v = uint8x16_t::splat(0xAF);
            assert_eq!(
                unload(v),
                [
                    0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF,
                    0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF, 0xAF
                ]
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_is_zero() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v = load([0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert!(!v.is_zero());
            let v = load([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert!(v.is_zero());
        }
        unsafe { test() }
    }

    #[test]
    fn vector_cmpeq() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 1]);
            let v2 =
                load([16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
            assert_eq!(
                unload(v1.cmpeq(v2)),
                [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF]
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_and() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v1 =
                load([0, 0, 0, 0, 0, 0b1001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            let v2 =
                load([0, 0, 0, 0, 0, 0b1010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                unload(v1.and(v2)),
                [0, 0, 0, 0, 0, 0b1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_or() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v1 =
                load([0, 0, 0, 0, 0, 0b1001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            let v2 =
                load([0, 0, 0, 0, 0, 0b1010, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                unload(v1.or(v2)),
                [0, 0, 0, 0, 0, 0b1011, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_8bit_lane_right() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v = load([
                0, 0, 0, 0, 0b1011, 0b0101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            assert_eq!(
                unload(v.shift_8bit_lane_right::<2>()),
                [0, 0, 0, 0, 0b0010, 0b0001, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_one_byte() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 = load([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.shift_in_one_byte(v2)),
                [32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_two_bytes() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 = load([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.shift_in_two_bytes(v2)),
                [31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shift_in_three_bytes() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 = load([
                17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]);
            assert_eq!(
                unload(v1.shift_in_three_bytes(v2)),
                [30, 31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13],
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_shuffle_bytes() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v1 =
                load([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            let v2 =
                load([0, 0, 0, 0, 4, 4, 4, 4, 8, 8, 8, 8, 12, 12, 12, 12]);
            assert_eq!(
                unload(v1.shuffle_bytes(v2)),
                [1, 1, 1, 1, 5, 5, 5, 5, 9, 9, 9, 9, 13, 13, 13, 13],
            );
        }
        unsafe { test() }
    }

    #[test]
    fn vector_for_each_64bit_lane() {
        #[target_feature(enable = "neon")]
        unsafe fn test() {
            let v = load([
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
                0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            ]);
            let mut lanes = [0u64; 2];
            v.for_each_64bit_lane(|i, lane| {
                lanes[i] = lane;
                None::<()>
            });
            assert_eq!(lanes, [0x0807060504030201, 0x100F0E0D0C0B0A09],);
        }
        unsafe { test() }
    }
}
