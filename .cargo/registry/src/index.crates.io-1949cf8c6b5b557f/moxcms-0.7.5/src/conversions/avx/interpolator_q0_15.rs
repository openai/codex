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
use std::arch::x86_64::*;
use std::ops::{Add, Mul, Sub};

#[repr(align(8), C)]
pub(crate) struct AvxAlignedI16(pub(crate) [i16; 4]);

#[cfg(feature = "options")]
pub(crate) struct TetrahedralAvxQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [AvxAlignedI16],
}

#[cfg(feature = "options")]
pub(crate) struct PyramidalAvxQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [AvxAlignedI16],
}

#[cfg(feature = "options")]
pub(crate) struct PrismaticAvxQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [AvxAlignedI16],
}

pub(crate) struct TrilinearAvxQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [AvxAlignedI16],
}

#[cfg(feature = "options")]
pub(crate) struct PrismaticAvxQ0_15Double<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [AvxAlignedI16],
    pub(crate) cube1: &'a [AvxAlignedI16],
}

pub(crate) struct TrilinearAvxQ0_15Double<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [AvxAlignedI16],
    pub(crate) cube1: &'a [AvxAlignedI16],
}

#[cfg(feature = "options")]
pub(crate) struct PyramidAvxFmaQ0_15Double<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [AvxAlignedI16],
    pub(crate) cube1: &'a [AvxAlignedI16],
}

#[cfg(feature = "options")]
pub(crate) struct TetrahedralAvxQ0_15Double<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [AvxAlignedI16],
    pub(crate) cube1: &'a [AvxAlignedI16],
}

pub(crate) trait AvxMdInterpolationQ0_15Double<'a, const GRID_SIZE: usize> {
    fn new(table0: &'a [AvxAlignedI16], table1: &'a [AvxAlignedI16]) -> Self;
    fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
    ) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse);
}

pub(crate) trait AvxMdInterpolationQ0_15<'a, const GRID_SIZE: usize> {
    fn new(table: &'a [AvxAlignedI16]) -> Self;
    fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
    ) -> AvxVectorQ0_15Sse;
}

trait Fetcher<T> {
    fn fetch(&self, x: i32, y: i32, z: i32) -> T;
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct AvxVectorQ0_15Sse {
    pub(crate) v: __m128i,
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct AvxVectorQ0_15 {
    pub(crate) v: __m256i,
}

impl AvxVectorQ0_15 {
    #[inline(always)]
    pub(crate) fn from_sse(lo: AvxVectorQ0_15Sse, hi: AvxVectorQ0_15Sse) -> AvxVectorQ0_15 {
        unsafe {
            AvxVectorQ0_15 {
                v: _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(lo.v), hi.v),
            }
        }
    }

    #[inline(always)]
    pub(crate) fn split(self) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse) {
        unsafe {
            (
                AvxVectorQ0_15Sse {
                    v: _mm256_castsi256_si128(self.v),
                },
                AvxVectorQ0_15Sse {
                    v: _mm256_extracti128_si256::<1>(self.v),
                },
            )
        }
    }
}

impl From<i16> for AvxVectorQ0_15Sse {
    #[inline(always)]
    fn from(v: i16) -> Self {
        AvxVectorQ0_15Sse {
            v: unsafe { _mm_set1_epi16(v) },
        }
    }
}

impl From<i16> for AvxVectorQ0_15 {
    #[inline(always)]
    fn from(v: i16) -> Self {
        AvxVectorQ0_15 {
            v: unsafe { _mm256_set1_epi16(v) },
        }
    }
}

impl Sub<AvxVectorQ0_15Sse> for AvxVectorQ0_15Sse {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: AvxVectorQ0_15Sse) -> Self::Output {
        AvxVectorQ0_15Sse {
            v: unsafe { _mm_sub_epi16(self.v, rhs.v) },
        }
    }
}

impl Sub<AvxVectorQ0_15> for AvxVectorQ0_15 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: AvxVectorQ0_15) -> Self::Output {
        AvxVectorQ0_15 {
            v: unsafe { _mm256_sub_epi16(self.v, rhs.v) },
        }
    }
}

