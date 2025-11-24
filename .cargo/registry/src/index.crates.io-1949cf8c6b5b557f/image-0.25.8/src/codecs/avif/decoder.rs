//! Decoding of AVIF images.
use crate::error::{
    DecodingError, ImageFormatHint, LimitError, LimitErrorKind, UnsupportedError,
    UnsupportedErrorKind,
};
use crate::{ColorType, ImageDecoder, ImageError, ImageFormat, ImageResult};
///
/// The [AVIF] specification defines an image derivative of the AV1 bitstream, an open video codec.
///
/// [AVIF]: https://aomediacodec.github.io/av1-avif/
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::marker::PhantomData;

use crate::codecs::avif::ycgco::{
    ycgco420_to_rgba10, ycgco420_to_rgba12, ycgco420_to_rgba8, ycgco422_to_rgba10,
    ycgco422_to_rgba12, ycgco422_to_rgba8, ycgco444_to_rgba10, ycgco444_to_rgba12,
    ycgco444_to_rgba8,
};
use crate::codecs::avif::yuv::*;
use dav1d::{PixelLayout, PlanarImageComponent};
use mp4parse::{read_avif, ParseStrictness};

fn error_map<E: Into<Box<dyn Error + Send + Sync>>>(err: E) -> ImageError {
    ImageError::Decoding(DecodingError::new(ImageFormat::Avif.into(), err))
}

/// AVIF Decoder.
///
/// Reads one image into the chosen input.
pub struct AvifDecoder<R> {
    inner: PhantomData<R>,
    picture: dav1d::Picture,
    alpha_picture: Option<dav1d::Picture>,
    icc_profile: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AvifDecoderError {
    AlphaPlaneFormat(PixelLayout),
    YuvLayoutOnIdentityMatrix(PixelLayout),
    UnsupportedLayoutAndMatrix(PixelLayout, YuvMatrixStrategy),
}

impl Display for AvifDecoderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AvifDecoderError::AlphaPlaneFormat(pixel_layout) => match pixel_layout {
                PixelLayout::I400 => unreachable!("This option must be handled correctly"),
                PixelLayout::I420 => f.write_str("Alpha layout must be 4:0:0, but it was 4:2:0"),
                PixelLayout::I422 => f.write_str("Alpha layout must be 4:0:0, but it was 4:2:2"),
                PixelLayout::I444 => f.write_str("Alpha layout must be 4:0:0, but it was 4:4:4"),
            },
            AvifDecoderError::YuvLayoutOnIdentityMatrix(pixel_layout) => match pixel_layout {
                PixelLayout::I400 => {
                    f.write_str("YUV layout on 'Identity' matrix must be 4:4:4, but it was 4:0:0")
                }
                PixelLayout::I420 => {
                    f.write_str("YUV layout on 'Identity' matrix must be 4:4:4, but it was 4:2:0")
                }
                PixelLayout::I422 => {
                    f.write_str("YUV layout on 'Identity' matrix must be 4:4:4, but it was 4:2:2")
                }
                PixelLayout::I444 => unreachable!("This option must be handled correctly"),
            },
            AvifDecoderError::UnsupportedLayoutAndMatrix(layout, matrix) => f.write_fmt(
                format_args!("YUV layout {layout:?} on matrix {matrix:?} is not supported",),
            ),
        }
    }
}

impl Error for AvifDecoderError {}

