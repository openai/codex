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
use crate::conversions::neon::interpolator::NeonVector;
use crate::math::{FusedMultiplyAdd, FusedMultiplyNegAdd};
use std::arch::aarch64::*;
use std::ops::{Add, Mul, Sub};

/// 3D CLUT NEON helper
///
/// Represents hexahedron.
pub(crate) struct CubeNeon<'a> {
    array: &'a [f32],
    x_stride: u32,
    y_stride: u32,
    grid_size: [u8; 3],
}

struct HexahedronFetch3<'a> {
    array: &'a [f32],
    x_stride: u32,
    y_stride: u32,
}

trait CubeFetch<T> {
    fn fetch(&self, x: i32, y: i32, z: i32) -> T;
}

impl CubeFetch<NeonVector> for HexahedronFetch3<'_> {
    #[inline(always)]
    fn fetch(&self, x: i32, y: i32, z: i32) -> NeonVector {
        let start = (x as u32 * self.x_stride + y as u32 * self.y_stride + z as u32) as usize * 3;
        unsafe {
            let k = self.array.get_unchecked(start..);
            let lo = vld1_f32(k.as_ptr());
            let hi = vld1_lane_f32::<0>(k.get_unchecked(2..).as_ptr(), vdup_n_f32(0.));
            NeonVector {
                v: vcombine_f32(lo, hi),
            }
        }
    }
}

impl<'a> CubeNeon<'a> {
    pub(crate) fn new(arr: &'a [f32], grid: [u8; 3], components: usize) -> Self {
        // This is safety precondition, array size must be not less than full grid size * components.
        // Needs to ensure that it is not missed somewhere else
        assert_eq!(
            grid[0] as usize * grid[1] as usize * grid[2] as usize * components,
            arr.len()
        );
        let y_stride = grid[1] as u32;
        let x_stride = y_stride * grid[0] as u32;
        CubeNeon {
            array: arr,
            x_stride,
            y_stride,
            grid_size: grid,
        }
    }

    #[inline(always)]
    fn trilinear<
        T: Copy
            + From<f32>
            + Sub<T, Output = T>
            + Mul<T, Output = T>
            + Add<T, Output = T>
            + FusedMultiplyNegAdd<T>
            + FusedMultiplyAdd<T>,
    >(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        fetch: impl CubeFetch<T>,
    ) -> T {
        let lin_x = lin_x.max(0.0).min(1.0);
        let lin_y = lin_y.max(0.0).min(1.0);
        let lin_z = lin_z.max(0.0).min(1.0);

        let scale_x = (self.grid_size[0] as i32 - 1) as f32;
        let scale_y = (self.grid_size[1] as i32 - 1) as f32;
        let scale_z = (self.grid_size[2] as i32 - 1) as f32;

        let x = (lin_x * scale_x).floor() as i32;
        let y = (lin_y * scale_y).floor() as i32;
        let z = (lin_z * scale_z).floor() as i32;

        let x_n = (lin_x * scale_x).ceil() as i32;
        let y_n = (lin_y * scale_y).ceil() as i32;
        let z_n = (lin_z * scale_z).ceil() as i32;

        let x_d = T::from(lin_x * scale_x - x as f32);
        let y_d = T::from(lin_y * scale_y - y as f32);
        let z_d = T::from(lin_z * scale_z - z as f32);

        let c000 = fetch.fetch(x, y, z);
        let c100 = fetch.fetch(x_n, y, z);
        let c010 = fetch.fetch(x, y_n, z);
        let c110 = fetch.fetch(x_n, y_n, z);
        let c001 = fetch.fetch(x, y, z_n);
        let c101 = fetch.fetch(x_n, y, z_n);
        let c011 = fetch.fetch(x, y_n, z_n);
        let c111 = fetch.fetch(x_n, y_n, z_n);

        let c00 = c000.neg_mla(c000, x_d).mla(c100, x_d);
        let c10 = c010.neg_mla(c010, x_d).mla(c110, x_d);
        let c01 = c001.neg_mla(c001, x_d).mla(c101, x_d);
        let c11 = c011.neg_mla(c011, x_d).mla(c111, x_d);

        let c0 = c00.neg_mla(c00, y_d).mla(c10, y_d);
        let c1 = c01.neg_mla(c01, y_d).mla(c11, y_d);

        c0.neg_mla(c0, z_d).mla(c1, z_d)
    }

