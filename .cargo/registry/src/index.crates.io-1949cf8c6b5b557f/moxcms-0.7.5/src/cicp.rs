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
use crate::gamma::{
    bt1361_to_linear, hlg_to_linear, iec61966_to_linear, log100_sqrt10_to_linear, log100_to_linear,
    pq_to_linear, smpte240_to_linear, smpte428_to_linear,
};
use crate::{
    Chromaticity, ColorProfile, Matrix3d, Matrix3f, XyYRepresentable,
    err::CmsError,
    trc::{ToneReprCurve, build_trc_table, curve_from_gamma},
};
use std::convert::TryFrom;

/// See [Rec. ITU-T H.273 (12/2016)](https://www.itu.int/rec/T-REC-H.273-201612-I/en) Table 2
/// Values 0, 3, 13–21, 23–255 are all reserved so all map to the same variant
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CicpColorPrimaries {
    /// For future use by ITU-T | ISO/IEC
    Reserved,
    /// Rec. ITU-R BT.709-6<br />
    /// Rec. ITU-R BT.1361-0 conventional colour gamut system and extended colour gamut system (historical)<br />
    /// IEC 61966-2-1 sRGB or sYCC IEC 61966-2-4<br />
    /// Society of Motion Picture and Television Engineers (MPTE) RP 177 (1993) Annex B<br />
    Bt709 = 1,
    /// Unspecified<br />
    /// Image characteristics are unknown or are determined by the application.
    Unspecified = 2,
    /// Rec. ITU-R BT.470-6 System M (historical)<br />
    /// United States National Television System Committee 1953 Recommendation for transmission standards for color television<br />
    /// United States Federal Communications Commission (2003) Title 47 Code of Federal Regulations 73.682 (a) (20)<br />
    Bt470M = 4,
    /// Rec. ITU-R BT.470-6 System B, G (historical) Rec. ITU-R BT.601-7 625<br />
    /// Rec. ITU-R BT.1358-0 625 (historical)<br />
    /// Rec. ITU-R BT.1700-0 625 PAL and 625 SECAM<br />
    Bt470Bg = 5,
    /// Rec. ITU-R BT.601-7 525<br />
    /// Rec. ITU-R BT.1358-1 525 or 625 (historical) Rec. ITU-R BT.1700-0 NTSC<br />
    /// SMPTE 170M (2004)<br />
    /// (functionally the same as the value 7)<br />
    Bt601 = 6,
    /// SMPTE 240M (1999) (historical) (functionally the same as the value 6)<br />
    Smpte240 = 7,
    /// Generic film (colour filters using Illuminant C)<br />
    GenericFilm = 8,
    /// Rec. ITU-R BT.2020-2<br />
    /// Rec. ITU-R BT.2100-0<br />
    Bt2020 = 9,
    /// SMPTE ST 428-1<br />
    /// (CIE 1931 XYZ as in ISO 11664-1)<br />
    Xyz = 10,
    /// SMPTE RP 431-2 (2011)<br />
    Smpte431 = 11,
    /// SMPTE EG 432-1 (2010)<br />
    Smpte432 = 12,
    /// EBU Tech. 3213-E (1975)<br />
    Ebu3213 = 22,
}

impl TryFrom<u8> for CicpColorPrimaries {
    type Error = CmsError;