impl<R: Read> AvifDecoder<R> {
    /// Create a new decoder that reads its input from `r`.
    pub fn new(mut r: R) -> ImageResult<Self> {
        let ctx = read_avif(&mut r, ParseStrictness::Normal).map_err(error_map)?;
        let coded = ctx.primary_item_coded_data().unwrap_or_default();

        let mut primary_decoder = dav1d::Decoder::new().map_err(error_map)?;
        primary_decoder
            .send_data(coded.to_vec(), None, None, None)
            .map_err(error_map)?;
        let picture = read_until_ready(&mut primary_decoder)?;
        let alpha_item = ctx.alpha_item_coded_data().unwrap_or_default();
        let alpha_picture = if !alpha_item.is_empty() {
            let mut alpha_decoder = dav1d::Decoder::new().map_err(error_map)?;
            alpha_decoder
                .send_data(alpha_item.to_vec(), None, None, None)
                .map_err(error_map)?;
            Some(read_until_ready(&mut alpha_decoder)?)
        } else {
            None
        };
        let icc_profile = ctx
            .icc_colour_information()
            .map(|x| x.ok().unwrap_or_default())
            .map(|x| x.to_vec());

        match picture.bit_depth() {
            8 => (),
            10 | 12 => (),
            _ => {
                return ImageResult::Err(ImageError::Decoding(DecodingError::new(
                    ImageFormatHint::Exact(ImageFormat::Avif),
                    format!(
                        "Avif format does not support {} bit depth",
                        picture.bit_depth()
                    ),
                )))
            }
        };
        Ok(AvifDecoder {
            inner: PhantomData,
            picture,
            alpha_picture,
            icc_profile,
        })
    }
}

/// Reshaping incorrectly aligned or sized FFI data into Rust constraints
fn reshape_plane(source: &[u8], stride: usize, width: usize, height: usize) -> Vec<u16> {
    let mut target_plane = vec![0u16; width * height];
    for (shaped_row, src_row) in target_plane
        .chunks_exact_mut(width)
        .zip(source.chunks_exact(stride))
    {
        for (dst, src) in shaped_row.iter_mut().zip(src_row.chunks_exact(2)) {
            *dst = u16::from_ne_bytes([src[0], src[1]]);
        }
    }
    target_plane
}

struct Plane16View<'a> {
    data: std::borrow::Cow<'a, [u16]>,
    stride: usize,
}

impl Default for Plane16View<'_> {
    fn default() -> Self {
        Plane16View {
            data: std::borrow::Cow::Owned(vec![]),
            stride: 0,
        }
    }
}

/// This is correct to transmute FFI data for Y plane and Alpha plane
fn transmute_y_plane16(
    plane: &dav1d::Plane,
    stride: usize,
    width: usize,
    height: usize,
) -> Plane16View<'_> {
    let mut y_plane_stride = stride >> 1;

    let mut bind_y = vec![];
    let plane_ref = plane.as_ref();

    let mut shape_y_plane = || {
        y_plane_stride = width;
        bind_y = reshape_plane(plane_ref, stride, width, height);
    };

    if stride & 1 == 0 {
        match bytemuck::try_cast_slice(plane_ref) {
            Ok(slice) => Plane16View {
                data: std::borrow::Cow::Borrowed(slice),
                stride: y_plane_stride,
            },
            Err(_) => {
                shape_y_plane();
                Plane16View {
                    data: std::borrow::Cow::Owned(bind_y),
                    stride: y_plane_stride,
                }
            }
        }
    } else {
        shape_y_plane();
        Plane16View {
            data: std::borrow::Cow::Owned(bind_y),
            stride: y_plane_stride,
        }
    }
}

/// This is correct to transmute FFI data for Y plane and Alpha plane
fn transmute_chroma_plane16(
    plane: &dav1d::Plane,
    pixel_layout: PixelLayout,
    stride: usize,
    width: usize,
    height: usize,
) -> Plane16View<'_> {
    let plane_ref = plane.as_ref();
    let mut chroma_plane_stride = stride >> 1;
    let mut bind_chroma = vec![];

    let mut shape_chroma_plane = || {
        chroma_plane_stride = match pixel_layout {
            PixelLayout::I400 => unreachable!(),
            PixelLayout::I420 | PixelLayout::I422 => width.div_ceil(2),
            PixelLayout::I444 => width,
        };
        let u_plane_height = match pixel_layout {
            PixelLayout::I400 => unreachable!(),
            PixelLayout::I420 => height.div_ceil(2),
            PixelLayout::I422 | PixelLayout::I444 => height,
        };
        bind_chroma = reshape_plane(plane_ref, stride, chroma_plane_stride, u_plane_height);
    };

    if stride & 1 == 0 {
        match bytemuck::try_cast_slice(plane_ref) {
            Ok(slice) => Plane16View {
                data: std::borrow::Cow::Borrowed(slice),
                stride: chroma_plane_stride,
            },
            Err(_) => {
                shape_chroma_plane();
                Plane16View {
                    data: std::borrow::Cow::Owned(bind_chroma),
                    stride: chroma_plane_stride,
                }
            }
        }
    } else {
        shape_chroma_plane();
        Plane16View {
            data: std::borrow::Cow::Owned(bind_chroma),
            stride: chroma_plane_stride,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialOrd, Eq, PartialEq)]
