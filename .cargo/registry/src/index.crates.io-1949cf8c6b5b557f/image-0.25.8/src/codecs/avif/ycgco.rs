use crate::codecs::avif::yuv::{
    check_rgb_preconditions, check_yuv_plane_preconditions, qrshr, yuv420_to_rgbx_invoker,
    yuv422_to_rgbx_invoker, CbCrInverseTransform, HalvedRowHandler, PlaneDefinition,
    YuvChromaRange, YuvIntensityRange, YuvPlanarImage, YuvStandardMatrix,
};
use crate::ImageError;
use num_traits::AsPrimitive;

/// Computes YCgCo inverse in limited range
/// # Arguments
/// - `dst` - dest buffer
/// - `y_value` - Y value with subtracted bias
/// - `cb` - Cb value with subtracted bias
/// - `cr` - Cr value with subtracted bias
#[inline(always)]
fn ycgco_execute_limited<
    V: Copy + AsPrimitive<i32> + 'static + Sized,
    const PRECISION: i32,
    const CHANNELS: usize,
    const BIT_DEPTH: usize,
>(
    dst: &mut [V; CHANNELS],
    y_value: i32,
    cg: i32,
    co: i32,
    scale: i32,
) where
    i32: AsPrimitive<V>,
{
    let t0 = y_value - cg;

    let r = qrshr::<PRECISION, BIT_DEPTH>((t0 + co) * scale);
    let b = qrshr::<PRECISION, BIT_DEPTH>((t0 - co) * scale);
    let g = qrshr::<PRECISION, BIT_DEPTH>((y_value + cg) * scale);

    if CHANNELS == 4 {
        dst[0] = r.as_();
        dst[1] = g.as_();
        dst[2] = b.as_();
        dst[3] = ((1i32 << BIT_DEPTH) - 1).as_();
    } else if CHANNELS == 3 {
        dst[0] = r.as_();
        dst[1] = g.as_();
        dst[2] = b.as_();
    } else {
        unreachable!();
    }
}

/// Computes YCgCo inverse in full range
/// # Arguments
/// - `dst` - dest buffer
/// - `y_value` - Y value with subtracted bias
/// - `cb` - Cb value with subtracted bias
/// - `cr` - Cr value with subtracted bias
#[inline(always)]
fn ycgco_execute_full<
    V: Copy + AsPrimitive<i32> + 'static + Sized,
    const PRECISION: i32,
    const CHANNELS: usize,
    const BIT_DEPTH: usize,
>(
    dst: &mut [V; CHANNELS],
    y_value: i32,
    cg: i32,
    co: i32,
) where
    i32: AsPrimitive<V>,
{
    let t0 = y_value - cg;

    let max_value = (1i32 << BIT_DEPTH) - 1;

    let r = (t0 + co).clamp(0, max_value);
    let b = (t0 - co).clamp(0, max_value);
    let g = (y_value + cg).clamp(0, max_value);

    if CHANNELS == 4 {
        dst[0] = r.as_();
        dst[1] = g.as_();
        dst[2] = b.as_();
        dst[3] = max_value.as_();
    } else if CHANNELS == 3 {
        dst[0] = r.as_();
        dst[1] = g.as_();
        dst[2] = b.as_();
    } else {
        unreachable!();
    }
}

#[inline(always)]
fn process_halved_chroma_row_cgco<
    V: Copy + AsPrimitive<i32> + 'static + Sized,
    const PRECISION: i32,
    const CHANNELS: usize,
    const BIT_DEPTH: usize,