    #[allow(unreachable_patterns)]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            // Values 0, 3, 13–21, 23–255 are all reserved so all map to the
            // same variant.
            0 | 3 | 13..=21 | 23..=255 => Ok(Self::Reserved),
            1 => Ok(Self::Bt709),
            2 => Ok(Self::Unspecified),
            4 => Ok(Self::Bt470M),
            5 => Ok(Self::Bt470Bg),
            6 => Ok(Self::Bt601),
            7 => Ok(Self::Smpte240),
            8 => Ok(Self::GenericFilm),
            9 => Ok(Self::Bt2020),
            10 => Ok(Self::Xyz),
            11 => Ok(Self::Smpte431),
            12 => Ok(Self::Smpte432),
            22 => Ok(Self::Ebu3213),
            _ => Err(CmsError::InvalidCicp),
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ColorPrimaries {
    pub red: Chromaticity,
    pub green: Chromaticity,
    pub blue: Chromaticity,
}

/// See [Rec. ITU-T H.273 (12/2016)](https://www.itu.int/rec/T-REC-H.273-201612-I/en) Table 2.
impl ColorPrimaries {
    /// [ACEScg](https://en.wikipedia.org/wiki/Academy_Color_Encoding_System#ACEScg).
    pub const ACES_CG: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.713, y: 0.293 },
        green: Chromaticity { x: 0.165, y: 0.830 },
        blue: Chromaticity { x: 0.128, y: 0.044 },
    };

    /// [ACES2065-1](https://en.wikipedia.org/wiki/Academy_Color_Encoding_System#ACES2065-1).
    pub const ACES_2065_1: ColorPrimaries = ColorPrimaries {
        red: Chromaticity {
            x: 0.7347,
            y: 0.2653,
        },
        green: Chromaticity {
            x: 0.0000,
            y: 1.0000,
        },
        blue: Chromaticity {
            x: 0.0001,
            y: -0.0770,
        },
    };

    /// [Adobe RGB](https://en.wikipedia.org/wiki/Adobe_RGB_color_space) (1998).
    pub const ADOBE_RGB: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.64, y: 0.33 },
        green: Chromaticity { x: 0.21, y: 0.71 },
        blue: Chromaticity { x: 0.15, y: 0.06 },
    };

    /// [DCI P3](https://en.wikipedia.org/wiki/DCI-P3#DCI_P3).
    ///
    /// This is the same as [`DISPLAY_P3`](Self::DISPLAY_P3),
    /// [`SMPTE_431`](Self::SMPTE_431) and [`SMPTE_432`](Self::SMPTE_432).
    pub const DCI_P3: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.680, y: 0.320 },
        green: Chromaticity { x: 0.265, y: 0.690 },
        blue: Chromaticity { x: 0.150, y: 0.060 },
    };

    /// [Diplay P3](https://en.wikipedia.org/wiki/DCI-P3#Display_P3).
    ///
    /// This is the same as [`DCI_P3`](Self::DCI_P3),
    /// [`SMPTE_431`](Self::SMPTE_431) and [`SMPTE_432`](Self::SMPTE_432).
    pub const DISPLAY_P3: ColorPrimaries = Self::DCI_P3;

    /// SMPTE RP 431-2 (2011).
    ///
    /// This is the same as [`DCI_P3`](Self::DCI_P3),
    /// [`DISPLAY_P3`](Self::DISPLAY_P3) and [`SMPTE_432`](Self::SMPTE_432).
    pub const SMPTE_431: ColorPrimaries = Self::DCI_P3;

    /// SMPTE EG 432-1 (2010).
    ///
    /// This is the same as [`DCI_P3`](Self::DCI_P3),
    /// [`DISPLAY_P3`](Self::DISPLAY_P3) and [`SMPTE_431`](Self::SMPTE_431).
    pub const SMPTE_432: ColorPrimaries = Self::DCI_P3;

    /// [ProPhoto RGB](https://en.wikipedia.org/wiki/ProPhoto_RGB_color_space).
    pub const PRO_PHOTO_RGB: ColorPrimaries = ColorPrimaries {
        red: Chromaticity {
            x: 0.734699,
            y: 0.265301,
        },
        green: Chromaticity {
            x: 0.159597,
            y: 0.840403,
        },
        blue: Chromaticity {
            x: 0.036598,
            y: 0.000105,
        },
    };

    /// Rec. ITU-R BT.709-6
    ///
    /// Rec. ITU-R BT.1361-0 conventional colour gamut system and extended
    /// colour gamut system (historical).
    ///
    /// IEC 61966-2-1 sRGB or sYCC IEC 61966-2-4).
    ///
    /// Society of Motion Picture and Television Engineers (MPTE) RP 177 (1993) Annex B.
    pub const BT_709: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.64, y: 0.33 },
        green: Chromaticity { x: 0.30, y: 0.60 },
        blue: Chromaticity { x: 0.15, y: 0.06 },
    };

    /// Rec. ITU-R BT.470-6 System M (historical).
    ///
    /// United States National Television System Committee 1953 Recommendation
    /// for transmission standards for color television.
    ///
    /// United States Federal Communications Commission (2003) Title 47 Code of
    /// Federal Regulations 73.682 (a) (20).
    pub const BT_470M: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.67, y: 0.33 },
        green: Chromaticity { x: 0.21, y: 0.71 },
        blue: Chromaticity { x: 0.14, y: 0.08 },
    };

    /// Rec. ITU-R BT.470-6 System B, G (historical) Rec. ITU-R BT.601-7 625.
    ///
    /// Rec. ITU-R BT.1358-0 625 (historical).
    /// Rec. ITU-R BT.1700-0 625 PAL and 625 SECAM.
    pub const BT_470BG: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.64, y: 0.33 },
        green: Chromaticity { x: 0.29, y: 0.60 },
        blue: Chromaticity { x: 0.15, y: 0.06 },
    };

    /// Rec. ITU-R BT.601-7 525.
    ///
    /// Rec. ITU-R BT.1358-1 525 or 625 (historical) Rec. ITU-R BT.1700-0 NTSC.
    ///
    /// SMPTE 170M (2004) (functionally the same as the [`SMPTE_240`](Self::SMPTE_240)).
    pub const BT_601: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.630, y: 0.340 },
        green: Chromaticity { x: 0.310, y: 0.595 },
        blue: Chromaticity { x: 0.155, y: 0.070 },
    };

    /// SMPTE 240M (1999) (historical) (functionally the same as [`BT_601`](Self::BT_601)).
    pub const SMPTE_240: ColorPrimaries = Self::BT_601;

    /// Generic film (colour filters using Illuminant C).
    pub const GENERIC_FILM: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.681, y: 0.319 },
        green: Chromaticity { x: 0.243, y: 0.692 },
        blue: Chromaticity { x: 0.145, y: 0.049 },
    };

    /// Rec. ITU-R BT.2020-2.
    ///
    /// Rec. ITU-R BT.2100-0.
    pub const BT_2020: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.708, y: 0.292 },
        green: Chromaticity { x: 0.170, y: 0.797 },
        blue: Chromaticity { x: 0.131, y: 0.046 },
    };

    /// SMPTE ST 428-1 (CIE 1931 XYZ as in ISO 11664-1).
    pub const XYZ: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 1.0, y: 0.0 },
        green: Chromaticity { x: 0.0, y: 1.0 },
        blue: Chromaticity { x: 0.0, y: 0.0 },
    };

    /// EBU Tech. 3213-E (1975).
    pub const EBU_3213: ColorPrimaries = ColorPrimaries {
        red: Chromaticity { x: 0.630, y: 0.340 },
        green: Chromaticity { x: 0.295, y: 0.605 },
        blue: Chromaticity { x: 0.155, y: 0.077 },
    };
}