enum YuvMatrixStrategy {
    KrKb(YuvStandardMatrix),
    CgCo,
    Identity,
}

/// Getting one of prebuilt matrix of fails
fn get_matrix(
    david_matrix: dav1d::pixel::MatrixCoefficients,
) -> Result<YuvMatrixStrategy, ImageError> {
    match david_matrix {
        dav1d::pixel::MatrixCoefficients::Identity => Ok(YuvMatrixStrategy::Identity),
        dav1d::pixel::MatrixCoefficients::BT709 => {
            Ok(YuvMatrixStrategy::KrKb(YuvStandardMatrix::Bt709))
        }
        // This is arguable, some applications prefer to go with Bt.709 as default,
        // and some applications prefer Bt.601 as default.
        // For ex. `Chrome` always prefer Bt.709 even for SD content
        // However, nowadays standard should be Bt.709 for HD+ size otherwise Bt.601
        dav1d::pixel::MatrixCoefficients::Unspecified => {
            Ok(YuvMatrixStrategy::KrKb(YuvStandardMatrix::Bt709))
        }
        dav1d::pixel::MatrixCoefficients::Reserved => Err(ImageError::Unsupported(
            UnsupportedError::from_format_and_kind(
                ImageFormat::Avif.into(),
                UnsupportedErrorKind::GenericFeature(
                    "Using 'Reserved' color matrix is not supported".to_string(),
                ),
            ),
        )),
        dav1d::pixel::MatrixCoefficients::BT470M => {
            Ok(YuvMatrixStrategy::KrKb(YuvStandardMatrix::Bt470_6))
        }
        dav1d::pixel::MatrixCoefficients::BT470BG => {
            Ok(YuvMatrixStrategy::KrKb(YuvStandardMatrix::Bt601))
        }
        dav1d::pixel::MatrixCoefficients::ST170M => {
            Ok(YuvMatrixStrategy::KrKb(YuvStandardMatrix::Smpte240))
        }
        dav1d::pixel::MatrixCoefficients::ST240M => {
            Ok(YuvMatrixStrategy::KrKb(YuvStandardMatrix::Smpte240))
        }
        dav1d::pixel::MatrixCoefficients::YCgCo => Ok(YuvMatrixStrategy::CgCo),
        dav1d::pixel::MatrixCoefficients::BT2020NonConstantLuminance => {
            Ok(YuvMatrixStrategy::KrKb(YuvStandardMatrix::Bt2020))
        }
        dav1d::pixel::MatrixCoefficients::BT2020ConstantLuminance => {
            // This matrix significantly differs from others because linearize values is required
            // to compute Y instead of Y'.
            // Actually it is almost everywhere is not implemented.
            // Libavif + libheif missing this also so actually AVIF images
            // with CL BT.2020 might be made only by mistake
            Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Avif.into(),
                    UnsupportedErrorKind::GenericFeature(
                        "BT2020ConstantLuminance matrix is not supported".to_string(),
                    ),
                ),
            ))
        }
        dav1d::pixel::MatrixCoefficients::ST2085 => Err(ImageError::Unsupported(
            UnsupportedError::from_format_and_kind(
                ImageFormat::Avif.into(),
                UnsupportedErrorKind::GenericFeature("ST2085 matrix is not supported".to_string()),
            ),
        )),
        dav1d::pixel::MatrixCoefficients::ChromaticityDerivedConstantLuminance
        | dav1d::pixel::MatrixCoefficients::ChromaticityDerivedNonConstantLuminance => Err(
            ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                ImageFormat::Avif.into(),
                UnsupportedErrorKind::GenericFeature(
                    "Chromaticity Derived Luminance matrix is not supported".to_string(),
                ),
            )),
        ),
        dav1d::pixel::MatrixCoefficients::ICtCp => Err(ImageError::Unsupported(
            UnsupportedError::from_format_and_kind(
                ImageFormat::Avif.into(),
                UnsupportedErrorKind::GenericFeature(
                    "ICtCp Derived Luminance matrix is not supported".to_string(),
                ),
            ),
        )),
    }
}

