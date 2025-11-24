/*
 * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
 * //
 * // Redistribution and use in source and binary forms, with or without modification,
 * // are permitted provided that the following conditions are met:
 * //
 * // 1.  Redistributions of source code must retain the above copyright notice, this
 * // list of conditions and the following disclaimer.
 * //
 * // 2.  Redistributions in binary form must reproduce the above copyright notice,
 * // this list of conditions and the following disclaimer in the documentation
 * // and/or other materials provided with the distribution.
 * //
 * // 3.  Neither the name of the copyright holder nor the names of its
 * // contributors may be used to endorse or promote products derived from
 * // this software without specific prior written permission.
 * //
 * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
use crate::conversions::interpolator::BarycentricWeight;
use crate::math::FusedMultiplyAdd;
use num_traits::AsPrimitive;
use std::arch::aarch64::*;
use std::ops::{Add, Mul, Sub};

#[repr(align(8), C)]
pub(crate) struct NeonAlignedI16x4(pub(crate) [i16; 4]);

#[cfg(feature = "options")]
pub(crate) struct TetrahedralNeonQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [NeonAlignedI16x4],
}

#[cfg(feature = "options")]
pub(crate) struct PyramidalNeonQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [NeonAlignedI16x4],
}

pub(crate) struct TrilinearNeonQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [NeonAlignedI16x4],
}

#[cfg(feature = "options")]
pub(crate) struct PyramidalNeonQ0_15Double<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [NeonAlignedI16x4],
    pub(crate) cube1: &'a [NeonAlignedI16x4],
}

#[cfg(feature = "options")]
pub(crate) struct PrismaticNeonQ0_15Double<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [NeonAlignedI16x4],
    pub(crate) cube1: &'a [NeonAlignedI16x4],
}

pub(crate) struct TrilinearNeonQ0_15Double<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [NeonAlignedI16x4],
    pub(crate) cube1: &'a [NeonAlignedI16x4],
}

#[cfg(feature = "options")]
pub(crate) struct TetrahedralNeonQ0_15Double<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [NeonAlignedI16x4],
    pub(crate) cube1: &'a [NeonAlignedI16x4],
}

#[cfg(feature = "options")]
pub(crate) struct PrismaticNeonQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [NeonAlignedI16x4],
}

trait Fetcher<T> {
    fn fetch(&self, x: i32, y: i32, z: i32) -> T;
}

struct TetrahedralNeonQ0_15FetchVector<'a, const GRID_SIZE: usize> {
    cube: &'a [NeonAlignedI16x4],
}

struct TetrahedralNeonQ0_15FetchVectorDouble<'a, const GRID_SIZE: usize> {
    cube0: &'a [NeonAlignedI16x4],
    cube1: &'a [NeonAlignedI16x4],
}

#[derive(Copy, Clone)]
pub(crate) struct NeonVectorQ0_15 {
    pub(crate) v: int16x4_t,
}

#[derive(Copy, Clone)]
pub(crate) struct NeonVectorQ0_15Double {
    pub(crate) v: int16x8_t,
}

impl From<i16> for NeonVectorQ0_15 {
    #[inline(always)]
    fn from(v: i16) -> Self {
        NeonVectorQ0_15 {
            v: unsafe { vdup_n_s16(v) },
        }
    }
}

impl From<i16> for NeonVectorQ0_15Double {
    #[inline(always)]
    fn from(v: i16) -> Self {
        NeonVectorQ0_15Double {
            v: unsafe { vdupq_n_s16(v) },
        }
    }
}

impl Sub<NeonVectorQ0_15> for NeonVectorQ0_15 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: NeonVectorQ0_15) -> Self::Output {
        NeonVectorQ0_15 {
            v: unsafe { vsub_s16(self.v, rhs.v) },
        }
    }
}

impl Mul<NeonVectorQ0_15> for NeonVectorQ0_15 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: NeonVectorQ0_15) -> Self::Output {
        NeonVectorQ0_15 {
            v: unsafe { vqrdmulh_s16(self.v, rhs.v) },
        }
    }
}

impl Sub<NeonVectorQ0_15Double> for NeonVectorQ0_15Double {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: NeonVectorQ0_15Double) -> Self::Output {
        NeonVectorQ0_15Double {
            v: unsafe { vsubq_s16(self.v, rhs.v) },
        }
    }
}

impl Mul<NeonVectorQ0_15Double> for NeonVectorQ0_15Double {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: NeonVectorQ0_15Double) -> Self::Output {
        NeonVectorQ0_15Double {
            v: unsafe { vqrdmulhq_s16(self.v, rhs.v) },
        }
    }
}

impl Add<NeonVectorQ0_15> for NeonVectorQ0_15 {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: NeonVectorQ0_15) -> Self::Output {
        NeonVectorQ0_15 {
            v: unsafe { vadd_s16(self.v, rhs.v) },
        }
    }
}

impl Add<NeonVectorQ0_15Double> for NeonVectorQ0_15Double {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: NeonVectorQ0_15Double) -> Self::Output {
        NeonVectorQ0_15Double {
            v: unsafe { vaddq_s16(self.v, rhs.v) },
        }
    }
}

impl FusedMultiplyAdd<NeonVectorQ0_15> for NeonVectorQ0_15 {
    #[inline(always)]
    fn mla(&self, b: NeonVectorQ0_15, c: NeonVectorQ0_15) -> NeonVectorQ0_15 {
        NeonVectorQ0_15 {
            v: unsafe { vqrdmlah_s16(self.v, b.v, c.v) },
        }
    }
}

impl NeonVectorQ0_15 {
    #[inline(always)]
    fn neg_mla(&self, b: NeonVectorQ0_15, c: NeonVectorQ0_15) -> NeonVectorQ0_15 {
        NeonVectorQ0_15 {
            v: unsafe { vqrdmlsh_s16(self.v, b.v, c.v) },
        }
    }
}

impl NeonVectorQ0_15Double {
    #[inline(always)]
    fn neg_mla(&self, b: NeonVectorQ0_15Double, c: NeonVectorQ0_15Double) -> NeonVectorQ0_15Double {
        NeonVectorQ0_15Double {
            v: unsafe { vqrdmlshq_s16(self.v, b.v, c.v) },
        }
    }
}

impl NeonVectorQ0_15Double {
    #[inline(always)]
    fn mla(&self, b: NeonVectorQ0_15Double, c: NeonVectorQ0_15) -> NeonVectorQ0_15Double {
        NeonVectorQ0_15Double {
            v: unsafe { vqrdmlahq_s16(self.v, b.v, vcombine_s16(c.v, c.v)) },
        }
    }

    #[inline(always)]
    pub(crate) fn split(self) -> (NeonVectorQ0_15, NeonVectorQ0_15) {
        unsafe {
            (
                NeonVectorQ0_15 {
                    v: vget_low_s16(self.v),
                },
                NeonVectorQ0_15 {
                    v: vget_high_s16(self.v),
                },
            )
        }
    }
}

impl<const GRID_SIZE: usize> Fetcher<NeonVectorQ0_15>
    for TetrahedralNeonQ0_15FetchVector<'_, GRID_SIZE>
{
    fn fetch(&self, x: i32, y: i32, z: i32) -> NeonVectorQ0_15 {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx = unsafe { self.cube.get_unchecked(offset..) };
        NeonVectorQ0_15 {
            v: unsafe { vld1_s16(jx.as_ptr() as *const i16) },
        }
    }
}

impl<const GRID_SIZE: usize> Fetcher<NeonVectorQ0_15Double>
    for TetrahedralNeonQ0_15FetchVectorDouble<'_, GRID_SIZE>
{
    fn fetch(&self, x: i32, y: i32, z: i32) -> NeonVectorQ0_15Double {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx0 = unsafe { self.cube0.get_unchecked(offset..) };
        let jx1 = unsafe { self.cube1.get_unchecked(offset..) };
        NeonVectorQ0_15Double {
            v: unsafe {
                vcombine_s16(
                    vld1_s16(jx0.as_ptr() as *const i16),
                    vld1_s16(jx1.as_ptr() as *const i16),
                )
            },
        }
    }
}

pub(crate) trait NeonMdInterpolationQ0_15<'a, const GRID_SIZE: usize> {
    fn new(table: &'a [NeonAlignedI16x4]) -> Self;
    fn inter3_neon<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
    ) -> NeonVectorQ0_15;
}

pub(crate) trait NeonMdInterpolationQ0_15Double<'a, const GRID_SIZE: usize> {
    fn new(table0: &'a [NeonAlignedI16x4], table1: &'a [NeonAlignedI16x4]) -> Self;
    fn inter3_neon<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
    ) -> (NeonVectorQ0_15, NeonVectorQ0_15);
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> TetrahedralNeonQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<NeonVectorQ0_15>,
    ) -> NeonVectorQ0_15 {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let rx = lut_r.w;
        let ry = lut_g.w;
        let rz = lut_b.w;

        let c0 = r.fetch(x, y, z);

        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = r.fetch(x_n, y, z) - c0;
                c2 = r.fetch(x_n, y_n, z) - r.fetch(x_n, y, z);
                c3 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y_n, z);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = r.fetch(x_n, y, z) - c0;
                c2 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y, z_n);
                c3 = r.fetch(x_n, y, z_n) - r.fetch(x_n, y, z);
            } else {
                //rz > rx && rx >= ry
                c1 = r.fetch(x_n, y, z_n) - r.fetch(x, y, z_n);
                c2 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y, z_n);
                c3 = r.fetch(x, y, z_n) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = r.fetch(x_n, y_n, z) - r.fetch(x, y_n, z);
            c2 = r.fetch(x, y_n, z) - c0;
            c3 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y_n, z);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = r.fetch(x_n, y_n, z_n) - r.fetch(x, y_n, z_n);
            c2 = r.fetch(x, y_n, z) - c0;
            c3 = r.fetch(x, y_n, z_n) - r.fetch(x, y_n, z);
        } else {
            //rz > ry && ry > rx
            c1 = r.fetch(x_n, y_n, z_n) - r.fetch(x, y_n, z_n);
            c2 = r.fetch(x, y_n, z_n) - r.fetch(x, y, z_n);
            c3 = r.fetch(x, y, z_n) - c0;
        }
        let s0 = c0.mla(c1, NeonVectorQ0_15::from(rx));
        let s1 = s0.mla(c2, NeonVectorQ0_15::from(ry));
        s1.mla(c3, NeonVectorQ0_15::from(rz))
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> TetrahedralNeonQ0_15Double<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<NeonVectorQ0_15Double>,
    ) -> (NeonVectorQ0_15, NeonVectorQ0_15) {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let rx = lut_r.w;
        let ry = lut_g.w;
        let rz = lut_b.w;

        let c0 = r.fetch(x, y, z);

        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = r.fetch(x_n, y, z) - c0;
                c2 = r.fetch(x_n, y_n, z) - r.fetch(x_n, y, z);
                c3 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y_n, z);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = r.fetch(x_n, y, z) - c0;
                c2 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y, z_n);
                c3 = r.fetch(x_n, y, z_n) - r.fetch(x_n, y, z);
            } else {
                //rz > rx && rx >= ry
                c1 = r.fetch(x_n, y, z_n) - r.fetch(x, y, z_n);
                c2 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y, z_n);
                c3 = r.fetch(x, y, z_n) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = r.fetch(x_n, y_n, z) - r.fetch(x, y_n, z);
            c2 = r.fetch(x, y_n, z) - c0;
            c3 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y_n, z);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = r.fetch(x_n, y_n, z_n) - r.fetch(x, y_n, z_n);
            c2 = r.fetch(x, y_n, z) - c0;
            c3 = r.fetch(x, y_n, z_n) - r.fetch(x, y_n, z);
        } else {
            //rz > ry && ry > rx
            c1 = r.fetch(x_n, y_n, z_n) - r.fetch(x, y_n, z_n);
            c2 = r.fetch(x, y_n, z_n) - r.fetch(x, y, z_n);
            c3 = r.fetch(x, y, z_n) - c0;
        }
        let s0 = c0.mla(c1, NeonVectorQ0_15::from(rx));
        let s1 = s0.mla(c2, NeonVectorQ0_15::from(ry));
        s1.mla(c3, NeonVectorQ0_15::from(rz)).split()
    }
}

macro_rules! define_md_inter_neon {
    ($interpolator: ident) => {
        impl<'a, const GRID_SIZE: usize> NeonMdInterpolationQ0_15<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline(always)]
            fn new(table: &'a [NeonAlignedI16x4]) -> Self {
                Self { cube: table }
            }

            #[inline(always)]
            fn inter3_neon<U: AsPrimitive<usize>, const BINS: usize>(
                &self,
                in_r: U,
                in_g: U,
                in_b: U,
                lut: &[BarycentricWeight<i16>; BINS],
            ) -> NeonVectorQ0_15 {
                self.interpolate(
                    in_r,
                    in_g,
                    in_b,
                    lut,
                    TetrahedralNeonQ0_15FetchVector::<GRID_SIZE> { cube: self.cube },
                )
            }
        }
    };
}

macro_rules! define_md_inter_neon_d {
    ($interpolator: ident) => {
        impl<'a, const GRID_SIZE: usize> NeonMdInterpolationQ0_15Double<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline(always)]
            fn new(table0: &'a [NeonAlignedI16x4], table1: &'a [NeonAlignedI16x4]) -> Self {
                Self {
                    cube0: table0,
                    cube1: table1,
                }
            }

            #[inline(always)]
            fn inter3_neon<U: AsPrimitive<usize>, const BINS: usize>(
                &self,
                in_r: U,
                in_g: U,
                in_b: U,
                lut: &[BarycentricWeight<i16>; BINS],
            ) -> (NeonVectorQ0_15, NeonVectorQ0_15) {
                self.interpolate(
                    in_r,
                    in_g,
                    in_b,
                    lut,
                    TetrahedralNeonQ0_15FetchVectorDouble::<GRID_SIZE> {
                        cube0: self.cube0,
                        cube1: self.cube1,
                    },
                )
            }
        }
    };
}

#[cfg(feature = "options")]
define_md_inter_neon!(TetrahedralNeonQ0_15);
#[cfg(feature = "options")]
define_md_inter_neon!(PyramidalNeonQ0_15);
#[cfg(feature = "options")]
define_md_inter_neon!(PrismaticNeonQ0_15);
define_md_inter_neon!(TrilinearNeonQ0_15);
#[cfg(feature = "options")]
define_md_inter_neon_d!(PrismaticNeonQ0_15Double);
#[cfg(feature = "options")]
define_md_inter_neon_d!(PyramidalNeonQ0_15Double);
#[cfg(feature = "options")]
define_md_inter_neon_d!(TetrahedralNeonQ0_15Double);
define_md_inter_neon_d!(TrilinearNeonQ0_15Double);

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PyramidalNeonQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<NeonVectorQ0_15>,
    ) -> NeonVectorQ0_15 {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let c0 = r.fetch(x, y, z);

        let w0 = NeonVectorQ0_15::from(db);
        let w1 = NeonVectorQ0_15::from(dr);
        let w2 = NeonVectorQ0_15::from(dg);

        if dr > db && dg > db {
            let x0 = r.fetch(x_n, y_n, z_n);
            let x1 = r.fetch(x_n, y_n, z);
            let x2 = r.fetch(x_n, y, z);
            let x3 = r.fetch(x, y_n, z);

            let w3 = w1 * w2;

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3)
        } else if db > dr && dg > dr {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y_n, z_n);
            let x2 = r.fetch(x, y_n, z_n);
            let x3 = r.fetch(x, y_n, z);

            let w3 = w2 * w0;

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3)
        } else {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y, z);
            let x2 = r.fetch(x_n, y, z_n);
            let x3 = r.fetch(x_n, y_n, z_n);

            let w3 = w0 * w1;

            let c1 = x0 - c0;
            let c2 = x1 - c0;
            let c3 = x3 - x2;
            let c4 = c0 - x1 - x0 + x2;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3)
        }
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PyramidalNeonQ0_15Double<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<NeonVectorQ0_15Double>,
    ) -> (NeonVectorQ0_15, NeonVectorQ0_15) {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let c0 = r.fetch(x, y, z);

        let w0 = NeonVectorQ0_15::from(db);
        let w1 = NeonVectorQ0_15::from(dr);
        let w2 = NeonVectorQ0_15::from(dg);

        if dr > db && dg > db {
            let w3 = NeonVectorQ0_15::from(dr) * NeonVectorQ0_15::from(dg);
            let x0 = r.fetch(x_n, y_n, z_n);
            let x1 = r.fetch(x_n, y_n, z);
            let x2 = r.fetch(x_n, y, z);
            let x3 = r.fetch(x, y_n, z);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        } else if db > dr && dg > dr {
            let w3 = NeonVectorQ0_15::from(dg) * NeonVectorQ0_15::from(db);
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y_n, z_n);
            let x2 = r.fetch(x, y_n, z_n);
            let x3 = r.fetch(x, y_n, z);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        } else {
            let w3 = NeonVectorQ0_15::from(db) * NeonVectorQ0_15::from(dr);
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y, z);
            let x2 = r.fetch(x_n, y, z_n);
            let x3 = r.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - c0;
            let c3 = x3 - x2;
            let c4 = c0 - x1 - x0 + x2;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        }
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PrismaticNeonQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<NeonVectorQ0_15>,
    ) -> NeonVectorQ0_15 {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let c0 = r.fetch(x, y, z);

        let w0 = NeonVectorQ0_15::from(db);
        let w1 = NeonVectorQ0_15::from(dr);
        let w2 = NeonVectorQ0_15::from(dg);

        if db > dr {
            let w3 = w2 * w0;
            let w4 = w1 * w2;
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y, z_n);
            let x2 = r.fetch(x, y_n, z);
            let x3 = r.fetch(x, y_n, z_n);
            let x4 = r.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - x0;
            let c3 = x2 - c0;
            let c4 = c0 - x2 - x0 + x3;
            let c5 = x0 - x3 - x1 + x4;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            let s3 = s2.mla(c4, w3);
            s3.mla(c5, w4)
        } else {
            let w3 = w2 * w0;
            let w4 = w1 * w2;
            let x0 = r.fetch(x_n, y, z);
            let x1 = r.fetch(x_n, y, z_n);
            let x2 = r.fetch(x, y_n, z);
            let x3 = r.fetch(x_n, y_n, z);
            let x4 = r.fetch(x_n, y_n, z_n);

            let c1 = x1 - x0;
            let c2 = x0 - c0;
            let c3 = x2 - c0;
            let c4 = x0 - x3 - x1 + x4;
            let c5 = c0 - x2 - x0 + x3;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            let s3 = s2.mla(c4, w3);
            s3.mla(c5, w4)
        }
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PrismaticNeonQ0_15Double<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        rv: impl Fetcher<NeonVectorQ0_15Double>,
    ) -> (NeonVectorQ0_15, NeonVectorQ0_15) {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let c0 = rv.fetch(x, y, z);

        let w0 = NeonVectorQ0_15::from(db);
        let w1 = NeonVectorQ0_15::from(dr);
        let w2 = NeonVectorQ0_15::from(dg);
        let w3 = NeonVectorQ0_15::from(dg) * NeonVectorQ0_15::from(db);
        let w4 = NeonVectorQ0_15::from(dr) * NeonVectorQ0_15::from(dg);

        if db > dr {
            let x0 = rv.fetch(x, y, z_n);
            let x1 = rv.fetch(x_n, y, z_n);
            let x2 = rv.fetch(x, y_n, z);
            let x3 = rv.fetch(x, y_n, z_n);
            let x4 = rv.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - x0;
            let c3 = x2 - c0;
            let c4 = c0 - x2 - x0 + x3;
            let c5 = x0 - x3 - x1 + x4;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            let s3 = s2.mla(c4, w3);
            s3.mla(c5, w4).split()
        } else {
            let x0 = rv.fetch(x_n, y, z);
            let x1 = rv.fetch(x_n, y, z_n);
            let x2 = rv.fetch(x, y_n, z);
            let x3 = rv.fetch(x_n, y_n, z);
            let x4 = rv.fetch(x_n, y_n, z_n);

            let c1 = x1 - x0;
            let c2 = x0 - c0;
            let c3 = x2 - c0;
            let c4 = x0 - x3 - x1 + x4;
            let c5 = c0 - x2 - x0 + x3;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            let s3 = s2.mla(c4, w3);
            s3.mla(c5, w4).split()
        }
    }
}

impl<const GRID_SIZE: usize> TrilinearNeonQ0_15Double<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<NeonVectorQ0_15Double>,
    ) -> (NeonVectorQ0_15, NeonVectorQ0_15) {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let w0 = NeonVectorQ0_15::from(dr);
        let w1 = NeonVectorQ0_15::from(dg);
        let w2 = NeonVectorQ0_15::from(db);

        let c000 = r.fetch(x, y, z);
        let c100 = r.fetch(x_n, y, z);
        let c010 = r.fetch(x, y_n, z);
        let c110 = r.fetch(x_n, y_n, z);
        let c001 = r.fetch(x, y, z_n);
        let c101 = r.fetch(x_n, y, z_n);
        let c011 = r.fetch(x, y_n, z_n);
        let c111 = r.fetch(x_n, y_n, z_n);

        let dx = NeonVectorQ0_15Double::from(dr);

        let c00 = c000.neg_mla(c000, dx).mla(c100, w0);
        let c10 = c010.neg_mla(c010, dx).mla(c110, w0);
        let c01 = c001.neg_mla(c001, dx).mla(c101, w0);
        let c11 = c011.neg_mla(c011, dx).mla(c111, w0);

        let dy = NeonVectorQ0_15Double::from(dg);

        let c0 = c00.neg_mla(c00, dy).mla(c10, w1);
        let c1 = c01.neg_mla(c01, dy).mla(c11, w1);

        let dz = NeonVectorQ0_15Double::from(db);

        c0.neg_mla(c0, dz).mla(c1, w2).split()
    }
}

impl<const GRID_SIZE: usize> TrilinearNeonQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<NeonVectorQ0_15>,
    ) -> NeonVectorQ0_15 {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let w0 = NeonVectorQ0_15::from(dr);
        let w1 = NeonVectorQ0_15::from(dg);
        let w2 = NeonVectorQ0_15::from(db);

        let c000 = r.fetch(x, y, z);
        let c100 = r.fetch(x_n, y, z);
        let c010 = r.fetch(x, y_n, z);
        let c110 = r.fetch(x_n, y_n, z);
        let c001 = r.fetch(x, y, z_n);
        let c101 = r.fetch(x_n, y, z_n);
        let c011 = r.fetch(x, y_n, z_n);
        let c111 = r.fetch(x_n, y_n, z_n);

        let dx = NeonVectorQ0_15::from(dr);

        let c00 = c000.neg_mla(c000, dx).mla(c100, w0);
        let c10 = c010.neg_mla(c010, dx).mla(c110, w0);
        let c01 = c001.neg_mla(c001, dx).mla(c101, w0);
        let c11 = c011.neg_mla(c011, dx).mla(c111, w0);

        let dy = NeonVectorQ0_15::from(dg);

        let c0 = c00.neg_mla(c00, dy).mla(c10, w1);
        let c1 = c01.neg_mla(c01, dy).mla(c11, w1);

        let dz = NeonVectorQ0_15::from(db);

        c0.neg_mla(c0, dz).mla(c1, w2)
    }
}