impl ColorPrimaries {
    /// Returns RGB -> XYZ conversion matrix
    ///
    /// # Arguments
    ///
    /// * `white_point`: [Chromaticity] or [crate::XyY] or any item conforming [XyYRepresentable]
    ///
    /// returns: [Matrix3d]
    pub fn transform_to_xyz_d(self, white_point: impl XyYRepresentable) -> Matrix3d {
        let red_xyz = self.red.to_scaled_xyzd();
        let green_xyz = self.green.to_scaled_xyzd();
        let blue_xyz = self.blue.to_scaled_xyzd();

        let xyz_matrix = Matrix3d {
            v: [
                [red_xyz.x, green_xyz.x, blue_xyz.x],
                [red_xyz.y, green_xyz.y, blue_xyz.y],
                [red_xyz.z, green_xyz.z, blue_xyz.z],
            ],
        };
        ColorProfile::rgb_to_xyz_d(xyz_matrix, white_point.to_xyy().to_xyzd())
    }

    /// Returns RGB -> XYZ conversion matrix
    ///
    /// # Arguments
    ///
    /// * `white_point`: [Chromaticity] or [crate::XyY] or any item conforming [XyYRepresentable]
    ///
    /// returns: [Matrix3f]
    pub fn transform_to_xyz(self, white_point: impl XyYRepresentable) -> Matrix3f {
        let red_xyz = self.red.to_scaled_xyz();
        let green_xyz = self.green.to_scaled_xyz();
        let blue_xyz = self.blue.to_scaled_xyz();

        let xyz_matrix = Matrix3f {
            v: [
                [red_xyz.x, green_xyz.x, blue_xyz.x],
                [red_xyz.y, green_xyz.y, blue_xyz.y],
                [red_xyz.z, green_xyz.z, blue_xyz.z],
            ],
        };
        ColorProfile::rgb_to_xyz_static(xyz_matrix, white_point.to_xyy().to_xyz())
    }
}

