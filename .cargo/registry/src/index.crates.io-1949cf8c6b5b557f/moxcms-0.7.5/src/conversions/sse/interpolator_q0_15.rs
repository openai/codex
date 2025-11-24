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
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::ops::{Add, Mul, Sub};

#[repr(align(8), C)]
pub(crate) struct SseAlignedI16x4(pub(crate) [i16; 4]);

#[cfg(feature = "options")]
pub(crate) struct TetrahedralSseQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [SseAlignedI16x4],
}

#[cfg(feature = "options")]
pub(crate) struct PyramidalSseQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [SseAlignedI16x4],
}

#[cfg(feature = "options")]
pub(crate) struct PrismaticSseQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [SseAlignedI16x4],
}

pub(crate) struct TrilinearSseQ0_15<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [SseAlignedI16x4],
}

trait Fetcher<T> {
    fn fetch(&self, x: i32, y: i32, z: i32) -> T;
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct SseVector {
    pub(crate) v: __m128i,
}

impl From<i16> for SseVector {
    #[inline(always)]
    fn from(v: i16) -> Self {
        SseVector {
            v: unsafe { _mm_set1_epi16(v) },
        }
    }
}

impl Sub<SseVector> for SseVector {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: SseVector) -> Self::Output {
        SseVector {
            v: unsafe { _mm_sub_epi16(self.v, rhs.v) },
        }
    }
}

impl Add<SseVector> for SseVector {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: SseVector) -> Self::Output {
        SseVector {
            v: unsafe { _mm_add_epi16(self.v, rhs.v) },
        }
    }
}

impl Mul<SseVector> for SseVector {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: SseVector) -> Self::Output {
        SseVector {
            v: unsafe { _mm_mulhrs_epi16(self.v, rhs.v) },
        }
    }
}

impl FusedMultiplyAdd<SseVector> for SseVector {
    #[inline(always)]
    fn mla(&self, b: SseVector, c: SseVector) -> SseVector {
        SseVector {
            v: unsafe { _mm_add_epi16(self.v, _mm_mulhrs_epi16(b.v, c.v)) },
        }
    }
}

struct TetrahedralSseQ0_15FetchVector<'a, const GRID_SIZE: usize> {
    cube: &'a [SseAlignedI16x4],
}

impl<const GRID_SIZE: usize> Fetcher<SseVector> for TetrahedralSseQ0_15FetchVector<'_, GRID_SIZE> {
    #[inline(always)]
    fn fetch(&self, x: i32, y: i32, z: i32) -> SseVector {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx = unsafe { self.cube.get_unchecked(offset..) };
        SseVector {
            v: unsafe { _mm_loadu_si64(jx.as_ptr() as *const _) },
        }
    }
}

pub(crate) trait SseMdInterpolationQ0_15<'a, const GRID_SIZE: usize> {
    fn new(table: &'a [SseAlignedI16x4]) -> Self;
    fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
    ) -> SseVector;
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> TetrahedralSseQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<SseVector>,
    ) -> SseVector {
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
        let s0 = c0.mla(c1, SseVector::from(rx));
        let s1 = s0.mla(c2, SseVector::from(ry));
        s1.mla(c3, SseVector::from(rz))
    }
}

macro_rules! define_inter_sse {
    ($interpolator: ident) => {
        impl<'a, const GRID_SIZE: usize> SseMdInterpolationQ0_15<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline]
            fn new(table: &'a [SseAlignedI16x4]) -> Self {
                Self { cube: table }
            }

            #[inline(always)]
            fn inter3_sse<U: AsPrimitive<usize>, const BINS: usize>(
                &self,
                in_r: U,
                in_g: U,
                in_b: U,
                lut: &[BarycentricWeight<i16>; BINS],
            ) -> SseVector {
                self.interpolate(
                    in_r,
                    in_g,
                    in_b,
                    lut,
                    TetrahedralSseQ0_15FetchVector::<GRID_SIZE> { cube: self.cube },
                )
            }
        }
    };
}

