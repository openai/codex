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
use crate::mlaf::{fmla, mlaf};
use crate::transform::PointeeSizeExpressible;
use crate::{Rgb, TransferCharacteristics};
use num_traits::AsPrimitive;
use pxfm::{
    dirty_powf, f_exp, f_exp10, f_exp10f, f_expf, f_log, f_log10, f_log10f, f_logf, f_pow, f_powf,
};

#[inline]
/// Linear transfer function for sRGB
fn srgb_to_linear(gamma: f64) -> f64 {
    if gamma < 0f64 {
        0f64
    } else if gamma < 12.92f64 * 0.0030412825601275209f64 {
        gamma * (1f64 / 12.92f64)
    } else if gamma < 1.0f64 {
        f_pow(
            (gamma + 0.0550107189475866f64) / 1.0550107189475866f64,
            2.4f64,
        )
    } else {
        1.0f64
    }
}

#[inline]
/// Linear transfer function for sRGB
fn srgb_to_linearf_extended(gamma: f32) -> f32 {
    if gamma < 12.92 * 0.0030412825601275209 {
        gamma * (1. / 12.92f32)
    } else {
        dirty_powf((gamma + 0.0550107189475866) / 1.0550107189475866, 2.4)
    }
}

#[inline]
/// Gamma transfer function for sRGB
fn srgb_from_linear(linear: f64) -> f64 {
    if linear < 0.0f64 {
        0.0f64
    } else if linear < 0.0030412825601275209f64 {
        linear * 12.92f64
    } else if linear < 1.0f64 {
        fmla(
            1.0550107189475866f64,
            f_pow(linear, 1.0f64 / 2.4f64),
            -0.0550107189475866f64,
        )
    } else {
        1.0f64
    }
}

#[inline]
/// Gamma transfer function for sRGB
pub(crate) fn srgb_from_linear_extended(linear: f32) -> f32 {
    if linear < 0.0030412825601275209f32 {
        linear * 12.92f32
    } else {
        fmla(
            1.0550107189475866f32,
            dirty_powf(linear, 1.0f32 / 2.4f32),
            -0.0550107189475866f32,
        )
    }
}

#[inline]
/// Linear transfer function for Rec.709
fn rec709_to_linear(gamma: f64) -> f64 {
    if gamma < 0.0f64 {
        0.0f64
    } else if gamma < 4.5f64 * 0.018053968510807f64 {
        gamma * (1f64 / 4.5f64)
    } else if gamma < 1.0f64 {
        f_pow(
            (gamma + 0.09929682680944f64) / 1.09929682680944f64,
            1.0f64 / 0.45f64,
        )
    } else {
        1.0f64
    }
}

#[inline]
/// Linear transfer function for Rec.709
fn rec709_to_linearf_extended(gamma: f32) -> f32 {
    if gamma < 4.5 * 0.018053968510807 {
        gamma * (1. / 4.5)
    } else {
        f_powf((gamma + 0.09929682680944) / 1.09929682680944, 1.0 / 0.45)
    }
}

#[inline]
/// Gamma transfer function for Rec.709
fn rec709_from_linear(linear: f64) -> f64 {
    if linear < 0.0f64 {
        0.0f64
    } else if linear < 0.018053968510807f64 {
        linear * 4.5f64
    } else if linear < 1.0f64 {
        fmla(
            1.09929682680944f64,
            f_pow(linear, 0.45f64),
            -0.09929682680944f64,
        )
    } else {
        1.0f64
    }
}

#[inline]
/// Gamma transfer function for Rec.709
fn rec709_from_linearf_extended(linear: f32) -> f32 {
    if linear < 0.018053968510807 {
        linear * 4.5
    } else {
        fmla(
            1.09929682680944,
            dirty_powf(linear, 0.45),
            -0.09929682680944,
        )
    }
}

#[inline]
/// Linear transfer function for Smpte 428
pub(crate) fn smpte428_to_linear(gamma: f64) -> f64 {
    const SCALE: f64 = 1. / 0.91655527974030934f64;
    f_pow(gamma.max(0.).min(1f64), 2.6f64) * SCALE
}

#[inline]
/// Linear transfer function for Smpte 428
pub(crate) fn smpte428_to_linearf_extended(gamma: f32) -> f32 {
    const SCALE: f32 = 1. / 0.91655527974030934;
    dirty_powf(gamma.max(0.), 2.6) * SCALE
}

#[inline]
/// Gamma transfer function for Smpte 428
fn smpte428_from_linear(linear: f64) -> f64 {
    const POWER_VALUE: f64 = 1.0f64 / 2.6f64;
    f_pow(0.91655527974030934f64 * linear.max(0.), POWER_VALUE)
}

