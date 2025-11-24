use std::sync::Arc;

/// CICP (coding independent code points) defines the colorimetric interpretation of rgb-ish color
/// components.
use crate::{
    color::FromPrimitive,
    error::{ParameterError, ParameterErrorKind},
    math::multiply_accumulate,
    traits::{
        private::{LayoutWithColor, SealedPixelWithColorType},
        PixelWithColorType,
    },
    utils::vec_try_with_capacity,
    DynamicImage, ImageError, Pixel, Primitive,
};

/// Reference: <https://www.itu.int/rec/T-REC-H.273-202407-I/en> (V4)
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Cicp {
    /// Defines the exact color of red, green, blue primary colors.
    pub primaries: CicpColorPrimaries,
    /// The electro-optical transfer function (EOTF) that maps color components to linear values.
    pub transfer: CicpTransferCharacteristics,
    /// A matrix between linear values and primary color representation.
    ///
    /// For an RGB space this is the identity matrix.
    pub matrix: CicpMatrixCoefficients,
    /// Whether the color components use all bits of the encoded values, or have headroom.
    ///
    /// For compute purposes, `image` only supports [`CicpVideoFullRangeFlag::FullRange`] and you
    /// get errors when trying to pass a non-full-range color profile to transform APIs such as
    /// [`DynamicImage::apply_color_space`] or [`CicpTransform::new`].
    pub full_range: CicpVideoFullRangeFlag,
}

/// An internal representation of what our `T: PixelWithColorType` can do, i.e. ImageBuffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct CicpRgb {
    pub(crate) primaries: CicpColorPrimaries,
    pub(crate) transfer: CicpTransferCharacteristics,
    pub(crate) luminance: DerivedLuminance,
}

/// Defines the exact color of red, green, blue primary colors.
///
/// Each set defines the CIE 1931 XYZ (2°) color space coordinates of the primary colors and an
/// illuminant/whitepoint under which those colors are viewed.
///
/// Refer to Rec H.273 Table 2.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CicpColorPrimaries {
    /// ITU-R BT.709-6
    SRgb = 1,
    /// Explicitly, the color space is not determined.
    Unspecified = 2,
    /// ITU-R BT.470-6 System M
    RgbM = 4,
    /// ITU-R BT.470-6 System B, G
    RgbB = 5,
    /// SMPTE 170M
    /// functionally equivalent to 7
    Bt601 = 6,
    /// SMPTE 240M
    /// functionally equivalent to 6
    Rgb240m = 7,
    /// Generic film (colour filters using Illuminant C)
    GenericFilm = 8,
    /// Rec. ITU-R BT.2020-2
    /// Rec. ITU-R BT.2100-2
    Rgb2020 = 9,
    /// SMPTE ST 428-1
    ///
    /// (CIE 1931 XYZ as in ISO/CIE 11664-1)
    Xyz = 10,
    /// SMPTE RP 431-2 (aka. DCI P3)
    SmpteRp431 = 11,
    /// SMPTE EG 432-1, DCI P3 variant with the D65 whitepoint (matching sRGB and BT.2020)
    SmpteRp432 = 12,
    /// Corresponds to value 22 but
    ///
    /// > No corresponding industry specification identified
    ///
    /// But moxcms identifies it as EBU Tech 3213-E: <https://tech.ebu.ch/docs/tech/tech3213.pdf>
    ///
    /// However, there are some differences in the second digit of red's CIE 1931 and the precision
    /// is only 2 digits whereas CICP names three; so unsure if this is fully accurate as the
    /// actual source material.
    Industry22 = 22,
}

impl CicpColorPrimaries {
    fn to_moxcms(self) -> moxcms::CicpColorPrimaries {
        use moxcms::CicpColorPrimaries as M;

        match self {
            CicpColorPrimaries::SRgb => M::Bt709,
            CicpColorPrimaries::Unspecified => M::Unspecified,
            CicpColorPrimaries::RgbM => M::Bt470M,
            CicpColorPrimaries::RgbB => M::Bt470Bg,
            CicpColorPrimaries::Bt601 => M::Bt601,
            CicpColorPrimaries::Rgb240m => M::Smpte240,
            CicpColorPrimaries::GenericFilm => M::GenericFilm,
            CicpColorPrimaries::Rgb2020 => M::Bt2020,
            CicpColorPrimaries::Xyz => M::Xyz,
            CicpColorPrimaries::SmpteRp431 => M::Smpte431,
            CicpColorPrimaries::SmpteRp432 => M::Smpte432,
            CicpColorPrimaries::Industry22 => M::Ebu3213,
        }
    }
}

/// The transfer characteristics, expressing relation between encoded values and linear color
/// values.
///
/// Refer to Rec H.273 Table 3.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CicpTransferCharacteristics {
    /// Rec. ITU-R BT.709-6
    /// Rec. ITU-R BT.1361-0 conventional
    /// (functionally the same as the values 6, 14 and 15)
    Bt709 = 1,
    /// Explicitly, the transfer characteristics are not determined.
    Unspecified = 2,
    /// Rec. ITU-R BT.470-6 System M (historical)
    /// United States National Television System Committee 1953 Recommendation for transmission standards for color television
    /// United States Federal Communications Commission (2003) Title 47 Code of Federal Regulations 73.682 (a) (20)
    /// Rec. ITU-R BT.1700-0 625 PAL and 625 SECAM
    ///
    /// Assumed gamma of 2.2
    Bt470M = 4,
    /// Rec. ITU-R BT.470-6 System B, G (historical)
    Bt470BG = 5,
    /// Rec. ITU-R BT.601-7 525 or 625
    /// Rec. ITU-R BT.1358-1 525 or 625 (historical)
    /// Rec. ITU-R BT.1700-0 NTSC
    /// SMPTE ST 170 (functionally the same as the values 1, 14 and 15)
    Bt601 = 6,
    /// SMPTE ST 240
    Smpte240m = 7,
    /// Linear transfer characteristics
    Linear = 8,
    /// Logarithmic transfer characteristic (100:1 range)
    Log100 = 9,
    /// Logarithmic transfer characteristic (100 * Sqrt( 10 ) : 1 range)
    LogSqrt = 10,
    /// IEC 61966-2-4
    Iec61966_2_4 = 11,
    /// Rec. ITU-R BT.1361-0 extended colour gamut system (historical)
    Bt1361 = 12,
    /// IEC 61966-2-1 sRGB (with MatrixCoefficients equal to 0)
    /// IEC 61966-2-1 sYCC (with MatrixCoefficients equal to 5)
    SRgb = 13,
    /// Rec. ITU-R BT.2020-2 (10-bit system)
    /// (functionally the same as the values 1, 6 and 15)
    Bt2020_10bit = 14,
    /// Rec. ITU-R BT.2020-2 (12-bit system)
    /// (functionally the same as the values 1, 6 and 14)
    Bt2020_12bit = 15,
    /// SMPTE ST 2084 for 10-, 12-, 14- and 16-bit systems
    /// Rec. ITU-R BT.2100-2 perceptual quantization (PQ) system
    Smpte2084 = 16,
    /// SMPTE ST 428-1
    Smpte428 = 17,
    /// ARIB STD-B67
    /// Rec. ITU-R BT.2100-2 hybrid log- gamma (HLG) system
    Bt2100Hlg = 18,
}

impl CicpTransferCharacteristics {
    fn to_moxcms(self) -> moxcms::TransferCharacteristics {
        use moxcms::TransferCharacteristics as T;

        match self {
            CicpTransferCharacteristics::Bt709 => T::Bt709,
            CicpTransferCharacteristics::Unspecified => T::Unspecified,
            CicpTransferCharacteristics::Bt470M => T::Bt470M,
            CicpTransferCharacteristics::Bt470BG => T::Bt470Bg,
            CicpTransferCharacteristics::Bt601 => T::Bt601,
            CicpTransferCharacteristics::Smpte240m => T::Smpte240,
            CicpTransferCharacteristics::Linear => T::Linear,
            CicpTransferCharacteristics::Log100 => T::Log100,
            CicpTransferCharacteristics::LogSqrt => T::Log100sqrt10,
            CicpTransferCharacteristics::Iec61966_2_4 => T::Iec61966,
            CicpTransferCharacteristics::Bt1361 => T::Bt1361,
            CicpTransferCharacteristics::SRgb => T::Srgb,
            CicpTransferCharacteristics::Bt2020_10bit => T::Bt202010bit,
            CicpTransferCharacteristics::Bt2020_12bit => T::Bt202012bit,
            CicpTransferCharacteristics::Smpte2084 => T::Smpte2084,
            CicpTransferCharacteristics::Smpte428 => T::Smpte428,
            CicpTransferCharacteristics::Bt2100Hlg => T::Hlg,
        }
    }
}

