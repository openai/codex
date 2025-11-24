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
use crate::chad::BRADFORD_D;
use crate::cicp::create_rec709_parametric;
use crate::trc::{ToneReprCurve, curve_from_gamma};
use crate::{
    CicpColorPrimaries, CicpProfile, ColorPrimaries, ColorProfile, DataColorSpace,
    LocalizableString, Matrix3d, MatrixCoefficients, ProfileClass, ProfileText, RenderingIntent,
    TransferCharacteristics, XyY,
};
use pxfm::{copysignk, exp, floor, pow};

/// From lcms: `cmsWhitePointFromTemp`
/// tempK must be >= 4000. and <= 25000.
/// Invalid values of tempK will return
/// (x,y,Y) = (-1.0, -1.0, -1.0)
/// similar to argyll: `icx_DTEMP2XYZ()`
const fn white_point_from_temperature(temp_k: i32) -> XyY {
    let mut white_point = XyY {
        x: 0.,
        y: 0.,
        yb: 0.,
    };
    // No optimization provided.
    let temp_k = temp_k as f64; // Square
    let temp_k2 = temp_k * temp_k; // Cube
    let temp_k3 = temp_k2 * temp_k;
    // For correlated color temperature (T) between 4000K and 7000K:
    let x = if temp_k > 4000.0 && temp_k <= 7000.0 {
        -4.6070 * (1E9 / temp_k3) + 2.9678 * (1E6 / temp_k2) + 0.09911 * (1E3 / temp_k) + 0.244063
    } else if temp_k > 7000.0 && temp_k <= 25000.0 {
        -2.0064 * (1E9 / temp_k3) + 1.9018 * (1E6 / temp_k2) + 0.24748 * (1E3 / temp_k) + 0.237040
    } else {
        // or for correlated color temperature (T) between 7000K and 25000K:
        // Invalid tempK
        white_point.x = -1.0;
        white_point.y = -1.0;
        white_point.yb = -1.0;
        debug_assert!(false, "invalid temp");
        return white_point;
    };
    // Obtain y(x)
    let y = -3.000 * (x * x) + 2.870 * x - 0.275;
    // wave factors (not used, but here for futures extensions)
    // let M1 = (-1.3515 - 1.7703*x + 5.9114 *y)/(0.0241 + 0.2562*x - 0.7341*y);
    // let M2 = (0.0300 - 31.4424*x + 30.0717*y)/(0.0241 + 0.2562*x - 0.7341*y);
    // Fill white_point struct
    white_point.x = x;
    white_point.y = y;
    white_point.yb = 1.0;
    white_point
}

pub const WHITE_POINT_D50: XyY = white_point_from_temperature(5003);
pub const WHITE_POINT_D60: XyY = white_point_from_temperature(6000);
pub const WHITE_POINT_D65: XyY = white_point_from_temperature(6504);
pub const WHITE_POINT_DCI_P3: XyY = white_point_from_temperature(6300);

// https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2100-2-201807-I!!PDF-F.pdf
// Perceptual Quantization / SMPTE standard ST.2084
#[inline]
const fn pq_curve(x: f64) -> f64 {
    const M1: f64 = 2610.0 / 16384.0;
    const M2: f64 = (2523.0 / 4096.0) * 128.0;
    const C1: f64 = 3424.0 / 4096.0;
    const C2: f64 = (2413.0 / 4096.0) * 32.0;
    const C3: f64 = (2392.0 / 4096.0) * 32.0;

    if x == 0.0 {
        return 0.0;
    }
    let sign = x;
    let x = x.abs();

    let xpo = pow(x, 1.0 / M2);
    let num = (xpo - C1).max(0.0);
    let den = C2 - C3 * xpo;
    let res = pow(num / den, 1.0 / M1);

    copysignk(res, sign)
}

pub(crate) const fn build_trc_table_pq() -> [u16; 4096] {
    let mut table = [0u16; 4096];

    const NUM_ENTRIES: usize = 4096;
    let mut i = 0usize;
    while i < NUM_ENTRIES {
        let x: f64 = i as f64 / (NUM_ENTRIES - 1) as f64;
        let y: f64 = pq_curve(x);
        let mut output: f64;
        output = y * 65535.0 + 0.5;
        if output > 65535.0 {
            output = 65535.0
        }
        if output < 0.0 {
            output = 0.0
        }
        table[i] = floor(output) as u16;
        i += 1;
    }
    table
}

pub(crate) const fn build_trc_table_hlg() -> [u16; 4096] {
    let mut table = [0u16; 4096];

    const NUM_ENTRIES: usize = 4096;
    let mut i = 0usize;
    while i < NUM_ENTRIES {
        let x: f64 = i as f64 / (NUM_ENTRIES - 1) as f64;
        let y: f64 = hlg_curve(x);
        let mut output: f64;
        output = y * 65535.0 + 0.5;
        if output > 65535.0 {
            output = 65535.0
        }
        if output < 0.0 {
            output = 0.0
        }
        table[i] = floor(output) as u16;
        i += 1;
    }
    table
}