impl Add<AvxVectorQ0_15Sse> for AvxVectorQ0_15Sse {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: AvxVectorQ0_15Sse) -> Self::Output {
        AvxVectorQ0_15Sse {
            v: unsafe { _mm_add_epi16(self.v, rhs.v) },
        }
    }
}

impl Mul<AvxVectorQ0_15Sse> for AvxVectorQ0_15Sse {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: AvxVectorQ0_15Sse) -> Self::Output {
        AvxVectorQ0_15Sse {
            v: unsafe { _mm_mulhrs_epi16(self.v, rhs.v) },
        }
    }
}

impl Add<AvxVectorQ0_15> for AvxVectorQ0_15 {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: AvxVectorQ0_15) -> Self::Output {
        AvxVectorQ0_15 {
            v: unsafe { _mm256_add_epi16(self.v, rhs.v) },
        }
    }
}

impl Mul<AvxVectorQ0_15> for AvxVectorQ0_15 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: AvxVectorQ0_15) -> Self::Output {
        AvxVectorQ0_15 {
            v: unsafe { _mm256_mulhrs_epi16(self.v, rhs.v) },
        }
    }
}

impl FusedMultiplyAdd<AvxVectorQ0_15Sse> for AvxVectorQ0_15Sse {
    #[inline(always)]
    fn mla(&self, b: AvxVectorQ0_15Sse, c: AvxVectorQ0_15Sse) -> AvxVectorQ0_15Sse {
        AvxVectorQ0_15Sse {
            v: unsafe { _mm_add_epi16(_mm_mulhrs_epi16(b.v, c.v), self.v) },
        }
    }
}

impl FusedMultiplyAdd<AvxVectorQ0_15> for AvxVectorQ0_15 {
    #[inline(always)]
    fn mla(&self, b: AvxVectorQ0_15, c: AvxVectorQ0_15) -> AvxVectorQ0_15 {
        AvxVectorQ0_15 {
            v: unsafe { _mm256_add_epi16(_mm256_mulhrs_epi16(b.v, c.v), self.v) },
        }
    }
}

struct TetrahedralAvxSseFetchVector<'a, const GRID_SIZE: usize> {
    cube: &'a [AvxAlignedI16],
}

struct TetrahedralAvxFetchVector<'a, const GRID_SIZE: usize> {
    cube0: &'a [AvxAlignedI16],
    cube1: &'a [AvxAlignedI16],
}

impl<const GRID_SIZE: usize> Fetcher<AvxVectorQ0_15> for TetrahedralAvxFetchVector<'_, GRID_SIZE> {
    #[inline(always)]
    fn fetch(&self, x: i32, y: i32, z: i32) -> AvxVectorQ0_15 {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx0 = unsafe { self.cube0.get_unchecked(offset..) };
        let jx1 = unsafe { self.cube1.get_unchecked(offset..) };
        AvxVectorQ0_15 {
            v: unsafe {
                _mm256_inserti128_si256::<1>(
                    _mm256_castsi128_si256(_mm_loadu_si64(jx0.as_ptr() as *const _)),
                    _mm_loadu_si64(jx1.as_ptr() as *const _),
                )
            },
        }
    }
}

impl<const GRID_SIZE: usize> Fetcher<AvxVectorQ0_15Sse>
    for TetrahedralAvxSseFetchVector<'_, GRID_SIZE>
{
    #[inline(always)]
    fn fetch(&self, x: i32, y: i32, z: i32) -> AvxVectorQ0_15Sse {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx = unsafe { self.cube.get_unchecked(offset..) };
        AvxVectorQ0_15Sse {
            v: unsafe { _mm_loadu_si64(jx.as_ptr() as *const _) },
        }
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> TetrahedralAvxQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<AvxVectorQ0_15Sse>,
    ) -> AvxVectorQ0_15Sse {
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
        let s0 = c0.mla(c1, AvxVectorQ0_15Sse::from(rx));
        let s1 = s0.mla(c2, AvxVectorQ0_15Sse::from(ry));
        s1.mla(c3, AvxVectorQ0_15Sse::from(rz))
    }
}

