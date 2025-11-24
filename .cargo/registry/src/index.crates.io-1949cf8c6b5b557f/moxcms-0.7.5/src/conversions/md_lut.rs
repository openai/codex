/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
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
use crate::math::{FusedMultiplyAdd, FusedMultiplyNegAdd};
use crate::mlaf::{mlaf, neg_mlaf};
use crate::nd_array::{ArrayFetch, lerp};
use crate::{Vector3f, Vector3i};
use num_traits::MulAdd;
use std::array::from_fn;
use std::marker::PhantomData;
use std::ops::{Add, Mul, Neg, Sub};

pub(crate) struct MultidimensionalLut {
    pub(crate) grid_strides: [u32; 16],
    pub(crate) grid_filling_size: [u32; 16],
    pub(crate) grid_scale: [f32; 16],
    pub(crate) output_inks: usize,
}

struct FastCube<T, F: ArrayFetch<T>> {
    fetch: F,
    _phantom: PhantomData<T>,
}

struct ArrayFetchVectorN<'a> {
    array: &'a [f32],
    x_stride: u32,
    y_stride: u32,
    z_stride: u32,
    output_inks: usize,
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
pub(crate) struct NVector<T, const N: usize> {
    pub(crate) v: [T; N],
}

impl<T: Copy, const N: usize> NVector<T, N> {
    pub(crate) fn from_slice(v: &[T; N]) -> Self {
        Self { v: *v }
    }
}

impl<T: Copy, const N: usize> From<T> for NVector<T, N> {
    #[inline]
    fn from(value: T) -> Self {
        Self { v: [value; N] }
    }
}

impl<T: Copy + Add<T, Output = T> + Mul<T, Output = T> + MulAdd<T, Output = T>, const N: usize>
    FusedMultiplyAdd<NVector<T, N>> for NVector<T, N>
{
    #[inline]
    fn mla(&self, b: NVector<T, N>, c: NVector<T, N>) -> NVector<T, N> {
        Self {
            v: from_fn(|i| mlaf(self.v[i], b.v[i], c.v[i])),
        }
    }
}

impl<
    T: Copy + Add<T, Output = T> + Mul<T, Output = T> + MulAdd<T, Output = T> + Neg<Output = T>,
    const N: usize,
> FusedMultiplyNegAdd<NVector<T, N>> for NVector<T, N>
{
    #[inline]
    fn neg_mla(&self, b: NVector<T, N>, c: NVector<T, N>) -> NVector<T, N> {
        Self {
            v: from_fn(|i| neg_mlaf(self.v[i], b.v[i], c.v[i])),
        }
    }
}

impl<T: Sub<Output = T> + Default + Copy, const N: usize> Sub<NVector<T, N>> for NVector<T, N> {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: NVector<T, N>) -> Self::Output {
        Self {
            v: from_fn(|i| self.v[i] - rhs.v[i]),
        }
    }
}

impl<T: Add<Output = T> + Default + Copy, const N: usize> Add<NVector<T, N>> for NVector<T, N> {
    type Output = Self;

    #[inline]
    fn add(self, rhs: NVector<T, N>) -> Self::Output {
        Self {
            v: from_fn(|i| self.v[i] + rhs.v[i]),
        }
    }
}

impl<T: Mul<Output = T> + Default + Copy, const N: usize> Mul<NVector<T, N>> for NVector<T, N> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: NVector<T, N>) -> Self::Output {
        Self {
            v: from_fn(|i| self.v[i] * rhs.v[i]),
        }
    }
}

impl<const N: usize> ArrayFetch<NVector<f32, N>> for ArrayFetchVectorN<'_> {
    #[inline(always)]
    fn fetch(&self, x: i32, y: i32, z: i32) -> NVector<f32, N> {
        let start = (x as u32 * self.x_stride + y as u32 * self.y_stride + z as u32 * self.z_stride)
            as usize
            * self.output_inks;
        let k = &self.array[start..start + N];
        NVector::<f32, N>::from_slice(k.try_into().unwrap())
    }
}