#[inline]
/// Gamma transfer function for Smpte 428
fn smpte428_from_linearf(linear: f32) -> f32 {
    const POWER_VALUE: f32 = 1.0 / 2.6;
    dirty_powf(0.91655527974030934 * linear.max(0.), POWER_VALUE)
}

#[inline]
/// Linear transfer function for Smpte 240
pub(crate) fn smpte240_to_linear(gamma: f64) -> f64 {
    if gamma < 0.0 {
        0.0
    } else if gamma < 4.0 * 0.022821585529445 {
        gamma / 4.0
    } else if gamma < 1.0 {
        f_pow((gamma + 0.111572195921731) / 1.111572195921731, 1.0 / 0.45)
    } else {
        1.0
    }
}

#[inline]
/// Linear transfer function for Smpte 240
pub(crate) fn smpte240_to_linearf_extended(gamma: f32) -> f32 {
    if gamma < 4.0 * 0.022821585529445 {
        gamma / 4.0
    } else {
        dirty_powf((gamma + 0.111572195921731) / 1.111572195921731, 1.0 / 0.45)
    }
}

#[inline]
/// Gamma transfer function for Smpte 240
fn smpte240_from_linear(linear: f64) -> f64 {
    if linear < 0.0 {
        0.0
    } else if linear < 0.022821585529445 {
        linear * 4.0
    } else if linear < 1.0 {
        fmla(1.111572195921731, f_pow(linear, 0.45), -0.111572195921731)
    } else {
        1.0
    }
}

#[inline]
/// Gamma transfer function for Smpte 240
fn smpte240_from_linearf_extended(linear: f32) -> f32 {
    if linear < 0.022821585529445 {
        linear * 4.0
    } else {
        fmla(1.111572195921731, f_powf(linear, 0.45), -0.111572195921731)
    }
}

#[inline]
/// Gamma transfer function for Log100
fn log100_from_linear(linear: f64) -> f64 {
    if linear <= 0.01f64 {
        0.
    } else {
        1. + f_log10(linear.min(1.)) / 2.0
    }
}

#[inline]
/// Gamma transfer function for Log100
fn log100_from_linearf(linear: f32) -> f32 {
    if linear <= 0.01 {
        0.
    } else {
        1. + f_log10f(linear.min(1.)) / 2.0
    }
}

#[inline]
/// Linear transfer function for Log100
pub(crate) fn log100_to_linear(gamma: f64) -> f64 {
    // The function is non-bijective so choose the middle of [0, 0.00316227766f].
    const MID_INTERVAL: f64 = 0.01 / 2.;
    if gamma <= 0. {
        MID_INTERVAL
    } else {
        f_exp10(2. * (gamma.min(1.) - 1.))
    }
}

#[inline]
/// Linear transfer function for Log100
pub(crate) fn log100_to_linearf(gamma: f32) -> f32 {
    // The function is non-bijective so choose the middle of [0, 0.00316227766f].
    const MID_INTERVAL: f32 = 0.01 / 2.;
    if gamma <= 0. {
        MID_INTERVAL
    } else {
        f_exp10f(2. * (gamma.min(1.) - 1.))
    }
}

#[inline]
/// Linear transfer function for Log100Sqrt10
pub(crate) fn log100_sqrt10_to_linear(gamma: f64) -> f64 {
    // The function is non-bijective so choose the middle of [0, 0.00316227766f].
    const MID_INTERVAL: f64 = 0.00316227766 / 2.;
    if gamma <= 0. {
        MID_INTERVAL
    } else {
        f_exp10(2.5 * (gamma.min(1.) - 1.))
    }
}

#[inline]
/// Linear transfer function for Log100Sqrt10
pub(crate) fn log100_sqrt10_to_linearf(gamma: f32) -> f32 {
    // The function is non-bijective so choose the middle of [0, 0.00316227766f].
    const MID_INTERVAL: f32 = 0.00316227766 / 2.;
    if gamma <= 0. {
        MID_INTERVAL
    } else {
        f_exp10f(2.5 * (gamma.min(1.) - 1.))
    }
}

#[inline]
/// Gamma transfer function for Log100Sqrt10
fn log100_sqrt10_from_linear(linear: f64) -> f64 {
    if linear <= 0.00316227766 {
        0.0
    } else {
        1.0 + f_log10(linear.min(1.)) / 2.5
    }
}

#[inline]
/// Gamma transfer function for Log100Sqrt10
fn log100_sqrt10_from_linearf(linear: f32) -> f32 {
    if linear <= 0.00316227766 {
        0.0
    } else {
        1.0 + f_log10f(linear.min(1.)) / 2.5
    }
}

