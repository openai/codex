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
use crate::nd_array::lerp;
use std::arch::aarch64::{vcombine_f32, vdup_n_f32, vld1_f32, vld1_lane_f32};
use std::ops::{Add, Mul, Sub};

/// 4D CLUT helper.
///
/// Represents hypercube.
pub(crate) struct HypercubeNeon<'a> {
    array: &'a [f32],
    x_stride: u32,
    y_stride: u32,
    z_stride: u32,
    grid_size: [u8; 4],
}

trait Fetcher4<T> {
    fn fetch(&self, x: i32, y: i32, z: i32, w: i32) -> T;
}

struct Fetch4Vec3<'a> {
    array: &'a [f32],
    x_stride: u32,
    y_stride: u32,
    z_stride: u32,
}

impl Fetcher4<NeonVector> for Fetch4Vec3<'_> {
    #[inline(always)]
    fn fetch(&self, x: i32, y: i32, z: i32, w: i32) -> NeonVector {
        let start = (x as u32 * self.x_stride
            + y as u32 * self.y_stride
            + z as u32 * self.z_stride
            + w as u32) as usize
            * 3;
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

impl<'a> HypercubeNeon<'a> {
    pub(crate) fn new(arr: &'a [f32], grid: [u8; 4], components: usize) -> Self {
        // This is safety precondition, array size must be not less than full grid size * components.
        // Needs to ensure that it is not missed somewhere else
        assert_eq!(
            grid[0] as usize * grid[1] as usize * grid[2] as usize * grid[3] as usize * components,
            arr.len()
        );
        let z_stride = grid[2] as u32;
        let y_stride = z_stride * grid[1] as u32;
        let x_stride = y_stride * grid[0] as u32;
        HypercubeNeon {
            array: arr,
            x_stride,
            y_stride,
            z_stride,
            grid_size: grid,
        }
    }

    #[inline(always)]
    fn quadlinear<
        T: From<f32>
            + Add<T, Output = T>
            + Mul<T, Output = T>
            + FusedMultiplyAdd<T>
            + Sub<T, Output = T>
            + Copy
            + FusedMultiplyNegAdd<T>,
    >(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        lin_w: f32,
        r: impl Fetcher4<T>,
    ) -> T {
        let lin_x = lin_x.max(0.0).min(1.0);
        let lin_y = lin_y.max(0.0).min(1.0);
        let lin_z = lin_z.max(0.0).min(1.0);
        let lin_w = lin_w.max(0.0).min(1.0);

        let scale_x = (self.grid_size[0] as i32 - 1) as f32;
        let scale_y = (self.grid_size[1] as i32 - 1) as f32;
        let scale_z = (self.grid_size[2] as i32 - 1) as f32;
        let scale_w = (self.grid_size[3] as i32 - 1) as f32;

        let x = (lin_x * scale_x).floor() as i32;
        let y = (lin_y * scale_y).floor() as i32;
        let z = (lin_z * scale_z).floor() as i32;
        let w = (lin_w * scale_w).floor() as i32;

        let x_n = (lin_x * scale_x).ceil() as i32;
        let y_n = (lin_y * scale_y).ceil() as i32;
        let z_n = (lin_z * scale_z).ceil() as i32;
        let w_n = (lin_w * scale_w).ceil() as i32;

        let x_d = T::from(lin_x * scale_x - x as f32);
        let y_d = T::from(lin_y * scale_y - y as f32);
        let z_d = T::from(lin_z * scale_z - z as f32);
        let w_d = T::from(lin_w * scale_w - w as f32);

        let r_x1 = lerp(r.fetch(x, y, z, w), r.fetch(x_n, y, z, w), x_d);
        let r_x2 = lerp(r.fetch(x, y_n, z, w), r.fetch(x_n, y_n, z, w), x_d);
        let r_y1 = lerp(r_x1, r_x2, y_d);
        let r_x3 = lerp(r.fetch(x, y, z_n, w), r.fetch(x_n, y, z_n, w), x_d);
        let r_x4 = lerp(r.fetch(x, y_n, z_n, w), r.fetch(x_n, y_n, z_n, w), x_d);
        let r_y2 = lerp(r_x3, r_x4, y_d);
        let r_z1 = lerp(r_y1, r_y2, z_d);

        let r_x1 = lerp(r.fetch(x, y, z, w_n), r.fetch(x_n, y, z, w_n), x_d);
        let r_x2 = lerp(r.fetch(x, y_n, z, w_n), r.fetch(x_n, y_n, z, w_n), x_d);
        let r_y1 = lerp(r_x1, r_x2, y_d);
        let r_x3 = lerp(r.fetch(x, y, z_n, w_n), r.fetch(x_n, y, z_n, w_n), x_d);
        let r_x4 = lerp(r.fetch(x, y_n, z_n, w_n), r.fetch(x_n, y_n, z_n, w_n), x_d);
        let r_y2 = lerp(r_x3, r_x4, y_d);
        let r_z2 = lerp(r_y1, r_y2, z_d);
        lerp(r_z1, r_z2, w_d)
    }

    #[inline]
    pub(crate) fn quadlinear_vec3(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        lin_w: f32,
    ) -> NeonVector {
        self.quadlinear(
            lin_x,
            lin_y,
            lin_z,
            lin_w,
            Fetch4Vec3 {
                array: self.array,
                x_stride: self.x_stride,
                y_stride: self.y_stride,
                z_stride: self.z_stride,
            },
        )
    }

    #[cfg(feature = "options")]
    #[inline(always)]
    fn pyramid<
        T: From<f32>
            + Add<T, Output = T>
            + Mul<T, Output = T>
            + FusedMultiplyAdd<T>
            + Sub<T, Output = T>
            + Copy
            + FusedMultiplyNegAdd<T>,
    >(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        lin_w: f32,
        r: impl Fetcher4<T>,
    ) -> T {
        let lin_x = lin_x.max(0.0).min(1.0);
        let lin_y = lin_y.max(0.0).min(1.0);
        let lin_z = lin_z.max(0.0).min(1.0);
        let lin_w = lin_w.max(0.0).min(1.0);

        let scale_x = (self.grid_size[0] as i32 - 1) as f32;
        let scale_y = (self.grid_size[1] as i32 - 1) as f32;
        let scale_z = (self.grid_size[2] as i32 - 1) as f32;
        let scale_w = (self.grid_size[3] as i32 - 1) as f32;

        let x = (lin_x * scale_x).floor() as i32;
        let y = (lin_y * scale_y).floor() as i32;
        let z = (lin_z * scale_z).floor() as i32;
        let w = (lin_w * scale_w).floor() as i32;

        let x_n = (lin_x * scale_x).ceil() as i32;
        let y_n = (lin_y * scale_y).ceil() as i32;
        let z_n = (lin_z * scale_z).ceil() as i32;
        let w_n = (lin_w * scale_w).ceil() as i32;

        let dr = lin_x * scale_x - x as f32;
        let dg = lin_y * scale_y - y as f32;
        let db = lin_z * scale_z - z as f32;
        let dw = lin_w * scale_w - w as f32;

        let c0 = r.fetch(x, y, z, w);

        let w0 = if dr > db && dg > db {
            let x0 = r.fetch(x_n, y_n, z_n, w);
            let x1 = r.fetch(x_n, y_n, z, w);
            let x2 = r.fetch(x_n, y, z, w);
            let x3 = r.fetch(x, y_n, z, w);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(dr * dg))
        } else if db > dr && dg > dr {
            let x0 = r.fetch(x, y, z_n, w);
            let x1 = r.fetch(x_n, y_n, z_n, w);
            let x2 = r.fetch(x, y_n, z_n, w);
            let x3 = r.fetch(x, y_n, z, w);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(dg * db))
        } else {
            let x0 = r.fetch(x, y, z_n, w);
            let x1 = r.fetch(x_n, y, z, w);
            let x2 = r.fetch(x_n, y, z_n, w);
            let x3 = r.fetch(x_n, y_n, z_n, w);

            let c1 = x0 - c0;
            let c2 = x1 - c0;
            let c3 = x3 - x2;
            let c4 = c0 - x1 - x0 + x2;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(db * dr))
        };

        let c0 = r.fetch(x, y, z, w_n);

        let w1 = if dr > db && dg > db {
            let x0 = r.fetch(x_n, y_n, z_n, w_n);
            let x1 = r.fetch(x_n, y_n, z, w_n);
            let x2 = r.fetch(x_n, y, z, w_n);
            let x3 = r.fetch(x, y_n, z, w_n);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(dr * dg))
        } else if db > dr && dg > dr {
            let x0 = r.fetch(x, y, z_n, w_n);
            let x1 = r.fetch(x_n, y_n, z_n, w_n);
            let x2 = r.fetch(x, y_n, z_n, w_n);
            let x3 = r.fetch(x, y_n, z, w_n);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(dg * db))
        } else {
            let x0 = r.fetch(x, y, z_n, w_n);
            let x1 = r.fetch(x_n, y, z, w_n);
            let x2 = r.fetch(x_n, y, z_n, w_n);
            let x3 = r.fetch(x_n, y_n, z_n, w_n);

            let c1 = x0 - c0;
            let c2 = x1 - c0;
            let c3 = x3 - x2;
            let c4 = c0 - x1 - x0 + x2;

            let s0 = c0.mla(c1, T::from(db));
            let s1 = s0.mla(c2, T::from(dr));
            let s2 = s1.mla(c3, T::from(dg));
            s2.mla(c4, T::from(db * dr))
        };
        w0.neg_mla(w0, T::from(dw)).mla(w1, T::from(dw))
    }

    #[cfg(feature = "options")]
    #[inline]
    pub(crate) fn pyramid_vec3(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        lin_w: f32,
    ) -> NeonVector {
        self.pyramid(
            lin_x,
            lin_y,
            lin_z,
            lin_w,
            Fetch4Vec3 {
                array: self.array,
                x_stride: self.x_stride,
                y_stride: self.y_stride,
                z_stride: self.z_stride,
            },
        )
    }

    #[cfg(feature = "options")]
    #[inline(always)]
    fn prism<
        T: From<f32>
            + Add<T, Output = T>
            + Mul<T, Output = T>
            + FusedMultiplyAdd<T>
            + Sub<T, Output = T>
            + Copy
            + FusedMultiplyNegAdd<T>,
    >(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        lin_w: f32,
        r: impl Fetcher4<T>,
    ) -> T {
        let lin_x = lin_x.max(0.0).min(1.0);
        let lin_y = lin_y.max(0.0).min(1.0);
        let lin_z = lin_z.max(0.0).min(1.0);
        let lin_w = lin_w.max(0.0).min(1.0);

        let scale_x = (self.grid_size[0] as i32 - 1) as f32;
        let scale_y = (self.grid_size[1] as i32 - 1) as f32;
        let scale_z = (self.grid_size[2] as i32 - 1) as f32;
        let scale_w = (self.grid_size[3] as i32 - 1) as f32;

        let x = (lin_x * scale_x).floor() as i32;
        let y = (lin_y * scale_y).floor() as i32;
        let z = (lin_z * scale_z).floor() as i32;
        let w = (lin_w * scale_w).floor() as i32;

        let x_n = (lin_x * scale_x).ceil() as i32;
        let y_n = (lin_y * scale_y).ceil() as i32;
        let z_n = (lin_z * scale_z).ceil() as i32;
        let w_n = (lin_w * scale_w).ceil() as i32;

        let dr = lin_x * scale_x - x as f32;
        let dg = lin_y * scale_y - y as f32;
        let db = lin_z * scale_z - z as f32;
        let dw = lin_w * scale_w - w as f32;

        let c0 = r.fetch(x, y, z, w);

        let w0 = if db >= dr {
            let x0 = r.fetch(x, y, z_n, w);
            let x1 = r.fetch(x_n, y, z_n, w);
            let x2 = r.fetch(x, y_n, z, w);
            let x3 = r.fetch(x, y_n, z_n, w);
            let x4 = r.fetch(x_n, y_n, z_n, w);

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
            let x0 = r.fetch(x_n, y, z, w);
            let x1 = r.fetch(x_n, y, z_n, w);
            let x2 = r.fetch(x, y_n, z, w);
            let x3 = r.fetch(x_n, y_n, z, w);
            let x4 = r.fetch(x_n, y_n, z_n, w);

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
        };

        let c0 = r.fetch(x, y, z, w_n);

        let w1 = if db >= dr {
            let x0 = r.fetch(x, y, z_n, w_n);
            let x1 = r.fetch(x_n, y, z_n, w_n);
            let x2 = r.fetch(x, y_n, z, w_n);
            let x3 = r.fetch(x, y_n, z_n, w_n);
            let x4 = r.fetch(x_n, y_n, z_n, w_n);

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
            let x0 = r.fetch(x_n, y, z, w_n);
            let x1 = r.fetch(x_n, y, z_n, w_n);
            let x2 = r.fetch(x, y_n, z, w_n);
            let x3 = r.fetch(x_n, y_n, z, w_n);
            let x4 = r.fetch(x_n, y_n, z_n, w_n);

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
        };
        w0.neg_mla(w0, T::from(dw)).mla(w1, T::from(dw))
    }

    #[cfg(feature = "options")]
    #[inline]
    pub(crate) fn prism_vec3(&self, lin_x: f32, lin_y: f32, lin_z: f32, lin_w: f32) -> NeonVector {
        self.prism(
            lin_x,
            lin_y,
            lin_z,
            lin_w,
            Fetch4Vec3 {
                array: self.array,
                x_stride: self.x_stride,
                y_stride: self.y_stride,
                z_stride: self.z_stride,
            },
        )
    }

    #[cfg(feature = "options")]
    #[inline(always)]
    fn tetra<
        T: From<f32>
            + Add<T, Output = T>
            + Mul<T, Output = T>
            + FusedMultiplyAdd<T>
            + Sub<T, Output = T>
            + Copy
            + FusedMultiplyNegAdd<T>,
    >(
        &self,
        lin_x: f32,
        lin_y: f32,
        lin_z: f32,
        lin_w: f32,
        r: impl Fetcher4<T>,
    ) -> T {
        let lin_x = lin_x.max(0.0).min(1.0);
        let lin_y = lin_y.max(0.0).min(1.0);
        let lin_z = lin_z.max(0.0).min(1.0);
        let lin_w = lin_w.max(0.0).min(1.0);

        let scale_x = (self.grid_size[0] as i32 - 1) as f32;
        let scale_y = (self.grid_size[1] as i32 - 1) as f32;
        let scale_z = (self.grid_size[2] as i32 - 1) as f32;
        let scale_w = (self.grid_size[3] as i32 - 1) as f32;

        let x = (lin_x * scale_x).floor() as i32;
        let y = (lin_y * scale_y).floor() as i32;
        let z = (lin_z * scale_z).floor() as i32;
        let w = (lin_w * scale_w).floor() as i32;

        let x_n = (lin_x * scale_x).ceil() as i32;
        let y_n = (lin_y * scale_y).ceil() as i32;
        let z_n = (lin_z * scale_z).ceil() as i32;
        let w_n = (lin_w * scale_w).ceil() as i32;

        let rx = lin_x * scale_x - x as f32;
        let ry = lin_y * scale_y - y as f32;
        let rz = lin_z * scale_z - z as f32;
        let rw = lin_w * scale_w - w as f32;

        let c0 = r.fetch(x, y, z, w);
        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = r.fetch(x_n, y, z, w) - c0;
                c2 = r.fetch(x_n, y_n, z, w) - r.fetch(x_n, y, z, w);
                c3 = r.fetch(x_n, y_n, z_n, w) - r.fetch(x_n, y_n, z, w);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = r.fetch(x_n, y, z, w) - c0;
                c2 = r.fetch(x_n, y_n, z_n, w) - r.fetch(x_n, y, z_n, w);
                c3 = r.fetch(x_n, y, z_n, w) - r.fetch(x_n, y, z, w);
            } else {
                //rz > rx && rx >= ry
                c1 = r.fetch(x_n, y, z_n, w) - r.fetch(x, y, z_n, w);
                c2 = r.fetch(x_n, y_n, z_n, w) - r.fetch(x_n, y, z_n, w);
                c3 = r.fetch(x, y, z_n, w) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = r.fetch(x_n, y_n, z, w) - r.fetch(x, y_n, z, w);
            c2 = r.fetch(x, y_n, z, w) - c0;
            c3 = r.fetch(x_n, y_n, z_n, w) - r.fetch(x_n, y_n, z, w);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = r.fetch(x_n, y_n, z_n, w) - r.fetch(x, y_n, z_n, w);
            c2 = r.fetch(x, y_n, z, w) - c0;
            c3 = r.fetch(x, y_n, z_n, w) - r.fetch(x, y_n, z, w);
        } else {
            //rz > ry && ry > rx
            c1 = r.fetch(x_n, y_n, z_n, w) - r.fetch(x, y_n, z_n, w);
            c2 = r.fetch(x, y_n, z_n, w) - r.fetch(x, y, z_n, w);
            c3 = r.fetch(x, y, z_n, w) - c0;
        }
        let s0 = c0.mla(c1, T::from(rx));
        let s1 = s0.mla(c2, T::from(ry));
        let w0 = s1.mla(c3, T::from(rz));

        let c0 = r.fetch(x, y, z, w_n);
        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = r.fetch(x_n, y, z, w_n) - c0;
                c2 = r.fetch(x_n, y_n, z, w_n) - r.fetch(x_n, y, z, w_n);
                c3 = r.fetch(x_n, y_n, z_n, w_n) - r.fetch(x_n, y_n, z, w_n);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = r.fetch(x_n, y, z, w_n) - c0;
                c2 = r.fetch(x_n, y_n, z_n, w_n) - r.fetch(x_n, y, z_n, w_n);
                c3 = r.fetch(x_n, y, z_n, w_n) - r.fetch(x_n, y, z, w_n);
            } else {
                //rz > rx && rx >= ry
                c1 = r.fetch(x_n, y, z_n, w_n) - r.fetch(x, y, z_n, w_n);
                c2 = r.fetch(x_n, y_n, z_n, w_n) - r.fetch(x_n, y, z_n, w_n);
                c3 = r.fetch(x, y, z_n, w_n) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = r.fetch(x_n, y_n, z, w_n) - r.fetch(x, y_n, z, w_n);
            c2 = r.fetch(x, y_n, z, w_n) - c0;
            c3 = r.fetch(x_n, y_n, z_n, w_n) - r.fetch(x_n, y_n, z, w_n);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = r.fetch(x_n, y_n, z_n, w_n) - r.fetch(x, y_n, z_n, w_n);
            c2 = r.fetch(x, y_n, z, w_n) - c0;
            c3 = r.fetch(x, y_n, z_n, w_n) - r.fetch(x, y_n, z, w_n);
        } else {
            //rz > ry && ry > rx
            c1 = r.fetch(x_n, y_n, z_n, w_n) - r.fetch(x, y_n, z_n, w_n);
            c2 = r.fetch(x, y_n, z_n, w_n) - r.fetch(x, y, z_n, w_n);
            c3 = r.fetch(x, y, z_n, w_n) - c0;
        }
        let s0 = c0.mla(c1, T::from(rx));
        let s1 = s0.mla(c2, T::from(ry));
        let w1 = s1.mla(c3, T::from(rz));
        w0.neg_mla(w0, T::from(rw)).mla(w1, T::from(rw))
    }

    #[cfg(feature = "options")]
    #[inline]
    pub(crate) fn tetra_vec3(&self, lin_x: f32, lin_y: f32, lin_z: f32, lin_w: f32) -> NeonVector {
        self.tetra(
            lin_x,
            lin_y,
            lin_z,
            lin_w,
            Fetch4Vec3 {
                array: self.array,
                x_stride: self.x_stride,
                y_stride: self.y_stride,
                z_stride: self.z_stride,
            },
        )
    }
}
