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
use crate::math::{FusedMultiplyAdd, FusedMultiplyNegAdd};
use num_traits::AsPrimitive;
use std::arch::x86_64::*;
use std::ops::{Add, Mul, Sub};

#[repr(align(16), C)]
pub(crate) struct SseAlignedF32(pub(crate) [f32; 4]);

#[cfg(feature = "options")]
pub(crate) struct TetrahedralAvxFma<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [SseAlignedF32],
}

#[cfg(feature = "options")]
pub(crate) struct PyramidalAvxFma<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [SseAlignedF32],
}

#[cfg(feature = "options")]
pub(crate) struct PrismaticAvxFma<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [SseAlignedF32],
}

pub(crate) struct TrilinearAvxFma<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [SseAlignedF32],
}

#[cfg(feature = "options")]
pub(crate) struct PrismaticAvxFmaDouble<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [SseAlignedF32],
    pub(crate) cube1: &'a [SseAlignedF32],
}

pub(crate) struct TrilinearAvxFmaDouble<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [SseAlignedF32],
    pub(crate) cube1: &'a [SseAlignedF32],
}

#[cfg(feature = "options")]
pub(crate) struct PyramidAvxFmaDouble<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [SseAlignedF32],
    pub(crate) cube1: &'a [SseAlignedF32],
}

#[cfg(feature = "options")]
pub(crate) struct TetrahedralAvxFmaDouble<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [SseAlignedF32],
    pub(crate) cube1: &'a [SseAlignedF32],
}

pub(crate) trait AvxMdInterpolationDouble<'a, const GRID_SIZE: usize> {
    fn new(table0: &'a [SseAlignedF32], table1: &'a [SseAlignedF32]) -> Self;
    fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
    ) -> (AvxVectorSse, AvxVectorSse);
}

pub(crate) trait AvxMdInterpolation<'a, const GRID_SIZE: usize> {
    fn new(table: &'a [SseAlignedF32]) -> Self;
    fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
    ) -> AvxVectorSse;
}

trait Fetcher<T> {
    fn fetch(&self, x: i32, y: i32, z: i32) -> T;
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct AvxVectorSse {
    pub(crate) v: __m128,
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct AvxVector {
    pub(crate) v: __m256,
}

impl AvxVector {
    #[inline(always)]
    pub(crate) fn from_sse(lo: AvxVectorSse, hi: AvxVectorSse) -> AvxVector {
        unsafe {
            AvxVector {
                v: _mm256_insertf128_ps::<1>(_mm256_castps128_ps256(lo.v), hi.v),
            }
        }
    }

    #[inline(always)]
    pub(crate) fn split(self) -> (AvxVectorSse, AvxVectorSse) {
        unsafe {
            (
                AvxVectorSse {
                    v: _mm256_castps256_ps128(self.v),
                },
                AvxVectorSse {
                    v: _mm256_extractf128_ps::<1>(self.v),
                },
            )
        }
    }
}

impl From<f32> for AvxVectorSse {
    #[inline(always)]
    fn from(v: f32) -> Self {
        AvxVectorSse {
            v: unsafe { _mm_set1_ps(v) },
        }
    }
}

impl From<f32> for AvxVector {
    #[inline(always)]
    fn from(v: f32) -> Self {
        AvxVector {
            v: unsafe { _mm256_set1_ps(v) },
        }
    }
}

impl Sub<AvxVectorSse> for AvxVectorSse {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: AvxVectorSse) -> Self::Output {
        AvxVectorSse {
            v: unsafe { _mm_sub_ps(self.v, rhs.v) },
        }
    }
}

impl Sub<AvxVector> for AvxVector {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: AvxVector) -> Self::Output {
        AvxVector {
            v: unsafe { _mm256_sub_ps(self.v, rhs.v) },
        }
    }
}

impl Add<AvxVectorSse> for AvxVectorSse {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: AvxVectorSse) -> Self::Output {
        AvxVectorSse {
            v: unsafe { _mm_add_ps(self.v, rhs.v) },
        }
    }
}

impl Mul<AvxVectorSse> for AvxVectorSse {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: AvxVectorSse) -> Self::Output {
        AvxVectorSse {
            v: unsafe { _mm_mul_ps(self.v, rhs.v) },
        }
    }
}

impl AvxVector {
    #[inline(always)]
    pub(crate) fn neg_mla(self, b: AvxVector, c: AvxVector) -> Self {
        Self {
            v: unsafe { _mm256_fnmadd_ps(b.v, c.v, self.v) },
        }
    }
}