macro_rules! define_interp_avx {
    ($interpolator: ident) => {
        impl<'a, const GRID_SIZE: usize> AvxMdInterpolationQ0_15<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline(always)]
            fn new(table: &'a [AvxAlignedI16]) -> Self {
                Self { cube: table }
            }

            #[inline(always)]
            fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
                &self,
                in_r: U,
                in_g: U,
                in_b: U,
                lut: &[BarycentricWeight<i16>; BINS],
            ) -> AvxVectorQ0_15Sse {
                self.interpolate(
                    in_r,
                    in_g,
                    in_b,
                    lut,
                    TetrahedralAvxSseFetchVector::<GRID_SIZE> { cube: self.cube },
                )
            }
        }
    };
}

#[cfg(feature = "options")]
macro_rules! define_interp_avx_d {
    ($interpolator: ident) => {
        impl<'a, const GRID_SIZE: usize> AvxMdInterpolationQ0_15Double<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline(always)]
            fn new(table0: &'a [AvxAlignedI16], table1: &'a [AvxAlignedI16]) -> Self {
                Self {
                    cube0: table0,
                    cube1: table1,
                }
            }

            #[inline(always)]
            fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
                &self,
                in_r: U,
                in_g: U,
                in_b: U,
                lut: &[BarycentricWeight<i16>; BINS],
            ) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse) {
                self.interpolate(
                    in_r,
                    in_g,
                    in_b,
                    lut,
                    TetrahedralAvxSseFetchVector::<GRID_SIZE> { cube: self.cube0 },
                    TetrahedralAvxSseFetchVector::<GRID_SIZE> { cube: self.cube1 },
                )
            }
        }
    };
}

#[cfg(feature = "options")]
define_interp_avx!(TetrahedralAvxQ0_15);
#[cfg(feature = "options")]
define_interp_avx!(PyramidalAvxQ0_15);
#[cfg(feature = "options")]
define_interp_avx!(PrismaticAvxQ0_15);
define_interp_avx!(TrilinearAvxQ0_15);
#[cfg(feature = "options")]
define_interp_avx_d!(PrismaticAvxQ0_15Double);
#[cfg(feature = "options")]
define_interp_avx_d!(PyramidAvxFmaQ0_15Double);

#[cfg(feature = "options")]
impl<'a, const GRID_SIZE: usize> AvxMdInterpolationQ0_15Double<'a, GRID_SIZE>
    for TetrahedralAvxQ0_15Double<'a, GRID_SIZE>
{
    #[inline(always)]
    fn new(table0: &'a [AvxAlignedI16], table1: &'a [AvxAlignedI16]) -> Self {
        Self {
            cube0: table0,
            cube1: table1,
        }
    }

    #[inline(always)]
    fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
    ) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse) {
        self.interpolate(
            in_r,
            in_g,
            in_b,
            lut,
            TetrahedralAvxFetchVector::<GRID_SIZE> {
                cube0: self.cube0,
                cube1: self.cube1,
            },
        )
    }
}