>(
    image: YuvPlanarImage<V>,
    rgba: &mut [V],
    _: &CbCrInverseTransform<i32>,
    range: &YuvChromaRange,
) where
    i32: AsPrimitive<V>,
{
    let max_value = (1i32 << BIT_DEPTH) - 1;

    // If the stride is larger than the plane size,
    // it might contain junk data beyond the actual valid region.
    // To avoid processing artifacts when working with odd-sized images,
    // the buffer is reshaped to its actual size,
    // preventing accidental use of invalid values from the trailing region.

    let y_plane = &image.y_plane[..image.width];
    let chroma_size = image.width.div_ceil(2);
    let u_plane = &image.u_plane[..chroma_size];
    let v_plane = &image.v_plane[..chroma_size];
    let rgba = &mut rgba[..image.width * CHANNELS];

    let bias_y = range.bias_y as i32;
    let bias_uv = range.bias_uv as i32;
    let y_iter = y_plane.chunks_exact(2);
    let rgb_chunks = rgba.chunks_exact_mut(CHANNELS * 2);

    let scale_coef = ((max_value as f32 / range.range_y as f32) * (1 << PRECISION) as f32) as i32;

    for (((y_src, &u_src), &v_src), rgb_dst) in y_iter.zip(u_plane).zip(v_plane).zip(rgb_chunks) {
        let y_value0: i32 = y_src[0].as_() - bias_y;
        let cg_value: i32 = u_src.as_() - bias_uv;
        let co_value: i32 = v_src.as_() - bias_uv;

        let dst0 = &mut rgb_dst[..CHANNELS];

        ycgco_execute_limited::<V, PRECISION, CHANNELS, BIT_DEPTH>(
            dst0.try_into().unwrap(),
            y_value0,
            cg_value,
            co_value,
            scale_coef,
        );

        let y_value1 = y_src[1].as_() - bias_y;

        let dst1 = &mut rgb_dst[CHANNELS..2 * CHANNELS];

        ycgco_execute_limited::<V, PRECISION, CHANNELS, BIT_DEPTH>(
            dst1.try_into().unwrap(),
            y_value1,
            cg_value,
            co_value,
            scale_coef,
        );
    }

    // Process remainder if width is odd.
    if image.width & 1 != 0 {
        let y_left = y_plane.chunks_exact(2).remainder();
        let rgb_chunks = rgba
            .chunks_exact_mut(CHANNELS * 2)
            .into_remainder()
            .chunks_exact_mut(CHANNELS);
        let u_iter = u_plane.iter().rev();
        let v_iter = v_plane.iter().rev();

        for (((y_src, u_src), v_src), rgb_dst) in
            y_left.iter().zip(u_iter).zip(v_iter).zip(rgb_chunks)
        {
            let y_value = y_src.as_() - bias_y;
            let cg_value = u_src.as_() - bias_uv;
            let co_value = v_src.as_() - bias_uv;

            ycgco_execute_limited::<V, PRECISION, CHANNELS, BIT_DEPTH>(
                rgb_dst.try_into().unwrap(),
                y_value,
                cg_value,
                co_value,
                scale_coef,
            );
        }
    }
}

/// Converts YCgCo 444 planar format to Rgba
///
/// # Arguments
///
/// * `image`: see [YuvPlanarImage]
/// * `rgba`: RGB image layout
/// * `range`: see [YuvIntensityRange]
///
fn ycgco444_to_rgbx_impl<
    V: Copy + AsPrimitive<i32> + 'static + Sized,
    const CHANNELS: usize,
    const BIT_DEPTH: usize,
>(
    image: YuvPlanarImage<V>,
    rgba: &mut [V],
    yuv_range: YuvIntensityRange,
) -> Result<(), ImageError>
where
    i32: AsPrimitive<V>,
{
    assert!(
        CHANNELS == 3 || CHANNELS == 4,
        "YUV 4:4:4 -> RGB is implemented only on 3 and 4 channels"
    );
    assert!(
        (8..=16).contains(&BIT_DEPTH),
        "Invalid bit depth is provided"
    );
    assert!(
        if BIT_DEPTH > 8 {
            size_of::<V>() == 2
        } else {
            size_of::<V>() == 1
        },
        "Unsupported bit depth and data type combination"
    );

    let y_plane = image.y_plane;
    let u_plane = image.u_plane;
    let v_plane = image.v_plane;
    let y_stride = image.y_stride;
    let u_stride = image.u_stride;
    let v_stride = image.v_stride;
    let height = image.height;
    let width = image.width;

    check_yuv_plane_preconditions(y_plane, PlaneDefinition::Y, y_stride, height)?;
    check_yuv_plane_preconditions(u_plane, PlaneDefinition::U, u_stride, height)?;
    check_yuv_plane_preconditions(v_plane, PlaneDefinition::V, v_stride, height)?;

    check_rgb_preconditions(rgba, image.width * CHANNELS, height)?;

    let range = yuv_range.get_yuv_range(BIT_DEPTH as u32);
    const PRECISION: i32 = 13;

    let bias_y = range.bias_y as i32;
    let bias_uv = range.bias_uv as i32;

    let rgb_stride = width * CHANNELS;

    let y_iter = y_plane.chunks_exact(y_stride);
    let rgb_iter = rgba.chunks_exact_mut(rgb_stride);
    let u_iter = u_plane.chunks_exact(u_stride);
    let v_iter = v_plane.chunks_exact(v_stride);

    let max_value: i32 = (1 << BIT_DEPTH) - 1;

    // All branches on generic const will be optimized out.
    for (((y_src, u_src), v_src), rgb) in y_iter.zip(u_iter).zip(v_iter).zip(rgb_iter) {
        let rgb_chunks = rgb.chunks_exact_mut(CHANNELS);
        match yuv_range {
            YuvIntensityRange::Tv => {
                let y_coef =
                    ((max_value as f32 / range.range_y as f32) * (1 << PRECISION) as f32) as i32;
                for (((y_src, u_src), v_src), rgb_dst) in
                    y_src.iter().zip(u_src).zip(v_src).zip(rgb_chunks)
                {
                    let y_value = y_src.as_() - bias_y;
                    let cg_value = u_src.as_() - bias_uv;
                    let co_value = v_src.as_() - bias_uv;

                    ycgco_execute_limited::<V, PRECISION, CHANNELS, BIT_DEPTH>(
                        rgb_dst.try_into().unwrap(),
                        y_value,
                        cg_value,
                        co_value,
                        y_coef,
                    );
                }
            }
            YuvIntensityRange::Pc => {
                for (((y_src, u_src), v_src), rgb_dst) in
                    y_src.iter().zip(u_src).zip(v_src).zip(rgb_chunks)
                {
                    let y_value = y_src.as_() - bias_y;
                    let cg_value = u_src.as_() - bias_uv;
                    let co_value = v_src.as_() - bias_uv;

                    ycgco_execute_full::<V, PRECISION, CHANNELS, BIT_DEPTH>(
                        rgb_dst.try_into().unwrap(),
                        y_value,
                        cg_value,
                        co_value,
                    );
                }
            }
        }
    }

    Ok(())
}