impl FusedMultiplyNegAdd<AvxVectorSse> for AvxVectorSse {
    #[inline(always)]
    fn neg_mla(&self, b: AvxVectorSse, c: AvxVectorSse) -> Self {
        Self {
            v: unsafe { _mm_fnmadd_ps(b.v, c.v, self.v) },
        }
    }
}

impl Add<AvxVector> for AvxVector {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: AvxVector) -> Self::Output {
        AvxVector {
            v: unsafe { _mm256_add_ps(self.v, rhs.v) },
        }
    }
}

impl Mul<AvxVector> for AvxVector {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: AvxVector) -> Self::Output {
        AvxVector {
            v: unsafe { _mm256_mul_ps(self.v, rhs.v) },
        }
    }
}

impl FusedMultiplyAdd<AvxVectorSse> for AvxVectorSse {
    #[inline(always)]
    fn mla(&self, b: AvxVectorSse, c: AvxVectorSse) -> AvxVectorSse {
        AvxVectorSse {
            v: unsafe { _mm_fmadd_ps(b.v, c.v, self.v) },
        }
    }
}

impl FusedMultiplyAdd<AvxVector> for AvxVector {
    #[inline(always)]
    fn mla(&self, b: AvxVector, c: AvxVector) -> AvxVector {
        AvxVector {
            v: unsafe { _mm256_fmadd_ps(b.v, c.v, self.v) },
        }
    }
}

struct TetrahedralAvxSseFetchVector<'a, const GRID_SIZE: usize> {
    cube: &'a [SseAlignedF32],
}

struct TetrahedralAvxFetchVector<'a, const GRID_SIZE: usize> {
    cube0: &'a [SseAlignedF32],
    cube1: &'a [SseAlignedF32],
}

impl<const GRID_SIZE: usize> Fetcher<AvxVector> for TetrahedralAvxFetchVector<'_, GRID_SIZE> {
    #[inline(always)]
    fn fetch(&self, x: i32, y: i32, z: i32) -> AvxVector {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx0 = unsafe { self.cube0.get_unchecked(offset..) };
        let jx1 = unsafe { self.cube1.get_unchecked(offset..) };
        AvxVector {
            v: unsafe {
                _mm256_insertf128_ps::<1>(
                    _mm256_castps128_ps256(_mm_load_ps(jx0.as_ptr() as *const f32)),
                    _mm_load_ps(jx1.as_ptr() as *const f32),
                )
            },
        }
    }
}

impl<const GRID_SIZE: usize> Fetcher<AvxVectorSse> for TetrahedralAvxSseFetchVector<'_, GRID_SIZE> {
    #[inline(always)]
    fn fetch(&self, x: i32, y: i32, z: i32) -> AvxVectorSse {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx = unsafe { self.cube.get_unchecked(offset..) };
        AvxVectorSse {
            v: unsafe { _mm_load_ps(jx.as_ptr() as *const f32) },
        }
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> TetrahedralAvxFma<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<AvxVectorSse>,
    ) -> AvxVectorSse {
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
        let s0 = c0.mla(c1, AvxVectorSse::from(rx));
        let s1 = s0.mla(c2, AvxVectorSse::from(ry));
        s1.mla(c3, AvxVectorSse::from(rz))
    }
}

