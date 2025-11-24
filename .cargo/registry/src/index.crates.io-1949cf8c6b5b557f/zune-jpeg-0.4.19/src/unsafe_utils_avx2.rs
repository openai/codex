/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(all(feature = "x86", any(target_arch = "x86", target_arch = "x86_64")))]
//! This module provides unsafe ways to do some things
#![allow(clippy::wildcard_imports)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;
use core::ops::{Add, AddAssign, Mul, MulAssign, Sub};

/// A copy of `_MM_SHUFFLE()` that doesn't require
/// a nightly compiler
#[inline]
const fn shuffle(z: i32, y: i32, x: i32, w: i32) -> i32 {
    (z << 6) | (y << 4) | (x << 2) | w
}

/// An abstraction of an AVX ymm register that
///allows some things to not look ugly
#[derive(Clone, Copy)]
pub struct YmmRegister {
    /// An AVX register
    pub(crate) mm256: __m256i
}

impl Add for YmmRegister {
    type Output = YmmRegister;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        unsafe {
            return YmmRegister {
                mm256: _mm256_add_epi32(self.mm256, rhs.mm256)
            };
        }
    }
}

impl Add<i32> for YmmRegister {
    type Output = YmmRegister;

    #[inline]
    fn add(self, rhs: i32) -> Self::Output {
        unsafe {
            let tmp = _mm256_set1_epi32(rhs);

            return YmmRegister {
                mm256: _mm256_add_epi32(self.mm256, tmp)
            };
        }
    }
}

impl Sub for YmmRegister {
    type Output = YmmRegister;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        unsafe {
            return YmmRegister {
                mm256: _mm256_sub_epi32(self.mm256, rhs.mm256)
            };
        }
    }
}

impl AddAssign for YmmRegister {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        unsafe {
            self.mm256 = _mm256_add_epi32(self.mm256, rhs.mm256);
        }
    }
}

impl AddAssign<i32> for YmmRegister {
    #[inline]
    fn add_assign(&mut self, rhs: i32) {
        unsafe {
            let tmp = _mm256_set1_epi32(rhs);

            self.mm256 = _mm256_add_epi32(self.mm256, tmp);
        }
    }
}

impl Mul for YmmRegister {
    type Output = YmmRegister;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        unsafe {
            YmmRegister {
                mm256: _mm256_mullo_epi32(self.mm256, rhs.mm256)
            }
        }
    }
}

impl Mul<i32> for YmmRegister {
    type Output = YmmRegister;

    #[inline]
    fn mul(self, rhs: i32) -> Self::Output {
        unsafe {
            let tmp = _mm256_set1_epi32(rhs);

            YmmRegister {
                mm256: _mm256_mullo_epi32(self.mm256, tmp)
            }
        }
    }
}

impl MulAssign for YmmRegister {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        unsafe {
            self.mm256 = _mm256_mullo_epi32(self.mm256, rhs.mm256);
        }
    }
}

impl MulAssign<i32> for YmmRegister {
    #[inline]
    fn mul_assign(&mut self, rhs: i32) {
        unsafe {
            let tmp = _mm256_set1_epi32(rhs);

            self.mm256 = _mm256_mullo_epi32(self.mm256, tmp);
        }
    }
}

impl MulAssign<__m256i> for YmmRegister {
    #[inline]
    fn mul_assign(&mut self, rhs: __m256i) {
        unsafe {
            self.mm256 = _mm256_mullo_epi32(self.mm256, rhs);
        }
    }
}

type Reg = YmmRegister;

/// Transpose an array of 8 by 8 i32's using avx intrinsics
///
/// This was translated from [here](https://newbedev.com/transpose-an-8x8-float-using-avx-avx2)
#[allow(unused_parens, clippy::too_many_arguments)]
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn transpose(
    v0: &mut Reg, v1: &mut Reg, v2: &mut Reg, v3: &mut Reg, v4: &mut Reg, v5: &mut Reg,
    v6: &mut Reg, v7: &mut Reg
) {
    macro_rules! merge_epi32 {
        ($v0:tt,$v1:tt,$v2:tt,$v3:tt) => {
            let va = _mm256_permute4x64_epi64($v0, shuffle(3, 1, 2, 0));

            let vb = _mm256_permute4x64_epi64($v1, shuffle(3, 1, 2, 0));

            $v2 = _mm256_unpacklo_epi32(va, vb);

            $v3 = _mm256_unpackhi_epi32(va, vb);
        };
    }

    macro_rules! merge_epi64 {
        ($v0:tt,$v1:tt,$v2:tt,$v3:tt) => {
            let va = _mm256_permute4x64_epi64($v0, shuffle(3, 1, 2, 0));

            let vb = _mm256_permute4x64_epi64($v1, shuffle(3, 1, 2, 0));

            $v2 = _mm256_unpacklo_epi64(va, vb);

            $v3 = _mm256_unpackhi_epi64(va, vb);
        };
    }

    macro_rules! merge_si128 {
        ($v0:tt,$v1:tt,$v2:tt,$v3:tt) => {
            $v2 = _mm256_permute2x128_si256($v0, $v1, shuffle(0, 2, 0, 0));

            $v3 = _mm256_permute2x128_si256($v0, $v1, shuffle(0, 3, 0, 1));
        };
    }

    let (w0, w1, w2, w3, w4, w5, w6, w7);

    merge_epi32!((v0.mm256), (v1.mm256), w0, w1);

    merge_epi32!((v2.mm256), (v3.mm256), w2, w3);

    merge_epi32!((v4.mm256), (v5.mm256), w4, w5);

    merge_epi32!((v6.mm256), (v7.mm256), w6, w7);

    let (x0, x1, x2, x3, x4, x5, x6, x7);

    merge_epi64!(w0, w2, x0, x1);

    merge_epi64!(w1, w3, x2, x3);

    merge_epi64!(w4, w6, x4, x5);

    merge_epi64!(w5, w7, x6, x7);

    merge_si128!(x0, x4, (v0.mm256), (v1.mm256));

    merge_si128!(x1, x5, (v2.mm256), (v3.mm256));

    merge_si128!(x2, x6, (v4.mm256), (v5.mm256));

    merge_si128!(x3, x7, (v6.mm256), (v7.mm256));
}
