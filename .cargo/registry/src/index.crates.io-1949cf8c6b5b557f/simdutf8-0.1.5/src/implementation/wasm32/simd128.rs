//! Contains the wasm32 UTF-8 validation implementation.

use core::arch::wasm32::{
    u8x16, u8x16_all_true, u8x16_gt, u8x16_lt, u8x16_shr, u8x16_shuffle, u8x16_splat,
    u8x16_sub_sat, u8x16_swizzle, v128, v128_and, v128_any_true, v128_or, v128_xor,
};

use crate::implementation::helpers::Utf8CheckAlgorithm;

// wasm32 SIMD primitives

type SimdU8Value = crate::implementation::helpers::SimdU8Value<v128>;

#[repr(C, align(16))]
struct AlignV128Array([u8; 16]);

impl SimdU8Value {
    #[inline]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::cast_possible_wrap)]
    #[allow(clippy::cast_ptr_alignment)]
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
        let arr = AlignV128Array([
            v16, v17, v18, v19, v20, v21, v22, v23, v24, v25, v26, v27, v28, v29, v30, v31,
        ]);
        Self::from(*(arr.0.as_ptr().cast::<v128>()))
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::cast_possible_wrap)]
    #[allow(clippy::cast_ptr_alignment)]
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
        let arr = AlignV128Array([
            v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15,
        ]);
        Self::from(*(arr.0.as_ptr().cast::<v128>()))
    }

    #[inline]
    unsafe fn load_from(ptr: *const u8) -> Self {
        Self::from(ptr.cast::<v128>().read_unaligned())
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
        Self::from(u8x16_swizzle(
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
        Self::from(u8x16_splat(val))
    }

    #[inline]
    #[allow(clippy::cast_possible_wrap)]
    unsafe fn splat0() -> Self {
        Self::from(u8x16(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0))
    }

    #[inline]
    unsafe fn or(self, b: Self) -> Self {
        Self::from(v128_or(self.0, b.0))
    }

    #[inline]
    unsafe fn and(self, b: Self) -> Self {
        Self::from(v128_and(self.0, b.0))
    }

    #[inline]
    unsafe fn xor(self, b: Self) -> Self {
        Self::from(v128_xor(self.0, b.0))
    }

    #[inline]
    unsafe fn saturating_sub(self, b: Self) -> Self {
        Self::from(u8x16_sub_sat(self.0, b.0))
    }

    // ugly but shr<N> requires const generics

    #[allow(clippy::cast_lossless)]
    #[inline]
    unsafe fn shr4(self) -> Self {
        Self::from(u8x16_shr(self.0, 4))
    }

    // ugly but prev<N> requires const generics

    // TODO make this into a macro

    #[allow(clippy::cast_lossless)]
    #[inline]
    unsafe fn prev1(self, prev: Self) -> Self {
        Self::from(u8x16_shuffle::<
            15,
            16,
            17,
            18,
            19,
            20,
            21,
            22,
            23,
            24,
            25,
            26,
            27,
            28,
            29,
            30,
        >(prev.0, self.0))
    }

    // ugly but prev<N> requires const generics

    #[allow(clippy::cast_lossless)]
    #[inline]
    unsafe fn prev2(self, prev: Self) -> Self {
        Self::from(u8x16_shuffle::<
            14,
            15,
            16,
            17,
            18,
            19,
            20,
            21,
            22,
            23,
            24,
            25,
            26,
            27,
            28,
            29,
        >(prev.0, self.0))
    }

    // ugly but prev<N> requires const generics

    #[allow(clippy::cast_lossless)]
    #[inline]
    unsafe fn prev3(self, prev: Self) -> Self {
        Self::from(u8x16_shuffle::<
            13,
            14,
            15,
            16,
            17,
            18,
            19,
            20,
            21,
            22,
            23,
            24,
            25,
            26,
            27,
            28,
        >(prev.0, self.0))
    }

    #[inline]
    unsafe fn unsigned_gt(self, other: Self) -> Self {
        Self::from(u8x16_gt(self.0, other.0))
    }

    #[inline]
    unsafe fn any_bit_set(self) -> bool {
        v128_any_true(self.0)
    }

    #[inline]
    unsafe fn is_ascii(self) -> bool {
        // We don't want to use u8x16_bitmask as that is inefficient on NEON.
        // For x86 shifts should also be avoided.
        u8x16_all_true(u8x16_lt(self.0, u8x16_splat(0b1000_0000_u8)))
    }
}

impl From<v128> for SimdU8Value {
    #[inline]
    fn from(v: v128) -> Self {
        Self(v)
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
const fn simd_prefetch(_ptr: *const u8) {
    // no-op
}

const PREFETCH: bool = false;
use crate::implementation::helpers::TempSimdChunkA16 as TempSimdChunk;
simd_input_128_bit!("simd128");
algorithm_simd!("simd128");