/// See [Rec. ITU-T H.273 (12/2016)](https://www.itu.int/rec/T-REC-H.273-201612-I/en) Table 3
/// Values 0, 3, 19–255 are all reserved so all map to the same variant
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransferCharacteristics {
    /// For future use by ITU-T | ISO/IEC
    Reserved,
    /// Rec. ITU-R BT.709-6<br />
    /// Rec. ITU-R BT.1361-0 conventional colour gamut system (historical)<br />
    /// (functionally the same as the values 6, 14 and 15)    <br />
    Bt709 = 1,
    /// Image characteristics are unknown or are determined by the application.<br />
    Unspecified = 2,
    /// Rec. ITU-R BT.470-6 System M (historical)<br />
    /// United States National Television System Committee 1953 Recommendation for transmission standards for color television<br />
    /// United States Federal Communications Commission (2003) Title 47 Code of Federal Regulations 73.682 (a) (20)<br />
    /// Rec. ITU-R BT.1700-0 625 PAL and 625 SECAM<br />
    Bt470M = 4,
    /// Rec. ITU-R BT.470-6 System B, G (historical)<br />
    Bt470Bg = 5,
    /// Rec. ITU-R BT.601-7 525 or 625<br />
    /// Rec. ITU-R BT.1358-1 525 or 625 (historical)<br />
    /// Rec. ITU-R BT.1700-0 NTSC SMPTE 170M (2004)<br />
    /// (functionally the same as the values 1, 14 and 15)<br />
    Bt601 = 6,
    /// SMPTE 240M (1999) (historical)<br />
    Smpte240 = 7,
    /// Linear transfer characteristics<br />
    Linear = 8,
    /// Logarithmic transfer characteristic (100:1 range)<br />
    Log100 = 9,
    /// Logarithmic transfer characteristic (100 * Sqrt( 10 ) : 1 range)<br />
    Log100sqrt10 = 10,
    /// IEC 61966-2-4<br />
    Iec61966 = 11,
    /// Rec. ITU-R BT.1361-0 extended colour gamut system (historical)<br />
    Bt1361 = 12,
    /// IEC 61966-2-1 sRGB or sYCC<br />
    Srgb = 13,
    /// Rec. ITU-R BT.2020-2 (10-bit system)<br />
    /// (functionally the same as the values 1, 6 and 15)<br />
    Bt202010bit = 14,
    /// Rec. ITU-R BT.2020-2 (12-bit system)<br />
    /// (functionally the same as the values 1, 6 and 14)<br />
    Bt202012bit = 15,
    /// SMPTE ST 2084 for 10-, 12-, 14- and 16-bitsystems<br />
    /// Rec. ITU-R BT.2100-0 perceptual quantization (PQ) system<br />
    Smpte2084 = 16,
    /// SMPTE ST 428-1<br />
    Smpte428 = 17,
    /// ARIB STD-B67<br />
    /// Rec. ITU-R BT.2100-0 hybrid log- gamma (HLG) system<br />
    Hlg = 18,
}

impl TryFrom<u8> for TransferCharacteristics {
    type Error = CmsError;