#[cfg(feature = "options")]
define_inter_sse!(TetrahedralSseQ0_15);
#[cfg(feature = "options")]
define_inter_sse!(PyramidalSseQ0_15);
#[cfg(feature = "options")]
define_inter_sse!(PrismaticSseQ0_15);
define_inter_sse!(TrilinearSseQ0_15);

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PyramidalSseQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<SseVector>,
    ) -> SseVector {
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

        if dr > db && dg > db {
            let x0 = r.fetch(x_n, y_n, z_n);
            let x1 = r.fetch(x_n, y_n, z);
            let x2 = r.fetch(x_n, y, z);
            let x3 = r.fetch(x, y_n, z);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, SseVector::from(db));
            let s1 = s0.mla(c2, SseVector::from(dr));
            let s2 = s1.mla(c3, SseVector::from(dg));
            s2.mla(c4, SseVector::from(dr) * SseVector::from(dg))
        } else if db > dr && dg > dr {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y_n, z_n);
            let x2 = r.fetch(x, y_n, z_n);
            let x3 = r.fetch(x, y_n, z);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, SseVector::from(db));
            let s1 = s0.mla(c2, SseVector::from(dr));
            let s2 = s1.mla(c3, SseVector::from(dg));
            s2.mla(c4, SseVector::from(dg) * SseVector::from(db))
        } else {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y, z);
            let x2 = r.fetch(x_n, y, z_n);
            let x3 = r.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - c0;
            let c3 = x3 - x2;
            let c4 = c0 - x1 - x0 + x2;

            let s0 = c0.mla(c1, SseVector::from(db));
            let s1 = s0.mla(c2, SseVector::from(dr));
            let s2 = s1.mla(c3, SseVector::from(dg));
            s2.mla(c4, SseVector::from(db) * SseVector::from(dr))
        }
    }
}

#[cfg(feature = "options")]
impl<const GRID_SIZE: usize> PrismaticSseQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<SseVector>,
    ) -> SseVector {
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

            let s0 = c0.mla(c1, SseVector::from(db));
            let s1 = s0.mla(c2, SseVector::from(dr));
            let s2 = s1.mla(c3, SseVector::from(dg));
            let s3 = s2.mla(c4, SseVector::from(dg) * SseVector::from(db));
            s3.mla(c5, SseVector::from(dr) * SseVector::from(dg))
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

            let s0 = c0.mla(c1, SseVector::from(db));
            let s1 = s0.mla(c2, SseVector::from(dr));
            let s2 = s1.mla(c3, SseVector::from(dg));
            let s3 = s2.mla(c4, SseVector::from(dg) * SseVector::from(db));
            s3.mla(c5, SseVector::from(dr) * SseVector::from(dg))
        }
    }
}

impl<const GRID_SIZE: usize> TrilinearSseQ0_15<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<i16>; BINS],
        r: impl Fetcher<SseVector>,
    ) -> SseVector {
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
        let q_max = SseVector::from(Q_MAX);
        let w0 = SseVector::from(dr);
        let w1 = SseVector::from(dg);
        let w2 = SseVector::from(db);
        let dx = q_max - SseVector::from(dr);
        let dy = q_max - SseVector::from(dg);
        let dz = q_max - SseVector::from(db);

        let c000 = r.fetch(x, y, z);
        let c100 = r.fetch(x_n, y, z);
        let c010 = r.fetch(x, y_n, z);
        let c110 = r.fetch(x_n, y_n, z);
        let c001 = r.fetch(x, y, z_n);
        let c101 = r.fetch(x_n, y, z_n);
        let c011 = r.fetch(x, y_n, z_n);
        let c111 = r.fetch(x_n, y_n, z_n);

        let c00 = (c000 * dx).mla(c100, w0);
        let c10 = (c010 * dx).mla(c110, w0);
        let c01 = (c001 * dx).mla(c101, w0);
        let c11 = (c011 * dx).mla(c111, w0);

        let c0 = (c00 * dy).mla(c10, w1);
        let c1 = (c01 * dy).mla(c11, w1);

        (c0 * dz).mla(c1, w2)
    }
}
