/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(all(feature = "neon", target_arch = "aarch64"))]
// TODO can this be extended to armv7

//! This module provides unsafe ways to do some things
#![allow(clippy::wildcard_imports)]

use core::arch::aarch64::*;
use core::ops::{Add, AddAssign, BitOr, BitOrAssign, Mul, MulAssign, Sub};

pub type VecType = int32x4x2_t;

pub unsafe fn loadu(src: *const i32) -> VecType {
    vld1q_s32_x2(src as *const _)
}

/// An abstraction of an AVX ymm register that
///allows some things to not look ugly
#[derive(Clone, Copy)]
pub struct YmmRegister {
    /// An AVX register
    pub(crate) mm256: VecType
}

impl YmmRegister {
    #[inline]
    pub unsafe fn load(src: *const i32) -> Self {
        loadu(src).into()
    }

    #[inline]
    pub fn map2(self, other: Self, f: impl Fn(int32x4_t, int32x4_t) -> int32x4_t) -> Self {
        let m0 = f(self.mm256.0, other.mm256.0);
        let m1 = f(self.mm256.1, other.mm256.1);

        YmmRegister {
            mm256: int32x4x2_t(m0, m1)
        }
    }

    #[inline]
    pub fn all_zero(self) -> bool {
        unsafe {
            let both = vorrq_s32(self.mm256.0, self.mm256.1);
            let both_unsigned = vreinterpretq_u32_s32(both);
            0 == vmaxvq_u32(both_unsigned)
        }
    }

    #[inline]
    pub fn const_shl<const N: i32>(self) -> Self {
        // Ensure that we logically shift left
        unsafe {
            let m0 = vreinterpretq_s32_u32(vshlq_n_u32::<N>(vreinterpretq_u32_s32(self.mm256.0)));
            let m1 = vreinterpretq_s32_u32(vshlq_n_u32::<N>(vreinterpretq_u32_s32(self.mm256.1)));

            YmmRegister {
                mm256: int32x4x2_t(m0, m1)
            }
        }
    }

    #[inline]
    pub fn const_shra<const N: i32>(self) -> Self {
        unsafe {
            let i0 = vshrq_n_s32::<N>(self.mm256.0);
            let i1 = vshrq_n_s32::<N>(self.mm256.1);

            YmmRegister {
                mm256: int32x4x2_t(i0, i1)
            }
        }
    }
}

impl<T> Add<T> for YmmRegister
where
    T: Into<Self>
{
    type Output = YmmRegister;

    #[inline]
    fn add(self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        unsafe { self.map2(rhs, |a, b| vaddq_s32(a, b)) }
    }
}

impl<T> Sub<T> for YmmRegister
where
    T: Into<Self>
{
    type Output = YmmRegister;

    #[inline]
    fn sub(self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        unsafe { self.map2(rhs, |a, b| vsubq_s32(a, b)) }
    }
}

impl<T> AddAssign<T> for YmmRegister
where
    T: Into<Self>
{
    #[inline]
    fn add_assign(&mut self, rhs: T) {
        let rhs: Self = rhs.into();
        *self = *self + rhs;
    }
}

impl<T> Mul<T> for YmmRegister
where
    T: Into<Self>
{
    type Output = YmmRegister;

    #[inline]
    fn mul(self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        unsafe { self.map2(rhs, |a, b| vmulq_s32(a, b)) }
    }
}

impl<T> MulAssign<T> for YmmRegister
where
    T: Into<Self>
{
    #[inline]
    fn mul_assign(&mut self, rhs: T) {
        let rhs: Self = rhs.into();
        *self = *self * rhs;
    }
}

impl<T> BitOr<T> for YmmRegister
where
    T: Into<Self>
{
    type Output = YmmRegister;

    #[inline]
    fn bitor(self, rhs: T) -> Self::Output {
        let rhs = rhs.into();
        unsafe { self.map2(rhs, |a, b| vorrq_s32(a, b)) }
    }
}

impl<T> BitOrAssign<T> for YmmRegister
where
    T: Into<Self>
{
    #[inline]
    fn bitor_assign(&mut self, rhs: T) {
        let rhs: Self = rhs.into();
        *self = *self | rhs;
    }
}

impl From<i32> for YmmRegister {
    #[inline]
    fn from(val: i32) -> Self {
        unsafe {
            let dup = vdupq_n_s32(val);

            YmmRegister {
                mm256: int32x4x2_t(dup, dup)
            }
        }
    }
}