impl<T, F: ArrayFetch<T>> FastCube<T, F>
where
    T: Copy
        + From<f32>
        + Sub<T, Output = T>
        + Mul<T, Output = T>
        + Add<T, Output = T>
        + FusedMultiplyNegAdd<T>
        + FusedMultiplyAdd<T>,
{
    #[inline(always)]
    fn tetra(&self, src: Vector3i, src_next: Vector3i, w: Vector3f) -> T {
        let x = src.v[0];
        let y = src.v[1];
        let z = src.v[2];

        let x_n = src_next.v[0];
        let y_n = src_next.v[1];
        let z_n = src_next.v[2];

        let rx = w.v[0];
        let ry = w.v[1];
        let rz = w.v[2];

        let c0 = self.fetch.fetch(x, y, z);
        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = self.fetch.fetch(x_n, y, z) - c0;
                c2 = self.fetch.fetch(x_n, y_n, z) - self.fetch.fetch(x_n, y, z);
                c3 = self.fetch.fetch(x_n, y_n, z_n) - self.fetch.fetch(x_n, y_n, z);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = self.fetch.fetch(x_n, y, z) - c0;
                c2 = self.fetch.fetch(x_n, y_n, z_n) - self.fetch.fetch(x_n, y, z_n);
                c3 = self.fetch.fetch(x_n, y, z_n) - self.fetch.fetch(x_n, y, z);
            } else {
                //rz > rx && rx >= ry
                c1 = self.fetch.fetch(x_n, y, z_n) - self.fetch.fetch(x, y, z_n);
                c2 = self.fetch.fetch(x_n, y_n, z_n) - self.fetch.fetch(x_n, y, z_n);
                c3 = self.fetch.fetch(x, y, z_n) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = self.fetch.fetch(x_n, y_n, z) - self.fetch.fetch(x, y_n, z);
            c2 = self.fetch.fetch(x, y_n, z) - c0;
            c3 = self.fetch.fetch(x_n, y_n, z_n) - self.fetch.fetch(x_n, y_n, z);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = self.fetch.fetch(x_n, y_n, z_n) - self.fetch.fetch(x, y_n, z_n);
            c2 = self.fetch.fetch(x, y_n, z) - c0;
            c3 = self.fetch.fetch(x, y_n, z_n) - self.fetch.fetch(x, y_n, z);
        } else {
            //rz > ry && ry > rx
            c1 = self.fetch.fetch(x_n, y_n, z_n) - self.fetch.fetch(x, y_n, z_n);
            c2 = self.fetch.fetch(x, y_n, z_n) - self.fetch.fetch(x, y, z_n);
            c3 = self.fetch.fetch(x, y, z_n) - c0;
        }
        let s0 = c0.mla(c1, T::from(rx));
        let s1 = s0.mla(c2, T::from(ry));
        s1.mla(c3, T::from(rz))
    }
}

impl MultidimensionalLut {
    pub(crate) fn new(grid_size: [u8; 16], input_inks: usize, output_inks: usize) -> Self {
        assert!(input_inks <= 16);
        let mut grid_strides = [1u32; 16];
        let mut grid_filling_size = [1u32; 16];

        for (ink, dst_stride) in grid_strides.iter_mut().take(input_inks - 1).enumerate() {
            let mut stride = 1u32;
            let how_many = input_inks.saturating_sub(ink).saturating_sub(1);
            for &grid_stride in grid_size.iter().take(how_many) {
                stride *= grid_stride as u32;
            }
            *dst_stride = stride;
        }

        for (ink, dst_stride) in grid_filling_size.iter_mut().take(input_inks).enumerate() {
            let mut stride = output_inks as u32;
            let how_many = input_inks.saturating_sub(ink).saturating_sub(1);
            for &grid_stride in grid_size.iter().take(how_many) {
                stride *= grid_stride as u32;
            }
            *dst_stride = stride;
        }

        let mut grid_strides_f = [0f32; 16];

        for (dst, src) in grid_strides_f
            .iter_mut()
            .zip(grid_size.iter())
            .take(input_inks)
        {
            *dst = (*src - 1) as f32;
        }

        Self {
            grid_strides,
            grid_scale: grid_strides_f,
            grid_filling_size,
            output_inks,
        }
    }
}