impl<'a, const GRID_SIZE: usize> AvxMdInterpolationQ0_15Double<'a, GRID_SIZE>
    for TrilinearAvxQ0_15Double<'a, GRID_SIZE>
{
    #[inline(always)]
    fn new(table0: &'a [AvxAlignedI16], table1: &'a [AvxAlignedI16]) -> Self {
        Self {
            cube0: table0,
            cube1: table1,
        }
    }

    #[inline(always)]
    fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
    ) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse) {
        self.interpolate(
            in_r,
            in_g,
            in_b,
            lut,
            TetrahedralAvxFetchVector::<GRID_SIZE> {
                cube0: self.cube0,
                cube1: self.cube1,
            },
        )
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PyramidalAvxQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<AvxVectorQ0_15Sse>,
    ) -> AvxVectorQ0_15Sse {
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

        let w0 = AvxVectorQ0_15Sse::from(db);
        let w1 = AvxVectorQ0_15Sse::from(dr);
        let w2 = AvxVectorQ0_15Sse::from(dg);

        if dr > db && dg > db {
            let w3 = AvxVectorQ0_15Sse::from(dr) * AvxVectorQ0_15Sse::from(dg);
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
            s2.mla(c4, w3)
        } else if db > dr && dg > dr {
            let w3 = AvxVectorQ0_15Sse::from(dg) * AvxVectorQ0_15Sse::from(db);

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
            s2.mla(c4, w3)
        } else {
            let w3 = AvxVectorQ0_15Sse::from(db) * AvxVectorQ0_15Sse::from(dr);

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
            s2.mla(c4, w3)
        }
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PrismaticAvxQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<AvxVectorQ0_15Sse>,
    ) -> AvxVectorQ0_15Sse {
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

        let w0 = AvxVectorQ0_15Sse::from(db);
        let w1 = AvxVectorQ0_15Sse::from(dr);
        let w2 = AvxVectorQ0_15Sse::from(dg);
        let w3 = AvxVectorQ0_15Sse::from(dg) * AvxVectorQ0_15Sse::from(db);
        let w4 = AvxVectorQ0_15Sse::from(dr) * AvxVectorQ0_15Sse::from(dg);

        if db > dr {
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
impl<const GRID_SIZE: usize> PrismaticAvxQ0_15Double<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r0: impl Fetcher<AvxVectorQ0_15Sse>,
        r1: impl Fetcher<AvxVectorQ0_15Sse>,
    ) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse) {
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

        let c0_0 = r0.fetch(x, y, z);
        let c0_1 = r0.fetch(x, y, z);

        let w0 = AvxVectorQ0_15::from(db);
        let w1 = AvxVectorQ0_15::from(dr);
        let w2 = AvxVectorQ0_15::from(dg);
        let w3 = AvxVectorQ0_15::from(dg) * AvxVectorQ0_15::from(db);
        let w4 = AvxVectorQ0_15::from(dr) * AvxVectorQ0_15::from(dg);

        let c0 = AvxVectorQ0_15::from_sse(c0_0, c0_1);

        if db > dr {
            let x0_0 = r0.fetch(x, y, z_n);
            let x1_0 = r0.fetch(x_n, y, z_n);
            let x2_0 = r0.fetch(x, y_n, z);
            let x3_0 = r0.fetch(x, y_n, z_n);
            let x4_0 = r0.fetch(x_n, y_n, z_n);

            let x0_1 = r1.fetch(x, y, z_n);
            let x1_1 = r1.fetch(x_n, y, z_n);
            let x2_1 = r1.fetch(x, y_n, z);
            let x3_1 = r1.fetch(x, y_n, z_n);
            let x4_1 = r1.fetch(x_n, y_n, z_n);

            let x0 = AvxVectorQ0_15::from_sse(x0_0, x0_1);
            let x1 = AvxVectorQ0_15::from_sse(x1_0, x1_1);
            let x2 = AvxVectorQ0_15::from_sse(x2_0, x2_1);
            let x3 = AvxVectorQ0_15::from_sse(x3_0, x3_1);
            let x4 = AvxVectorQ0_15::from_sse(x4_0, x4_1);

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
            let x0_0 = r0.fetch(x_n, y, z);
            let x1_0 = r0.fetch(x_n, y, z_n);
            let x2_0 = r0.fetch(x, y_n, z);
            let x3_0 = r0.fetch(x_n, y_n, z);
            let x4_0 = r0.fetch(x_n, y_n, z_n);

            let x0_1 = r1.fetch(x_n, y, z);
            let x1_1 = r1.fetch(x_n, y, z_n);
            let x2_1 = r1.fetch(x, y_n, z);
            let x3_1 = r1.fetch(x_n, y_n, z);
            let x4_1 = r1.fetch(x_n, y_n, z_n);

            let x0 = AvxVectorQ0_15::from_sse(x0_0, x0_1);
            let x1 = AvxVectorQ0_15::from_sse(x1_0, x1_1);
            let x2 = AvxVectorQ0_15::from_sse(x2_0, x2_1);
            let x3 = AvxVectorQ0_15::from_sse(x3_0, x3_1);
            let x4 = AvxVectorQ0_15::from_sse(x4_0, x4_1);

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

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PyramidAvxFmaQ0_15Double<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r0: impl Fetcher<AvxVectorQ0_15Sse>,
        r1: impl Fetcher<AvxVectorQ0_15Sse>,
    ) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse) {
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

        let c0_0 = r0.fetch(x, y, z);
        let c0_1 = r1.fetch(x, y, z);

        let w0 = AvxVectorQ0_15::from(db);
        let w1 = AvxVectorQ0_15::from(dr);
        let w2 = AvxVectorQ0_15::from(dg);

        let c0 = AvxVectorQ0_15::from_sse(c0_0, c0_1);

        if dr > db && dg > db {
            let w3 = AvxVectorQ0_15::from(dr) * AvxVectorQ0_15::from(dg);

            let x0_0 = r0.fetch(x_n, y_n, z_n);
            let x1_0 = r0.fetch(x_n, y_n, z);
            let x2_0 = r0.fetch(x_n, y, z);
            let x3_0 = r0.fetch(x, y_n, z);

            let x0_1 = r1.fetch(x_n, y_n, z_n);
            let x1_1 = r1.fetch(x_n, y_n, z);
            let x2_1 = r1.fetch(x_n, y, z);
            let x3_1 = r1.fetch(x, y_n, z);

            let x0 = AvxVectorQ0_15::from_sse(x0_0, x0_1);
            let x1 = AvxVectorQ0_15::from_sse(x1_0, x1_1);
            let x2 = AvxVectorQ0_15::from_sse(x2_0, x2_1);
            let x3 = AvxVectorQ0_15::from_sse(x3_0, x3_1);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        } else if db > dr && dg > dr {
            let w3 = AvxVectorQ0_15::from(dg) * AvxVectorQ0_15::from(db);

            let x0_0 = r0.fetch(x, y, z_n);
            let x1_0 = r0.fetch(x_n, y_n, z_n);
            let x2_0 = r0.fetch(x, y_n, z_n);
            let x3_0 = r0.fetch(x, y_n, z);

            let x0_1 = r1.fetch(x, y, z_n);
            let x1_1 = r1.fetch(x_n, y_n, z_n);
            let x2_1 = r1.fetch(x, y_n, z_n);
            let x3_1 = r1.fetch(x, y_n, z);

            let x0 = AvxVectorQ0_15::from_sse(x0_0, x0_1);
            let x1 = AvxVectorQ0_15::from_sse(x1_0, x1_1);
            let x2 = AvxVectorQ0_15::from_sse(x2_0, x2_1);
            let x3 = AvxVectorQ0_15::from_sse(x3_0, x3_1);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        } else {
            let w3 = AvxVectorQ0_15::from(db) * AvxVectorQ0_15::from(dr);

            let x0_0 = r0.fetch(x, y, z_n);
            let x1_0 = r0.fetch(x_n, y, z);
            let x2_0 = r0.fetch(x_n, y, z_n);
            let x3_0 = r0.fetch(x_n, y_n, z_n);

            let x0_1 = r1.fetch(x, y, z_n);
            let x1_1 = r1.fetch(x_n, y, z);
            let x2_1 = r1.fetch(x_n, y, z_n);
            let x3_1 = r1.fetch(x_n, y_n, z_n);

            let x0 = AvxVectorQ0_15::from_sse(x0_0, x0_1);
            let x1 = AvxVectorQ0_15::from_sse(x1_0, x1_1);
            let x2 = AvxVectorQ0_15::from_sse(x2_0, x2_1);
            let x3 = AvxVectorQ0_15::from_sse(x3_0, x3_1);

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
impl<const GRID_SIZE: usize> TetrahedralAvxQ0_15Double<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        rv: impl Fetcher<AvxVectorQ0_15>,
    ) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse) {
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

        let c0 = rv.fetch(x, y, z);

        let w0 = AvxVectorQ0_15::from(rx);
        let w1 = AvxVectorQ0_15::from(ry);
        let w2 = AvxVectorQ0_15::from(rz);

        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = rv.fetch(x_n, y, z) - c0;
                c2 = rv.fetch(x_n, y_n, z) - rv.fetch(x_n, y, z);
                c3 = rv.fetch(x_n, y_n, z_n) - rv.fetch(x_n, y_n, z);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = rv.fetch(x_n, y, z) - c0;
                c2 = rv.fetch(x_n, y_n, z_n) - rv.fetch(x_n, y, z_n);
                c3 = rv.fetch(x_n, y, z_n) - rv.fetch(x_n, y, z);
            } else {
                //rz > rx && rx >= ry
                c1 = rv.fetch(x_n, y, z_n) - rv.fetch(x, y, z_n);
                c2 = rv.fetch(x_n, y_n, z_n) - rv.fetch(x_n, y, z_n);
                c3 = rv.fetch(x, y, z_n) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = rv.fetch(x_n, y_n, z) - rv.fetch(x, y_n, z);
            c2 = rv.fetch(x, y_n, z) - c0;
            c3 = rv.fetch(x_n, y_n, z_n) - rv.fetch(x_n, y_n, z);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = rv.fetch(x_n, y_n, z_n) - rv.fetch(x, y_n, z_n);
            c2 = rv.fetch(x, y_n, z) - c0;
            c3 = rv.fetch(x, y_n, z_n) - rv.fetch(x, y_n, z);
        } else {
            //rz > ry && ry > rx
            c1 = rv.fetch(x_n, y_n, z_n) - rv.fetch(x, y_n, z_n);
            c2 = rv.fetch(x, y_n, z_n) - rv.fetch(x, y, z_n);
            c3 = rv.fetch(x, y, z_n) - c0;
        }
        let s0 = c0.mla(c1, w0);
        let s1 = s0.mla(c2, w1);
        s1.mla(c3, w2).split()
    }
}

