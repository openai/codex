//! Contains the aarch64 UTF-8 validation implementation.

use core::arch::aarch64::{
    uint8x16_t, vandq_u8, vcgtq_u8, vdupq_n_u8, veorq_u8, vextq_u8, vld1q_u8, vmaxvq_u8,
    vmovq_n_u8, vorrq_u8, vqsubq_u8, vqtbl1q_u8, vshrq_n_u8,
};

use crate::implementation::helpers::Utf8CheckAlgorithm;

// aarch64 SIMD primitives

type SimdU8Value = crate::implementation::helpers::SimdU8Value<uint8x16_t>;

impl SimdU8Value {
    #[inline]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::cast_possible_wrap)]
    unsafe fn from_32_cut_off_leading(
        _v0: u8,
        _v1: u8,
        _v2: u8,
        _v3: u8,
        _v4: u8,
        _v5: u8,
        _v6: u8,
        _v7: u8,
        _v8: u8,
        _v9: u8,
        _v10: u8,
        _v11: u8,
        _v12: u8,
        _v13: u8,
        _v14: u8,
        _v15: u8,
        v16: u8,
        v17: u8,
        v18: u8,
        v19: u8,
        v20: u8,
        v21: u8,
        v22: u8,
        v23: u8,
        v24: u8,
        v25: u8,
        v26: u8,
        v27: u8,
        v28: u8,
        v29: u8,
        v30: u8,
        v31: u8,
    ) -> Self {
        let arr: [u8; 16] = [
            v16, v17, v18, v19, v20, v21, v22, v23, v24, v25, v26, v27, v28, v29, v30, v31,
        ];
        Self::from(vld1q_u8(arr.as_ptr()))
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::cast_possible_wrap)]
    unsafe fn repeat_16(
        v0: u8,
        v1: u8,
        v2: u8,
        v3: u8,
        v4: u8,
        v5: u8,
        v6: u8,
        v7: u8,
        v8: u8,
        v9: u8,
        v10: u8,
        v11: u8,
        v12: u8,
        v13: u8,
        v14: u8,
        v15: u8,
    ) -> Self {
        let arr: [u8; 16] = [
            v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15,
        ];
        Self::from(vld1q_u8(arr.as_ptr()))
    }

    #[inline]
    #[allow(clippy::cast_ptr_alignment)]
    unsafe fn load_from(ptr: *const u8) -> Self {
        // WORKAROUND for https://github.com/rust-lang/stdarch/issues/1148
        // The vld1q_u8 intrinsic is currently broken, it treats it as individual
        // byte loads so the compiler sometimes decides it is a better to load
        // individual bytes to "optimize" a subsequent SIMD shuffle
        //
        // This code forces a full 128-bit load.
        let mut dst = core::mem::MaybeUninit::<uint8x16_t>::uninit();
        core::ptr::copy_nonoverlapping(
            ptr.cast::<u8>(),
            dst.as_mut_ptr().cast::<u8>(),
            core::mem::size_of::<uint8x16_t>(),
        );
        Self::from(dst.assume_init())
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    unsafe fn lookup_16(
        self,
        v0: u8,
        v1: u8,
        v2: u8,
        v3: u8,
        v4: u8,
        v5: u8,
        v6: u8,
        v7: u8,
        v8: u8,
        v9: u8,
        v10: u8,
        v11: u8,
        v12: u8,
        v13: u8,
        v14: u8,
        v15: u8,
    ) -> Self {
        Self::from(vqtbl1q_u8(
            Self::repeat_16(
                v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15,
            )
            .0,
            self.0,
        ))
    }

    #[inline]
    #[allow(clippy::cast_possible_wrap)]
    unsafe fn splat(val: u8) -> Self {
        Self::from(vmovq_n_u8(val))
    }

    #[inline]
    #[allow(clippy::cast_possible_wrap)]
    unsafe fn splat0() -> Self {
        Self::from(vdupq_n_u8(0))
    }

    #[inline]
    unsafe fn or(self, b: Self) -> Self {
        Self::from(vorrq_u8(self.0, b.0))
    }

    #[inline]
    unsafe fn and(self, b: Self) -> Self {
        Self::from(vandq_u8(self.0, b.0))
    }

    #[inline]
    unsafe fn xor(self, b: Self) -> Self {
        Self::from(veorq_u8(self.0, b.0))
    }

    #[inline]
    unsafe fn saturating_sub(self, b: Self) -> Self {
        Self::from(vqsubq_u8(self.0, b.0))
    }

    // ugly but shr<N> requires const generics

    #[allow(clippy::cast_lossless)]
    #[inline]
    unsafe fn shr4(self) -> Self {
        Self::from(vshrq_n_u8(self.0, 4))
    }

    // ugly but prev<N> requires const generics

    #[allow(clippy::cast_lossless)]
    #[inline]
    unsafe fn prev1(self, prev: Self) -> Self {
        Self::from(vextq_u8(prev.0, self.0, 16 - 1))
    }

    // ugly but prev<N> requires const generics

    #[allow(clippy::cast_lossless)]
    #[inline]
    unsafe fn prev2(self, prev: Self) -> Self {
        Self::from(vextq_u8(prev.0, self.0, 16 - 2))
    }

    // ugly but prev<N> requires const generics

    #[allow(clippy::cast_lossless)]
    #[inline]
    unsafe fn prev3(self, prev: Self) -> Self {
        Self::from(vextq_u8(prev.0, self.0, 16 - 3))
    }

    #[inline]
    unsafe fn unsigned_gt(self, other: Self) -> Self {
        Self::from(vcgtq_u8(self.0, other.0))
    }

    #[inline]
    unsafe fn any_bit_set(self) -> bool {
        vmaxvq_u8(self.0) != 0
    }

    #[inline]
    unsafe fn is_ascii(self) -> bool {
        vmaxvq_u8(self.0) < 0b1000_0000_u8
    }
}

impl From<uint8x16_t> for SimdU8Value {
    #[inline]
    fn from(val: uint8x16_t) -> Self {
        Self(val)
    }
}

impl Utf8CheckAlgorithm<SimdU8Value> {
    #[inline]
    unsafe fn must_be_2_3_continuation(prev2: SimdU8Value, prev3: SimdU8Value) -> SimdU8Value {
        let is_third_byte = prev2.unsigned_gt(SimdU8Value::splat(0b1110_0000 - 1));
        let is_fourth_byte = prev3.unsigned_gt(SimdU8Value::splat(0b1111_0000 - 1));

        is_third_byte.or(is_fourth_byte)
    }
}

#[inline]
#[cfg(feature = "aarch64_neon_prefetch")]
unsafe fn simd_prefetch(ptr: *const u8) {
    use core::arch::aarch64::{_prefetch, _PREFETCH_LOCALITY3, _PREFETCH_READ};
    _prefetch(ptr.cast::<i8>(), _PREFETCH_READ, _PREFETCH_LOCALITY3);
}

#[inline]
#[cfg(not(feature = "aarch64_neon_prefetch"))]
unsafe fn simd_prefetch(_ptr: *const u8) {}

const PREFETCH: bool = false;
use crate::implementation::helpers::TempSimdChunkA16 as TempSimdChunk;
simd_input_128_bit!("not_used");
algorithm_simd!("not_used");