    #[allow(unreachable_patterns)]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 | 3 | 19..=255 => Ok(Self::Reserved),
            1 => Ok(Self::Bt709),
            2 => Ok(Self::Unspecified),
            4 => Ok(Self::Bt470M),
            5 => Ok(Self::Bt470Bg),
            6 => Ok(Self::Bt601),
            7 => Ok(Self::Smpte240), // unimplemented
            8 => Ok(Self::Linear),
            9 => Ok(Self::Log100),
            10 => Ok(Self::Log100sqrt10),
            11 => Ok(Self::Iec61966), // unimplemented
            12 => Ok(Self::Bt1361),   // unimplemented
            13 => Ok(Self::Srgb),
            14 => Ok(Self::Bt202010bit),
            15 => Ok(Self::Bt202012bit),
            16 => Ok(Self::Smpte2084),
            17 => Ok(Self::Smpte428), // unimplemented
            18 => Ok(Self::Hlg),
            _ => Err(CmsError::InvalidCicp),
        }
    }
}

impl CicpColorPrimaries {
    pub(crate) const fn has_chromaticity(self) -> bool {
        self as u8 != Self::Reserved as u8 && self as u8 != Self::Unspecified as u8
    }

    pub(crate) const fn white_point(self) -> Result<Chromaticity, CmsError> {
        Ok(match self {
            Self::Reserved => return Err(CmsError::UnsupportedColorPrimaries(self as u8)),
            Self::Bt709
            | Self::Bt470Bg
            | Self::Bt601
            | Self::Smpte240
            | Self::Bt2020
            | Self::Smpte432
            | Self::Ebu3213 => Chromaticity::D65,
            Self::Unspecified => return Err(CmsError::UnsupportedColorPrimaries(self as u8)),
            Self::Bt470M => Chromaticity { x: 0.310, y: 0.316 },
            Self::GenericFilm => Chromaticity { x: 0.310, y: 0.316 },
            Self::Xyz => Chromaticity {
                x: 1. / 3.,
                y: 1. / 3.,
            },
            Self::Smpte431 => Chromaticity { x: 0.314, y: 0.351 },
        })
    }
}

impl TryFrom<CicpColorPrimaries> for ColorPrimaries {
    type Error = CmsError;

    fn try_from(value: CicpColorPrimaries) -> Result<Self, Self::Error> {
        match value {
            CicpColorPrimaries::Reserved => Err(CmsError::UnsupportedColorPrimaries(value as u8)),
            CicpColorPrimaries::Bt709 => Ok(ColorPrimaries::BT_709),
            CicpColorPrimaries::Unspecified => {
                Err(CmsError::UnsupportedColorPrimaries(value as u8))
            }
            CicpColorPrimaries::Bt470M => Ok(ColorPrimaries::BT_470M),
            CicpColorPrimaries::Bt470Bg => Ok(ColorPrimaries::BT_470BG),
            CicpColorPrimaries::Bt601 | CicpColorPrimaries::Smpte240 => Ok(ColorPrimaries::BT_601),
            CicpColorPrimaries::GenericFilm => Ok(ColorPrimaries::GENERIC_FILM),
            CicpColorPrimaries::Bt2020 => Ok(ColorPrimaries::BT_2020),
            CicpColorPrimaries::Xyz => Ok(ColorPrimaries::XYZ),
            // These two share primaries, but have distinct white points
            CicpColorPrimaries::Smpte431 | CicpColorPrimaries::Smpte432 => {
                Ok(ColorPrimaries::SMPTE_431)
            }
            CicpColorPrimaries::Ebu3213 => Ok(ColorPrimaries::EBU_3213),
        }
    }
}

impl TransferCharacteristics {
    pub(crate) fn has_transfer_curve(self) -> bool {
        self != Self::Reserved && self != Self::Unspecified
    }
}

pub(crate) fn create_rec709_parametric() -> [f32; 5] {
    const POW_EXP: f32 = 0.45;

    const G: f32 = 1. / POW_EXP;
    const B: f32 = (0.09929682680944f64 / 1.09929682680944f64) as f32;
    const C: f32 = 1f32 / 4.5f32;
    const D: f32 = (4.5f64 * 0.018053968510807f64) as f32;
    const A: f32 = (1. / 1.09929682680944f64) as f32;

    [G, A, B, C, D]
}