impl<const GRID_SIZE: usize> TrilinearAvxQ0_15Double<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        rv: impl Fetcher<AvxVectorQ0_15>,
    ) -> (AvxVectorQ0_15Sse, AvxVectorQ0_15Sse) {
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

        const Q_MAX: i16 = ((1i32 << 15i32) - 1) as i16;

        let q_max = AvxVectorQ0_15::from(Q_MAX);
        let w0 = AvxVectorQ0_15::from(rx);
        let w1 = AvxVectorQ0_15::from(ry);
        let w2 = AvxVectorQ0_15::from(rz);
        let dx = q_max - w0;
        let dy = q_max - w1;
        let dz = q_max - w2;

        let c000 = rv.fetch(x, y, z);
        let c100 = rv.fetch(x_n, y, z);
        let c010 = rv.fetch(x, y_n, z);
        let c110 = rv.fetch(x_n, y_n, z);
        let c001 = rv.fetch(x, y, z_n);
        let c101 = rv.fetch(x_n, y, z_n);
        let c011 = rv.fetch(x, y_n, z_n);
        let c111 = rv.fetch(x_n, y_n, z_n);

        let c00 = (c000 * dx).mla(c100, w0);
        let c10 = (c010 * dx).mla(c110, w0);
        let c01 = (c001 * dx).mla(c101, w0);
        let c11 = (c011 * dx).mla(c111, w0);

        let c0 = (c00 * dy).mla(c10, w1);
        let c1 = (c01 * dy).mla(c11, w1);

        (c0 * dz).mla(c1, w2).split()
    }
}