///
/// Refer to Rec H.273 Table 4.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CicpMatrixCoefficients {
    /// The identity matrix.
    /// Typically used for GBR (often referred to as RGB); however, may also be used for YZX (often referred to as XYZ);
    /// IEC 61966-2-1 sRGB
    /// SMPTE ST 428-1
    Identity = 0,
    /// Rec. ITU-R BT.709-6
    /// Rec. ITU-R BT.1361-0 conventional colour gamut system and extended colour gamut system (historical)
    /// IEC 61966-2-4 xvYCC709
    /// SMPTE RP 177 Annex B
    Bt709 = 1,
    /// Explicitly, the matrix coefficients are not determined.
    Unspecified = 2,
    /// United States Federal Communications Commission (2003) Title 47 Code of Federal Regulations 73.682 (a) (20)
    UsFCC = 4,
    ///  Rec. ITU-R BT.470-6 System B, G (historical)
    /// Rec. ITU-R BT.601-7 625
    /// Rec. ITU-R BT.1358-0 625 (historical)
    /// Rec. ITU-R BT.1700-0 625 PAL and 625 SECAM
    /// IEC 61966-2-1 sYCC
    /// IEC 61966-2-4 xvYCC601
    /// (functionally the same as the value 6)
    Bt470BG = 5,
    /// (functionally the same as the value 5)
    Smpte170m = 6,
    /// SMPTE ST 240
    Smpte240m = 7,
    /// YCgCo
    YCgCo = 8,
    /// Rec. ITU-R BT.2020-2 (non-constant luminance)
    /// Rec. ITU-R BT.2100-2 Y′CbCr
    Bt2020NonConstant = 9,
    /// Rec. ITU-R BT.2020-2 (constant luminance)
    Bt2020Constant = 10,
    /// SMPTE ST 2085
    Smpte2085 = 11,
    /// Chromaticity-derived non-constant luminance system
    ChromaticityDerivedNonConstant = 12,
    /// Chromaticity-derived constant luminance system
    ChromaticityDerivedConstant = 13,
    /// Rec. ITU-R BT.2100-2 ICTCp
    Bt2100 = 14,
    /// Colour representation developed in SMPTE as IPT-PQ-C2.
    IptPqC2 = 15,
    /// YCgCo with added bit-depth (2-bit).
    YCgCoRe = 16,
    /// YCgCo with added bit-depth (1-bit).
    YCgCoRo = 17,
}

impl CicpMatrixCoefficients {
    fn to_moxcms(self) -> Option<moxcms::MatrixCoefficients> {
        use moxcms::MatrixCoefficients as M;

        Some(match self {
            CicpMatrixCoefficients::Identity => M::Identity,
            CicpMatrixCoefficients::Unspecified => M::Unspecified,
            CicpMatrixCoefficients::Bt709 => M::Bt709,
            CicpMatrixCoefficients::UsFCC => M::Fcc,
            CicpMatrixCoefficients::Bt470BG => M::Bt470Bg,
            CicpMatrixCoefficients::Smpte170m => M::Smpte170m,
            CicpMatrixCoefficients::Smpte240m => M::Smpte240m,
            CicpMatrixCoefficients::YCgCo => M::YCgCo,
            CicpMatrixCoefficients::Bt2020NonConstant => M::Bt2020Ncl,
            CicpMatrixCoefficients::Bt2020Constant => M::Bt2020Cl,
            CicpMatrixCoefficients::Smpte2085 => M::Smpte2085,
            CicpMatrixCoefficients::ChromaticityDerivedNonConstant => M::ChromaticityDerivedNCL,
            CicpMatrixCoefficients::ChromaticityDerivedConstant => M::ChromaticityDerivedCL,
            CicpMatrixCoefficients::Bt2100 => M::ICtCp,
            CicpMatrixCoefficients::IptPqC2
            | CicpMatrixCoefficients::YCgCoRe
            | CicpMatrixCoefficients::YCgCoRo => return None,
        })
    }
}

/// The used encoded value range.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CicpVideoFullRangeFlag {
    /// The color components are encoded in a limited range, e.g., 16-235 for 8-bit.
    ///
    /// Do note that `image` does not support computing with this setting (yet).
    NarrowRange = 0,
    /// The color components are encoded in the full range, e.g., 0-255 for 8-bit.
    FullRange = 1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum DerivedLuminance {
    /// Luminance is calculated in linear space:
    ///     Y' = dot(K_rgb, RGB)'
    #[allow(dead_code)] // We do not support this yet but should prepare call sites for the
    // eventuality.
    Constant,
    /// Luminance is calculated in the transferred space:
    ///     Y' = dot(K_rgb, RGB')
    NonConstant,
}

/// Apply to colors of the input color space to get output color values.
///
/// We do not support all possible Cicp color spaces, but when we support one then all builtin
/// `Pixel` types can be converted with their respective components. This value is used to signify
/// that some particular combination is supported.
#[derive(Clone)]
pub struct CicpTransform {
    from: Cicp,
    into: Cicp,
    u8: RgbTransforms<u8>,
    u16: RgbTransforms<u16>,
    f32: RgbTransforms<f32>,
    // Converting RGB to Y in the output.
    output_coefs: [f32; 3],
}

pub(crate) type CicpApplicable<'lt, C> = dyn Fn(&[C], &mut [C]) + Send + Sync + 'lt;