#[inline]
/// Gamma transfer function for Bt.1361
fn bt1361_from_linear(linear: f64) -> f64 {
    if linear < -0.25 {
        -0.25
    } else if linear < 0.0 {
        fmla(
            -0.27482420670236,
            f_pow(-4.0 * linear, 0.45),
            0.02482420670236,
        )
    } else if linear < 0.018053968510807 {
        linear * 4.5
    } else if linear < 1.0 {
        fmla(1.09929682680944, f_pow(linear, 0.45), -0.09929682680944)
    } else {
        1.0
    }
}

#[inline]
/// Gamma transfer function for Bt.1361
fn bt1361_from_linearf(linear: f32) -> f32 {
    if linear < -0.25 {
        -0.25
    } else if linear < 0.0 {
        fmla(
            -0.27482420670236,
            dirty_powf(-4.0 * linear, 0.45),
            0.02482420670236,
        )
    } else if linear < 0.018053968510807 {
        linear * 4.5
    } else if linear < 1.0 {
        fmla(
            1.09929682680944,
            dirty_powf(linear, 0.45),
            -0.09929682680944,
        )
    } else {
        1.0
    }
}

#[inline]
/// Linear transfer function for Bt.1361
pub(crate) fn bt1361_to_linear(gamma: f64) -> f64 {
    if gamma < -0.25f64 {
        -0.25f64
    } else if gamma < 0.0f64 {
        f_pow(
            (gamma - 0.02482420670236f64) / -0.27482420670236f64,
            1.0f64 / 0.45f64,
        ) / -4.0f64
    } else if gamma < 4.5 * 0.018053968510807 {
        gamma / 4.5
    } else if gamma < 1.0 {
        f_pow((gamma + 0.09929682680944) / 1.09929682680944, 1.0 / 0.45)
    } else {
        1.0f64
    }
}

#[inline]
/// Linear transfer function for Bt.1361
fn bt1361_to_linearf(gamma: f32) -> f32 {
    if gamma < -0.25 {
        -0.25
    } else if gamma < 0.0 {
        dirty_powf((gamma - 0.02482420670236) / -0.27482420670236, 1.0 / 0.45) / -4.0
    } else if gamma < 4.5 * 0.018053968510807 {
        gamma / 4.5
    } else if gamma < 1.0 {
        dirty_powf((gamma + 0.09929682680944) / 1.09929682680944, 1.0 / 0.45)
    } else {
        1.0
    }
}

#[inline(always)]
/// Pure gamma transfer function for gamma 2.2
fn pure_gamma_function(x: f64, gamma: f64) -> f64 {
    if x <= 0f64 {
        0f64
    } else if x >= 1f64 {
        1f64
    } else {
        f_pow(x, gamma)
    }
}

#[inline(always)]
/// Pure gamma transfer function for gamma 2.2
fn pure_gamma_function_f(x: f32, gamma: f32) -> f32 {
    if x <= 0. { 0. } else { dirty_powf(x, gamma) }
}

#[inline]
pub(crate) fn iec61966_to_linear(gamma: f64) -> f64 {
    if gamma < -4.5f64 * 0.018053968510807f64 {
        f_pow(
            (-gamma + 0.09929682680944f64) / -1.09929682680944f64,
            1.0 / 0.45,
        )
    } else if gamma < 4.5f64 * 0.018053968510807f64 {
        gamma / 4.5
    } else {
        f_pow(
            (gamma + 0.09929682680944f64) / 1.09929682680944f64,
            1.0 / 0.45,
        )
    }
}

#[inline]
fn iec61966_to_linearf(gamma: f32) -> f32 {
    if gamma < -4.5 * 0.018053968510807 {
        dirty_powf((-gamma + 0.09929682680944) / -1.09929682680944, 1.0 / 0.45)
    } else if gamma < 4.5 * 0.018053968510807 {
        gamma / 4.5
    } else {
        dirty_powf((gamma + 0.09929682680944) / 1.09929682680944, 1.0 / 0.45)
    }
}

#[inline]
fn iec61966_from_linear(v: f64) -> f64 {
    if v < -0.018053968510807f64 {
        fmla(-1.09929682680944f64, f_pow(-v, 0.45), 0.09929682680944f64)
    } else if v < 0.018053968510807f64 {
        v * 4.5f64
    } else {
        fmla(1.09929682680944f64, f_pow(v, 0.45), -0.09929682680944f64)
    }
}

#[inline]
fn iec61966_from_linearf(v: f32) -> f32 {
    if v < -0.018053968510807 {
        fmla(-1.09929682680944, dirty_powf(-v, 0.45), 0.09929682680944)
    } else if v < 0.018053968510807 {
        v * 4.5
    } else {
        fmla(1.09929682680944, dirty_powf(v, 0.45), -0.09929682680944)
    }
}