    #[cfg(feature = "options")]
    #[inline]
    fn pyramid<
        T: Copy
            + From<f32>
            + Sub<T, Output = T>
            + Mul<T, Output = T>
            + Add<T, Output = T>
            + FusedMultiplyAdd<T>,
    >(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        fetch: impl CubeFetch<T>,
    ) -> T {
        let lin_x = lin_x.max(0.0).min(1.0);
        let lin_y = lin_y.max(0.0).min(1.0);
        let lin_z = lin_z.max(0.0).min(1.0);

        let scale_x = (self.grid_size[0] as i32 - 1) as f32;
        let scale_y = (self.grid_size[1] as i32 - 1) as f32;
        let scale_z = (self.grid_size[2] as i32 - 1) as f32;

        let x = (lin_x * scale_x).floor() as i32;
        let y = (lin_y * scale_y).floor() as i32;
        let z = (lin_z * scale_z).floor() as i32;

        let x_n = (lin_x * scale_x).ceil() as i32;
        let y_n = (lin_y * scale_y).ceil() as i32;
        let z_n = (lin_z * scale_z).ceil() as i32;

        let dr = lin_x * scale_x - x as f32;
        let dg = lin_y * scale_y - y as f32;
        let db = lin_z * scale_z - z as f32;

        let c0 = fetch.fetch(x, y, z);

        if dr > db && dg > db {
            let x0 = fetch.fetch(x_n, y_n, z_n);
            let x1 = fetch.fetch(x_n, y_n, z);
            let x2 = fetch.fetch(x_n, y, z);
            let x3 = fetch.fetch(x, y_n, z);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(dr * dg))
        } else if db > dr && dg > dr {
            let x0 = fetch.fetch(x, y, z_n);
            let x1 = fetch.fetch(x_n, y_n, z_n);
            let x2 = fetch.fetch(x, y_n, z_n);
            let x3 = fetch.fetch(x, y_n, z);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(dg * db))
        } else {
            let x0 = fetch.fetch(x, y, z_n);
            let x1 = fetch.fetch(x_n, y, z);
            let x2 = fetch.fetch(x_n, y, z_n);
            let x3 = fetch.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - c0;
            let c3 = x3 - x2;
            let c4 = c0 - x1 - x0 + x2;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(db * dr))
        }
    }

    #[cfg(feature = "options")]
    #[inline]
    fn tetra<
        T: Copy
            + From<f32>
            + Sub<T, Output = T>
            + Mul<T, Output = T>
            + Add<T, Output = T>
            + FusedMultiplyAdd<T>,
    >(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        fetch: impl CubeFetch<T>,
    ) -> T {
        let lin_x = lin_x.max(0.0).min(1.0);
        let lin_y = lin_y.max(0.0).min(1.0);
        let lin_z = lin_z.max(0.0).min(1.0);

        let scale_x = (self.grid_size[0] as i32 - 1) as f32;
        let scale_y = (self.grid_size[1] as i32 - 1) as f32;
        let scale_z = (self.grid_size[2] as i32 - 1) as f32;

        let x = (lin_x * scale_x).floor() as i32;
        let y = (lin_y * scale_y).floor() as i32;
        let z = (lin_z * scale_z).floor() as i32;

        let x_n = (lin_x * scale_x).ceil() as i32;
        let y_n = (lin_y * scale_y).ceil() as i32;
        let z_n = (lin_z * scale_z).ceil() as i32;

        let rx = lin_x * scale_x - x as f32;
        let ry = lin_y * scale_y - y as f32;
        let rz = lin_z * scale_z - z as f32;

        let c0 = fetch.fetch(x, y, z);
        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = fetch.fetch(x_n, y, z) - c0;
                c2 = fetch.fetch(x_n, y_n, z) - fetch.fetch(x_n, y, z);
                c3 = fetch.fetch(x_n, y_n, z_n) - fetch.fetch(x_n, y_n, z);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = fetch.fetch(x_n, y, z) - c0;
                c2 = fetch.fetch(x_n, y_n, z_n) - fetch.fetch(x_n, y, z_n);
                c3 = fetch.fetch(x_n, y, z_n) - fetch.fetch(x_n, y, z);
            } else {
                //rz > rx && rx >= ry
                c1 = fetch.fetch(x_n, y, z_n) - fetch.fetch(x, y, z_n);
                c2 = fetch.fetch(x_n, y_n, z_n) - fetch.fetch(x_n, y, z_n);
                c3 = fetch.fetch(x, y, z_n) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = fetch.fetch(x_n, y_n, z) - fetch.fetch(x, y_n, z);
            c2 = fetch.fetch(x, y_n, z) - c0;
            c3 = fetch.fetch(x_n, y_n, z_n) - fetch.fetch(x_n, y_n, z);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = fetch.fetch(x_n, y_n, z_n) - fetch.fetch(x, y_n, z_n);
            c2 = fetch.fetch(x, y_n, z) - c0;
            c3 = fetch.fetch(x, y_n, z_n) - fetch.fetch(x, y_n, z);
        } else {
            //rz > ry && ry > rx
            c1 = fetch.fetch(x_n, y_n, z_n) - fetch.fetch(x, y_n, z_n);
            c2 = fetch.fetch(x, y_n, z_n) - fetch.fetch(x, y, z_n);
            c3 = fetch.fetch(x, y, z_n) - c0;
        }
        let s0 = c0.mla(c1, T::from(rx));
        let s1 = s0.mla(c2, T::from(ry));
        s1.mla(c3, T::from(rz))
    }

    #[cfg(feature = "options")]
    #[inline]
    fn prism<
        T: Copy
            + From<f32>
            + Sub<T, Output = T>
            + Mul<T, Output = T>
            + Add<T, Output = T>
            + FusedMultiplyAdd<T>,
    >(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        fetch: impl CubeFetch<T>,
    ) -> T {
        let lin_x = lin_x.max(0.0).min(1.0);
        let lin_y = lin_y.max(0.0).min(1.0);
        let lin_z = lin_z.max(0.0).min(1.0);

        let scale_x = (self.grid_size[0] as i32 - 1) as f32;
        let scale_y = (self.grid_size[1] as i32 - 1) as f32;
        let scale_z = (self.grid_size[2] as i32 - 1) as f32;

        let x = (lin_x * scale_x).floor() as i32;
        let y = (lin_y * scale_y).floor() as i32;
        let z = (lin_z * scale_z).floor() as i32;

        let x_n = (lin_x * scale_x).ceil() as i32;
        let y_n = (lin_y * scale_y).ceil() as i32;
        let z_n = (lin_z * scale_z).ceil() as i32;

        let dr = lin_x * scale_x - x as f32;
        let dg = lin_y * scale_y - y as f32;
        let db = lin_z * scale_z - z as f32;

        let c0 = fetch.fetch(x, y, z);

        if db >= dr {
            let x0 = fetch.fetch(x, y, z_n);
            let x1 = fetch.fetch(x_n, y, z_n);
            let x2 = fetch.fetch(x, y_n, z);
            let x3 = fetch.fetch(x, y_n, z_n);
            let x4 = fetch.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - x0;
            let c3 = x2 - c0;
            let c4 = c0 - x2 - x0 + x3;
            let c5 = x0 - x3 - x1 + x4;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            let s3 = s2.mla(c4, T::from(dg * db));
            s3.mla(c5, T::from(dr * dg))
        } else {
            let x0 = fetch.fetch(x_n, y, z);
            let x1 = fetch.fetch(x_n, y, z_n);
            let x2 = fetch.fetch(x, y_n, z);
            let x3 = fetch.fetch(x_n, y_n, z);
            let x4 = fetch.fetch(x_n, y_n, z_n);

            let c1 = x1 - x0;
            let c2 = x0 - c0;
            let c3 = x2 - c0;
            let c4 = x0 - x3 - x1 + x4;
            let c5 = c0 - x2 - x0 + x3;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            let s3 = s2.mla(c4, T::from(dg * db));
            s3.mla(c5, T::from(dr * dg))
        }
    }

    #[inline]
    pub(crate) fn trilinear_vec3(&self, lin_x: f32, lin_y: f32, lin_z: f32) -> NeonVector {
        self.trilinear(
            lin_x,
            lin_y,
            lin_z,
            HexahedronFetch3 {
                array: self.array,
                x_stride: self.x_stride,
                y_stride: self.y_stride,
            },
        )
    }

    #[cfg(feature = "options")]
    #[inline]
    pub(crate) fn prism_vec3(&self, lin_x: f32, lin_y: f32, lin_z: f32) -> NeonVector {
        self.prism(
            lin_x,
            lin_y,
            lin_z,
            HexahedronFetch3 {
                array: self.array,
                x_stride: self.x_stride,
                y_stride: self.y_stride,
            },
        )
    }

    #[cfg(feature = "options")]
    #[inline]
    pub(crate) fn pyramid_vec3(&self, lin_x: f32, lin_y: f32, lin_z: f32) -> NeonVector {
        self.pyramid(
            lin_x,
            lin_y,
            lin_z,
            HexahedronFetch3 {
                array: self.array,
                x_stride: self.x_stride,
                y_stride: self.y_stride,
            },
        )
    }

    #[cfg(feature = "options")]
    #[inline]
    pub(crate) fn tetra_vec3(&self, lin_x: f32, lin_y: f32, lin_z: f32) -> NeonVector {
        self.tetra(
            lin_x,
            lin_y,
            lin_z,
            HexahedronFetch3 {
                array: self.array,
                x_stride: self.x_stride,
                y_stride: self.y_stride,
            },
        )
    }
}