#[derive(Clone)]
struct RgbTransforms<C> {
    slices: [Arc<CicpApplicable<'static, C>>; 4],
    luma_rgb: [Arc<CicpApplicable<'static, C>>; 4],
    rgb_luma: [Arc<CicpApplicable<'static, C>>; 4],
    luma_luma: [Arc<CicpApplicable<'static, C>>; 4],
}

impl CicpTransform {
    /// Construct a transform between two color spaces.
    ///
    /// Returns `Some` if the transform is guaranteed to be supported by `image`. Both color spaces
    /// are well understood and can be expected to be supported in future versions. However, we do
    /// not make guarantees about adjusting the rounding modes, accuracy, and exact numeric values
    /// used in the transform. Also, out-of-gamut colors may be handled differently per API.
    ///
    /// Returns `None` if the transformation is not (yet) supported.
    ///
    /// This is used with [`ConvertColorOptions`][`crate::ConvertColorOptions`] in
    /// [`ImageBuffer::copy_from_color_space`][`crate::ImageBuffer::copy_from_color_space`],
    /// [`DynamicImage::copy_from_color_space`][`DynamicImage::copy_from_color_space`].
    pub fn new(from: Cicp, into: Cicp) -> Option<Self> {
        if !from.qualify_stability() || !into.qualify_stability() {
            // To avoid regressions, we do not support all kinds of transforms from the start.
            // Instead, a selected list will be gradually enlarged as more in-depth tests are done
            // and the selected implementation library is checked for suitability in use.
            return None;
        }

        // Unused, but introduces symmetry to the supported color space transforms. That said we
        // calculate the derived luminance coefficients for all color that have a matching moxcms
        // profile so this really should not block anything.
        let _input_coefs = from.into_rgb().derived_luminance()?;
        let output_coefs = into.into_rgb().derived_luminance()?;

        let mox_from = from.to_moxcms_compute_profile()?;
        let mox_into = into.to_moxcms_compute_profile()?;

        let opt = moxcms::TransformOptions::default();

        let f32_fallback = {
            let try_f32 = Self::LAYOUTS.map(|(from_layout, into_layout)| {
                let (from, from_layout) = mox_from.map_layout(from_layout);
                let (into, into_layout) = mox_into.map_layout(into_layout);

                from.create_transform_f32(from_layout, into, into_layout, opt)
                    .map(Arc::<dyn moxcms::TransformExecutor<f32> + Send + Sync>::from)
                    .ok()
            });

            if try_f32.iter().any(Option::is_none) {
                return None;
            }

            try_f32.map(Option::unwrap)
        };

        // TODO: really these should be lazy, eh?
        Some(CicpTransform {
            from,
            into,
            u8: Self::build_transforms(
                Self::LAYOUTS.map(|(from_layout, into_layout)| {
                    let (from, from_layout) = mox_from.map_layout(from_layout);
                    let (into, into_layout) = mox_into.map_layout(into_layout);

                    from.create_transform_8bit(from_layout, into, into_layout, opt)
                        .map(Arc::<dyn moxcms::TransformExecutor<_> + Send + Sync>::from)
                        .ok()
                }),
                f32_fallback.clone(),
                output_coefs,
            )?,
            u16: Self::build_transforms(
                Self::LAYOUTS.map(|(from_layout, into_layout)| {
                    let (from, from_layout) = mox_from.map_layout(from_layout);
                    let (into, into_layout) = mox_into.map_layout(into_layout);

                    from.create_transform_16bit(from_layout, into, into_layout, opt)
                        .map(Arc::<dyn moxcms::TransformExecutor<_> + Send + Sync>::from)
                        .ok()
                }),
                f32_fallback.clone(),
                output_coefs,
            )?,
            f32: Self::build_transforms(
                f32_fallback.clone().map(Some),
                f32_fallback.clone(),
                output_coefs,
            )?,
            output_coefs,
        })
    }

    /// For a Pixel with known color layout (`ColorType`) get a transform that is accurate.
    ///
    /// This returns `None` if we do not support the transform. At writing that is true for
    /// instance for transforms involved 'Luma` pixels which are interpreted as the `Y` in a
    /// `YCbCr` color based off the actual whitepoint, with coefficients according to each
    /// primary's luminance. Only Rgb transforms are supported via `moxcms`.
    ///
    /// Maybe provide publicly?
    pub(crate) fn supported_transform_fn<From: PixelWithColorType, Into: PixelWithColorType>(
        &self,
    ) -> &'_ CicpApplicable<'_, From::Subpixel> {
        use crate::traits::private::double_dispatch_transform_from_sealed;
        double_dispatch_transform_from_sealed::<From, Into>(self)
    }

    /// Does this transform realize the conversion `from` to `into`.
    pub(crate) fn check_applicable(&self, from: Cicp, into: Cicp) -> Result<(), ImageError> {
        let check_expectation = |expected, found| {
            if expected == found {
                Ok(())
            } else {
                Err(ParameterError::from_kind(
                    ParameterErrorKind::CicpMismatch { expected, found },
                ))
            }
        };

        check_expectation(self.from, from).map_err(ImageError::Parameter)?;
        check_expectation(self.into, into).map_err(ImageError::Parameter)?;

        Ok(())
    }

    fn build_transforms<P: ColorComponentForCicp + Default + 'static>(
        trs: [Option<Arc<dyn moxcms::TransformExecutor<P> + Send + Sync>>; 4],
        f32: [Arc<dyn moxcms::TransformExecutor<f32> + Send + Sync>; 4],
        output_coef: [f32; 3],
    ) -> Option<RgbTransforms<P>> {
        // We would use `[array]::try_map` here, but it is not stable yet.
        if trs.iter().any(Option::is_none) {
            return None;
        }

        let trs = trs.map(Option::unwrap);

        // rgb-rgb transforms are done directly via moxcms.
        let slices = trs.clone().map(|tr| {
            Arc::new(move |input: &[P], output: &mut [P]| {
                tr.transform(input, output).expect("transform failed")
            }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>
        });

        const N: usize = 256;

        // luma-rgb transforms expand the Luma to Rgb (and LumaAlpha to Rgba)
        let luma_rgb = {
            let [tr33, tr34, tr43, tr44] = f32.clone();

            [
                Arc::new(move |input: &[P], output: &mut [P]| {
                    let mut ibuffer = [0.0f32; 3 * N];
                    let mut obuffer = [0.0f32; 3 * N];

                    for (luma, output) in input.chunks(N).zip(output.chunks_mut(3 * N)) {
                        let n = luma.len();
                        let ibuffer = &mut ibuffer[..3 * n];
                        let obuffer = &mut obuffer[..3 * n];
                        Self::expand_luma_rgb(luma, ibuffer);
                        tr33.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgb(obuffer, output);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    let mut ibuffer = [0.0f32; 3 * N];
                    let mut obuffer = [0.0f32; 4 * N];

                    for (luma, output) in input.chunks(N).zip(output.chunks_mut(4 * N)) {
                        let n = luma.len();
                        let ibuffer = &mut ibuffer[..3 * n];
                        let obuffer = &mut obuffer[..4 * n];
                        Self::expand_luma_rgb(luma, ibuffer);
                        tr34.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgba(obuffer, output);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    let mut ibuffer = [0.0f32; 4 * N];
                    let mut obuffer = [0.0f32; 3 * N];

                    for (luma, output) in input.chunks(2 * N).zip(output.chunks_mut(3 * N)) {
                        let n = luma.len() / 2;
                        let ibuffer = &mut ibuffer[..4 * n];
                        let obuffer = &mut obuffer[..3 * n];
                        Self::expand_luma_rgba(luma, ibuffer);
                        tr43.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgb(obuffer, output);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    let mut ibuffer = [0.0f32; 4 * N];
                    let mut obuffer = [0.0f32; 4 * N];

                    for (luma, output) in input.chunks(2 * N).zip(output.chunks_mut(4 * N)) {
                        let n = luma.len() / 2;
                        let ibuffer = &mut ibuffer[..4 * n];
                        let obuffer = &mut obuffer[..4 * n];
                        Self::expand_luma_rgba(luma, ibuffer);
                        tr44.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgba(obuffer, output);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
            ]
        };

        // rgb-luma transforms contract Rgb to Luma (and Rgba to LumaAlpha)
        let rgb_luma = {
            let [tr33, tr34, tr43, tr44] = f32.clone();

            [
                Arc::new(move |input: &[P], output: &mut [P]| {
                    debug_assert_eq!(input.len() / 3, output.len());

                    let mut ibuffer = [0.0f32; 3 * N];
                    let mut obuffer = [0.0f32; 3 * N];

                    for (rgb, output) in input.chunks(3 * N).zip(output.chunks_mut(N)) {
                        let n = output.len();
                        let ibuffer = &mut ibuffer[..3 * n];
                        let obuffer = &mut obuffer[..3 * n];
                        Self::expand_rgb(rgb, ibuffer);
                        tr33.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgb_luma(obuffer, output, output_coef);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    debug_assert_eq!(input.len() / 3, output.len() / 2);

                    let mut ibuffer = [0.0f32; 3 * N];
                    let mut obuffer = [0.0f32; 4 * N];

                    for (rgb, output) in input.chunks(4 * N).zip(output.chunks_mut(2 * N)) {
                        let n = output.len() / 2;
                        let ibuffer = &mut ibuffer[..3 * n];
                        let obuffer = &mut obuffer[..4 * n];
                        Self::expand_rgb(rgb, ibuffer);
                        tr34.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgba_luma(obuffer, output, output_coef);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    debug_assert_eq!(input.len() / 4, output.len());

                    let mut ibuffer = [0.0f32; 4 * N];
                    let mut obuffer = [0.0f32; 3 * N];

                    for (rgba, output) in input.chunks(4 * N).zip(output.chunks_mut(N)) {
                        let n = output.len();
                        let ibuffer = &mut ibuffer[..4 * n];
                        let obuffer = &mut obuffer[..3 * n];
                        Self::expand_rgba(rgba, ibuffer);
                        tr43.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgb_luma(obuffer, output, output_coef);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    debug_assert_eq!(input.len() / 4, output.len() / 2);

                    let mut ibuffer = [0.0f32; 4 * N];
                    let mut obuffer = [0.0f32; 4 * N];

                    for (rgba, output) in input.chunks(4 * N).zip(output.chunks_mut(2 * N)) {
                        let n = output.len() / 2;
                        let ibuffer = &mut ibuffer[..4 * n];
                        let obuffer = &mut obuffer[..4 * n];
                        Self::expand_rgba(rgba, ibuffer);
                        tr44.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgba_luma(obuffer, output, output_coef);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
            ]
        };

        // luma-luma both expand and contract
        let luma_luma = {
            let [tr33, tr34, tr43, tr44] = f32.clone();

            [
                Arc::new(move |input: &[P], output: &mut [P]| {
                    debug_assert_eq!(input.len(), output.len());
                    let mut ibuffer = [0.0f32; 3 * N];
                    let mut obuffer = [0.0f32; 3 * N];

                    for (luma, output) in input.chunks(N).zip(output.chunks_mut(N)) {
                        let n = luma.len();
                        let ibuffer = &mut ibuffer[..3 * n];
                        let obuffer = &mut obuffer[..3 * n];
                        Self::expand_luma_rgb(luma, ibuffer);
                        tr33.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgb_luma(obuffer, output, output_coef);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    debug_assert_eq!(input.len(), output.len() / 2);
                    let mut ibuffer = [0.0f32; 3 * N];
                    let mut obuffer = [0.0f32; 4 * N];

                    for (luma, output) in input.chunks(N).zip(output.chunks_mut(2 * N)) {
                        let n = luma.len();
                        let ibuffer = &mut ibuffer[..3 * n];
                        let obuffer = &mut obuffer[..4 * n];
                        Self::expand_luma_rgb(luma, ibuffer);
                        tr34.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgba_luma(obuffer, output, output_coef);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    debug_assert_eq!(input.len() / 2, output.len());
                    let mut ibuffer = [0.0f32; 4 * N];
                    let mut obuffer = [0.0f32; 3 * N];

                    for (luma, output) in input.chunks(2 * N).zip(output.chunks_mut(N)) {
                        let n = luma.len() / 2;
                        let ibuffer = &mut ibuffer[..4 * n];
                        let obuffer = &mut obuffer[..3 * n];
                        Self::expand_luma_rgba(luma, ibuffer);
                        tr43.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgb_luma(obuffer, output, output_coef);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
                Arc::new(move |input: &[P], output: &mut [P]| {
                    debug_assert_eq!(input.len() / 2, output.len() / 2);
                    let mut ibuffer = [0.0f32; 4 * N];
                    let mut obuffer = [0.0f32; 4 * N];

                    for (luma, output) in input.chunks(2 * N).zip(output.chunks_mut(2 * N)) {
                        let n = luma.len() / 2;
                        let ibuffer = &mut ibuffer[..4 * n];
                        let obuffer = &mut obuffer[..4 * n];
                        Self::expand_luma_rgba(luma, ibuffer);
                        tr44.transform(ibuffer, obuffer).expect("transform failed");
                        Self::clamp_rgba_luma(obuffer, output, output_coef);
                    }
                }) as Arc<dyn Fn(&[P], &mut [P]) + Send + Sync>,
            ]
        };

        Some(RgbTransforms {
            slices,
            luma_rgb,
            rgb_luma,
            luma_luma,
        })
    }

    pub(crate) fn transform_dynamic(&self, lhs: &mut DynamicImage, rhs: &DynamicImage) {
        const STEP: usize = 256;

        let mut ibuffer = [0.0f32; 4 * STEP];
        let mut obuffer = [0.0f32; 4 * STEP];

        let pixels = (u64::from(lhs.width()) * u64::from(lhs.height())) as usize;

        let input_samples;
        let output_samples;

        let inner_transform = match (
            LayoutWithColor::from(lhs.color()),
            LayoutWithColor::from(rhs.color()),
        ) {
            (
                LayoutWithColor::Luma | LayoutWithColor::Rgb,
                LayoutWithColor::Luma | LayoutWithColor::Rgb,
            ) => {
                output_samples = 3;
                input_samples = 3;
                &*self.f32.slices[0]
            }
            (
                LayoutWithColor::LumaAlpha | LayoutWithColor::Rgba,
                LayoutWithColor::Luma | LayoutWithColor::Rgb,
            ) => {
                output_samples = 4;
                input_samples = 3;
                &*self.f32.slices[1]
            }
            (
                LayoutWithColor::Luma | LayoutWithColor::Rgb,
                LayoutWithColor::LumaAlpha | LayoutWithColor::Rgba,
            ) => {
                output_samples = 3;
                input_samples = 4;
                &*self.f32.slices[2]
            }
            (
                LayoutWithColor::LumaAlpha | LayoutWithColor::Rgba,
                LayoutWithColor::LumaAlpha | LayoutWithColor::Rgba,
            ) => {
                output_samples = 4;
                input_samples = 4;
                &*self.f32.slices[3]
            }
        };

        for start_idx in (0..pixels).step_by(STEP) {
            let end_idx = (start_idx + STEP).min(pixels);
            let count = end_idx - start_idx;

            // Expand pixels from `other` into `ibuffer`. All of these have different types, so
            // here's two large switch statements.
            match rhs {
                DynamicImage::ImageLuma8(buf) => {
                    CicpTransform::expand_luma_rgb(
                        &buf.inner_pixels()[start_idx..end_idx],
                        &mut ibuffer[..3 * count],
                    );
                }
                DynamicImage::ImageLumaA8(buf) => {
                    CicpTransform::expand_luma_rgba(
                        &buf.inner_pixels()[2 * start_idx..2 * end_idx],
                        &mut ibuffer[..4 * count],
                    );
                }
                DynamicImage::ImageRgb8(buf) => {
                    CicpTransform::expand_rgb(
                        &buf.inner_pixels()[3 * start_idx..3 * end_idx],
                        &mut ibuffer[..3 * count],
                    );
                }
                DynamicImage::ImageRgba8(buf) => {
                    CicpTransform::expand_rgba(
                        &buf.inner_pixels()[4 * start_idx..4 * end_idx],
                        &mut ibuffer[..4 * count],
                    );
                }
                DynamicImage::ImageLuma16(buf) => {
                    CicpTransform::expand_luma_rgb(
                        &buf.inner_pixels()[start_idx..end_idx],
                        &mut ibuffer[..3 * count],
                    );
                }
                DynamicImage::ImageLumaA16(buf) => {
                    CicpTransform::expand_luma_rgba(
                        &buf.inner_pixels()[2 * start_idx..2 * end_idx],
                        &mut ibuffer[..4 * count],
                    );
                }
                DynamicImage::ImageRgb16(buf) => {
                    CicpTransform::expand_rgb(
                        &buf.inner_pixels()[3 * start_idx..3 * end_idx],
                        &mut ibuffer[..3 * count],
                    );
                }

                DynamicImage::ImageRgba16(buf) => {
                    CicpTransform::expand_rgba(
                        &buf.inner_pixels()[4 * start_idx..4 * end_idx],
                        &mut ibuffer[..4 * count],
                    );
                }
                DynamicImage::ImageRgb32F(buf) => {
                    CicpTransform::expand_rgb(
                        &buf.inner_pixels()[3 * start_idx..3 * end_idx],
                        &mut ibuffer[..3 * count],
                    );
                }
                DynamicImage::ImageRgba32F(buf) => {
                    CicpTransform::expand_rgba(
                        &buf.inner_pixels()[4 * start_idx..4 * end_idx],
                        &mut ibuffer[..4 * count],
                    );
                }
            }

            let islice = &ibuffer[..input_samples * count];
            let oslice = &mut obuffer[..output_samples * count];

            inner_transform(islice, oslice);

            match lhs {
                DynamicImage::ImageLuma8(buf) => {
                    CicpTransform::clamp_rgb_luma(
                        &obuffer[..3 * count],
                        &mut buf.inner_pixels_mut()[start_idx..end_idx],
                        self.output_coefs,
                    );
                }
                DynamicImage::ImageLumaA8(buf) => {
                    CicpTransform::clamp_rgba_luma(
                        &obuffer[..4 * count],
                        &mut buf.inner_pixels_mut()[2 * start_idx..2 * end_idx],
                        self.output_coefs,
                    );
                }
                DynamicImage::ImageRgb8(buf) => {
                    CicpTransform::clamp_rgb(
                        &obuffer[..3 * count],
                        &mut buf.inner_pixels_mut()[3 * start_idx..3 * end_idx],
                    );
                }
                DynamicImage::ImageRgba8(buf) => {
                    CicpTransform::clamp_rgba(
                        &obuffer[..4 * count],
                        &mut buf.inner_pixels_mut()[4 * start_idx..4 * end_idx],
                    );
                }
                DynamicImage::ImageLuma16(buf) => {
                    CicpTransform::clamp_rgb_luma(
                        &obuffer[..3 * count],
                        &mut buf.inner_pixels_mut()[start_idx..end_idx],
                        self.output_coefs,
                    );
                }
                DynamicImage::ImageLumaA16(buf) => {
                    CicpTransform::clamp_rgba_luma(
                        &obuffer[..4 * count],
                        &mut buf.inner_pixels_mut()[2 * start_idx..2 * end_idx],
                        self.output_coefs,
                    );
                }
                DynamicImage::ImageRgb16(buf) => {
                    CicpTransform::clamp_rgba(
                        &obuffer[..3 * count],
                        &mut buf.inner_pixels_mut()[3 * start_idx..3 * end_idx],
                    );
                }

                DynamicImage::ImageRgba16(buf) => {
                    CicpTransform::clamp_rgba(
                        &obuffer[..4 * count],
                        &mut buf.inner_pixels_mut()[4 * start_idx..4 * end_idx],
                    );
                }
                DynamicImage::ImageRgb32F(buf) => {
                    CicpTransform::clamp_rgb(
                        &obuffer[..3 * count],
                        &mut buf.inner_pixels_mut()[3 * start_idx..3 * end_idx],
                    );
                }
                DynamicImage::ImageRgba32F(buf) => {
                    CicpTransform::clamp_rgba(
                        &obuffer[..4 * count],
                        &mut buf.inner_pixels_mut()[4 * start_idx..4 * end_idx],
                    );
                }
            }
        }
    }

    // Note on this design: When we dispatch into this function, we have a `Self` type that is
    // qualified to have the appropriate bound here. However, for the target type of the transform
    // we have, e.g., `Rgba<Self::Subpixel>`. Now we know that these are also with color for the
    // most part but we can not convince the compiler (indeed, there is or was an asymmetry with
    // gray pixels where they do not have float equivalents). It is hence necessary to provide the
    // output layout as a runtime parameter, not a compile-time type.
    pub(crate) fn select_transform_u8<P: SealedPixelWithColorType<TransformableSubpixel = u8>>(
        &self,
        into: LayoutWithColor,
    ) -> &Arc<CicpApplicable<'static, u8>> {
        self.u8.select_transform::<P>(into)
    }

    pub(crate) fn select_transform_u16<O: SealedPixelWithColorType<TransformableSubpixel = u16>>(
        &self,
        into: LayoutWithColor,
    ) -> &Arc<CicpApplicable<'static, u16>> {
        self.u16.select_transform::<O>(into)
    }

    pub(crate) fn select_transform_f32<O: SealedPixelWithColorType<TransformableSubpixel = f32>>(
        &self,
        into: LayoutWithColor,
    ) -> &Arc<CicpApplicable<'static, f32>> {
        self.f32.select_transform::<O>(into)
    }

    const LAYOUTS: [(LayoutWithColor, LayoutWithColor); 4] = [
        (LayoutWithColor::Rgb, LayoutWithColor::Rgb),
        (LayoutWithColor::Rgb, LayoutWithColor::Rgba),
        (LayoutWithColor::Rgba, LayoutWithColor::Rgb),
        (LayoutWithColor::Rgba, LayoutWithColor::Rgba),
    ];

    pub(crate) fn expand_luma_rgb<P: ColorComponentForCicp>(luma: &[P], rgb: &mut [f32]) {
        for (&pix, rgb) in luma.iter().zip(rgb.chunks_exact_mut(3)) {
            let luma = pix.expand_to_f32();
            rgb[0] = luma;
            rgb[1] = luma;
            rgb[2] = luma;
        }
    }

    pub(crate) fn expand_luma_rgba<P: ColorComponentForCicp>(luma: &[P], rgb: &mut [f32]) {
        for (pix, rgb) in luma.chunks_exact(2).zip(rgb.chunks_exact_mut(4)) {
            let luma = pix[0].expand_to_f32();
            rgb[0] = luma;
            rgb[1] = luma;
            rgb[2] = luma;
            rgb[3] = pix[1].expand_to_f32();
        }
    }

    pub(crate) fn expand_rgb<P: ColorComponentForCicp>(input: &[P], output: &mut [f32]) {
        for (&component, val) in input.iter().zip(output) {
            *val = component.expand_to_f32();
        }
    }

    pub(crate) fn expand_rgba<P: ColorComponentForCicp>(input: &[P], output: &mut [f32]) {
        for (&component, val) in input.iter().zip(output) {
            *val = component.expand_to_f32();
        }
    }

    pub(crate) fn clamp_rgb<P: ColorComponentForCicp>(input: &[f32], output: &mut [P]) {
        // Everything is mapped..
        for (&component, val) in input.iter().zip(output) {
            *val = P::clamp_from_f32(component);
        }
    }

    pub(crate) fn clamp_rgba<P: ColorComponentForCicp>(input: &[f32], output: &mut [P]) {
        for (&component, val) in input.iter().zip(output) {
            *val = P::clamp_from_f32(component);
        }
    }

    pub(crate) fn clamp_rgb_luma<P: ColorComponentForCicp>(
        input: &[f32],
        output: &mut [P],
        coef: [f32; 3],
    ) {
        for (rgb, pix) in input.chunks_exact(3).zip(output) {
            let mut luma = 0.0;

            for (&component, coef) in rgb.iter().zip(coef) {
                luma = multiply_accumulate(luma, component, coef);
            }

            *pix = P::clamp_from_f32(luma);
        }
    }

    pub(crate) fn clamp_rgba_luma<P: ColorComponentForCicp>(
        input: &[f32],
        output: &mut [P],
        coef: [f32; 3],
    ) {
        for (rgba, pix) in input.chunks_exact(4).zip(output.chunks_exact_mut(2)) {
            let mut luma = 0.0;

            for (&component, coef) in rgba[..3].iter().zip(coef) {
                luma = multiply_accumulate(luma, component, coef);
            }

            pix[0] = P::clamp_from_f32(luma);
            pix[1] = P::clamp_from_f32(rgba[3]);
        }
    }
}

impl CicpRgb {
    /// Internal utility for converting color buffers of different pixel representations, assuming
    /// they have this same cicp. This method returns a buffer, avoiding the pre-zeroing
    /// the vector.
    pub(crate) fn cast_pixels<FromColor, IntoColor>(
        &self,
        buffer: &[FromColor::Subpixel],
        // Since this is not performance sensitive, we can use a dyn closure here instead of an
        // impl closure just in case we call this from multiple different paths.
        color_space_fallback: &dyn Fn() -> [f32; 3],
    ) -> Vec<IntoColor::Subpixel>
    where
        FromColor: Pixel + SealedPixelWithColorType<TransformableSubpixel = FromColor::Subpixel>,
        IntoColor: Pixel,
        IntoColor: CicpPixelCast<FromColor>,
        FromColor::Subpixel: ColorComponentForCicp,
        IntoColor::Subpixel: ColorComponentForCicp + FromPrimitive<FromColor::Subpixel>,
    {
        use crate::traits::private::PrivateToken;
        let from_layout = <FromColor as SealedPixelWithColorType>::layout(PrivateToken);
        let into_layout = <IntoColor as SealedPixelWithColorType>::layout(PrivateToken);

        let mut output = match self.cast_pixels_from_subpixels(buffer, from_layout, into_layout) {
            Ok(ok) => return ok,
            Err(buffer) => buffer,
        };

        // If we get here we need to transform through Rgb(a) 32F
        let color_space_coefs = self
            .derived_luminance()
            // Since `cast_pixels` must be infallible we have no choice but to fallback to
            // something here. This something is chosen by the caller, which would allow them to
            // detect it has happened.
            .unwrap_or_else(color_space_fallback);

        let pixels = buffer.len() / from_layout.channels();

        // All of the following is done in-place; so we must allow the buffer space in which the
        // output is written ahead of time although such initialization is technically redundant.
        // We best do this once to allow for a very efficient memset initialization.
        output.resize(
            pixels * into_layout.channels(),
            <IntoColor::Subpixel as Primitive>::DEFAULT_MIN_VALUE,
        );

        Self::cast_pixels_by_fallback(
            buffer,
            output.as_mut_slice(),
            from_layout,
            into_layout,
            color_space_coefs,
        );
        output
    }

    fn cast_pixels_by_fallback<
        From: Primitive + ColorComponentForCicp,
        Into: ColorComponentForCicp,
    >(
        buffer: &[From],
        output: &mut [Into],
        from_layout: LayoutWithColor,
        into_layout: LayoutWithColor,
        color_space_coefs: [f32; 3],
    ) {
        use LayoutWithColor as Layout;

        const STEP: usize = 256;
        let pixels = buffer.len() / from_layout.channels();

        let mut ibuffer = [0.0f32; 4 * STEP];
        let mut obuffer = [0.0f32; 4 * STEP];

        let ibuf_step = match from_layout {
            Layout::Rgb | Layout::Luma => 3,
            Layout::Rgba | Layout::LumaAlpha => 4,
        };

        let obuf_step = match into_layout {
            Layout::Rgb | Layout::Luma => 3,
            Layout::Rgba | Layout::LumaAlpha => 4,
        };

        for start_idx in (0..pixels).step_by(STEP) {
            let end_idx = (start_idx + STEP).min(pixels);
            let count = end_idx - start_idx;

            let ibuffer = &mut ibuffer[..ibuf_step * count];

            match from_layout {
                Layout::Rgb => {
                    CicpTransform::expand_rgb(&buffer[3 * start_idx..3 * end_idx], ibuffer)
                }
                Layout::Rgba => {
                    CicpTransform::expand_rgba(&buffer[4 * start_idx..4 * end_idx], ibuffer)
                }
                Layout::Luma => {
                    CicpTransform::expand_luma_rgb(&buffer[start_idx..end_idx], ibuffer)
                }
                Layout::LumaAlpha => {
                    CicpTransform::expand_luma_rgba(&buffer[2 * start_idx..2 * end_idx], ibuffer)
                }
            }

            // Add or subtract the alpha channel. We could do that as part of the store but this
            // keeps the code simpler—there is a one-to-one correspondence with the methods needed
            // for a full conversion.
            let obuffer = match (ibuf_step, obuf_step) {
                (3, 4) => {
                    for (rgb, rgba) in ibuffer
                        .chunks_exact(3)
                        .zip(obuffer.chunks_exact_mut(4))
                        .take(count)
                    {
                        rgba[0] = rgb[0];
                        rgba[1] = rgb[1];
                        rgba[2] = rgb[2];
                        rgba[3] = 1.0;
                    }

                    &obuffer[..4 * count]
                }
                (4, 3) => {
                    for (rgba, rgb) in ibuffer
                        .chunks_exact(4)
                        .zip(obuffer.chunks_exact_mut(3))
                        .take(count)
                    {
                        rgb[0] = rgba[0];
                        rgb[1] = rgba[1];
                        rgb[2] = rgba[2];
                    }

                    &obuffer[..3 * count]
                }
                (n, m) => {
                    debug_assert_eq!(n, m);
                    &ibuffer[..m * count]
                }
            };

            match into_layout {
                Layout::Rgb => {
                    CicpTransform::clamp_rgb(obuffer, &mut output[3 * start_idx..3 * end_idx]);
                }
                Layout::Rgba => {
                    CicpTransform::clamp_rgba(obuffer, &mut output[4 * start_idx..4 * end_idx]);
                }
                Layout::Luma => {
                    CicpTransform::clamp_rgb_luma(
                        obuffer,
                        &mut output[start_idx..end_idx],
                        color_space_coefs,
                    );
                }
                Layout::LumaAlpha => {
                    CicpTransform::clamp_rgba_luma(
                        obuffer,
                        &mut output[2 * start_idx..2 * end_idx],
                        color_space_coefs,
                    );
                }
            }
        }
    }

    /// Make sure this is only monomorphized for subpixel combinations, not for every pixel
    /// combination! There's ample time to do that in `cast_pixels`.
    pub(crate) fn cast_pixels_from_subpixels<FromSubpixel, IntoSubpixel>(
        &self,
        buffer: &[FromSubpixel],
        from_layout: LayoutWithColor,
        into_layout: LayoutWithColor,
    ) -> Result<Vec<IntoSubpixel>, Vec<IntoSubpixel>>
    where
        FromSubpixel: ColorComponentForCicp,
        IntoSubpixel: ColorComponentForCicp + FromPrimitive<FromSubpixel> + Primitive,
    {
        use crate::traits::private::LayoutWithColor as Layout;

        assert!(buffer.len() % from_layout.channels() == 0);
        let pixels = buffer.len() / from_layout.channels();

        let mut output: Vec<IntoSubpixel> = vec_try_with_capacity(pixels * into_layout.channels())
            // Not entirely failsafe, if you expand luma to rgba you can get a factor of 4 but at
            // least this will not overflow. And that's why I'm a fan of in-place operations.
            .expect("input layout already allocated with appropriate layout");
        let map_channel = <IntoSubpixel as FromPrimitive<FromSubpixel>>::from_primitive;

        match (from_layout, into_layout) {
            // First detect if we can use simple channel-by-channel component conversion.
            (Layout::Rgb, Layout::Rgb)
            | (Layout::Rgba, Layout::Rgba)
            | (Layout::Luma, Layout::Luma)
            | (Layout::LumaAlpha, Layout::LumaAlpha) => {
                output.extend(buffer.iter().copied().map(map_channel));
            }
            (Layout::Rgb, Layout::Rgba) => {
                // Use `as_chunks` with Rust 1.88
                output.extend(buffer.chunks_exact(3).flat_map(|rgb| {
                    let &rgb: &[_; 3] = rgb.try_into().unwrap();
                    let [r, g, b] = rgb.map(map_channel);
                    let a = <IntoSubpixel as Primitive>::DEFAULT_MAX_VALUE;
                    [r, g, b, a]
                }));
            }
            (Layout::Rgba, Layout::Rgb) => {
                output.extend(buffer.chunks_exact(4).flat_map(|rgb| {
                    let &[r, g, b, _]: &[_; 4] = rgb.try_into().unwrap();
                    [r, g, b].map(map_channel)
                }));
            }
            (Layout::Luma, Layout::LumaAlpha) => {
                output.extend(buffer.iter().copied().flat_map(|luma| {
                    let l = map_channel(luma);
                    let a = <IntoSubpixel as Primitive>::DEFAULT_MAX_VALUE;
                    [l, a]
                }));
            }
            (Layout::LumaAlpha, Layout::Luma) => {
                output.extend(buffer.chunks_exact(2).map(|rgb| {
                    let &[luma, _]: &[_; 2] = rgb.try_into().unwrap();
                    map_channel(luma)
                }));
            }
            _ => return Err(output),
        }

        Ok(output)
    }
}

/// Color types that can be converted by [`CicpRgb::cast_pixels`].
///
/// This is a utility to avoid dealing with lots of bounds everywhere. In the actual implementation
/// we avoid the concrete pixel types and care just about the layout (as a runtime property) and
/// the channel type to be promotable into a float for normalization. If the pixels have layouts
/// that are convertible with intra-channel numerics we instead try and promote the channels via
/// `Primitive` instead.
pub(crate) trait CicpPixelCast<FromColor>
where
    // Ensure we can get components from both, get the layout, and that all components are
    // compatible with our intermediate connection space (rgba32f).
    Self: Pixel + SealedPixelWithColorType<TransformableSubpixel = <Self as Pixel>::Subpixel>,
    FromColor:
        Pixel + SealedPixelWithColorType<TransformableSubpixel = <FromColor as Pixel>::Subpixel>,
    Self::Subpixel: ColorComponentForCicp + FromPrimitive<FromColor::Subpixel>,
    FromColor::Subpixel: ColorComponentForCicp,
{
}

impl<FromColor, IntoColor> CicpPixelCast<FromColor> for IntoColor
where
    IntoColor: Pixel + SealedPixelWithColorType<TransformableSubpixel = IntoColor::Subpixel>,
    FromColor: Pixel + SealedPixelWithColorType<TransformableSubpixel = FromColor::Subpixel>,
    IntoColor::Subpixel: ColorComponentForCicp + FromPrimitive<FromColor::Subpixel>,
    FromColor::Subpixel: ColorComponentForCicp,
{
}

pub(crate) trait ColorComponentForCicp: Copy {
    fn expand_to_f32(self) -> f32;

    fn clamp_from_f32(val: f32) -> Self;
}

impl ColorComponentForCicp for u8 {
    fn expand_to_f32(self) -> f32 {
        const R: f32 = 1.0 / u8::MAX as f32;
        self as f32 * R
    }

    #[inline]
    fn clamp_from_f32(val: f32) -> Self {
        // Note: saturating conversion does the clamp for us
        (val * Self::MAX as f32).round() as u8
    }
}

impl ColorComponentForCicp for u16 {
    fn expand_to_f32(self) -> f32 {
        const R: f32 = 1.0 / u16::MAX as f32;
        self as f32 * R
    }

    #[inline]
    fn clamp_from_f32(val: f32) -> Self {
        // Note: saturating conversion does the clamp for us
        (val * Self::MAX as f32).round() as u16
    }
}

impl ColorComponentForCicp for f32 {
    fn expand_to_f32(self) -> f32 {
        self
    }

    fn clamp_from_f32(val: f32) -> Self {
        val
    }
}

impl<P> RgbTransforms<P> {
    fn select_transform<O: SealedPixelWithColorType>(
        &self,
        into: LayoutWithColor,
    ) -> &Arc<CicpApplicable<'static, P>> {
        use crate::traits::private::{LayoutWithColor as Layout, PrivateToken};
        let from = O::layout(PrivateToken);

        match (from, into) {
            (Layout::Rgb, Layout::Rgb) => &self.slices[0],
            (Layout::Rgb, Layout::Rgba) => &self.slices[1],
            (Layout::Rgba, Layout::Rgb) => &self.slices[2],
            (Layout::Rgba, Layout::Rgba) => &self.slices[3],
            (Layout::Rgb, Layout::Luma) => &self.rgb_luma[0],
            (Layout::Rgb, Layout::LumaAlpha) => &self.rgb_luma[1],
            (Layout::Rgba, Layout::Luma) => &self.rgb_luma[2],
            (Layout::Rgba, Layout::LumaAlpha) => &self.rgb_luma[3],
            (Layout::Luma, Layout::Rgb) => &self.luma_rgb[0],
            (Layout::Luma, Layout::Rgba) => &self.luma_rgb[1],
            (Layout::LumaAlpha, Layout::Rgb) => &self.luma_rgb[2],
            (Layout::LumaAlpha, Layout::Rgba) => &self.luma_rgb[3],
            (Layout::Luma, Layout::Luma) => &self.luma_luma[0],
            (Layout::Luma, Layout::LumaAlpha) => &self.luma_luma[1],
            (Layout::LumaAlpha, Layout::Luma) => &self.luma_luma[2],
            (Layout::LumaAlpha, Layout::LumaAlpha) => &self.luma_luma[3],
        }
    }
}

impl Cicp {
    /// The sRGB color space, BT.709 transfer function and D65 whitepoint.
    pub const SRGB: Self = Cicp {
        primaries: CicpColorPrimaries::SRgb,
        transfer: CicpTransferCharacteristics::SRgb,
        matrix: CicpMatrixCoefficients::Identity,
        full_range: CicpVideoFullRangeFlag::FullRange,
    };

    /// SRGB primaries and whitepoint with linear samples.
    pub const SRGB_LINEAR: Self = Cicp {
        primaries: CicpColorPrimaries::SRgb,
        transfer: CicpTransferCharacteristics::Linear,
        matrix: CicpMatrixCoefficients::Identity,
        full_range: CicpVideoFullRangeFlag::FullRange,
    };

    /// The  Display-P3 color space, a wide-gamut choice with SMPTE RP 432-2 primaries.
    ///
    /// Note that this modern Display P3 uses a D65 whitepoint. Use the primaries `SmpteRp431` for
    /// the previous standard. The advantage of the new standard is the color system shares its
    /// whitepoint with sRGB and BT.2020.
    pub const DISPLAY_P3: Self = Cicp {
        primaries: CicpColorPrimaries::SmpteRp432,
        transfer: CicpTransferCharacteristics::SRgb,
        matrix: CicpMatrixCoefficients::Identity,
        full_range: CicpVideoFullRangeFlag::FullRange,
    };

    /// Get an compute representation of an ICC profile for RGB.
    ///
    /// Note you should *not* be using this profile for export in a file, as discussed below.
    ///
    /// This is straightforward for Rgb and RgbA representations.
    ///
    /// Our luma models a Y component of a YCbCr color space. It turns out that ICC V4 does
    /// not support pure Luma in any other whitepoint apart from D50 (the native profile
    /// connection space). The use of a grayTRC does *not* take the chromatic adaptation
    /// matrix into account. Of course we can encode the adaptation into the TRC as a
    /// coefficient, the Y component of the product of the whitepoint adaptation matrix
    /// inverse and the pcs's whitepoint XYZ, but that is only correct for gray -> gray
    /// conversion (and that coefficient should generally be `1`).
    ///
    /// Hence we use a YCbCr. The data->pcs path could be modelled by ("M" curves, matrix, "B"
    /// curves) where B curves or M curves are all the identity, depending on whether constant or
    /// non-constant luma is in use. This is a subset of the capabilities that a lutAToBType
    /// allows. Unfortunately, this is not implemented in moxcms yet and for efficiency we would
    /// like to have a masked `create_transform_*` in which the CbCr channels are discarded /
    /// assumed 0 instead of them being in memory. Due to this special case and for supporting
    /// conversions between sample types, we implement said promotion as part of conversion to
    /// Rgba32F in this crate.
    ///
    /// For export to file, it would arguably correct to use a carefully crafted gray profile which
    /// we may implement in another function. That is, we could setup a tone reproduction curve
    /// which maps each sample value (which ICC regards as D50) into XYZ D50 in such a way that it
    /// _appears_ with the correct D50 luminance that we would get if we had used the conversion
    /// unders its true input whitepoint. The resulting color has a slightly wrong chroma as it is
    /// linearly dependent on D50 instead, but it's brightness would be correctly presented. At
    /// least for perceptual intent this might be alright.
    fn to_moxcms_compute_profile(self) -> Option<ColorProfile> {
        let mut rgb = moxcms::ColorProfile::new_srgb();

        rgb.update_rgb_colorimetry_from_cicp(moxcms::CicpProfile {
            color_primaries: self.primaries.to_moxcms(),
            transfer_characteristics: self.transfer.to_moxcms(),
            matrix_coefficients: self.matrix.to_moxcms()?,
            full_range: match self.full_range {
                CicpVideoFullRangeFlag::NarrowRange => false,
                CicpVideoFullRangeFlag::FullRange => true,
            },
        });

        Some(ColorProfile { rgb })
    }

    /// Whether we have invested enough testing to ensure that color values can be assumed to be
    /// stable and correspond to an intended effect, in particular if there even is a well-defined
    /// meaning to these color spaces.
    ///
    /// For instance, our current code for the 'luma' equivalent space assumes that the color space
    /// has a shared transfer function for all its color components. Also the judgment should not
    /// depend on whether we can represent the profile in `moxcms` but rather if we understand the
    /// profile well enough so that conversion implemented through another library can be derived.
    /// (Consider the case of a builtin transform-while-encoding that may be more performant for a
    /// format that does not support CICP or ICC profiles.)
    ///
    /// A stable profile should also have `derived_luminance` implemented.
    pub(crate) const fn qualify_stability(&self) -> bool {
        const _: () = {
            // Out public constants _should_ be stable.
            assert!(Cicp::SRGB.qualify_stability());
            assert!(Cicp::SRGB_LINEAR.qualify_stability());
            assert!(Cicp::DISPLAY_P3.qualify_stability());
        };

        matches!(self.full_range, CicpVideoFullRangeFlag::FullRange)
            && matches!(
                self.matrix,
                // For pure RGB color
                CicpMatrixCoefficients::Identity
                    // The equivalent of our Luma color as a type..
                    | CicpMatrixCoefficients::ChromaticityDerivedNonConstant
            )
            && matches!(
                self.primaries,
                CicpColorPrimaries::SRgb
                    | CicpColorPrimaries::SmpteRp431
                    | CicpColorPrimaries::SmpteRp432
                    | CicpColorPrimaries::Bt601
                    | CicpColorPrimaries::Rgb240m
            )
            && matches!(
                self.transfer,
                CicpTransferCharacteristics::SRgb
                    | CicpTransferCharacteristics::Bt709
                    | CicpTransferCharacteristics::Bt601
                    | CicpTransferCharacteristics::Linear
            )
    }

    /// Discard matrix and range information.
    pub(crate) const fn into_rgb(self) -> CicpRgb {
        CicpRgb {
            primaries: self.primaries,
            transfer: self.transfer,
            // NOTE: if we add support for constant luminance (through the CMS having support for
            // the Luma->YCbCr->Rgb expansion natively or otherwise) then consider if we should
            // track here whether the matrix was `Identity` or `ChromaticityDerivedNonConstant` so
            // that the `ImageBuffer::color_space()` function roundtrips the value. It may be
            // important to know whether the non-constant chromaticity was an invention by `image`
            // or part of the file. The colorimetry is the same either way.
            luminance: DerivedLuminance::NonConstant,
        }
    }

    pub(crate) fn try_into_rgb(self) -> Result<CicpRgb, ImageError> {
        if Cicp::from(self.into_rgb()) != self {
            Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::RgbCicpRequired(self),
            )))
        } else {
            Ok(self.into_rgb())
        }
    }
}

impl CicpRgb {
    /// Calculate the luminance cofactors according to Rec H.273 (39) and (40).
    ///
    /// Returns cofactors for red, green, and blue in that order.
    pub(crate) fn derived_luminance(&self) -> Option<[f32; 3]> {
        let primaries = match self.primaries {
            CicpColorPrimaries::SRgb => moxcms::ColorPrimaries::BT_709,
            CicpColorPrimaries::RgbM => moxcms::ColorPrimaries::BT_470M,
            CicpColorPrimaries::RgbB => moxcms::ColorPrimaries::BT_470BG,
            CicpColorPrimaries::Bt601 => moxcms::ColorPrimaries::BT_601,
            CicpColorPrimaries::Rgb240m => moxcms::ColorPrimaries::SMPTE_240,
            CicpColorPrimaries::GenericFilm => moxcms::ColorPrimaries::GENERIC_FILM,
            CicpColorPrimaries::Rgb2020 => moxcms::ColorPrimaries::BT_2020,
            CicpColorPrimaries::Xyz => moxcms::ColorPrimaries::XYZ,
            CicpColorPrimaries::SmpteRp431 => moxcms::ColorPrimaries::DISPLAY_P3,
            CicpColorPrimaries::SmpteRp432 => moxcms::ColorPrimaries::DISPLAY_P3,
            CicpColorPrimaries::Industry22 => moxcms::ColorPrimaries::EBU_3213,
            CicpColorPrimaries::Unspecified => return None,
        };

        const ILLUMINANT_C: moxcms::Chromaticity = moxcms::Chromaticity::new(0.310, 0.316);

        let whitepoint = match self.primaries {
            CicpColorPrimaries::SRgb => moxcms::Chromaticity::D65,
            CicpColorPrimaries::RgbM => ILLUMINANT_C,
            CicpColorPrimaries::RgbB => moxcms::Chromaticity::D65,
            CicpColorPrimaries::Bt601 => moxcms::Chromaticity::D65,
            CicpColorPrimaries::Rgb240m => moxcms::Chromaticity::D65,
            CicpColorPrimaries::GenericFilm => ILLUMINANT_C,
            CicpColorPrimaries::Rgb2020 => moxcms::Chromaticity::D65,
            CicpColorPrimaries::Xyz => moxcms::Chromaticity::new(1. / 3., 1. / 3.),
            CicpColorPrimaries::SmpteRp431 => moxcms::Chromaticity::new(0.314, 0.351),
            CicpColorPrimaries::SmpteRp432 => moxcms::Chromaticity::D65,
            CicpColorPrimaries::Industry22 => moxcms::Chromaticity::D65,
            CicpColorPrimaries::Unspecified => return None,
        };

        let matrix = primaries.transform_to_xyz(whitepoint);

        // Our result is the Y row of this matrix.
        Some(matrix.v[1])
    }
}

impl From<CicpRgb> for Cicp {
    fn from(cicp: CicpRgb) -> Self {
        Cicp {
            primaries: cicp.primaries,
            transfer: cicp.transfer,
            matrix: CicpMatrixCoefficients::Identity,
            full_range: CicpVideoFullRangeFlag::FullRange,
        }
    }
}

/// An RGB profile with its related (same tone-mapping) gray profile.
///
/// This is the whole input information which we must be able to pass to the CMS in a support
/// transform, to handle all possible combinations of `ColorType` pixels that can be thrown at us.
/// For instance, in a previous iteration we had a separate gray profile here (but now handle that
/// internally by expansion to RGB through an YCbCr). Future iterations may add additional structs
/// to be computed for validating `CicpTransform::new`.
struct ColorProfile {
    rgb: moxcms::ColorProfile,
}

impl ColorProfile {
    fn map_layout(&self, layout: LayoutWithColor) -> (&moxcms::ColorProfile, moxcms::Layout) {
        match layout {
            LayoutWithColor::Rgb => (&self.rgb, moxcms::Layout::Rgb),
            LayoutWithColor::Rgba => (&self.rgb, moxcms::Layout::Rgba),
            // See comment in `to_moxcms_profile`.
            LayoutWithColor::Luma | LayoutWithColor::LumaAlpha => unreachable!(),
        }
    }
}

#[cfg(test)]
#[test]
fn moxcms() {
    let l = moxcms::TransferCharacteristics::Linear;
    assert_eq!(l.linearize(1.0), 1.0);
    assert_eq!(l.gamma(1.0), 1.0);

    assert_eq!(l.gamma(0.5), 0.5);
}

#[cfg(test)]
#[test]
fn derived_luminance() {
    let luminance = Cicp::SRGB.into_rgb().derived_luminance();
    let [kr, kg, kb] = luminance.unwrap();
    assert!((kr - 0.2126).abs() < 1e-4);
    assert!((kg - 0.7152).abs() < 1e-4);
    assert!((kb - 0.0722).abs() < 1e-4);
}

#[cfg(test)]
mod tests {
    use super::{Cicp, CicpTransform};
    use crate::{Luma, LumaA, Rgb, Rgba};

    #[test]
    fn can_create_transforms() {
        assert!(CicpTransform::new(Cicp::SRGB, Cicp::SRGB).is_some());
        assert!(CicpTransform::new(Cicp::SRGB, Cicp::DISPLAY_P3).is_some());
        assert!(CicpTransform::new(Cicp::DISPLAY_P3, Cicp::SRGB).is_some());
        assert!(CicpTransform::new(Cicp::DISPLAY_P3, Cicp::DISPLAY_P3).is_some());
    }

    fn no_coefficient_fallback() -> [f32; 3] {
        panic!("Fallback coefficients required")
    }

    #[test]
    fn transform_pixels_srgb() {
        // Non-constant luminance so:
        // Y = dot(rgb, coefs)
        let data = [255, 0, 0, 255];
        let color = Cicp::SRGB.into_rgb();
        let rgba = color.cast_pixels::<Rgba<u8>, Rgb<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(rgba, [255, 0, 0]);
        let luma = color.cast_pixels::<Rgba<u8>, Luma<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma, [54]); // 255 * 0.2126
        let luma_a = color.cast_pixels::<Rgba<u8>, LumaA<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma_a, [54, 255]);
    }

    #[test]
    fn transform_pixels_srgb_16() {
        // Non-constant luminance so:
        // Y = dot(rgb, coefs)
        let data = [u16::MAX / 2];
        let color = Cicp::SRGB.into_rgb();
        let rgba = color.cast_pixels::<Luma<u16>, Rgb<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(rgba, [127; 3]);
        let luma = color.cast_pixels::<Luma<u16>, Luma<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma, [127]);
        let luma_a = color.cast_pixels::<Luma<u16>, LumaA<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma_a, [127, 255]);

        let data = [u16::MAX / 2 + 1];
        let color = Cicp::SRGB.into_rgb();
        let rgba = color.cast_pixels::<Luma<u16>, Rgb<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(rgba, [128; 3]);
        let luma = color.cast_pixels::<Luma<u16>, Luma<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma, [128]);
        let luma_a = color.cast_pixels::<Luma<u16>, LumaA<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma_a, [128, 255]);
    }

    #[test]
    fn transform_pixels_srgb_luma_alpha() {
        // Non-constant luminance so:
        // Y = dot(rgb, coefs)
        let data = [u16::MAX / 2, u16::MAX];
        let color = Cicp::SRGB.into_rgb();
        let rgba = color.cast_pixels::<LumaA<u16>, Rgb<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(rgba, [127; 3]);
        let luma = color.cast_pixels::<LumaA<u16>, Luma<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma, [127]);
        let luma = color.cast_pixels::<LumaA<u16>, LumaA<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma, [127, u8::MAX]);
        let luma_a = color.cast_pixels::<LumaA<u16>, LumaA<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma_a, [127, 255]);

        let data = [u16::MAX / 2 + 1, u16::MAX];
        let color = Cicp::SRGB.into_rgb();
        let rgba = color.cast_pixels::<LumaA<u16>, Rgb<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(rgba, [128; 3]);
        let luma = color.cast_pixels::<LumaA<u16>, Luma<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma, [128]);
        let luma = color.cast_pixels::<LumaA<u16>, LumaA<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma, [128, u8::MAX]);
        let luma_a = color.cast_pixels::<LumaA<u16>, LumaA<u8>>(&data, &no_coefficient_fallback);
        assert_eq!(luma_a, [128, 255]);
    }
}