#[inline]
/// Pure gamma transfer function for gamma 2.2
fn gamma2p2_from_linear(linear: f64) -> f64 {
    pure_gamma_function(linear, 1f64 / 2.2f64)
}

#[inline]
/// Pure gamma transfer function for gamma 2.2
fn gamma2p2_from_linear_f(linear: f32) -> f32 {
    pure_gamma_function_f(linear, 1. / 2.2)
}

#[inline]
/// Linear transfer function for gamma 2.2
fn gamma2p2_to_linear(gamma: f64) -> f64 {
    pure_gamma_function(gamma, 2.2f64)
}

#[inline]
/// Linear transfer function for gamma 2.2
fn gamma2p2_to_linear_f(gamma: f32) -> f32 {
    pure_gamma_function_f(gamma, 2.2)
}

#[inline]
/// Pure gamma transfer function for gamma 2.8
fn gamma2p8_from_linear(linear: f64) -> f64 {
    pure_gamma_function(linear, 1f64 / 2.8f64)
}

#[inline]
/// Pure gamma transfer function for gamma 2.8
fn gamma2p8_from_linear_f(linear: f32) -> f32 {
    pure_gamma_function_f(linear, 1. / 2.8)
}

#[inline]
/// Linear transfer function for gamma 2.8
fn gamma2p8_to_linear(gamma: f64) -> f64 {
    pure_gamma_function(gamma, 2.8f64)
}

#[inline]
/// Linear transfer function for gamma 2.8
fn gamma2p8_to_linear_f(gamma: f32) -> f32 {
    pure_gamma_function_f(gamma, 2.8)
}

#[inline]
/// Linear transfer function for PQ
pub(crate) fn pq_to_linear(gamma: f64) -> f64 {
    if gamma > 0.0 {
        let pow_gamma = f_pow(gamma, 1.0 / 78.84375);
        let num = (pow_gamma - 0.8359375).max(0.);
        let den = mlaf(18.8515625, -18.6875, pow_gamma).max(f64::MIN);
        f_pow(num / den, 1.0 / 0.1593017578125)
    } else {
        0.0
    }
}

#[inline]
/// Linear transfer function for PQ
pub(crate) fn pq_to_linearf(gamma: f32) -> f32 {
    if gamma > 0.0 {
        let pow_gamma = f_powf(gamma, 1.0 / 78.84375);
        let num = (pow_gamma - 0.8359375).max(0.);
        let den = mlaf(18.8515625, -18.6875, pow_gamma).max(f32::MIN);
        f_powf(num / den, 1.0 / 0.1593017578125)
    } else {
        0.0
    }
}

#[inline]
/// Gamma transfer function for PQ
fn pq_from_linear(linear: f64) -> f64 {
    if linear > 0.0 {
        let linear = linear.clamp(0., 1.);
        let pow_linear = f_pow(linear, 0.1593017578125);
        let num = fmla(0.1640625, pow_linear, -0.1640625);
        let den = mlaf(1.0, 18.6875, pow_linear);
        f_pow(1.0 + num / den, 78.84375)
    } else {
        0.0
    }
}

#[inline]
/// Gamma transfer function for PQ
pub(crate) fn pq_from_linearf(linear: f32) -> f32 {
    if linear > 0.0 {
        let linear = linear.max(0.);
        let pow_linear = f_powf(linear, 0.1593017578125);
        let num = fmla(0.1640625, pow_linear, -0.1640625);
        let den = mlaf(1.0, 18.6875, pow_linear);
        f_powf(1.0 + num / den, 78.84375)
    } else {
        0.0
    }
}

#[inline]
/// Linear transfer function for HLG
pub(crate) fn hlg_to_linear(gamma: f64) -> f64 {
    if gamma < 0.0 {
        return 0.0;
    }
    if gamma <= 0.5 {
        f_pow((gamma * gamma) * (1.0 / 3.0), 1.2)
    } else {
        f_pow(
            (f_exp((gamma - 0.55991073) / 0.17883277) + 0.28466892) / 12.0,
            1.2,
        )
    }
}

#[inline]
/// Linear transfer function for HLG
pub(crate) fn hlg_to_linearf(gamma: f32) -> f32 {
    if gamma < 0.0 {
        return 0.0;
    }
    if gamma <= 0.5 {
        f_powf((gamma * gamma) * (1.0 / 3.0), 1.2)
    } else {
        f_powf(
            (f_expf((gamma - 0.55991073) / 0.17883277) + 0.28466892) / 12.0,
            1.2,
        )
    }
}