pub(crate) fn linear_4i_vec3f_direct<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    lx: f32,
    ly: f32,
    lz: f32,
    lw: f32,
) -> NVector<f32, N> {
    let lin_x = lx.max(0.0).min(1.0);
    let lin_y = ly.max(0.0).min(1.0);
    let lin_z = lz.max(0.0).min(1.0);
    let lin_w = lw.max(0.0).min(1.0);

    let scale_x = lut.grid_scale[0];
    let scale_y = lut.grid_scale[1];
    let scale_z = lut.grid_scale[2];
    let scale_w = lut.grid_scale[3];

    let lx = lin_x * scale_x;
    let ly = lin_y * scale_y;
    let lz = lin_z * scale_z;
    let lw = lin_w * scale_w;

    let x = lx.floor() as i32;
    let y = ly.floor() as i32;
    let z = lz.floor() as i32;
    let w = lw.floor() as i32;

    let src_x = Vector3i { v: [x, y, z] };

    let x_n = lx.ceil() as i32;
    let y_n = ly.ceil() as i32;
    let z_n = lz.ceil() as i32;
    let w_n = lw.ceil() as i32;

    let src_next = Vector3i { v: [x_n, y_n, z_n] };

    let x_w = lx - x as f32;
    let y_w = ly - y as f32;
    let z_w = lz - z as f32;
    let w_w = lw - w as f32;

    let weights = Vector3f { v: [x_w, y_w, z_w] };

    let cube0 = &arr[(w as usize * lut.grid_filling_size[3] as usize)..];
    let cube1 = &arr[(w_n as usize * lut.grid_filling_size[3] as usize)..];

    let fast_cube0 = FastCube {
        fetch: ArrayFetchVectorN {
            array: cube0,
            x_stride: lut.grid_strides[0],
            y_stride: lut.grid_strides[1],
            z_stride: lut.grid_strides[2],
            output_inks: lut.output_inks,
        },
        _phantom: PhantomData,
    };
    let fast_cube1 = FastCube {
        fetch: ArrayFetchVectorN {
            array: cube1,
            x_stride: lut.grid_strides[0],
            y_stride: lut.grid_strides[1],
            z_stride: lut.grid_strides[2],
            output_inks: lut.output_inks,
        },
        _phantom: PhantomData,
    };
    let w0 = fast_cube0.tetra(src_x, src_next, weights);
    let w1 = fast_cube1.tetra(src_x, src_next, weights);
    lerp(w0, w1, NVector::<f32, N>::from(w_w))
}

pub(crate) fn linear_3i_vec3f_direct<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    linear_3i_vec3f(lut, arr, inputs[0], inputs[1], inputs[2])
}

fn linear_3i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    x: f32,
    y: f32,
    z: f32,
) -> NVector<f32, N> {
    let lin_x = x.max(0.0).min(1.0);
    let lin_y = y.max(0.0).min(1.0);
    let lin_z = z.max(0.0).min(1.0);

    let scale_x = lut.grid_scale[0];
    let scale_y = lut.grid_scale[1];
    let scale_z = lut.grid_scale[2];

    let lx = lin_x * scale_x;
    let ly = lin_y * scale_y;
    let lz = lin_z * scale_z;

    let x = lx.floor() as i32;
    let y = ly.floor() as i32;
    let z = lz.floor() as i32;

    let src_x = Vector3i { v: [x, y, z] };

    let x_n = lx.ceil() as i32;
    let y_n = ly.ceil() as i32;
    let z_n = lz.ceil() as i32;

    let src_next = Vector3i { v: [x_n, y_n, z_n] };

    let x_w = lx - x as f32;
    let y_w = ly - y as f32;
    let z_w = lz - z as f32;

    let weights = Vector3f { v: [x_w, y_w, z_w] };

    let fast_cube = FastCube {
        fetch: ArrayFetchVectorN {
            array: arr,
            x_stride: lut.grid_strides[0],
            y_stride: lut.grid_strides[1],
            z_stride: lut.grid_strides[2],
            output_inks: lut.output_inks,
        },
        _phantom: PhantomData,
    };

    fast_cube.tetra(src_x, src_next, weights)
}

