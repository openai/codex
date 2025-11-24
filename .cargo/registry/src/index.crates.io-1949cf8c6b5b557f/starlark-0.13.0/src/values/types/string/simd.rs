/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/// Fixed length byte vector API.
#[allow(dead_code)] // FIXME: Investigate if this is really needed, fails on Mac
pub(crate) trait Vector: Copy {
    /// Fill the vector with given byte value.
    unsafe fn splat(byte: u8) -> Self;
    /// Load the vector from given memory address.
    unsafe fn load_unaligned(ptr: *const u8) -> Self;
    /// Store the vector to given memory address.
    unsafe fn store_unaligned(self, ptr: *mut u8);
    /// **signed** element-wise comparison of the vector.
    unsafe fn cmplt(self, other: Self) -> Self;
    /// Element-wise comparison. Result elements contain 0 for false or 0xff for true.
    unsafe fn cmpeq(self, other: Self) -> Self;
    /// Bitwise or.
    unsafe fn or(self, other: Self) -> Self;
    /// Mask of the most significant bit of each element.
    /// For 16-bytes vector this instruction fills lower 16 bits of the result.
    unsafe fn movemask(self) -> u32;
}

/// Run different code depending on whether SIMD is available or not.
pub(crate) trait SwitchHaveSimd<R>
where
    Self: Sized,
{
    /// This function is called when SIMD is not available.
    fn no_simd(self) -> R;
    /// This function is called when SIMD is available.
    #[allow(dead_code)] // FIXME: Investigate if this is really needed, fails on Mac
    fn simd<V: Vector>(self) -> R;

    /// Call either `simd` or `no_simd` function.
    fn switch(self) -> R {
        // Any x86_64 supports SSE2.
        #[cfg(target_feature = "sse2")]
        {
            #[cfg(target_arch = "x86")]
            use std::arch::x86::*;
            #[cfg(target_arch = "x86_64")]
            use std::arch::x86_64::*;

            if true {
                return self.simd::<__m128i>();
            }
        }

        self.no_simd()
    }
}

#[cfg(target_feature = "sse2")]
mod sse2 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    use crate::values::types::string::simd::Vector;

    impl Vector for __m128i {
        #[inline(always)]
        unsafe fn splat(byte: u8) -> Self {
            _mm_set1_epi8(byte as i8)
        }

        #[inline(always)]
        unsafe fn load_unaligned(ptr: *const u8) -> Self {
            _mm_loadu_si128(ptr as *const _)
        }

        #[inline(always)]
        unsafe fn store_unaligned(self, ptr: *mut u8) {
            _mm_storeu_si128(ptr as *mut _, self)
        }

        #[inline(always)]
        unsafe fn cmplt(self, other: Self) -> Self {
            _mm_cmplt_epi8(self, other)
        }

        #[inline(always)]
        unsafe fn cmpeq(self, other: Self) -> Self {
            _mm_cmpeq_epi8(self, other)
        }

        #[inline(always)]
        unsafe fn or(self, other: Self) -> Self {
            _mm_or_si128(self, other)
        }

        #[inline(always)]
        unsafe fn movemask(self) -> u32 {
            _mm_movemask_epi8(self) as u32
        }
    }
}