macro_rules! define_ycgco_half_chroma {
    ($name: ident, $invoker: ident, $storage: ident, $cn: expr, $bp: expr, $description: expr) => {
        #[doc = concat!($description, "
        
        # Arguments
        
        * `image`: see [YuvPlanarImage]
        * `rgb`: RGB image layout
        * `range`: see [YuvIntensityRange]
        * `matrix`: see [YuvStandardMatrix]")]
        pub(crate) fn $name(
            image: YuvPlanarImage<$storage>,
            rgb: &mut [$storage],
            range: YuvIntensityRange,
        ) -> Result<(), ImageError> {
            const P: i32 = 13;
            $invoker::<$storage, HalvedRowHandler<$storage>, P, $cn, $bp>(
                image,
                rgb,
                range,
                YuvStandardMatrix::Bt709,
                process_halved_chroma_row_cgco::<$storage, P, $cn, $bp>,
            )
        }
    };
}

const RGBA_CN: usize = 4;

define_ycgco_half_chroma!(
    ycgco420_to_rgba8,
    yuv420_to_rgbx_invoker,
    u8,
    RGBA_CN,
    8,
    "Converts YCgCo 420 8-bit planar format to Rgba 8-bit"
);

define_ycgco_half_chroma!(
    ycgco422_to_rgba8,
    yuv422_to_rgbx_invoker,
    u8,
    RGBA_CN,
    8,
    "Converts YCgCo 420 8-bit planar format to Rgba 8-bit"
);

define_ycgco_half_chroma!(
    ycgco420_to_rgba10,
    yuv420_to_rgbx_invoker,
    u16,
    RGBA_CN,
    10,
    "Converts YCgCo 420 10-bit planar format to Rgba 10-bit"
);

define_ycgco_half_chroma!(
    ycgco422_to_rgba10,
    yuv422_to_rgbx_invoker,
    u16,
    RGBA_CN,
    10,
    "Converts YCgCo 422 10-bit planar format to Rgba 10-bit"
);

define_ycgco_half_chroma!(
    ycgco420_to_rgba12,
    yuv420_to_rgbx_invoker,
    u16,
    RGBA_CN,
    12,
    "Converts YCgCo 420 12-bit planar format to Rgba 12-bit"
);

define_ycgco_half_chroma!(
    ycgco422_to_rgba12,
    yuv422_to_rgbx_invoker,
    u16,
    RGBA_CN,
    12,
    "Converts YCgCo 422 12-bit planar format to Rgba 12-bit"
);

macro_rules! define_ycgcg_full_chroma {
    ($name: ident, $storage: ident, $cn: expr, $bp: expr, $description: expr) => {
        #[doc = concat!($description, "
        
        # Arguments
        
        * `image`: see [YuvPlanarImage]
        * `rgba`: RGB image layout
        * `range`: see [YuvIntensityRange]
        * `matrix`: see [YuvStandardMatrix]
        ")]
        pub(crate) fn $name(
            image: YuvPlanarImage<$storage>,
            rgba: &mut [$storage],
            range: YuvIntensityRange,
        ) -> Result<(), ImageError> {
            ycgco444_to_rgbx_impl::<$storage, $cn, $bp>(image, rgba, range)
        }
    };
}

define_ycgcg_full_chroma!(
    ycgco444_to_rgba8,
    u8,
    RGBA_CN,
    8,
    "Converts YCgCo 444 planar format 8 bit-depth to Rgba 8 bit"
);
define_ycgcg_full_chroma!(
    ycgco444_to_rgba10,
    u16,
    RGBA_CN,
    10,
    "Converts YCgCo 444 planar format 10 bit-depth to Rgba 10 bit"
);
define_ycgcg_full_chroma!(
    ycgco444_to_rgba12,
    u16,
    RGBA_CN,
    12,
    "Converts YCgCo 444 planar format 12 bit-depth to Rgba 12 bit"
);