#[inline]
/// Gamma transfer function for HLG
fn hlg_from_linear(linear: f64) -> f64 {
    // Scale from extended SDR range to [0.0, 1.0].
    let mut linear = linear.clamp(0., 1.);
    // Inverse OOTF followed by OETF see Table 5 and Note 5i in ITU-R BT.2100-2 page 7-8.
    linear = f_pow(linear, 1.0 / 1.2);
    if linear < 0.0 {
        0.0
    } else if linear <= (1.0 / 12.0) {
        (3.0 * linear).sqrt()
    } else {
        fmla(
            0.17883277,
            f_log(fmla(12.0, linear, -0.28466892)),
            0.55991073,
        )
    }
}

#[inline]
/// Gamma transfer function for HLG
fn hlg_from_linearf(linear: f32) -> f32 {
    // Scale from extended SDR range to [0.0, 1.0].
    let mut linear = linear.max(0.);
    // Inverse OOTF followed by OETF see Table 5 and Note 5i in ITU-R BT.2100-2 page 7-8.
    linear = f_powf(linear, 1.0 / 1.2);
    if linear < 0.0 {
        0.0
    } else if linear <= (1.0 / 12.0) {
        (3.0 * linear).sqrt()
    } else {
        0.17883277 * f_logf(12.0 * linear - 0.28466892) + 0.55991073
    }
}

#[inline]
fn trc_linear(v: f64) -> f64 {
    v.min(1.).max(0.)
}

impl TransferCharacteristics {
    #[inline]
    pub fn linearize(self, v: f64) -> f64 {
        match self {
            TransferCharacteristics::Reserved => 0f64,
            TransferCharacteristics::Bt709
            | TransferCharacteristics::Bt601
            | TransferCharacteristics::Bt202010bit
            | TransferCharacteristics::Bt202012bit => rec709_to_linear(v),
            TransferCharacteristics::Unspecified => 0f64,
            TransferCharacteristics::Bt470M => gamma2p2_to_linear(v),
            TransferCharacteristics::Bt470Bg => gamma2p8_to_linear(v),
            TransferCharacteristics::Smpte240 => smpte240_to_linear(v),
            TransferCharacteristics::Linear => trc_linear(v),
            TransferCharacteristics::Log100 => log100_to_linear(v),
            TransferCharacteristics::Log100sqrt10 => log100_sqrt10_to_linear(v),
            TransferCharacteristics::Iec61966 => iec61966_to_linear(v),
            TransferCharacteristics::Bt1361 => bt1361_to_linear(v),
            TransferCharacteristics::Srgb => srgb_to_linear(v),
            TransferCharacteristics::Smpte2084 => pq_to_linear(v),
            TransferCharacteristics::Smpte428 => smpte428_to_linear(v),
            TransferCharacteristics::Hlg => hlg_to_linear(v),
        }
    }

    #[inline]
    pub fn gamma(self, v: f64) -> f64 {
        match self {
            TransferCharacteristics::Reserved => 0f64,
            TransferCharacteristics::Bt709
            | TransferCharacteristics::Bt601
            | TransferCharacteristics::Bt202010bit
            | TransferCharacteristics::Bt202012bit => rec709_from_linear(v),
            TransferCharacteristics::Unspecified => 0f64,
            TransferCharacteristics::Bt470M => gamma2p2_from_linear(v),
            TransferCharacteristics::Bt470Bg => gamma2p8_from_linear(v),
            TransferCharacteristics::Smpte240 => smpte240_from_linear(v),
            TransferCharacteristics::Linear => trc_linear(v),
            TransferCharacteristics::Log100 => log100_from_linear(v),
            TransferCharacteristics::Log100sqrt10 => log100_sqrt10_from_linear(v),
            TransferCharacteristics::Iec61966 => iec61966_from_linear(v),
            TransferCharacteristics::Bt1361 => bt1361_from_linear(v),
            TransferCharacteristics::Srgb => srgb_from_linear(v),
            TransferCharacteristics::Smpte2084 => pq_from_linear(v),
            TransferCharacteristics::Smpte428 => smpte428_from_linear(v),
            TransferCharacteristics::Hlg => hlg_from_linear(v),
        }
    }