// https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2100-2-201807-I!!PDF-F.pdf
// Hybrid Log-Gamma
const fn hlg_curve(x: f64) -> f64 {
    const BETA: f64 = 0.04;
    const RA: f64 = 5.591816309728916; // 1.0 / A where A = 0.17883277
    const B: f64 = 0.28466892; // 1.0 - 4.0 * A
    const C: f64 = 0.5599107295; // 0,5 –aln(4a)

    let e = (x * (1.0 - BETA) + BETA).max(0.0);

    if e == 0.0 {
        return 0.0;
    }

    let sign = e.abs();

    let res = if e <= 0.5 {
        e * e / 3.0
    } else {
        (exp((e - C) * RA) + B) / 12.0
    };

    copysignk(res, sign)
}

/// Perceptual Quantizer Lookup table
pub const PQ_LUT_TABLE: [u16; 4096] = build_trc_table_pq();
/// Hybrid Log Gamma Lookup table
pub const HLG_LUT_TABLE: [u16; 4096] = build_trc_table_hlg();

impl ColorProfile {
    const SRGB_COLORANTS: Matrix3d =
        ColorProfile::colorants_matrix(WHITE_POINT_D65, ColorPrimaries::BT_709);

    const DISPLAY_P3_COLORANTS: Matrix3d =
        ColorProfile::colorants_matrix(WHITE_POINT_D65, ColorPrimaries::SMPTE_432);

    const ADOBE_RGB_COLORANTS: Matrix3d =
        ColorProfile::colorants_matrix(WHITE_POINT_D65, ColorPrimaries::ADOBE_RGB);

    const DCI_P3_COLORANTS: Matrix3d =
        ColorProfile::colorants_matrix(WHITE_POINT_DCI_P3, ColorPrimaries::DCI_P3);

    const PRO_PHOTO_RGB_COLORANTS: Matrix3d =
        ColorProfile::colorants_matrix(WHITE_POINT_D50, ColorPrimaries::PRO_PHOTO_RGB);

    const BT2020_COLORANTS: Matrix3d =
        ColorProfile::colorants_matrix(WHITE_POINT_D65, ColorPrimaries::BT_2020);

    const ACES_2065_1_COLORANTS: Matrix3d =
        ColorProfile::colorants_matrix(WHITE_POINT_D60, ColorPrimaries::ACES_2065_1);

    const ACES_CG_COLORANTS: Matrix3d =
        ColorProfile::colorants_matrix(WHITE_POINT_D60, ColorPrimaries::ACES_CG);

    #[inline]
    fn basic_rgb_profile() -> ColorProfile {
        ColorProfile {
            profile_class: ProfileClass::DisplayDevice,
            rendering_intent: RenderingIntent::Perceptual,
            color_space: DataColorSpace::Rgb,
            pcs: DataColorSpace::Xyz,
            chromatic_adaptation: Some(BRADFORD_D),
            white_point: WHITE_POINT_D50.to_xyzd(),
            ..Default::default()
        }
    }

    /// Creates new profile from CICP
    pub fn new_from_cicp(cicp_color_primaries: CicpProfile) -> ColorProfile {
        let mut basic = ColorProfile::basic_rgb_profile();
        basic.update_rgb_colorimetry_from_cicp(cicp_color_primaries);
        basic
    }