impl TryFrom<TransferCharacteristics> for ToneReprCurve {
    type Error = CmsError;
    /// See [ICC.1:2010](https://www.color.org/specification/ICC1v43_2010-12.pdf)
    /// See [Rec. ITU-R BT.2100-2](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2100-2-201807-I!!PDF-E.pdf)
    fn try_from(value: TransferCharacteristics) -> Result<Self, Self::Error> {
        const NUM_TRC_TABLE_ENTRIES: i32 = 1024;

        Ok(match value {
            TransferCharacteristics::Reserved => {
                return Err(CmsError::UnsupportedTrc(value as u8));
            }
            TransferCharacteristics::Bt709
            | TransferCharacteristics::Bt601
            | TransferCharacteristics::Bt202010bit
            | TransferCharacteristics::Bt202012bit => {
                // The opto-electronic transfer characteristic function (OETF)
                // as defined in ITU-T H.273 table 3, row 1:
                //
                // V = (α * Lc^0.45) − (α − 1)  for 1 >= Lc >= β
                // V = 4.500 * Lc               for β >  Lc >= 0
                //
                // Inverting gives the electro-optical transfer characteristic
                // function (EOTF) which can be represented as ICC
                // parametricCurveType with 4 parameters (ICC.1:2010 Table 5).
                // Converting between the two (Lc ↔︎ Y, V ↔︎ X):
                //
                // Y = (a * X + b)^g  for (X >= d)
                // Y = c * X          for (X < d)
                //
                // g, a, b, c, d can then be defined in terms of α and β:
                //
                // g = 1 / 0.45
                // a = 1 / α
                // b = 1 - α
                // c = 1 / 4.500
                // d = 4.500 * β
                //
                // α and β are determined by solving the piecewise equations to
                // ensure continuity of both value and slope at the value β.
                // We use the values specified for 10-bit systems in
                // https://www.itu.int/rec/R-REC-BT.2020-2-201510-I Table 4
                // since this results in the similar values as available ICC
                // profiles after converting to s15Fixed16Number, providing us
                // good test coverage.

                ToneReprCurve::Parametric(create_rec709_parametric().to_vec())
            }
            TransferCharacteristics::Unspecified => {
                return Err(CmsError::UnsupportedTrc(value as u8));
            }
            TransferCharacteristics::Bt470M => curve_from_gamma(2.2),
            TransferCharacteristics::Bt470Bg => curve_from_gamma(2.8),
            TransferCharacteristics::Smpte240 => {
                let table = build_trc_table(NUM_TRC_TABLE_ENTRIES, smpte240_to_linear);
                ToneReprCurve::Lut(table)
            }
            TransferCharacteristics::Linear => curve_from_gamma(1.),
            TransferCharacteristics::Log100 => {
                let table = build_trc_table(NUM_TRC_TABLE_ENTRIES, log100_to_linear);
                ToneReprCurve::Lut(table)
            }
            TransferCharacteristics::Log100sqrt10 => {
                let table = build_trc_table(NUM_TRC_TABLE_ENTRIES, log100_sqrt10_to_linear);
                ToneReprCurve::Lut(table)
            }
            TransferCharacteristics::Iec61966 => {
                let table = build_trc_table(NUM_TRC_TABLE_ENTRIES, iec61966_to_linear);
                ToneReprCurve::Lut(table)
            }
            TransferCharacteristics::Bt1361 => {
                let table = build_trc_table(NUM_TRC_TABLE_ENTRIES, bt1361_to_linear);
                ToneReprCurve::Lut(table)
            }
            TransferCharacteristics::Srgb => {
                ToneReprCurve::Parametric(vec![2.4, 1. / 1.055, 0.055 / 1.055, 1. / 12.92, 0.04045])
            }
            TransferCharacteristics::Smpte2084 => {
                let table = build_trc_table(NUM_TRC_TABLE_ENTRIES, pq_to_linear);
                ToneReprCurve::Lut(table)
            }
            TransferCharacteristics::Smpte428 => {
                let table = build_trc_table(NUM_TRC_TABLE_ENTRIES, smpte428_to_linear);
                ToneReprCurve::Lut(table)
            }
            TransferCharacteristics::Hlg => {
                let table = build_trc_table(NUM_TRC_TABLE_ENTRIES, hlg_to_linear);
                ToneReprCurve::Lut(table)
            }
        })
    }
}