macro_rules! define_interp_avx {
    ($interpolator: ident) => {
        impl<'a, const GRID_SIZE: usize> AvxMdInterpolation<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline(always)]
            fn new(table: &'a [SseAlignedF32]) -> Self {
                Self { cube: table }
            }

            #[inline(always)]
            fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
                &self,
                in_r: U,
                in_g: U,
                in_b: U,
                lut: &[BarycentricWeight<f32>; BINS],
            ) -> AvxVectorSse {
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
        impl<'a, const GRID_SIZE: usize> AvxMdInterpolationDouble<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline(always)]
            fn new(table0: &'a [SseAlignedF32], table1: &'a [SseAlignedF32]) -> Self {
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
                lut: &[BarycentricWeight<f32>; BINS],
            ) -> (AvxVectorSse, AvxVectorSse) {
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
define_interp_avx!(TetrahedralAvxFma);
#[cfg(feature = "options")]
define_interp_avx!(PyramidalAvxFma);
#[cfg(feature = "options")]
define_interp_avx!(PrismaticAvxFma);
define_interp_avx!(TrilinearAvxFma);
#[cfg(feature = "options")]
define_interp_avx_d!(PrismaticAvxFmaDouble);
#[cfg(feature = "options")]
define_interp_avx_d!(PyramidAvxFmaDouble);

#[cfg(feature = "options")]
impl<'a, const GRID_SIZE: usize> AvxMdInterpolationDouble<'a, GRID_SIZE>
    for TetrahedralAvxFmaDouble<'a, GRID_SIZE>
{
    #[inline(always)]
    fn new(table0: &'a [SseAlignedF32], table1: &'a [SseAlignedF32]) -> Self {
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
        lut: &[BarycentricWeight<f32>; BINS],
    ) -> (AvxVectorSse, AvxVectorSse) {
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

impl<'a, const GRID_SIZE: usize> AvxMdInterpolationDouble<'a, GRID_SIZE>
    for TrilinearAvxFmaDouble<'a, GRID_SIZE>
{
    #[inline(always)]
    fn new(table0: &'a [SseAlignedF32], table1: &'a [SseAlignedF32]) -> Self {
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
        lut: &[BarycentricWeight<f32>; BINS],
    ) -> (AvxVectorSse, AvxVectorSse) {
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
impl<const GRID_SIZE: usize> PyramidalAvxFma<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<AvxVectorSse>,
    ) -> AvxVectorSse {
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

        let w0 = AvxVectorSse::from(db);
        let w1 = AvxVectorSse::from(dr);
        let w2 = AvxVectorSse::from(dg);

        if dr > db && dg > db {
            let w3 = AvxVectorSse::from(dr * dg);
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
            let w3 = AvxVectorSse::from(dg * db);

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
            let w3 = AvxVectorSse::from(db * dr);

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
impl<const GRID_SIZE: usize> PrismaticAvxFma<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<AvxVectorSse>,
    ) -> AvxVectorSse {
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

        let w0 = AvxVectorSse::from(db);
        let w1 = AvxVectorSse::from(dr);
        let w2 = AvxVectorSse::from(dg);
        let w3 = AvxVectorSse::from(dg * db);
        let w4 = AvxVectorSse::from(dr * dg);

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
impl<const GRID_SIZE: usize> PrismaticAvxFmaDouble<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r0: impl Fetcher<AvxVectorSse>,
        r1: impl Fetcher<AvxVectorSse>,
    ) -> (AvxVectorSse, AvxVectorSse) {
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

        let w0 = AvxVector::from(db);
        let w1 = AvxVector::from(dr);
        let w2 = AvxVector::from(dg);
        let w3 = AvxVector::from(dg * db);
        let w4 = AvxVector::from(dr * dg);

        let c0 = AvxVector::from_sse(c0_0, c0_1);

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

            let x0 = AvxVector::from_sse(x0_0, x0_1);
            let x1 = AvxVector::from_sse(x1_0, x1_1);
            let x2 = AvxVector::from_sse(x2_0, x2_1);
            let x3 = AvxVector::from_sse(x3_0, x3_1);
            let x4 = AvxVector::from_sse(x4_0, x4_1);

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

            let x0 = AvxVector::from_sse(x0_0, x0_1);
            let x1 = AvxVector::from_sse(x1_0, x1_1);
            let x2 = AvxVector::from_sse(x2_0, x2_1);
            let x3 = AvxVector::from_sse(x3_0, x3_1);
            let x4 = AvxVector::from_sse(x4_0, x4_1);

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
impl<const GRID_SIZE: usize> PyramidAvxFmaDouble<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r0: impl Fetcher<AvxVectorSse>,
        r1: impl Fetcher<AvxVectorSse>,
    ) -> (AvxVectorSse, AvxVectorSse) {
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

        let w0 = AvxVector::from(db);
        let w1 = AvxVector::from(dr);
        let w2 = AvxVector::from(dg);

        let c0 = AvxVector::from_sse(c0_0, c0_1);

        if dr > db && dg > db {
            let w3 = AvxVector::from(dr * dg);

            let x0_0 = r0.fetch(x_n, y_n, z_n);
            let x1_0 = r0.fetch(x_n, y_n, z);
            let x2_0 = r0.fetch(x_n, y, z);
            let x3_0 = r0.fetch(x, y_n, z);

            let x0_1 = r1.fetch(x_n, y_n, z_n);
            let x1_1 = r1.fetch(x_n, y_n, z);
            let x2_1 = r1.fetch(x_n, y, z);
            let x3_1 = r1.fetch(x, y_n, z);

            let x0 = AvxVector::from_sse(x0_0, x0_1);
            let x1 = AvxVector::from_sse(x1_0, x1_1);
            let x2 = AvxVector::from_sse(x2_0, x2_1);
            let x3 = AvxVector::from_sse(x3_0, x3_1);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        } else if db > dr && dg > dr {
            let w3 = AvxVector::from(dg * db);

            let x0_0 = r0.fetch(x, y, z_n);
            let x1_0 = r0.fetch(x_n, y_n, z_n);
            let x2_0 = r0.fetch(x, y_n, z_n);
            let x3_0 = r0.fetch(x, y_n, z);

            let x0_1 = r1.fetch(x, y, z_n);
            let x1_1 = r1.fetch(x_n, y_n, z_n);
            let x2_1 = r1.fetch(x, y_n, z_n);
            let x3_1 = r1.fetch(x, y_n, z);

            let x0 = AvxVector::from_sse(x0_0, x0_1);
            let x1 = AvxVector::from_sse(x1_0, x1_1);
            let x2 = AvxVector::from_sse(x2_0, x2_1);
            let x3 = AvxVector::from_sse(x3_0, x3_1);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        } else {
            let w3 = AvxVector::from(db * dr);

            let x0_0 = r0.fetch(x, y, z_n);
            let x1_0 = r0.fetch(x_n, y, z);
            let x2_0 = r0.fetch(x_n, y, z_n);
            let x3_0 = r0.fetch(x_n, y_n, z_n);

            let x0_1 = r1.fetch(x, y, z_n);
            let x1_1 = r1.fetch(x_n, y, z);
            let x2_1 = r1.fetch(x_n, y, z_n);
            let x3_1 = r1.fetch(x_n, y_n, z_n);

            let x0 = AvxVector::from_sse(x0_0, x0_1);
            let x1 = AvxVector::from_sse(x1_0, x1_1);
            let x2 = AvxVector::from_sse(x2_0, x2_1);
            let x3 = AvxVector::from_sse(x3_0, x3_1);

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
impl<const GRID_SIZE: usize> TetrahedralAvxFmaDouble<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        rv: impl Fetcher<AvxVector>,
    ) -> (AvxVectorSse, AvxVectorSse) {
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

        let w0 = AvxVector::from(rx);
        let w1 = AvxVector::from(ry);
        let w2 = AvxVector::from(rz);

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

impl<const GRID_SIZE: usize> TrilinearAvxFmaDouble<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        rv: impl Fetcher<AvxVector>,
    ) -> (AvxVectorSse, AvxVectorSse) {
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

        let w0 = AvxVector::from(rx);
        let w1 = AvxVector::from(ry);
        let w2 = AvxVector::from(rz);

        let c000 = rv.fetch(x, y, z);
        let c100 = rv.fetch(x_n, y, z);
        let c010 = rv.fetch(x, y_n, z);
        let c110 = rv.fetch(x_n, y_n, z);
        let c001 = rv.fetch(x, y, z_n);
        let c101 = rv.fetch(x_n, y, z_n);
        let c011 = rv.fetch(x, y_n, z_n);
        let c111 = rv.fetch(x_n, y_n, z_n);

        let dx = AvxVector::from(rx);

        let c00 = c000.neg_mla(c000, dx).mla(c100, w0);
        let c10 = c010.neg_mla(c010, dx).mla(c110, w0);
        let c01 = c001.neg_mla(c001, dx).mla(c101, w0);
        let c11 = c011.neg_mla(c011, dx).mla(c111, w0);

        let dy = AvxVector::from(ry);

        let c0 = c00.neg_mla(c00, dy).mla(c10, w1);
        let c1 = c01.neg_mla(c01, dy).mla(c11, w1);

        let dz = AvxVector::from(rz);

        c0.neg_mla(c0, dz).mla(c1, w2).split()
    }
}

impl<const GRID_SIZE: usize> TrilinearAvxFma<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<AvxVectorSse>,
    ) -> AvxVectorSse {
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

        let w0 = AvxVector::from(dr);
        let w1 = AvxVector::from(dg);
        let w2 = AvxVectorSse::from(db);

        let c000 = r.fetch(x, y, z);
        let c100 = r.fetch(x_n, y, z);
        let c010 = r.fetch(x, y_n, z);
        let c110 = r.fetch(x_n, y_n, z);
        let c001 = r.fetch(x, y, z_n);
        let c101 = r.fetch(x_n, y, z_n);
        let c011 = r.fetch(x, y_n, z_n);
        let c111 = r.fetch(x_n, y_n, z_n);

        let x000 = AvxVector::from_sse(c000, c001);
        let x010 = AvxVector::from_sse(c010, c011);
        let x011 = AvxVector::from_sse(c100, c101);
        let x111 = AvxVector::from_sse(c110, c111);

        let c00 = x000.neg_mla(x000, w0).mla(x011, w0);
        let c10 = x010.neg_mla(x010, w0).mla(x111, w0);

        let z0 = c00.neg_mla(c00, w1).mla(c10, w1);

        let (c0, c1) = z0.split();

        c0.neg_mla(c0, w2).mla(c1, w2)
    }
}
