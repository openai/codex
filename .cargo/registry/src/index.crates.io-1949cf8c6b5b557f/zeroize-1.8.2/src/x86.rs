//! [`Zeroize`] impls for x86 SIMD registers

use crate::{atomic_fence, volatile_write, Zeroize};

#[cfg(target_arch = "x86")]
use core::arch::x86::*;

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

macro_rules! impl_zeroize_for_simd_register {
    ($($type:ty),* $(,)?) => {
        $(
            impl Zeroize for $type {
                #[inline]
                fn zeroize(&mut self) {
                    volatile_write(self, unsafe { core::mem::zeroed() });
                    atomic_fence();
                }
            }
        )*
    };
}

impl_zeroize_for_simd_register!(__m128, __m128d, __m128i, __m256, __m256d, __m256i);

// NOTE: MSRV 1.72
#[cfg(feature = "simd")]
impl_zeroize_for_simd_register!(__m512, __m512d, __m512i);