/// Matrix Coefficients Enum (from ISO/IEC 23091-4 / MPEG CICP)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub enum MatrixCoefficients {
    Identity = 0,                // RGB (Identity matrix)
    Bt709 = 1,                   // Rec. 709
    Unspecified = 2,             // Unspecified
    Reserved = 3,                // Reserved
    Fcc = 4,                     // FCC
    Bt470Bg = 5,                 // BT.470BG / BT.601-625
    Smpte170m = 6,               // SMPTE 170M / BT.601-525
    Smpte240m = 7,               // SMPTE 240M
    YCgCo = 8,                   // YCgCo
    Bt2020Ncl = 9,               // BT.2020 (non-constant luminance)
    Bt2020Cl = 10,               // BT.2020 (constant luminance)
    Smpte2085 = 11,              // SMPTE ST 2085
    ChromaticityDerivedNCL = 12, // Chromaticity-derived non-constant luminance
    ChromaticityDerivedCL = 13,  // Chromaticity-derived constant luminance
    ICtCp = 14,                  // ICtCp
}

impl TryFrom<u8> for MatrixCoefficients {
    type Error = CmsError;

    fn try_from(value: u8) -> Result<Self, CmsError> {
        match value {
            0 => Ok(MatrixCoefficients::Identity),
            1 => Ok(MatrixCoefficients::Bt709),
            2 => Ok(MatrixCoefficients::Unspecified),
            3 => Ok(MatrixCoefficients::Reserved),
            4 => Ok(MatrixCoefficients::Fcc),
            5 => Ok(MatrixCoefficients::Bt470Bg),
            6 => Ok(MatrixCoefficients::Smpte170m),
            7 => Ok(MatrixCoefficients::Smpte240m),
            8 => Ok(MatrixCoefficients::YCgCo),
            9 => Ok(MatrixCoefficients::Bt2020Ncl),
            10 => Ok(MatrixCoefficients::Bt2020Cl),
            11 => Ok(MatrixCoefficients::Smpte2085),
            12 => Ok(MatrixCoefficients::ChromaticityDerivedNCL),
            13 => Ok(MatrixCoefficients::ChromaticityDerivedCL),
            14 => Ok(MatrixCoefficients::ICtCp),
            _ => Err(CmsError::InvalidCicp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WHITE_POINT_D65;

    #[test]
    fn test_to_xyz_using_absolute_coordinates() {
        let conversion_matrix = ColorPrimaries::BT_709.transform_to_xyz_d(WHITE_POINT_D65);
        assert!((conversion_matrix.v[0][0] - 0.4121524015214193).abs() < 1e-14);
        assert!((conversion_matrix.v[1][1] - 0.7153537403945436).abs() < 1e-14);
        assert!((conversion_matrix.v[2][2] - 0.9497138466283235).abs() < 1e-14);
    }

    #[test]
    fn test_to_xyz_using_absolute_coordinates_xyz() {
        let conversion_matrix = ColorPrimaries::XYZ.transform_to_xyz_d(WHITE_POINT_D65);
        assert!((conversion_matrix.v[0][0] - 0.95015469385536477).abs() < 1e-14);
        assert!((conversion_matrix.v[1][1] - 1.0).abs() < 1e-14);
        assert!((conversion_matrix.v[2][2] - 1.0882590676722474).abs() < 1e-14);
    }

    #[test]
    fn test_to_xyz_using_absolute_coordinates_f() {
        let conversion_matrix = ColorPrimaries::BT_709.transform_to_xyz(WHITE_POINT_D65);
        assert!((conversion_matrix.v[0][0] - 0.4121524015214193).abs() < 1e-5);
        assert!((conversion_matrix.v[1][1] - 0.7153537403945436).abs() < 1e-5);
        assert!((conversion_matrix.v[2][2] - 0.9497138466283235).abs() < 1e-5);
    }
}