impl<R: Read> ImageDecoder for AvifDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.picture.width(), self.picture.height())
    }

    fn color_type(&self) -> ColorType {
        if self.picture.bit_depth() == 8 {
            ColorType::Rgba8
        } else {
            ColorType::Rgba16
        }
    }

    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(self.icc_profile.clone())
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));

        let bit_depth = self.picture.bit_depth();

        // Normally this should never happen,
        // if this happens then there is an incorrect implementation somewhere else
        assert!(bit_depth == 8 || bit_depth == 10 || bit_depth == 12);

        let (width, height) = self.dimensions();
        // This is suspicious if this happens, better fail early
        if width == 0 || height == 0 {
            return Err(ImageError::Limits(LimitError::from_kind(
                LimitErrorKind::DimensionError,
            )));
        }

        let yuv_range = match self.picture.color_range() {
            dav1d::pixel::YUVRange::Limited => YuvIntensityRange::Tv,
            dav1d::pixel::YUVRange::Full => YuvIntensityRange::Pc,
        };

        let matrix_strategy = get_matrix(self.picture.matrix_coefficients())?;

        // Identity matrix should be possible only on 4:4:4
        if matrix_strategy == YuvMatrixStrategy::Identity
            && self.picture.pixel_layout() != PixelLayout::I444
        {
            return Err(ImageError::Decoding(DecodingError::new(
                ImageFormat::Avif.into(),
                AvifDecoderError::YuvLayoutOnIdentityMatrix(self.picture.pixel_layout()),
            )));
        }

        if matrix_strategy == YuvMatrixStrategy::CgCo
            && self.picture.pixel_layout() == PixelLayout::I400
        {
            return Err(ImageError::Decoding(DecodingError::new(
                ImageFormat::Avif.into(),
                AvifDecoderError::UnsupportedLayoutAndMatrix(
                    self.picture.pixel_layout(),
                    matrix_strategy,
                ),
            )));
        }

        if bit_depth == 8 {
            let ref_y = self.picture.plane(PlanarImageComponent::Y);
            let ref_u = self.picture.plane(PlanarImageComponent::U);
            let ref_v = self.picture.plane(PlanarImageComponent::V);

            let image = YuvPlanarImage {
                y_plane: ref_y.as_ref(),
                y_stride: self.picture.stride(PlanarImageComponent::Y) as usize,
                u_plane: ref_u.as_ref(),
                u_stride: self.picture.stride(PlanarImageComponent::U) as usize,
                v_plane: ref_v.as_ref(),
                v_stride: self.picture.stride(PlanarImageComponent::V) as usize,
                width: width as usize,
                height: height as usize,
            };

            match matrix_strategy {
                YuvMatrixStrategy::KrKb(standard) => {
                    let worker = match self.picture.pixel_layout() {
                        PixelLayout::I400 => yuv400_to_rgba8,
                        PixelLayout::I420 => yuv420_to_rgba8,
                        PixelLayout::I422 => yuv422_to_rgba8,
                        PixelLayout::I444 => yuv444_to_rgba8,
                    };

                    worker(image, buf, yuv_range, standard)?;
                }
                YuvMatrixStrategy::CgCo => {
                    let worker = match self.picture.pixel_layout() {
                        PixelLayout::I400 => unreachable!(),
                        PixelLayout::I420 => ycgco420_to_rgba8,
                        PixelLayout::I422 => ycgco422_to_rgba8,
                        PixelLayout::I444 => ycgco444_to_rgba8,
                    };

                    worker(image, buf, yuv_range)?;
                }
                YuvMatrixStrategy::Identity => {
                    let worker = match self.picture.pixel_layout() {
                        PixelLayout::I400 => unreachable!(),
                        PixelLayout::I420 => unreachable!(),
                        PixelLayout::I422 => unreachable!(),
                        PixelLayout::I444 => gbr_to_rgba8,
                    };

                    worker(image, buf, yuv_range)?;
                }
            }

            // Squashing alpha plane into a picture
            if let Some(picture) = self.alpha_picture {
                if picture.pixel_layout() != PixelLayout::I400 {
                    return Err(ImageError::Decoding(DecodingError::new(
                        ImageFormat::Avif.into(),
                        AvifDecoderError::AlphaPlaneFormat(picture.pixel_layout()),
                    )));
                }

                let stride = picture.stride(PlanarImageComponent::Y) as usize;
                let plane = picture.plane(PlanarImageComponent::Y);

                for (buf, slice) in Iterator::zip(
                    buf.chunks_exact_mut(width as usize * 4),
                    plane.as_ref().chunks_exact(stride),
                ) {
                    for (rgba, a_src) in buf.chunks_exact_mut(4).zip(slice) {
                        rgba[3] = *a_src;
                    }
                }
            }
        } else {
            // // 8+ bit-depth case
            if let Ok(buf) = bytemuck::try_cast_slice_mut(buf) {
                let target_slice: &mut [u16] = buf;
                self.process_16bit_picture(target_slice, yuv_range, matrix_strategy)?;
            } else {
                // If buffer from Decoder is unaligned
                let mut aligned_store = vec![0u16; buf.len() / 2];
                self.process_16bit_picture(&mut aligned_store, yuv_range, matrix_strategy)?;
                for (dst, src) in buf.chunks_exact_mut(2).zip(aligned_store.iter()) {
                    let bytes = src.to_ne_bytes();
                    dst[0] = bytes[0];
                    dst[1] = bytes[1];
                }
            }
        }

        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