    pub(crate) fn extended_gamma_tristimulus(self) -> fn(Rgb<f32>) -> Rgb<f32> {
        match self {
            TransferCharacteristics::Reserved => |x| Rgb::new(x.r, x.g, x.b),
            TransferCharacteristics::Bt709
            | TransferCharacteristics::Bt601
            | TransferCharacteristics::Bt202010bit
            | TransferCharacteristics::Bt202012bit => |x| {
                Rgb::new(
                    rec709_from_linearf_extended(x.r),
                    rec709_from_linearf_extended(x.g),
                    rec709_from_linearf_extended(x.b),
                )
            },
            TransferCharacteristics::Unspecified => |x| Rgb::new(x.r, x.g, x.b),
            TransferCharacteristics::Bt470M => |x| {
                Rgb::new(
                    gamma2p2_from_linear_f(x.r),
                    gamma2p2_from_linear_f(x.g),
                    gamma2p2_from_linear_f(x.b),
                )
            },
            TransferCharacteristics::Bt470Bg => |x| {
                Rgb::new(
                    gamma2p8_from_linear_f(x.r),
                    gamma2p8_from_linear_f(x.g),
                    gamma2p8_from_linear_f(x.b),
                )
            },
            TransferCharacteristics::Smpte240 => |x| {
                Rgb::new(
                    smpte240_from_linearf_extended(x.r),
                    smpte240_from_linearf_extended(x.g),
                    smpte240_from_linearf_extended(x.b),
                )
            },
            TransferCharacteristics::Linear => |x| Rgb::new(x.r, x.g, x.b),
            TransferCharacteristics::Log100 => |x| {
                Rgb::new(
                    log100_from_linearf(x.r),
                    log100_from_linearf(x.g),
                    log100_from_linearf(x.b),
                )
            },
            TransferCharacteristics::Log100sqrt10 => |x| {
                Rgb::new(
                    log100_sqrt10_from_linearf(x.r),
                    log100_sqrt10_from_linearf(x.g),
                    log100_sqrt10_from_linearf(x.b),
                )
            },
            TransferCharacteristics::Iec61966 => |x| {
                Rgb::new(
                    iec61966_from_linearf(x.r),
                    iec61966_from_linearf(x.g),
                    iec61966_from_linearf(x.b),
                )
            },
            TransferCharacteristics::Bt1361 => |x| {
                Rgb::new(
                    bt1361_from_linearf(x.r),
                    bt1361_from_linearf(x.g),
                    bt1361_from_linearf(x.b),
                )
            },
            TransferCharacteristics::Srgb => |x| {
                Rgb::new(
                    srgb_from_linear_extended(x.r),
                    srgb_from_linear_extended(x.g),
                    srgb_from_linear_extended(x.b),
                )
            },
            TransferCharacteristics::Smpte2084 => |x| {
                Rgb::new(
                    pq_from_linearf(x.r),
                    pq_from_linearf(x.g),
                    pq_from_linearf(x.b),
                )
            },
            TransferCharacteristics::Smpte428 => |x| {
                Rgb::new(
                    smpte428_from_linearf(x.r),
                    smpte428_from_linearf(x.g),
                    smpte428_from_linearf(x.b),
                )
            },
            TransferCharacteristics::Hlg => |x| {
                Rgb::new(
                    hlg_from_linearf(x.r),
                    hlg_from_linearf(x.g),
                    hlg_from_linearf(x.b),
                )
            },
        }
    }

    pub(crate) fn extended_gamma_single(self) -> fn(f32) -> f32 {
        match self {
            TransferCharacteristics::Reserved => |x| x,
            TransferCharacteristics::Bt709
            | TransferCharacteristics::Bt601
            | TransferCharacteristics::Bt202010bit
            | TransferCharacteristics::Bt202012bit => |x| rec709_from_linearf_extended(x),
            TransferCharacteristics::Unspecified => |x| x,
            TransferCharacteristics::Bt470M => |x| gamma2p2_from_linear_f(x),
            TransferCharacteristics::Bt470Bg => |x| gamma2p8_from_linear_f(x),
            TransferCharacteristics::Smpte240 => |x| smpte240_from_linearf_extended(x),
            TransferCharacteristics::Linear => |x| x,
            TransferCharacteristics::Log100 => |x| log100_from_linearf(x),
            TransferCharacteristics::Log100sqrt10 => |x| log100_sqrt10_from_linearf(x),
            TransferCharacteristics::Iec61966 => |x| iec61966_from_linearf(x),
            TransferCharacteristics::Bt1361 => |x| bt1361_from_linearf(x),
            TransferCharacteristics::Srgb => |x| srgb_from_linear_extended(x),
            TransferCharacteristics::Smpte2084 => |x| pq_from_linearf(x),
            TransferCharacteristics::Smpte428 => |x| smpte428_from_linearf(x),
            TransferCharacteristics::Hlg => |x| hlg_from_linearf(x),
        }
    }