impl<const GRID_SIZE: usize> TrilinearAvxQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<AvxVectorQ0_15Sse>,
    ) -> AvxVectorQ0_15Sse {
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

        const Q_MAX: i16 = ((1i32 << 15i32) - 1) as i16;

        let q_max = AvxVectorQ0_15Sse::from(Q_MAX);
        let q_max_avx = AvxVectorQ0_15::from(Q_MAX);
        let w0 = AvxVectorQ0_15::from(dr);
        let w1 = AvxVectorQ0_15::from(dg);
        let w2 = AvxVectorQ0_15Sse::from(db);
        let dx = q_max_avx - w0;
        let dy = q_max_avx - w1;
        let dz = q_max - w2;

        let c000 = r.fetch(x, y, z);
        let c100 = r.fetch(x_n, y, z);
        let c010 = r.fetch(x, y_n, z);
        let c110 = r.fetch(x_n, y_n, z);
        let c001 = r.fetch(x, y, z_n);
        let c101 = r.fetch(x_n, y, z_n);
        let c011 = r.fetch(x, y_n, z_n);
        let c111 = r.fetch(x_n, y_n, z_n);

        let x000 = AvxVectorQ0_15::from_sse(c000, c001);
        let x010 = AvxVectorQ0_15::from_sse(c010, c011);
        let x011 = AvxVectorQ0_15::from_sse(c100, c101);
        let x111 = AvxVectorQ0_15::from_sse(c110, c111);

        let c00 = (x000 * dx).mla(x011, w0);
        let c10 = (x010 * dx).mla(x111, w0);

        let c0 = (c00 * dy).mla(c10, w1);

        let (c0, c1) = c0.split();

        (c0 * dz).mla(c1, w2)
    }
}