impl<R: Read> AvifDecoder<R> {
    fn process_16bit_picture(
        &self,
        target: &mut [u16],
        yuv_range: YuvIntensityRange,
        matrix_strategy: YuvMatrixStrategy,
    ) -> ImageResult<()> {
        let y_dav1d_plane = self.picture.plane(PlanarImageComponent::Y);

        let (width, height) = (self.picture.width(), self.picture.height());
        let bit_depth = self.picture.bit_depth();

        // dav1d may return not aligned and not correctly constrained data,
        // or at least I can't find guarantees on that
        // so if it is happened, instead casting we'll need to reshape it into a target slice
        // required criteria: bytemuck allows this align of this data, and stride must be dividable by 2

        let y_plane_view = transmute_y_plane16(
            &y_dav1d_plane,
            self.picture.stride(PlanarImageComponent::Y) as usize,
            width as usize,
            height as usize,
        );

        let u_dav1d_plane = self.picture.plane(PlanarImageComponent::U);
        let v_dav1d_plane = self.picture.plane(PlanarImageComponent::V);
        let mut u_plane_view = Plane16View::default();
        let mut v_plane_view = Plane16View::default();

        if self.picture.pixel_layout() != PixelLayout::I400 {
            u_plane_view = transmute_chroma_plane16(
                &u_dav1d_plane,
                self.picture.pixel_layout(),
                self.picture.stride(PlanarImageComponent::U) as usize,
                width as usize,
                height as usize,
            );
            v_plane_view = transmute_chroma_plane16(
                &v_dav1d_plane,
                self.picture.pixel_layout(),
                self.picture.stride(PlanarImageComponent::V) as usize,
                width as usize,
                height as usize,
            );
        }

        let image = YuvPlanarImage {
            y_plane: y_plane_view.data.as_ref(),
            y_stride: y_plane_view.stride,
            u_plane: u_plane_view.data.as_ref(),
            u_stride: u_plane_view.stride,
            v_plane: v_plane_view.data.as_ref(),
            v_stride: v_plane_view.stride,
            width: width as usize,
            height: height as usize,
        };

        match matrix_strategy {
            YuvMatrixStrategy::KrKb(standard) => {
                let worker = match self.picture.pixel_layout() {
                    PixelLayout::I400 => {
                        if bit_depth == 10 {
                            yuv400_to_rgba10
                        } else {
                            yuv400_to_rgba12
                        }
                    }
                    PixelLayout::I420 => {
                        if bit_depth == 10 {
                            yuv420_to_rgba10
                        } else {
                            yuv420_to_rgba12
                        }
                    }
                    PixelLayout::I422 => {
                        if bit_depth == 10 {
                            yuv422_to_rgba10
                        } else {
                            yuv422_to_rgba12
                        }
                    }
                    PixelLayout::I444 => {
                        if bit_depth == 10 {
                            yuv444_to_rgba10
                        } else {
                            yuv444_to_rgba12
                        }
                    }
                };
                worker(image, target, yuv_range, standard)?;
            }
            YuvMatrixStrategy::CgCo => {
                let worker = match self.picture.pixel_layout() {
                    PixelLayout::I400 => unreachable!(),
                    PixelLayout::I420 => {
                        if bit_depth == 10 {
                            ycgco420_to_rgba10
                        } else {
                            ycgco420_to_rgba12
                        }
                    }
                    PixelLayout::I422 => {
                        if bit_depth == 10 {
                            ycgco422_to_rgba10
                        } else {
                            ycgco422_to_rgba12
                        }
                    }
                    PixelLayout::I444 => {
                        if bit_depth == 10 {
                            ycgco444_to_rgba10
                        } else {
                            ycgco444_to_rgba12
                        }
                    }
                };
                worker(image, target, yuv_range)?;
            }
            YuvMatrixStrategy::Identity => {
                let worker = match self.picture.pixel_layout() {
                    PixelLayout::I400 => unreachable!(),
                    PixelLayout::I420 => unreachable!(),
                    PixelLayout::I422 => unreachable!(),
                    PixelLayout::I444 => {
                        if bit_depth == 10 {
                            gbr_to_rgba10
                        } else {
                            gbr_to_rgba12
                        }
                    }
                };
                worker(image, target, yuv_range)?;
            }
        }

        // Squashing alpha plane into a picture
        if let Some(picture) = &self.alpha_picture {
            if picture.pixel_layout() != PixelLayout::I400 {
                return Err(ImageError::Decoding(DecodingError::new(
                    ImageFormat::Avif.into(),
                    AvifDecoderError::AlphaPlaneFormat(picture.pixel_layout()),
                )));
            }

            let a_dav1d_plane = picture.plane(PlanarImageComponent::Y);
            let a_plane_view = transmute_y_plane16(
                &a_dav1d_plane,
                picture.stride(PlanarImageComponent::Y) as usize,
                width as usize,
                height as usize,
            );

            for (buf, slice) in Iterator::zip(
                target.chunks_exact_mut(width as usize * 4),
                a_plane_view.data.as_ref().chunks_exact(a_plane_view.stride),
            ) {
                for (rgba, a_src) in buf.chunks_exact_mut(4).zip(slice) {
                    rgba[3] = *a_src;
                }
            }
        }

        // Expand current bit depth to target 16
        let target_expand_bits = 16u32 - self.picture.bit_depth() as u32;
        for item in target.iter_mut() {
            *item = (*item << target_expand_bits) | (*item >> (16 - target_expand_bits));
        }

        Ok(())
    }
}

/// `get_picture` and `send_pending_data` yield `Again` as a non-fatal error requesting more data is sent to the decoder
/// This ensures that in the case of `Again` all pending data is submitted
/// This should be called after `send_data` (which does not yield `Again` when called the first time)
fn read_until_ready(decoder: &mut dav1d::Decoder) -> ImageResult<dav1d::Picture> {
    loop {
        match decoder.get_picture() {
            Err(dav1d::Error::Again) => match decoder.send_pending_data() {
                Ok(()) => {}
                Err(dav1d::Error::Again) => {}
                Err(e) => return Err(error_map(e)),
            },
            r => return r.map_err(error_map),
        }
    }
}