    pub(crate) fn extended_linear_tristimulus(self) -> fn(Rgb<f32>) -> Rgb<f32> {
        match self {
            TransferCharacteristics::Reserved => |x| Rgb::new(x.r, x.g, x.b),
            TransferCharacteristics::Bt709
            | TransferCharacteristics::Bt601
            | TransferCharacteristics::Bt202010bit
            | TransferCharacteristics::Bt202012bit => |x| {
                Rgb::new(
                    rec709_to_linearf_extended(x.r),
                    rec709_to_linearf_extended(x.g),
                    rec709_to_linearf_extended(x.b),
                )
            },
            TransferCharacteristics::Unspecified => |x| Rgb::new(x.r, x.g, x.b),
            TransferCharacteristics::Bt470M => |x| {
                Rgb::new(
                    gamma2p2_to_linear_f(x.r),
                    gamma2p2_to_linear_f(x.g),
                    gamma2p2_to_linear_f(x.b),
                )
            },
            TransferCharacteristics::Bt470Bg => |x| {
                Rgb::new(
                    gamma2p8_to_linear_f(x.r),
                    gamma2p8_to_linear_f(x.g),
                    gamma2p8_to_linear_f(x.b),
                )
            },
            TransferCharacteristics::Smpte240 => |x| {
                Rgb::new(
                    smpte240_to_linearf_extended(x.r),
                    smpte240_to_linearf_extended(x.g),
                    smpte240_to_linearf_extended(x.b),
                )
            },
            TransferCharacteristics::Linear => |x| Rgb::new(x.r, x.g, x.b),
            TransferCharacteristics::Log100 => |x| {
                Rgb::new(
                    log100_to_linearf(x.r),
                    log100_to_linearf(x.g),
                    log100_to_linearf(x.b),
                )
            },
            TransferCharacteristics::Log100sqrt10 => |x| {
                Rgb::new(
                    log100_sqrt10_to_linearf(x.r),
                    log100_sqrt10_to_linearf(x.g),
                    log100_sqrt10_to_linearf(x.b),
                )
            },
            TransferCharacteristics::Iec61966 => |x| {
                Rgb::new(
                    iec61966_to_linearf(x.r),
                    iec61966_to_linearf(x.g),
                    iec61966_to_linearf(x.b),
                )
            },
            TransferCharacteristics::Bt1361 => |x| {
                Rgb::new(
                    bt1361_to_linearf(x.r),
                    bt1361_to_linearf(x.g),
                    bt1361_to_linearf(x.b),
                )
            },
            TransferCharacteristics::Srgb => |x| {
                Rgb::new(
                    srgb_to_linearf_extended(x.r),
                    srgb_to_linearf_extended(x.g),
                    srgb_to_linearf_extended(x.b),
                )
            },
            TransferCharacteristics::Smpte2084 => {
                |x| Rgb::new(pq_to_linearf(x.r), pq_to_linearf(x.g), pq_to_linearf(x.b))
            }
            TransferCharacteristics::Smpte428 => |x| {
                Rgb::new(
                    smpte428_to_linearf_extended(x.r),
                    smpte428_to_linearf_extended(x.g),
                    smpte428_to_linearf_extended(x.b),
                )
            },
            TransferCharacteristics::Hlg => |x| {
                Rgb::new(
                    hlg_to_linearf(x.r),
                    hlg_to_linearf(x.g),
                    hlg_to_linearf(x.b),
                )
            },
        }
    }

    pub(crate) fn extended_linear_single(self) -> fn(f32) -> f32 {
        match self {
            TransferCharacteristics::Reserved => |x| x,
            TransferCharacteristics::Bt709
            | TransferCharacteristics::Bt601
            | TransferCharacteristics::Bt202010bit
            | TransferCharacteristics::Bt202012bit => |x| rec709_to_linearf_extended(x),
            TransferCharacteristics::Unspecified => |x| x,
            TransferCharacteristics::Bt470M => |x| gamma2p2_to_linear_f(x),
            TransferCharacteristics::Bt470Bg => |x| gamma2p8_to_linear_f(x),
            TransferCharacteristics::Smpte240 => |x| smpte240_to_linearf_extended(x),
            TransferCharacteristics::Linear => |x| x,
            TransferCharacteristics::Log100 => |x| log100_to_linearf(x),
            TransferCharacteristics::Log100sqrt10 => |x| log100_sqrt10_to_linearf(x),
            TransferCharacteristics::Iec61966 => |x| iec61966_to_linearf(x),
            TransferCharacteristics::Bt1361 => |x| bt1361_to_linearf(x),
            TransferCharacteristics::Srgb => |x| srgb_to_linearf_extended(x),
            TransferCharacteristics::Smpte2084 => |x| pq_to_linearf(x),
            TransferCharacteristics::Smpte428 => |x| smpte428_to_linearf_extended(x),
            TransferCharacteristics::Hlg => |x| hlg_to_linearf(x),
        }
    }