    /// Creates new sRGB profile
    pub fn new_srgb() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::SRGB_COLORANTS);

        let curve =
            ToneReprCurve::Parametric(vec![2.4, 1. / 1.055, 0.055 / 1.055, 1. / 12.92, 0.04045]);
        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D65.to_xyzd());
        profile.cicp = Some(CicpProfile {
            color_primaries: CicpColorPrimaries::Bt709,
            transfer_characteristics: TransferCharacteristics::Srgb,
            matrix_coefficients: MatrixCoefficients::Bt709,
            full_range: false,
        });
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "sRGB IEC61966-2.1".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new Adobe RGB profile
    pub fn new_adobe_rgb() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::ADOBE_RGB_COLORANTS);

        let curve = curve_from_gamma(2.19921875f32);
        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D65.to_xyzd());
        profile.white_point = WHITE_POINT_D50.to_xyzd();
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Adobe RGB 1998".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new Display P3 profile
    pub fn new_display_p3() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::DISPLAY_P3_COLORANTS);

        let curve =
            ToneReprCurve::Parametric(vec![2.4, 1. / 1.055, 0.055 / 1.055, 1. / 12.92, 0.04045]);
        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D65.to_xyzd());
        profile.cicp = Some(CicpProfile {
            color_primaries: CicpColorPrimaries::Smpte431,
            transfer_characteristics: TransferCharacteristics::Srgb,
            matrix_coefficients: MatrixCoefficients::Bt709,
            full_range: false,
        });
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Display P3".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new Display P3 PQ profile
    pub fn new_display_p3_pq() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::DISPLAY_P3_COLORANTS);

        let curve = ToneReprCurve::Lut(PQ_LUT_TABLE.to_vec());

        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D65.to_xyzd());
        profile.cicp = Some(CicpProfile {
            color_primaries: CicpColorPrimaries::Smpte431,
            transfer_characteristics: TransferCharacteristics::Smpte2084,
            matrix_coefficients: MatrixCoefficients::Bt709,
            full_range: false,
        });
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Display P3 PQ".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new DCI P3 profile
    pub fn new_dci_p3() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::DCI_P3_COLORANTS);

        let curve = curve_from_gamma(2.6f32);
        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_DCI_P3.to_xyzd());
        profile.cicp = Some(CicpProfile {
            color_primaries: CicpColorPrimaries::Smpte432,
            transfer_characteristics: TransferCharacteristics::Srgb,
            matrix_coefficients: MatrixCoefficients::Bt709,
            full_range: false,
        });
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "DCI P3".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new ProPhoto RGB profile
    pub fn new_pro_photo_rgb() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::PRO_PHOTO_RGB_COLORANTS);

        let curve = curve_from_gamma(1.8f32);
        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D50.to_xyzd());
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "ProPhoto RGB".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new Bt.2020 profile
    pub fn new_bt2020() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::BT2020_COLORANTS);

        let curve = ToneReprCurve::Parametric(create_rec709_parametric().to_vec());
        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D65.to_xyzd());
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Rec.2020".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new Bt.2020 PQ profile
    pub fn new_bt2020_pq() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::BT2020_COLORANTS);

        let curve = ToneReprCurve::Lut(PQ_LUT_TABLE.to_vec());

        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D65.to_xyzd());
        profile.cicp = Some(CicpProfile {
            color_primaries: CicpColorPrimaries::Bt2020,
            transfer_characteristics: TransferCharacteristics::Smpte2084,
            matrix_coefficients: MatrixCoefficients::Bt709,
            full_range: false,
        });
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Rec.2020 PQ".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new Bt.2020 HLG profile
    pub fn new_bt2020_hlg() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::BT2020_COLORANTS);

        let curve = ToneReprCurve::Lut(HLG_LUT_TABLE.to_vec());

        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D65.to_xyzd());
        profile.cicp = Some(CicpProfile {
            color_primaries: CicpColorPrimaries::Bt2020,
            transfer_characteristics: TransferCharacteristics::Hlg,
            matrix_coefficients: MatrixCoefficients::Bt709,
            full_range: false,
        });
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Rec.2020 HLG".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new Monochrome profile
    pub fn new_gray_with_gamma(gamma: f32) -> ColorProfile {
        ColorProfile {
            gray_trc: Some(curve_from_gamma(gamma)),
            profile_class: ProfileClass::DisplayDevice,
            rendering_intent: RenderingIntent::Perceptual,
            color_space: DataColorSpace::Gray,
            media_white_point: Some(WHITE_POINT_D65.to_xyzd()),
            white_point: WHITE_POINT_D50.to_xyzd(),
            chromatic_adaptation: Some(BRADFORD_D),
            copyright: Some(ProfileText::Localizable(vec![LocalizableString::new(
                "en".to_string(),
                "US".to_string(),
                "Public Domain".to_string(),
            )])),
            ..Default::default()
        }
    }

    /// Creates new ACES 2065-1/AP0 profile
    pub fn new_aces_aces_2065_1_linear() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::ACES_2065_1_COLORANTS);

        let curve = ToneReprCurve::Lut(vec![]);
        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D60.to_xyzd());
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "ACES 2065-1".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }

    /// Creates new ACEScg profile
    pub fn new_aces_cg_linear() -> ColorProfile {
        let mut profile = ColorProfile::basic_rgb_profile();
        profile.update_colorants(ColorProfile::ACES_CG_COLORANTS);

        let curve = ToneReprCurve::Lut(vec![]);
        profile.red_trc = Some(curve.clone());
        profile.blue_trc = Some(curve.clone());
        profile.green_trc = Some(curve);
        profile.media_white_point = Some(WHITE_POINT_D60.to_xyzd());
        profile.description = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "ACEScg/AP1".to_string(),
        )]));
        profile.copyright = Some(ProfileText::Localizable(vec![LocalizableString::new(
            "en".to_string(),
            "US".to_string(),
            "Public Domain".to_string(),
        )]));
        profile
    }
}
