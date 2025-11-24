/*
 * // Copyright (c) Radzivon Bartoshyk 2/2025. All rights reserved.
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
use crate::{CmsError, Layout, Matrix3, Matrix3f, TransformExecutor};
use num_traits::AsPrimitive;

pub(crate) struct TransformMatrixShaper<T: Clone, const BUCKET: usize> {
    pub(crate) r_linear: Box<[f32; BUCKET]>,
    pub(crate) g_linear: Box<[f32; BUCKET]>,
    pub(crate) b_linear: Box<[f32; BUCKET]>,
    pub(crate) r_gamma: Box<[T; 65536]>,
    pub(crate) g_gamma: Box<[T; 65536]>,
    pub(crate) b_gamma: Box<[T; 65536]>,
    pub(crate) adaptation_matrix: Matrix3f,
}

/// Low memory footprint optimized routine for matrix shaper profiles with the same
/// Gamma and linear curves.
pub(crate) struct TransformMatrixShaperOptimized<T: Clone, const BUCKET: usize> {
    pub(crate) linear: Box<[f32; BUCKET]>,
    pub(crate) gamma: Box<[T; 65536]>,
    pub(crate) adaptation_matrix: Matrix3f,
}

impl<T: Clone + PointeeSizeExpressible, const BUCKET: usize> TransformMatrixShaper<T, BUCKET> {
    pub(crate) fn to_q2_13_n<
        R: Copy + 'static + Default,
        const PRECISION: i32,
        const LINEAR_CAP: usize,
        const GAMMA_LUT: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
    ) -> TransformMatrixShaperFixedPoint<R, T, BUCKET>
    where
        f32: AsPrimitive<R>,
    {
        let linear_scale = if T::FINITE {
            let lut_scale = (GAMMA_LUT - 1) as f32 / ((1 << BIT_DEPTH) - 1) as f32;
            ((1 << BIT_DEPTH) - 1) as f32 * lut_scale
        } else {
            let lut_scale = (GAMMA_LUT - 1) as f32 / (T::NOT_FINITE_LINEAR_TABLE_SIZE - 1) as f32;
            (T::NOT_FINITE_LINEAR_TABLE_SIZE - 1) as f32 * lut_scale
        };
        let mut new_box_r = Box::new([R::default(); BUCKET]);
        let mut new_box_g = Box::new([R::default(); BUCKET]);
        let mut new_box_b = Box::new([R::default(); BUCKET]);
        for (dst, &src) in new_box_r.iter_mut().zip(self.r_linear.iter()) {
            *dst = (src * linear_scale).round().as_();
        }
        for (dst, &src) in new_box_g.iter_mut().zip(self.g_linear.iter()) {
            *dst = (src * linear_scale).round().as_();
        }
        for (dst, &src) in new_box_b.iter_mut().zip(self.b_linear.iter()) {
            *dst = (src * linear_scale).round().as_();
        }
        let scale: f32 = (1i32 << PRECISION) as f32;
        let source_matrix = self.adaptation_matrix;
        let mut dst_matrix = Matrix3::<i16> { v: [[0i16; 3]; 3] };
        for i in 0..3 {
            for j in 0..3 {
                dst_matrix.v[i][j] = (source_matrix.v[i][j] * scale) as i16;
            }
        }
        TransformMatrixShaperFixedPoint {
            r_linear: new_box_r,
            g_linear: new_box_g,
            b_linear: new_box_b,
            r_gamma: self.r_gamma.clone(),
            g_gamma: self.g_gamma.clone(),
            b_gamma: self.b_gamma.clone(),
            adaptation_matrix: dst_matrix,
        }
    }
}

impl<T: Clone + PointeeSizeExpressible, const BUCKET: usize>
    TransformMatrixShaperOptimized<T, BUCKET>
{
    pub(crate) fn to_q2_13_n<
        R: Copy + 'static + Default,
        const PRECISION: i32,
        const LINEAR_CAP: usize,
        const GAMMA_LUT: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
    ) -> TransformMatrixShaperFixedPointOpt<R, i16, T, BUCKET>
    where
        f32: AsPrimitive<R>,
    {
        let linear_scale = if T::FINITE {
            let lut_scale = (GAMMA_LUT - 1) as f32 / ((1 << BIT_DEPTH) - 1) as f32;
            ((1 << BIT_DEPTH) - 1) as f32 * lut_scale
        } else {
            let lut_scale = (GAMMA_LUT - 1) as f32 / (T::NOT_FINITE_LINEAR_TABLE_SIZE - 1) as f32;
            (T::NOT_FINITE_LINEAR_TABLE_SIZE - 1) as f32 * lut_scale
        };
        let mut new_box_linear = Box::new([R::default(); BUCKET]);
        for (dst, src) in new_box_linear.iter_mut().zip(self.linear.iter()) {
            *dst = (*src * linear_scale).round().as_();
        }
        let scale: f32 = (1i32 << PRECISION) as f32;
        let source_matrix = self.adaptation_matrix;
        let mut dst_matrix = Matrix3::<i16> {
            v: [[i16::default(); 3]; 3],
        };
        for i in 0..3 {
            for j in 0..3 {
                dst_matrix.v[i][j] = (source_matrix.v[i][j] * scale) as i16;
            }
        }
        TransformMatrixShaperFixedPointOpt {
            linear: new_box_linear,
            gamma: self.gamma.clone(),
            adaptation_matrix: dst_matrix,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn to_q1_30_n<
        R: Copy + 'static + Default,
        const PRECISION: i32,
        const LINEAR_CAP: usize,
        const GAMMA_LUT: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
    ) -> TransformMatrixShaperFixedPointOpt<R, i32, T, BUCKET>
    where
        f32: AsPrimitive<R>,
        f64: AsPrimitive<R>,
    {
        // It is important to scale 1 bit more to compensate vqrdmlah Q0.31, because we're going to use Q1.30
        let table_size = if T::FINITE {
            (1 << BIT_DEPTH) - 1
        } else {
            T::NOT_FINITE_LINEAR_TABLE_SIZE - 1
        };
        let ext_bp = if T::FINITE {
            BIT_DEPTH as u32 + 1
        } else {
            let bp = (T::NOT_FINITE_LINEAR_TABLE_SIZE - 1).count_ones();
            bp + 1
        };
        let linear_scale = {
            let lut_scale = (GAMMA_LUT - 1) as f64 / table_size as f64;
            ((1u32 << ext_bp) - 1) as f64 * lut_scale
        };
        let mut new_box_linear = Box::new([R::default(); BUCKET]);
        for (dst, &src) in new_box_linear.iter_mut().zip(self.linear.iter()) {
            *dst = (src as f64 * linear_scale).round().as_();
        }
        let scale: f64 = (1i64 << PRECISION) as f64;
        let source_matrix = self.adaptation_matrix;
        let mut dst_matrix = Matrix3::<i32> {
            v: [[i32::default(); 3]; 3],
        };
        for i in 0..3 {
            for j in 0..3 {
                dst_matrix.v[i][j] = (source_matrix.v[i][j] as f64 * scale) as i32;
            }
        }
        TransformMatrixShaperFixedPointOpt {
            linear: new_box_linear,
            gamma: self.gamma.clone(),
            adaptation_matrix: dst_matrix,
        }
    }
}

#[allow(unused)]
struct TransformMatrixShaperScalar<
    T: Clone,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const BIT_DEPTH: usize,
> {
    pub(crate) profile: TransformMatrixShaper<T, LINEAR_CAP>,
}

#[allow(unused)]
struct TransformMatrixShaperOptScalar<
    T: Clone,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const BIT_DEPTH: usize,
> {
    pub(crate) profile: TransformMatrixShaperOptimized<T, LINEAR_CAP>,
}

#[cfg(any(
    any(target_arch = "x86", target_arch = "x86_64"),
    all(target_arch = "aarch64", target_feature = "neon")
))]
#[allow(unused)]
macro_rules! create_rgb_xyz_dependant_executor {
    ($dep_name: ident, $dependant: ident, $shaper: ident) => {
        pub(crate) fn $dep_name<
            T: Clone + Send + Sync + Default + PointeeSizeExpressible + Copy + 'static,
            const LINEAR_CAP: usize,
            const GAMMA_LUT: usize,
            const BIT_DEPTH: usize,
        >(
            src_layout: Layout,
            dst_layout: Layout,
            profile: $shaper<T, LINEAR_CAP>,
        ) -> Result<Box<dyn TransformExecutor<T> + Send + Sync>, CmsError>
        where
            u32: AsPrimitive<T>,
        {
            if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgba) {
                return Ok(Box::new($dependant::<
                    T,
                    { Layout::Rgba as u8 },
                    { Layout::Rgba as u8 },
                    LINEAR_CAP,
                    GAMMA_LUT,
                > {
                    profile,
                    bit_depth: BIT_DEPTH,
                }));
            } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgba) {
                return Ok(Box::new($dependant::<
                    T,
                    { Layout::Rgb as u8 },
                    { Layout::Rgba as u8 },
                    LINEAR_CAP,
                    GAMMA_LUT,
                > {
                    profile,
                    bit_depth: BIT_DEPTH,
                }));
            } else if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgb) {
                return Ok(Box::new($dependant::<
                    T,
                    { Layout::Rgba as u8 },
                    { Layout::Rgb as u8 },
                    LINEAR_CAP,
                    GAMMA_LUT,
                > {
                    profile,
                    bit_depth: BIT_DEPTH,
                }));
            } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgb) {
                return Ok(Box::new($dependant::<
                    T,
                    { Layout::Rgb as u8 },
                    { Layout::Rgb as u8 },
                    LINEAR_CAP,
                    GAMMA_LUT,
                > {
                    profile,
                    bit_depth: BIT_DEPTH,
                }));
            }
            Err(CmsError::UnsupportedProfileConnection)
        }
    };
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "sse"))]
use crate::conversions::sse::{TransformShaperRgbOptSse, TransformShaperRgbSse};

#[cfg(all(target_arch = "x86_64", feature = "avx"))]
use crate::conversions::avx::{TransformShaperRgbAvx, TransformShaperRgbOptAvx};

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "sse"))]
create_rgb_xyz_dependant_executor!(
    make_rgb_xyz_rgb_transform_sse_41,
    TransformShaperRgbSse,
    TransformMatrixShaper
);

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "sse"))]
create_rgb_xyz_dependant_executor!(
    make_rgb_xyz_rgb_transform_sse_41_opt,
    TransformShaperRgbOptSse,
    TransformMatrixShaperOptimized
);

#[cfg(all(target_arch = "x86_64", feature = "avx"))]
create_rgb_xyz_dependant_executor!(
    make_rgb_xyz_rgb_transform_avx2,
    TransformShaperRgbAvx,
    TransformMatrixShaper
);

#[cfg(all(target_arch = "x86_64", feature = "avx"))]
create_rgb_xyz_dependant_executor!(
    make_rgb_xyz_rgb_transform_avx2_opt,
    TransformShaperRgbOptAvx,
    TransformMatrixShaperOptimized
);

#[cfg(all(target_arch = "x86_64", feature = "avx512"))]
use crate::conversions::avx512::TransformShaperRgbOptAvx512;

#[cfg(all(target_arch = "x86_64", feature = "avx512"))]
create_rgb_xyz_dependant_executor!(
    make_rgb_xyz_rgb_transform_avx512_opt,
    TransformShaperRgbOptAvx512,
    TransformMatrixShaperOptimized
);

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon", feature = "neon")))]
pub(crate) fn make_rgb_xyz_rgb_transform<
    T: Clone + Send + Sync + PointeeSizeExpressible + 'static + Copy + Default,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const BIT_DEPTH: usize,
>(
    src_layout: Layout,
    dst_layout: Layout,
    profile: TransformMatrixShaper<T, LINEAR_CAP>,
) -> Result<Box<dyn TransformExecutor<T> + Send + Sync>, CmsError>
where
    u32: AsPrimitive<T>,
{
    #[cfg(all(feature = "avx", target_arch = "x86_64"))]
    if std::arch::is_x86_feature_detected!("avx2") {
        return make_rgb_xyz_rgb_transform_avx2::<T, LINEAR_CAP, GAMMA_LUT, BIT_DEPTH>(
            src_layout, dst_layout, profile,
        );
    }
    #[cfg(all(feature = "sse", any(target_arch = "x86", target_arch = "x86_64")))]
    if std::arch::is_x86_feature_detected!("sse4.1") {
        return make_rgb_xyz_rgb_transform_sse_41::<T, LINEAR_CAP, GAMMA_LUT, BIT_DEPTH>(
            src_layout, dst_layout, profile,
        );
    }
    if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgba) {
        return Ok(Box::new(TransformMatrixShaperScalar::<
            T,
            { Layout::Rgba as u8 },
            { Layout::Rgba as u8 },
            LINEAR_CAP,
            GAMMA_LUT,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgba) {
        return Ok(Box::new(TransformMatrixShaperScalar::<
            T,
            { Layout::Rgb as u8 },
            { Layout::Rgba as u8 },
            LINEAR_CAP,
            GAMMA_LUT,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgb) {
        return Ok(Box::new(TransformMatrixShaperScalar::<
            T,
            { Layout::Rgba as u8 },
            { Layout::Rgb as u8 },
            LINEAR_CAP,
            GAMMA_LUT,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgb) {
        return Ok(Box::new(TransformMatrixShaperScalar::<
            T,
            { Layout::Rgb as u8 },
            { Layout::Rgb as u8 },
            LINEAR_CAP,
            GAMMA_LUT,
            BIT_DEPTH,
        > {
            profile,
        }));
    }
    Err(CmsError::UnsupportedProfileConnection)
}

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon", feature = "neon")))]
pub(crate) fn make_rgb_xyz_rgb_transform_opt<
    T: Clone + Send + Sync + PointeeSizeExpressible + 'static + Copy + Default,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const BIT_DEPTH: usize,
>(
    src_layout: Layout,
    dst_layout: Layout,
    profile: TransformMatrixShaperOptimized<T, LINEAR_CAP>,
) -> Result<Box<dyn TransformExecutor<T> + Send + Sync>, CmsError>
where
    u32: AsPrimitive<T>,
{
    #[cfg(all(feature = "avx512", target_arch = "x86_64"))]
    if std::arch::is_x86_feature_detected!("avx512bw")
        && std::arch::is_x86_feature_detected!("avx512vl")
        && std::arch::is_x86_feature_detected!("fma")
    {
        return make_rgb_xyz_rgb_transform_avx512_opt::<T, LINEAR_CAP, GAMMA_LUT, BIT_DEPTH>(
            src_layout, dst_layout, profile,
        );
    }
    #[cfg(all(feature = "avx", target_arch = "x86_64"))]
    if std::arch::is_x86_feature_detected!("avx2") {
        return make_rgb_xyz_rgb_transform_avx2_opt::<T, LINEAR_CAP, GAMMA_LUT, BIT_DEPTH>(
            src_layout, dst_layout, profile,
        );
    }
    #[cfg(all(feature = "sse", any(target_arch = "x86", target_arch = "x86_64")))]
    if std::arch::is_x86_feature_detected!("sse4.1") {
        return make_rgb_xyz_rgb_transform_sse_41_opt::<T, LINEAR_CAP, GAMMA_LUT, BIT_DEPTH>(
            src_layout, dst_layout, profile,
        );
    }
    if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgba) {
        return Ok(Box::new(TransformMatrixShaperOptScalar::<
            T,
            { Layout::Rgba as u8 },
            { Layout::Rgba as u8 },
            LINEAR_CAP,
            GAMMA_LUT,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgba) {
        return Ok(Box::new(TransformMatrixShaperOptScalar::<
            T,
            { Layout::Rgb as u8 },
            { Layout::Rgba as u8 },
            LINEAR_CAP,
            GAMMA_LUT,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgba) && (dst_layout == Layout::Rgb) {
        return Ok(Box::new(TransformMatrixShaperOptScalar::<
            T,
            { Layout::Rgba as u8 },
            { Layout::Rgb as u8 },
            LINEAR_CAP,
            GAMMA_LUT,
            BIT_DEPTH,
        > {
            profile,
        }));
    } else if (src_layout == Layout::Rgb) && (dst_layout == Layout::Rgb) {
        return Ok(Box::new(TransformMatrixShaperOptScalar::<
            T,
            { Layout::Rgb as u8 },
            { Layout::Rgb as u8 },
            LINEAR_CAP,
            GAMMA_LUT,
            BIT_DEPTH,
        > {
            profile,
        }));
    }
    Err(CmsError::UnsupportedProfileConnection)
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon", feature = "neon"))]
use crate::conversions::neon::{TransformShaperRgbNeon, TransformShaperRgbOptNeon};
use crate::conversions::rgbxyz_fixed::{
    TransformMatrixShaperFixedPoint, TransformMatrixShaperFixedPointOpt,
};
use crate::transform::PointeeSizeExpressible;

#[cfg(all(target_arch = "aarch64", target_feature = "neon", feature = "neon"))]
create_rgb_xyz_dependant_executor!(
    make_rgb_xyz_rgb_transform,
    TransformShaperRgbNeon,
    TransformMatrixShaper
);

#[cfg(all(target_arch = "aarch64", target_feature = "neon", feature = "neon"))]
create_rgb_xyz_dependant_executor!(
    make_rgb_xyz_rgb_transform_opt,
    TransformShaperRgbOptNeon,
    TransformMatrixShaperOptimized
);

#[allow(unused)]
impl<
    T: Clone + PointeeSizeExpressible + Copy + Default + 'static,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const BIT_DEPTH: usize,
> TransformExecutor<T>
    for TransformMatrixShaperScalar<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT, BIT_DEPTH>
where
    u32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        use crate::mlaf::mlaf;
        let src_cn = Layout::from(SRC_LAYOUT);
        let dst_cn = Layout::from(DST_LAYOUT);
        let src_channels = src_cn.channels();
        let dst_channels = dst_cn.channels();

        if src.len() / src_channels != dst.len() / dst_channels {
            return Err(CmsError::LaneSizeMismatch);
        }
        if src.len() % src_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % dst_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }

        let transform = self.profile.adaptation_matrix;
        let scale = (GAMMA_LUT - 1) as f32;
        let max_colors: T = ((1 << BIT_DEPTH) - 1).as_();

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let r = self.profile.r_linear[src[src_cn.r_i()]._as_usize()];
            let g = self.profile.g_linear[src[src_cn.g_i()]._as_usize()];
            let b = self.profile.b_linear[src[src_cn.b_i()]._as_usize()];
            let a = if src_channels == 4 {
                src[src_cn.a_i()]
            } else {
                max_colors
            };

            let new_r = mlaf(
                0.5f32,
                mlaf(
                    mlaf(r * transform.v[0][0], g, transform.v[0][1]),
                    b,
                    transform.v[0][2],
                )
                .max(0f32)
                .min(1f32),
                scale,
            );

            let new_g = mlaf(
                0.5f32,
                mlaf(
                    mlaf(r * transform.v[1][0], g, transform.v[1][1]),
                    b,
                    transform.v[1][2],
                )
                .max(0f32)
                .min(1f32),
                scale,
            );

            let new_b = mlaf(
                0.5f32,
                mlaf(
                    mlaf(r * transform.v[2][0], g, transform.v[2][1]),
                    b,
                    transform.v[2][2],
                )
                .max(0f32)
                .min(1f32),
                scale,
            );

            dst[dst_cn.r_i()] = self.profile.r_gamma[(new_r as u16) as usize];
            dst[dst_cn.g_i()] = self.profile.g_gamma[(new_g as u16) as usize];
            dst[dst_cn.b_i()] = self.profile.b_gamma[(new_b as u16) as usize];
            if dst_channels == 4 {
                dst[dst_cn.a_i()] = a;
            }
        }

        Ok(())
    }
}

#[allow(unused)]
impl<
    T: Clone + PointeeSizeExpressible + Copy + Default + 'static,
    const SRC_LAYOUT: u8,
    const DST_LAYOUT: u8,
    const LINEAR_CAP: usize,
    const GAMMA_LUT: usize,
    const BIT_DEPTH: usize,
> TransformExecutor<T>
    for TransformMatrixShaperOptScalar<T, SRC_LAYOUT, DST_LAYOUT, LINEAR_CAP, GAMMA_LUT, BIT_DEPTH>
where
    u32: AsPrimitive<T>,
{
    fn transform(&self, src: &[T], dst: &mut [T]) -> Result<(), CmsError> {
        use crate::mlaf::mlaf;
        let src_cn = Layout::from(SRC_LAYOUT);
        let dst_cn = Layout::from(DST_LAYOUT);
        let src_channels = src_cn.channels();
        let dst_channels = dst_cn.channels();

        if src.len() / src_channels != dst.len() / dst_channels {
            return Err(CmsError::LaneSizeMismatch);
        }
        if src.len() % src_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }
        if dst.len() % dst_channels != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }

        let transform = self.profile.adaptation_matrix;
        let scale = (GAMMA_LUT - 1) as f32;
        let max_colors: T = ((1 << BIT_DEPTH) - 1).as_();

        for (src, dst) in src
            .chunks_exact(src_channels)
            .zip(dst.chunks_exact_mut(dst_channels))
        {
            let r = self.profile.linear[src[src_cn.r_i()]._as_usize()];
            let g = self.profile.linear[src[src_cn.g_i()]._as_usize()];
            let b = self.profile.linear[src[src_cn.b_i()]._as_usize()];
            let a = if src_channels == 4 {
                src[src_cn.a_i()]
            } else {
                max_colors
            };

            let new_r = mlaf(
                0.5f32,
                mlaf(
                    mlaf(r * transform.v[0][0], g, transform.v[0][1]),
                    b,
                    transform.v[0][2],
                )
                .max(0f32)
                .min(1f32),
                scale,
            );

            let new_g = mlaf(
                0.5f32,
                mlaf(
                    mlaf(r * transform.v[1][0], g, transform.v[1][1]),
                    b,
                    transform.v[1][2],
                )
                .max(0f32)
                .min(1f32),
                scale,
            );

            let new_b = mlaf(
                0.5f32,
                mlaf(
                    mlaf(r * transform.v[2][0], g, transform.v[2][1]),
                    b,
                    transform.v[2][2],
                )
                .max(0f32)
                .min(1f32),
                scale,
            );

            dst[dst_cn.r_i()] = self.profile.gamma[(new_r as u16) as usize];
            dst[dst_cn.g_i()] = self.profile.gamma[(new_g as u16) as usize];
            dst[dst_cn.b_i()] = self.profile.gamma[(new_b as u16) as usize];
            if dst_channels == 4 {
                dst[dst_cn.a_i()] = a;
            }
        }

        Ok(())
    }
}