pub(crate) fn linear_1i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let lin_x = inputs[0].max(0.0).min(1.0);

    let scale_x = lut.grid_scale[0];

    let lx = lin_x * scale_x;

    let x = lx.floor() as i32;

    let x_n = lx.ceil() as i32;

    let x_w = lx - x as f32;

    let x_stride = lut.grid_strides[0];

    let offset = |xi: i32| -> usize { (xi as u32 * x_stride) as usize * lut.output_inks };

    // Sample 2 corners
    let a = NVector::<f32, N>::from_slice(&arr[offset(x)..][..N].try_into().unwrap());
    let b = NVector::<f32, N>::from_slice(&arr[offset(x_n)..][..N].try_into().unwrap());

    a * NVector::<f32, N>::from(1.0 - x_w) + b * NVector::<f32, N>::from(x_w)
}

pub(crate) fn linear_2i_vec3f_direct<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    linear_2i_vec3f(lut, arr, inputs[0], inputs[1])
}

fn linear_2i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    x: f32,
    y: f32,
) -> NVector<f32, N> {
    let lin_x = x.max(0.0).min(1.0);
    let lin_y = y.max(0.0).min(1.0);

    let scale_x = lut.grid_scale[0];
    let scale_y = lut.grid_scale[1];

    let lx = lin_x * scale_x;
    let ly = lin_y * scale_y;

    let x = lx.floor() as i32;
    let y = ly.floor() as i32;

    let x_n = lx.ceil() as i32;
    let y_n = ly.ceil() as i32;

    let x_w = lx - x as f32;
    let y_w = ly - y as f32;

    let x_stride = lut.grid_strides[0];
    let y_stride = lut.grid_strides[1];

    let offset = |xi: i32, yi: i32| -> usize {
        (xi as u32 * x_stride + yi as u32 * y_stride) as usize * lut.output_inks
    };

    // Sample 4 corners
    let a = NVector::<f32, N>::from_slice(&arr[offset(x, y)..][..N].try_into().unwrap());
    let b = NVector::<f32, N>::from_slice(&arr[offset(x_n, y)..][..N].try_into().unwrap());
    let c = NVector::<f32, N>::from_slice(&arr[offset(x, y_n)..][..N].try_into().unwrap());
    let d = NVector::<f32, N>::from_slice(&arr[offset(x_n, y_n)..][..N].try_into().unwrap());

    let ab = a * NVector::<f32, N>::from(1.0 - x_w) + b * NVector::<f32, N>::from(x_w);
    let cd = c * NVector::<f32, N>::from(1.0 - x_w) + d * NVector::<f32, N>::from(x_w);

    ab * NVector::<f32, N>::from(1.0 - y_w) + cd * NVector::<f32, N>::from(y_w)
}

pub(crate) fn linear_4i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    linear_4i_vec3f_direct(lut, arr, inputs[0], inputs[1], inputs[2], inputs[3])
}

type FHandle<const N: usize> = fn(&MultidimensionalLut, &[f32], &[f32]) -> NVector<f32, N>;

#[inline(never)]
pub(crate) fn linear_n_i_vec3f<
    const N: usize,
    const I: usize,
    Handle: Fn(&MultidimensionalLut, &[f32], &[f32]) -> NVector<f32, N>,
>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
    handle: Handle,
) -> NVector<f32, N> {
    let lin_w = inputs[I];

    let w_c = lin_w.max(0.).min(1.);
    let scale_p = lut.grid_scale[I];
    let wf = w_c * scale_p;
    let w0 = wf.min(scale_p) as usize;
    let w1 = (wf + 1.).min(scale_p) as usize;
    let w = wf - w0 as f32;

    let cube0 = &arr[(w0 * lut.grid_filling_size[I] as usize)..];
    let cube1 = &arr[(w1 * lut.grid_filling_size[I] as usize)..];

    let inputs_sliced = &inputs[0..I];
    let w0 = handle(lut, cube0, inputs_sliced);
    let w1 = handle(lut, cube1, inputs_sliced);
    lerp(w0, w1, NVector::<f32, N>::from(w))
}