impl From<VecType> for YmmRegister {
    #[inline]
    fn from(mm256: VecType) -> Self {
        YmmRegister { mm256 }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
unsafe fn transpose4(
    v0: &mut int32x4_t, v1: &mut int32x4_t, v2: &mut int32x4_t, v3: &mut int32x4_t
) {
    let w0 = vtrnq_s32(
        vreinterpretq_s32_s64(vtrn1q_s64(
            vreinterpretq_s64_s32(*v0),
            vreinterpretq_s64_s32(*v2)
        )),
        vreinterpretq_s32_s64(vtrn1q_s64(
            vreinterpretq_s64_s32(*v1),
            vreinterpretq_s64_s32(*v3)
        ))
    );
    let w1 = vtrnq_s32(
        vreinterpretq_s32_s64(vtrn2q_s64(
            vreinterpretq_s64_s32(*v0),
            vreinterpretq_s64_s32(*v2)
        )),
        vreinterpretq_s32_s64(vtrn2q_s64(
            vreinterpretq_s64_s32(*v1),
            vreinterpretq_s64_s32(*v3)
        ))
    );

    *v0 = w0.0;
    *v1 = w0.1;
    *v2 = w1.0;
    *v3 = w1.1;
}

/// Transpose an array of 8 by 8 i32
/// Arm has dedicated interleave/transpose instructions
/// we:
/// 1. Transpose the upper left and lower right quadrants
/// 2. Swap and transpose the upper right and lower left quadrants
#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn transpose(
    v0: &mut YmmRegister, v1: &mut YmmRegister, v2: &mut YmmRegister, v3: &mut YmmRegister,
    v4: &mut YmmRegister, v5: &mut YmmRegister, v6: &mut YmmRegister, v7: &mut YmmRegister
) {
    use core::mem::swap;

    let ul0 = &mut v0.mm256.0;
    let ul1 = &mut v1.mm256.0;
    let ul2 = &mut v2.mm256.0;
    let ul3 = &mut v3.mm256.0;

    let ur0 = &mut v0.mm256.1;
    let ur1 = &mut v1.mm256.1;
    let ur2 = &mut v2.mm256.1;
    let ur3 = &mut v3.mm256.1;

    let ll0 = &mut v4.mm256.0;
    let ll1 = &mut v5.mm256.0;
    let ll2 = &mut v6.mm256.0;
    let ll3 = &mut v7.mm256.0;

    let lr0 = &mut v4.mm256.1;
    let lr1 = &mut v5.mm256.1;
    let lr2 = &mut v6.mm256.1;
    let lr3 = &mut v7.mm256.1;

    swap(ur0, ll0);
    swap(ur1, ll1);
    swap(ur2, ll2);
    swap(ur3, ll3);

    transpose4(ul0, ul1, ul2, ul3);

    transpose4(ur0, ur1, ur2, ur3);

    transpose4(ll0, ll1, ll2, ll3);

    transpose4(lr0, lr1, lr2, lr3);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpose() {
        fn get_val(i: usize, j: usize) -> i32 {
            ((i * 8) / (j + 1)) as i32
        }
        unsafe {
            let mut vals: [i32; 8 * 8] = [0; 8 * 8];

            for i in 0..8 {
                for j in 0..8 {
                    // some order-dependent value of i and j
                    let value = get_val(i, j);
                    vals[i * 8 + j] = value;
                }
            }

            let mut regs: [YmmRegister; 8] = core::mem::transmute(vals);
            let mut reg0 = regs[0];
            let mut reg1 = regs[1];
            let mut reg2 = regs[2];
            let mut reg3 = regs[3];
            let mut reg4 = regs[4];
            let mut reg5 = regs[5];
            let mut reg6 = regs[6];
            let mut reg7 = regs[7];

            transpose(
                &mut reg0, &mut reg1, &mut reg2, &mut reg3, &mut reg4, &mut reg5, &mut reg6,
                &mut reg7
            );

            regs[0] = reg0;
            regs[1] = reg1;
            regs[2] = reg2;
            regs[3] = reg3;
            regs[4] = reg4;
            regs[5] = reg5;
            regs[6] = reg6;
            regs[7] = reg7;

            let vals_from_reg: [i32; 8 * 8] = core::mem::transmute(regs);

            for i in 0..8 {
                for j in 0..i {
                    let orig = vals[i * 8 + j];
                    vals[i * 8 + j] = vals[j * 8 + i];
                    vals[j * 8 + i] = orig;
                }
            }

            for i in 0..8 {
                for j in 0..8 {
                    assert_eq!(vals[j * 8 + i], get_val(i, j));
                    assert_eq!(vals_from_reg[j * 8 + i], get_val(i, j));
                }
            }

            assert_eq!(vals, vals_from_reg);
        }
    }
}