    pub(crate) fn make_linear_table<
        T: PointeeSizeExpressible,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
    ) -> Box<[f32; N]> {
        let mut gamma_table = Box::new([0f32; N]);
        let max_value = if T::FINITE {
            (1 << BIT_DEPTH) - 1
        } else {
            T::NOT_FINITE_LINEAR_TABLE_SIZE - 1
        };
        let cap_values = if T::FINITE {
            (1u32 << BIT_DEPTH) as usize
        } else {
            T::NOT_FINITE_LINEAR_TABLE_SIZE
        };
        assert!(cap_values <= N, "Invalid lut table construction");
        let scale_value = 1f64 / max_value as f64;
        for (i, g) in gamma_table.iter_mut().enumerate().take(cap_values) {
            *g = self.linearize(i as f64 * scale_value) as f32;
        }
        gamma_table
    }

    pub(crate) fn make_gamma_table<
        T: Default + Copy + 'static + PointeeSizeExpressible,
        const BUCKET: usize,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
    ) -> Box<[T; BUCKET]>
    where
        f32: AsPrimitive<T>,
    {
        let mut table = Box::new([T::default(); BUCKET]);
        let max_range = 1f64 / (N - 1) as f64;
        let max_value = ((1 << BIT_DEPTH) - 1) as f64;
        if T::FINITE {
            for (v, output) in table.iter_mut().take(N).enumerate() {
                *output = ((self.gamma(v as f64 * max_range) * max_value) as f32)
                    .round()
                    .as_();
            }
        } else {
            for (v, output) in table.iter_mut().take(N).enumerate() {
                *output = (self.gamma(v as f64 * max_range) as f32).as_();
            }
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_test() {
        let srgb_0 = srgb_to_linear(0.5);
        let srgb_1 = srgb_from_linear(srgb_0);
        assert!((0.5 - srgb_1).abs() < 1e-9f64);
    }

    #[test]
    fn log100_sqrt10_test() {
        let srgb_0 = log100_sqrt10_to_linear(0.5);
        let srgb_1 = log100_sqrt10_from_linear(srgb_0);
        assert_eq!(0.5, srgb_1);
    }

    #[test]
    fn log100_test() {
        let srgb_0 = log100_to_linear(0.5);
        let srgb_1 = log100_from_linear(srgb_0);
        assert_eq!(0.5, srgb_1);
    }

    #[test]
    fn iec61966_test() {
        let srgb_0 = iec61966_to_linear(0.5);
        let srgb_1 = iec61966_from_linear(srgb_0);
        assert!((0.5 - srgb_1).abs() < 1e-9f64);
    }

    #[test]
    fn smpte240_test() {
        let srgb_0 = smpte240_to_linear(0.5);
        let srgb_1 = smpte240_from_linear(srgb_0);
        assert!((0.5 - srgb_1).abs() < 1e-9f64);
    }

    #[test]
    fn smpte428_test() {
        let srgb_0 = smpte428_to_linear(0.5);
        let srgb_1 = smpte428_from_linear(srgb_0);
        assert!((0.5 - srgb_1).abs() < 1e-9f64);
    }

    #[test]
    fn rec709_test() {
        let srgb_0 = rec709_to_linear(0.5);
        let srgb_1 = rec709_from_linear(srgb_0);
        assert!((0.5 - srgb_1).abs() < 1e-9f64);
    }

    #[test]
    fn rec709f_test() {
        let srgb_0 = rec709_to_linearf_extended(0.5);
        let srgb_1 = rec709_from_linearf_extended(srgb_0);
        assert!((0.5 - srgb_1).abs() < 1e-5f32);
    }

    #[test]
    fn srgbf_test() {
        let srgb_0 = srgb_to_linearf_extended(0.5);
        let srgb_1 = srgb_from_linear_extended(srgb_0);
        assert!((0.5 - srgb_1).abs() < 1e-5f32);
    }

    #[test]
    fn hlg_test() {
        let z0 = hlg_to_linear(0.5);
        let z1 = hlg_from_linear(z0);
        assert!((0.5 - z1).abs() < 1e-5f64);
    }

    #[test]
    fn pq_test() {
        let z0 = pq_to_linear(0.5);
        let z1 = pq_from_linear(z0);
        assert!((0.5 - z1).abs() < 1e-5f64);
    }

    #[test]
    fn pqf_test() {
        let z0 = pq_to_linearf(0.5);
        let z1 = pq_from_linearf(z0);
        assert!((0.5 - z1).abs() < 1e-5f32);
    }

    #[test]
    fn iec_test() {
        let z0 = iec61966_to_linear(0.5);
        let z1 = iec61966_from_linear(z0);
        assert!((0.5 - z1).abs() < 1e-5f64);
    }

    #[test]
    fn bt1361_test() {
        let z0 = bt1361_to_linear(0.5);
        let z1 = bt1361_from_linear(z0);
        assert!((0.5 - z1).abs() < 1e-5f64);
    }
}