#[inline(never)]
pub(crate) fn linear_5i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let lin_w = inputs[4];

    let w_c = lin_w.max(0.).min(1.);
    let scale_p = lut.grid_scale[4];
    let wf = w_c * scale_p;
    let w0 = wf.min(scale_p) as usize;
    let w1 = (wf + 1.).min(scale_p) as usize;
    let w = wf - w0 as f32;

    let cube0 = &arr[(w0 * lut.grid_filling_size[4] as usize)..];
    let cube1 = &arr[(w1 * lut.grid_filling_size[4] as usize)..];

    let w0 = linear_4i_vec3f_direct(lut, cube0, inputs[0], inputs[1], inputs[2], inputs[3]);
    let w1 = linear_4i_vec3f_direct(lut, cube1, inputs[0], inputs[1], inputs[2], inputs[3]);
    lerp(w0, w1, NVector::<f32, N>::from(w))
}

#[inline(never)]
pub(crate) fn linear_6i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_5i_vec3f::<N>;
    linear_n_i_vec3f::<N, 5, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_7i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_6i_vec3f::<N>;
    linear_n_i_vec3f::<N, 6, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_8i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_7i_vec3f::<N>;
    linear_n_i_vec3f::<N, 7, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_9i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_8i_vec3f::<N>;
    linear_n_i_vec3f::<N, 8, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_10i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_9i_vec3f::<N>;
    linear_n_i_vec3f::<N, 9, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_11i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_10i_vec3f::<N>;
    linear_n_i_vec3f::<N, 10, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_12i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_11i_vec3f::<N>;
    linear_n_i_vec3f::<N, 11, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_13i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_12i_vec3f::<N>;
    linear_n_i_vec3f::<N, 12, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_14i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_13i_vec3f::<N>;
    linear_n_i_vec3f::<N, 13, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn linear_15i_vec3f<const N: usize>(
    lut: &MultidimensionalLut,
    arr: &[f32],
    inputs: &[f32],
) -> NVector<f32, N> {
    let f = linear_14i_vec3f::<N>;
    linear_n_i_vec3f::<N, 14, FHandle<N>>(lut, arr, inputs, f)
}

#[inline(never)]
pub(crate) fn tetra_3i_to_any_vec(
    lut: &MultidimensionalLut,
    arr: &[f32],
    x: f32,
    y: f32,
    z: f32,
    dst: &mut [f32],
    inks: usize,
) {
    match inks {
        1 => {
            let vec3 = linear_3i_vec3f::<1>(lut, arr, x, y, z);
            dst[0] = vec3.v[0];
        }
        2 => {
            let vec3 = linear_3i_vec3f::<2>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        3 => {
            let vec3 = linear_3i_vec3f::<3>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        4 => {
            let vec3 = linear_3i_vec3f::<4>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        5 => {
            let vec3 = linear_3i_vec3f::<5>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        6 => {
            let vec3 = linear_3i_vec3f::<6>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        7 => {
            let vec3 = linear_3i_vec3f::<7>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        8 => {
            let vec3 = linear_3i_vec3f::<8>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        9 => {
            let vec3 = linear_3i_vec3f::<9>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        10 => {
            let vec3 = linear_3i_vec3f::<10>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        11 => {
            let vec3 = linear_3i_vec3f::<11>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        12 => {
            let vec3 = linear_3i_vec3f::<12>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        13 => {
            let vec3 = linear_3i_vec3f::<13>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        14 => {
            let vec3 = linear_3i_vec3f::<14>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        15 => {
            let vec3 = linear_3i_vec3f::<15>(lut, arr, x, y, z);
            for (dst, src) in dst.iter_mut().zip(vec3.v.iter()) {
                *dst = *src;
            }
        }
        _ => unreachable!(),
    }
}
