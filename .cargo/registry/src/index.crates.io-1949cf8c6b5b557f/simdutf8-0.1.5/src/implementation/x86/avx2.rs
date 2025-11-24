//! Contains the x86-64/x86 AVX2 UTF-8 validation implementation.

#![allow(clippy::too_many_arguments)]

#[cfg(target_arch = "x86")]
use core::arch::x86::{
    __m256i, _mm256_alignr_epi8, _mm256_and_si256, _mm256_cmpgt_epi8, _mm256_loadu_si256,
    _mm256_movemask_epi8, _mm256_or_si256, _mm256_permute2x128_si256, _mm256_set1_epi8,
    _mm256_setr_epi8, _mm256_setzero_si256, _mm256_shuffle_epi8, _mm256_srli_epi16,
    _mm256_subs_epu8, _mm256_testz_si256, _mm256_xor_si256, _mm_prefetch, _MM_HINT_T0,
};
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{
    __m256i, _mm256_alignr_epi8, _mm256_and_si256, _mm256_cmpgt_epi8, _mm256_loadu_si256,
    _mm256_movemask_epi8, _mm256_or_si256, _mm256_permute2x128_si256, _mm256_set1_epi8,
    _mm256_setr_epi8, _mm256_setzero_si256, _mm256_shuffle_epi8, _mm256_srli_epi16,
    _mm256_subs_epu8, _mm256_testz_si256, _mm256_xor_si256, _mm_prefetch, _MM_HINT_T0,
};

use crate::implementation::helpers::Utf8CheckAlgorithm;

// AVX 2 SIMD primitives

type SimdU8Value = crate::implementation::helpers::SimdU8Value<__m256i>;

impl SimdU8Value {
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn from_32_cut_off_leading(
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
        #[allow(clippy::cast_possible_wrap)]
        Self::from(_mm256_setr_epi8(
            v0 as i8, v1 as i8, v2 as i8, v3 as i8, v4 as i8, v5 as i8, v6 as i8, v7 as i8,
            v8 as i8, v9 as i8, v10 as i8, v11 as i8, v12 as i8, v13 as i8, v14 as i8, v15 as i8,
            v16 as i8, v17 as i8, v18 as i8, v19 as i8, v20 as i8, v21 as i8, v22 as i8, v23 as i8,
            v24 as i8, v25 as i8, v26 as i8, v27 as i8, v28 as i8, v29 as i8, v30 as i8, v31 as i8,
        ))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
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
        #[allow(clippy::cast_possible_wrap)]
        Self::from_32_cut_off_leading(
            v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v0, v1, v2, v3,
            v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15,
        )
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn load_from(ptr: *const u8) -> Self {
        #[allow(clippy::cast_ptr_alignment)]
        Self::from(_mm256_loadu_si256(ptr.cast::<__m256i>()))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
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
        Self::from(_mm256_shuffle_epi8(
            Self::repeat_16(
                v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15,
            )
            .0,
            self.0,
        ))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn splat(val: u8) -> Self {
        #[allow(clippy::cast_possible_wrap)]
        Self::from(_mm256_set1_epi8(val as i8))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn splat0() -> Self {
        Self::from(_mm256_setzero_si256())
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn or(self, b: Self) -> Self {
        Self::from(_mm256_or_si256(self.0, b.0))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn and(self, b: Self) -> Self {
        Self::from(_mm256_and_si256(self.0, b.0))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn xor(self, b: Self) -> Self {
        Self::from(_mm256_xor_si256(self.0, b.0))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn saturating_sub(self, b: Self) -> Self {
        Self::from(_mm256_subs_epu8(self.0, b.0))
    }

    // ugly but shr<N> requires const generics
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn shr4(self) -> Self {
        Self::from(_mm256_srli_epi16(self.0, 4)).and(Self::splat(0xFF >> 4))
    }

    // ugly but prev<N> requires const generics
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn prev1(self, prev: Self) -> Self {
        Self::from(_mm256_alignr_epi8(
            self.0,
            _mm256_permute2x128_si256(prev.0, self.0, 0x21),
            16 - 1,
        ))
    }

    // ugly but prev<N> requires const generics
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn prev2(self, prev: Self) -> Self {
        Self::from(_mm256_alignr_epi8(
            self.0,
            _mm256_permute2x128_si256(prev.0, self.0, 0x21),
            16 - 2,
        ))
    }

    // ugly but prev<N> requires const generics
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn prev3(self, prev: Self) -> Self {
        Self::from(_mm256_alignr_epi8(
            self.0,
            _mm256_permute2x128_si256(prev.0, self.0, 0x21),
            16 - 3,
        ))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn signed_gt(self, other: Self) -> Self {
        Self::from(_mm256_cmpgt_epi8(self.0, other.0))
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn any_bit_set(self) -> bool {
        _mm256_testz_si256(self.0, self.0) != 1
    }

    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn is_ascii(self) -> bool {
        _mm256_movemask_epi8(self.0) == 0
    }
}

impl From<__m256i> for SimdU8Value {
    #[inline]
    fn from(val: __m256i) -> Self {
        Self(val)
    }
}

impl Utf8CheckAlgorithm<SimdU8Value> {
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn must_be_2_3_continuation(prev2: SimdU8Value, prev3: SimdU8Value) -> SimdU8Value {
        let is_third_byte = prev2.saturating_sub(SimdU8Value::splat(0b1110_0000 - 1));
        let is_fourth_byte = prev3.saturating_sub(SimdU8Value::splat(0b1111_0000 - 1));

        is_third_byte
            .or(is_fourth_byte)
            .signed_gt(SimdU8Value::splat0())
    }
}

#[target_feature(enable = "avx2")]
#[inline]
unsafe fn simd_prefetch(ptr: *const u8) {
    _mm_prefetch(ptr.cast::<i8>(), _MM_HINT_T0);
}

const PREFETCH: bool = true;
use crate::implementation::helpers::TempSimdChunkA32 as TempSimdChunk;
simd_input_256_bit!("avx2");
algorithm_simd!("avx2");
