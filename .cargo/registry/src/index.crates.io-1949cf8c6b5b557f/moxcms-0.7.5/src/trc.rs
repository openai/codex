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
use crate::cicp::create_rec709_parametric;
use crate::matan::is_curve_linear16;
use crate::math::m_clamp;
use crate::mlaf::{mlaf, neg_mlaf};
use crate::transform::PointeeSizeExpressible;
use crate::writer::FloatToFixedU8Fixed8;
use crate::{CmsError, ColorProfile, DataColorSpace, Rgb, TransferCharacteristics};
use num_traits::AsPrimitive;
use pxfm::{dirty_powf, f_pow, f_powf};

#[derive(Clone, Debug)]
pub enum ToneReprCurve {
    Lut(Vec<u16>),
    Parametric(Vec<f32>),
}

impl ToneReprCurve {
    pub fn inverse(&self) -> Result<ToneReprCurve, CmsError> {
        match self {
            ToneReprCurve::Lut(lut) => {
                let inverse_length = lut.len().max(256);
                Ok(ToneReprCurve::Lut(invert_lut(lut, inverse_length)))
            }
            ToneReprCurve::Parametric(parametric) => ParametricCurve::new(parametric)
                .and_then(|x| x.invert())
                .map(|x| ToneReprCurve::Parametric([x.g, x.a, x.b, x.c, x.d, x.e, x.f].to_vec()))
                .ok_or(CmsError::BuildTransferFunction),
        }
    }

    /// Creates tone curve evaluator
    pub fn make_linear_evaluator(
        &self,
    ) -> Result<Box<dyn ToneCurveEvaluator + Send + Sync>, CmsError> {
        match self {
            ToneReprCurve::Lut(lut) => {
                if lut.is_empty() {
                    return Ok(Box::new(ToneCurveEvaluatorLinear {}));
                }
                if lut.len() == 1 {
                    let gamma = u8_fixed_8number_to_float(lut[0]);
                    return Ok(Box::new(ToneCurveEvaluatorPureGamma { gamma }));
                }
                let converted_curve = lut.iter().map(|&x| x as f32 / 65535.0).collect::<Vec<_>>();
                Ok(Box::new(ToneCurveLutEvaluator {
                    lut: converted_curve,
                }))
            }
            ToneReprCurve::Parametric(parametric) => {
                let parametric_curve =
                    ParametricCurve::new(parametric).ok_or(CmsError::BuildTransferFunction)?;
                Ok(Box::new(ToneCurveParametricEvaluator {
                    parametric: parametric_curve,
                }))
            }
        }
    }

    /// Creates tone curve evaluator from transfer characteristics
    pub fn make_cicp_linear_evaluator(
        transfer_characteristics: TransferCharacteristics,
    ) -> Result<Box<dyn ToneCurveEvaluator + Send + Sync>, CmsError> {
        if !transfer_characteristics.has_transfer_curve() {
            return Err(CmsError::BuildTransferFunction);
        }
        Ok(Box::new(ToneCurveCicpLinearEvaluator {
            trc: transfer_characteristics,
        }))
    }

    /// Creates tone curve inverse evaluator
    pub fn make_gamma_evaluator(
        &self,
    ) -> Result<Box<dyn ToneCurveEvaluator + Send + Sync>, CmsError> {
        match self {
            ToneReprCurve::Lut(lut) => {
                if lut.is_empty() {
                    return Ok(Box::new(ToneCurveEvaluatorLinear {}));
                }
                if lut.len() == 1 {
                    let gamma = 1. / u8_fixed_8number_to_float(lut[0]);
                    return Ok(Box::new(ToneCurveEvaluatorPureGamma { gamma }));
                }
                let inverted_lut = invert_lut(lut, 16384);
                let converted_curve = inverted_lut
                    .iter()
                    .map(|&x| x as f32 / 65535.0)
                    .collect::<Vec<_>>();
                Ok(Box::new(ToneCurveLutEvaluator {
                    lut: converted_curve,
                }))
            }
            ToneReprCurve::Parametric(parametric) => {
                let parametric_curve = ParametricCurve::new(parametric)
                    .and_then(|x| x.invert())
                    .ok_or(CmsError::BuildTransferFunction)?;
                Ok(Box::new(ToneCurveParametricEvaluator {
                    parametric: parametric_curve,
                }))
            }
        }
    }

    /// Creates tone curve inverse evaluator from transfer characteristics
    pub fn make_cicp_gamma_evaluator(
        transfer_characteristics: TransferCharacteristics,
    ) -> Result<Box<dyn ToneCurveEvaluator + Send + Sync>, CmsError> {
        if !transfer_characteristics.has_transfer_curve() {
            return Err(CmsError::BuildTransferFunction);
        }
        Ok(Box::new(ToneCurveCicpGammaEvaluator {
            trc: transfer_characteristics,
        }))
    }
}

struct ToneCurveCicpLinearEvaluator {
    trc: TransferCharacteristics,
}

struct ToneCurveCicpGammaEvaluator {
    trc: TransferCharacteristics,
}

impl ToneCurveEvaluator for ToneCurveCicpLinearEvaluator {
    fn evaluate_tristimulus(&self, rgb: Rgb<f32>) -> Rgb<f32> {
        Rgb::new(
            self.trc.linearize(rgb.r as f64) as f32,
            self.trc.linearize(rgb.g as f64) as f32,
            self.trc.linearize(rgb.b as f64) as f32,
        )
    }

    fn evaluate_value(&self, value: f32) -> f32 {
        self.trc.linearize(value as f64) as f32
    }
}

impl ToneCurveEvaluator for ToneCurveCicpGammaEvaluator {
    fn evaluate_tristimulus(&self, rgb: Rgb<f32>) -> Rgb<f32> {
        Rgb::new(
            self.trc.gamma(rgb.r as f64) as f32,
            self.trc.gamma(rgb.g as f64) as f32,
            self.trc.gamma(rgb.b as f64) as f32,
        )
    }

    fn evaluate_value(&self, value: f32) -> f32 {
        self.trc.gamma(value as f64) as f32
    }
}

struct ToneCurveLutEvaluator {
    lut: Vec<f32>,
}

impl ToneCurveEvaluator for ToneCurveLutEvaluator {
    fn evaluate_value(&self, value: f32) -> f32 {
        lut_interp_linear_float(value, &self.lut)
    }

    fn evaluate_tristimulus(&self, rgb: Rgb<f32>) -> Rgb<f32> {
        Rgb::new(
            lut_interp_linear_float(rgb.r, &self.lut),
            lut_interp_linear_float(rgb.g, &self.lut),
            lut_interp_linear_float(rgb.b, &self.lut),
        )
    }
}

pub(crate) fn build_trc_table(num_entries: i32, eotf: impl Fn(f64) -> f64) -> Vec<u16> {
    let mut table = vec![0u16; num_entries as usize];

    for (i, table_value) in table.iter_mut().enumerate() {
        let x: f64 = i as f64 / (num_entries - 1) as f64;
        let y: f64 = eotf(x);
        let mut output: f64;
        output = y * 65535.0 + 0.5;
        if output > 65535.0 {
            output = 65535.0
        }
        if output < 0.0 {
            output = 0.0
        }
        *table_value = output.floor() as u16;
    }
    table
}

/// Creates Tone Reproduction curve from gamma
pub fn curve_from_gamma(gamma: f32) -> ToneReprCurve {
    ToneReprCurve::Lut(vec![gamma.to_u8_fixed8()])
}

#[derive(Debug)]
struct ParametricCurve {
    g: f32,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

impl ParametricCurve {
    #[allow(clippy::many_single_char_names)]
    fn new(params: &[f32]) -> Option<ParametricCurve> {
        // convert from the variable number of parameters
        // contained in profiles to a unified representation.
        let g: f32 = params[0];
        match params[1..] {
            [] => Some(ParametricCurve {
                g,
                a: 1.,
                b: 0.,
                c: 1.,
                d: 0.,
                e: 0.,
                f: 0.,
            }),
            [a, b] => Some(ParametricCurve {
                g,
                a,
                b,
                c: 0.,
                d: -b / a,
                e: 0.,
                f: 0.,
            }),
            [a, b, c] => Some(ParametricCurve {
                g,
                a,
                b,
                c: 0.,
                d: -b / a,
                e: c,
                f: c,
            }),
            [a, b, c, d] => Some(ParametricCurve {
                g,
                a,
                b,
                c,
                d,
                e: 0.,
                f: 0.,
            }),
            [a, b, c, d, e, f] => Some(ParametricCurve {
                g,
                a,
                b,
                c,
                d,
                e,
                f,
            }),
            _ => None,
        }
    }

    fn is_linear(&self) -> bool {
        (self.g - 1.0).abs() < 1e-5
            && (self.a - 1.0).abs() < 1e-5
            && self.b.abs() < 1e-5
            && self.c.abs() < 1e-5
    }

    fn eval(&self, x: f32) -> f32 {
        if x < self.d {
            self.c * x + self.f
        } else {
            f_powf(self.a * x + self.b, self.g) + self.e
        }
    }

    #[allow(dead_code)]
    #[allow(clippy::many_single_char_names)]
    fn invert(&self) -> Option<ParametricCurve> {
        // First check if the function is continuous at the cross-over point d.
        let d1 = f_powf(self.a * self.d + self.b, self.g) + self.e;
        let d2 = self.c * self.d + self.f;

        if (d1 - d2).abs() > 0.1 {
            return None;
        }
        let d = d1;

        // y = (a * x + b)^g + e
        // y - e = (a * x + b)^g
        // (y - e)^(1/g) = a*x + b
        // (y - e)^(1/g) - b = a*x
        // (y - e)^(1/g)/a - b/a = x
        // ((y - e)/a^g)^(1/g) - b/a = x
        // ((1/(a^g)) * y - e/(a^g))^(1/g) - b/a = x
        let a = 1. / f_powf(self.a, self.g);
        let b = -self.e / f_powf(self.a, self.g);
        let g = 1. / self.g;
        let e = -self.b / self.a;

        // y = c * x + f
        // y - f = c * x
        // y/c - f/c = x
        let (c, f);
        if d <= 0. {
            c = 1.;
            f = 0.;
        } else {
            c = 1. / self.c;
            f = -self.f / self.c;
        }

        // if self.d > 0. and self.c == 0 as is likely with type 1 and 2 parametric function
        // then c and f will not be finite.
        if !(g.is_finite()
            && a.is_finite()
            && b.is_finite()
            && c.is_finite()
            && d.is_finite()
            && e.is_finite()
            && f.is_finite())
        {
            return None;
        }

        Some(ParametricCurve {
            g,
            a,
            b,
            c,
            d,
            e,
            f,
        })
    }
}

#[inline]
pub(crate) fn u8_fixed_8number_to_float(x: u16) -> f32 {
    // 0x0000 = 0.
    // 0x0100 = 1.
    // 0xffff = 255  + 255/256
    (x as i32 as f64 / 256.0) as f32
}

fn passthrough_table<T: PointeeSizeExpressible, const N: usize, const BIT_DEPTH: usize>()
-> Box<[f32; N]> {
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
        *g = (i as f64 * scale_value) as f32;
    }

    gamma_table
}

fn linear_forward_table<T: PointeeSizeExpressible, const N: usize, const BIT_DEPTH: usize>(
    gamma: u16,
) -> Box<[f32; N]> {
    let mut gamma_table = Box::new([0f32; N]);
    let gamma_float: f32 = u8_fixed_8number_to_float(gamma);
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
        *g = f_pow(i as f64 * scale_value, gamma_float as f64) as f32;
    }

    gamma_table
}

#[inline(always)]
pub(crate) fn lut_interp_linear_float(x: f32, table: &[f32]) -> f32 {
    let value = x.min(1.).max(0.) * (table.len() - 1) as f32;

    let upper: i32 = value.ceil() as i32;
    let lower: i32 = value.floor() as i32;

    let diff = upper as f32 - value;
    let tu = table[upper as usize];
    mlaf(neg_mlaf(tu, tu, diff), table[lower as usize], diff)
}

/// Lut interpolation float where values is already clamped
#[inline(always)]
#[allow(dead_code)]
pub(crate) fn lut_interp_linear_float_clamped(x: f32, table: &[f32]) -> f32 {
    let value = x * (table.len() - 1) as f32;

    let upper: i32 = value.ceil() as i32;
    let lower: i32 = value.floor() as i32;

    let diff = upper as f32 - value;
    let tu = table[upper as usize];
    mlaf(neg_mlaf(tu, tu, diff), table[lower as usize], diff)
}

#[inline]
pub(crate) fn lut_interp_linear(input_value: f64, table: &[u16]) -> f32 {
    let mut input_value = input_value;
    if table.is_empty() {
        return input_value as f32;
    }

    input_value *= (table.len() - 1) as f64;

    let upper: i32 = input_value.ceil() as i32;
    let lower: i32 = input_value.floor() as i32;
    let w0 = table[(upper as usize).min(table.len() - 1)] as f64;
    let w1 = 1. - (upper as f64 - input_value);
    let w2 = table[(lower as usize).min(table.len() - 1)] as f64;
    let w3 = upper as f64 - input_value;
    let value: f32 = mlaf(w2 * w3, w0, w1) as f32;
    value * (1.0 / 65535.0)
}

fn linear_lut_interpolate<T: PointeeSizeExpressible, const N: usize, const BIT_DEPTH: usize>(
    table: &[u16],
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
        *g = lut_interp_linear(i as f64 * scale_value, table);
    }
    gamma_table
}

fn linear_curve_parametric<T: PointeeSizeExpressible, const N: usize, const BIT_DEPTH: usize>(
    params: &[f32],
) -> Option<Box<[f32; N]>> {
    let params = ParametricCurve::new(params)?;
    let mut gamma_table = Box::new([0f32; N]);
    let max_value = if T::FINITE {
        (1 << BIT_DEPTH) - 1
    } else {
        T::NOT_FINITE_LINEAR_TABLE_SIZE - 1
    };
    let cap_value = if T::FINITE {
        1 << BIT_DEPTH
    } else {
        T::NOT_FINITE_LINEAR_TABLE_SIZE
    };
    let scale_value = 1f32 / max_value as f32;
    for (i, g) in gamma_table.iter_mut().enumerate().take(cap_value) {
        let x = i as f32 * scale_value;
        *g = m_clamp(params.eval(x), 0.0, 1.0);
    }
    Some(gamma_table)
}

fn linear_curve_parametric_s<const N: usize>(params: &[f32]) -> Option<Box<[f32; N]>> {
    let params = ParametricCurve::new(params)?;
    let mut gamma_table = Box::new([0f32; N]);
    let scale_value = 1f32 / (N - 1) as f32;
    for (i, g) in gamma_table.iter_mut().enumerate().take(N) {
        let x = i as f32 * scale_value;
        *g = m_clamp(params.eval(x), 0.0, 1.0);
    }
    Some(gamma_table)
}

pub(crate) fn make_gamma_linear_table<
    T: Default + Copy + 'static + PointeeSizeExpressible,
    const BUCKET: usize,
    const N: usize,
    const BIT_DEPTH: usize,
>() -> Box<[T; BUCKET]>
where
    f32: AsPrimitive<T>,
{
    let mut table = Box::new([T::default(); BUCKET]);
    let max_range = if T::FINITE {
        (1f64 / ((N - 1) as f64 / (1 << BIT_DEPTH) as f64)) as f32
    } else {
        (1f64 / ((N - 1) as f64)) as f32
    };
    for (v, output) in table.iter_mut().take(N).enumerate() {
        if T::FINITE {
            *output = (v as f32 * max_range).round().as_();
        } else {
            *output = (v as f32 * max_range).as_();
        }
    }
    table
}

#[inline]
fn lut_interp_linear_gamma_impl<
    T: Default + Copy + 'static + PointeeSizeExpressible,
    const N: usize,
    const BIT_DEPTH: usize,
>(
    input_value: u32,
    table: &[u16],
) -> T
where
    u32: AsPrimitive<T>,
{
    // Start scaling input_value to the length of the array: GAMMA_CAP*(length-1).
    // We'll divide out the GAMMA_CAP next
    let mut value: u32 = input_value * (table.len() - 1) as u32;
    let cap_value = N - 1;
    // equivalent to ceil(value/GAMMA_CAP)
    let upper: u32 = value.div_ceil(cap_value as u32);
    // equivalent to floor(value/GAMMA_CAP)
    let lower: u32 = value / cap_value as u32;
    // interp is the distance from upper to value scaled to 0..GAMMA_CAP
    let interp: u32 = value % cap_value as u32;
    let lw_value = table[lower as usize];
    let hw_value = table[upper as usize];
    // the table values range from 0..65535
    value = mlaf(
        hw_value as u32 * interp,
        lw_value as u32,
        (N - 1) as u32 - interp,
    ); // 0..(65535*GAMMA_CAP)

    // round and scale
    let max_colors = if T::FINITE { (1 << BIT_DEPTH) - 1 } else { 1 };
    value += (cap_value * 65535 / max_colors / 2) as u32; // scale to 0...max_colors
    value /= (cap_value * 65535 / max_colors) as u32;
    value.as_()
}

#[inline]
fn lut_interp_linear_gamma_impl_f32<
    T: Default + Copy + 'static + PointeeSizeExpressible,
    const N: usize,
    const BIT_DEPTH: usize,
>(
    input_value: u32,
    table: &[u16],
) -> T
where
    f32: AsPrimitive<T>,
{
    // Start scaling input_value to the length of the array: GAMMA_CAP*(length-1).
    // We'll divide out the GAMMA_CAP next
    let guess: u32 = input_value * (table.len() - 1) as u32;
    let cap_value = N - 1;
    // equivalent to ceil(value/GAMMA_CAP)
    let upper: u32 = guess.div_ceil(cap_value as u32);
    // equivalent to floor(value/GAMMA_CAP)
    let lower: u32 = guess / cap_value as u32;
    // interp is the distance from upper to value scaled to 0..GAMMA_CAP
    let interp: u32 = guess % cap_value as u32;
    let lw_value = table[lower as usize];
    let hw_value = table[upper as usize];
    // the table values range from 0..65535
    let mut value = mlaf(
        hw_value as f32 * interp as f32,
        lw_value as f32,
        (N - 1) as f32 - interp as f32,
    ); // 0..(65535*GAMMA_CAP)

    // round and scale
    let max_colors = if T::FINITE { (1 << BIT_DEPTH) - 1 } else { 1 };
    value /= (cap_value * 65535 / max_colors) as f32;
    value.as_()
}

#[doc(hidden)]
pub trait GammaLutInterpolate {
    fn gamma_lut_interp<
        T: Default + Copy + 'static + PointeeSizeExpressible,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        input_value: u32,
        table: &[u16],
    ) -> T
    where
        u32: AsPrimitive<T>,
        f32: AsPrimitive<T>;
}

macro_rules! gamma_lut_interp_fixed {
    ($i_type: ident) => {
        impl GammaLutInterpolate for $i_type {
            #[inline]
            fn gamma_lut_interp<
                T: Default + Copy + 'static + PointeeSizeExpressible,
                const N: usize,
                const BIT_DEPTH: usize,
            >(
                input_value: u32,
                table: &[u16],
            ) -> T
            where
                u32: AsPrimitive<T>,
            {
                lut_interp_linear_gamma_impl::<T, N, BIT_DEPTH>(input_value, table)
            }
        }
    };
}

gamma_lut_interp_fixed!(u8);
gamma_lut_interp_fixed!(u16);

macro_rules! gammu_lut_interp_float {
    ($f_type: ident) => {
        impl GammaLutInterpolate for $f_type {
            #[inline]
            fn gamma_lut_interp<
                T: Default + Copy + 'static + PointeeSizeExpressible,
                const N: usize,
                const BIT_DEPTH: usize,
            >(
                input_value: u32,
                table: &[u16],
            ) -> T
            where
                f32: AsPrimitive<T>,
                u32: AsPrimitive<T>,
            {
                lut_interp_linear_gamma_impl_f32::<T, N, BIT_DEPTH>(input_value, table)
            }
        }
    };
}

gammu_lut_interp_float!(f32);
gammu_lut_interp_float!(f64);

pub(crate) fn make_gamma_lut<
    T: Default + Copy + 'static + PointeeSizeExpressible + GammaLutInterpolate,
    const BUCKET: usize,
    const N: usize,
    const BIT_DEPTH: usize,
>(
    table: &[u16],
) -> Box<[T; BUCKET]>
where
    u32: AsPrimitive<T>,
    f32: AsPrimitive<T>,
{
    let mut new_table = Box::new([T::default(); BUCKET]);
    for (v, output) in new_table.iter_mut().take(N).enumerate() {
        *output = T::gamma_lut_interp::<T, N, BIT_DEPTH>(v as u32, table);
    }
    new_table
}

#[inline]
pub(crate) fn lut_interp_linear16(input_value: u16, table: &[u16]) -> u16 {
    // Start scaling input_value to the length of the array: 65535*(length-1).
    // We'll divide out the 65535 next
    let mut value: u32 = input_value as u32 * (table.len() as u32 - 1);
    let upper: u16 = value.div_ceil(65535) as u16; // equivalent to ceil(value/65535)
    let lower: u16 = (value / 65535) as u16; // equivalent to floor(value/65535)
    // interp is the distance from upper to value scaled to 0..65535
    let interp: u32 = value % 65535; // 0..65535*65535
    value = (table[upper as usize] as u32 * interp
        + table[lower as usize] as u32 * (65535 - interp))
        / 65535;
    value as u16
}

#[inline]
pub(crate) fn lut_interp_linear16_boxed<const N: usize>(input_value: u16, table: &[u16; N]) -> u16 {
    // Start scaling input_value to the length of the array: 65535*(length-1).
    // We'll divide out the 65535 next
    let mut value: u32 = input_value as u32 * (table.len() as u32 - 1);
    let upper: u16 = value.div_ceil(65535) as u16; // equivalent to ceil(value/65535)
    let lower: u16 = (value / 65535) as u16; // equivalent to floor(value/65535)
    // interp is the distance from upper to value scaled to 0..65535
    let interp: u32 = value % 65535; // 0..65535*65535
    value = (table[upper as usize] as u32 * interp
        + table[lower as usize] as u32 * (65535 - interp))
        / 65535;
    value as u16
}

fn make_gamma_pow_table<
    T: Default + Copy + 'static + PointeeSizeExpressible,
    const BUCKET: usize,
    const N: usize,
    const BIT_DEPTH: usize,
>(
    gamma: f32,
) -> Box<[T; BUCKET]>
where
    f32: AsPrimitive<T>,
{
    let mut table = Box::new([T::default(); BUCKET]);
    let scale = 1f32 / (N - 1) as f32;
    let cap = ((1 << BIT_DEPTH) - 1) as f32;
    if T::FINITE {
        for (v, output) in table.iter_mut().take(N).enumerate() {
            *output = (cap * f_powf(v as f32 * scale, gamma)).round().as_();
        }
    } else {
        for (v, output) in table.iter_mut().take(N).enumerate() {
            *output = (cap * f_powf(v as f32 * scale, gamma)).as_();
        }
    }
    table
}

fn make_gamma_parametric_table<
    T: Default + Copy + 'static + PointeeSizeExpressible,
    const BUCKET: usize,
    const N: usize,
    const BIT_DEPTH: usize,
>(
    parametric_curve: ParametricCurve,
) -> Box<[T; BUCKET]>
where
    f32: AsPrimitive<T>,
{
    let mut table = Box::new([T::default(); BUCKET]);
    let scale = 1f32 / (N - 1) as f32;
    let cap = ((1 << BIT_DEPTH) - 1) as f32;
    if T::FINITE {
        for (v, output) in table.iter_mut().take(N).enumerate() {
            *output = (cap * parametric_curve.eval(v as f32 * scale))
                .round()
                .as_();
        }
    } else {
        for (v, output) in table.iter_mut().take(N).enumerate() {
            *output = (cap * parametric_curve.eval(v as f32 * scale)).as_();
        }
    }
    table
}

#[inline]
fn compare_parametric(src: &[f32], dst: &[f32]) -> bool {
    for (src, dst) in src.iter().zip(dst.iter()) {
        if (src - dst).abs() > 1e-4 {
            return false;
        }
    }
    true
}

fn lut_inverse_interp16(value: u16, lut_table: &[u16]) -> u16 {
    let mut l: i32 = 1; // 'int' Give spacing for negative values
    let mut r: i32 = 0x10000;
    let mut x: i32 = 0;
    let mut res: i32;
    let length = lut_table.len() as i32;

    let mut num_zeroes: i32 = 0;
    for &item in lut_table.iter() {
        if item == 0 {
            num_zeroes += 1
        } else {
            break;
        }
    }

    if num_zeroes == 0 && value as i32 == 0 {
        return 0u16;
    }
    let mut num_of_polys: i32 = 0;
    for &item in lut_table.iter().rev() {
        if item == 0xffff {
            num_of_polys += 1
        } else {
            break;
        }
    }
    // Does the curve belong to this case?
    if num_zeroes > 1 || num_of_polys > 1 {
        let a_0: i32;
        let b_0: i32;
        // Identify if value fall downto 0 or FFFF zone
        if value as i32 == 0 {
            return 0u16;
        }
        // if (Value == 0xFFFF) return 0xFFFF;
        // else restrict to valid zone
        if num_zeroes > 1 {
            a_0 = (num_zeroes - 1) * 0xffff / (length - 1);
            l = a_0 - 1
        }
        if num_of_polys > 1 {
            b_0 = (length - 1 - num_of_polys) * 0xffff / (length - 1);
            r = b_0 + 1
        }
    }
    if r <= l {
        // If this happens LutTable is not invertible
        return 0u16;
    }

    while r > l {
        x = (l + r) / 2;
        res = lut_interp_linear16((x - 1) as u16, lut_table) as i32;
        if res == value as i32 {
            // Found exact match.
            return (x - 1) as u16;
        }
        if res > value as i32 {
            r = x - 1
        } else {
            l = x + 1
        }
    }

    // Not found, should we interpolate?

    // Get surrounding nodes
    debug_assert!(x >= 1);

    let val2: f64 = (length - 1) as f64 * ((x - 1) as f64 / 65535.0);
    let cell0: i32 = val2.floor() as i32;
    let cell1: i32 = val2.ceil() as i32;
    if cell0 == cell1 {
        return x as u16;
    }

    let y0: f64 = lut_table[cell0 as usize] as f64;
    let x0: f64 = 65535.0 * cell0 as f64 / (length - 1) as f64;
    let y1: f64 = lut_table[cell1 as usize] as f64;
    let x1: f64 = 65535.0 * cell1 as f64 / (length - 1) as f64;
    let a: f64 = (y1 - y0) / (x1 - x0);
    let b: f64 = mlaf(y0, -a, x0);
    if a.abs() < 0.01f64 {
        return x as u16;
    }
    let f: f64 = (value as i32 as f64 - b) / a;
    if f < 0.0 {
        return 0u16;
    }
    if f >= 65535.0 {
        return 0xffffu16;
    }
    (f + 0.5f64).floor() as u16
}

fn lut_inverse_interp16_boxed<const N: usize>(value: u16, lut_table: &[u16; N]) -> u16 {
    let mut l: i32 = 1; // 'int' Give spacing for negative values
    let mut r: i32 = 0x10000;
    let mut x: i32 = 0;
    let mut res: i32;
    let length = lut_table.len() as i32;

    let mut num_zeroes: i32 = 0;
    for &item in lut_table.iter() {
        if item == 0 {
            num_zeroes += 1
        } else {
            break;
        }
    }

    if num_zeroes == 0 && value as i32 == 0 {
        return 0u16;
    }
    let mut num_of_polys: i32 = 0;
    for &item in lut_table.iter().rev() {
        if item == 0xffff {
            num_of_polys += 1
        } else {
            break;
        }
    }
    // Does the curve belong to this case?
    if num_zeroes > 1 || num_of_polys > 1 {
        let a_0: i32;
        let b_0: i32;
        // Identify if value fall downto 0 or FFFF zone
        if value as i32 == 0 {
            return 0u16;
        }
        // if (Value == 0xFFFF) return 0xFFFF;
        // else restrict to valid zone
        if num_zeroes > 1 {
            a_0 = (num_zeroes - 1) * 0xffff / (length - 1);
            l = a_0 - 1
        }
        if num_of_polys > 1 {
            b_0 = (length - 1 - num_of_polys) * 0xffff / (length - 1);
            r = b_0 + 1
        }
    }
    if r <= l {
        // If this happens LutTable is not invertible
        return 0u16;
    }

    while r > l {
        x = (l + r) / 2;
        res = lut_interp_linear16_boxed((x - 1) as u16, lut_table) as i32;
        if res == value as i32 {
            // Found exact match.
            return (x - 1) as u16;
        }
        if res > value as i32 {
            r = x - 1
        } else {
            l = x + 1
        }
    }

    // Not found, should we interpolate?

    // Get surrounding nodes
    debug_assert!(x >= 1);

    let val2: f64 = (length - 1) as f64 * ((x - 1) as f64 / 65535.0);
    let cell0: i32 = val2.floor() as i32;
    let cell1: i32 = val2.ceil() as i32;
    if cell0 == cell1 {
        return x as u16;
    }

    let y0: f64 = lut_table[cell0 as usize] as f64;
    let x0: f64 = 65535.0 * cell0 as f64 / (length - 1) as f64;
    let y1: f64 = lut_table[cell1 as usize] as f64;
    let x1: f64 = 65535.0 * cell1 as f64 / (length - 1) as f64;
    let a: f64 = (y1 - y0) / (x1 - x0);
    let b: f64 = mlaf(y0, -a, x0);
    if a.abs() < 0.01f64 {
        return x as u16;
    }
    let f: f64 = (value as i32 as f64 - b) / a;
    if f < 0.0 {
        return 0u16;
    }
    if f >= 65535.0 {
        return 0xffffu16;
    }
    (f + 0.5f64).floor() as u16
}

fn invert_lut(table: &[u16], out_length: usize) -> Vec<u16> {
    // For now, we invert the lut by creating a lut of size out_length
    // and attempting to look up a value for each entry using lut_inverse_interp16
    let mut output = vec![0u16; out_length];
    let scale_value = 65535f64 / (out_length - 1) as f64;
    for (i, out) in output.iter_mut().enumerate() {
        let x: f64 = i as f64 * scale_value;
        let input: u16 = (x + 0.5f64).floor() as u16;
        *out = lut_inverse_interp16(input, table);
    }
    output
}

fn invert_lut_boxed<const N: usize>(table: &[u16; N], out_length: usize) -> Vec<u16> {
    // For now, we invert the lut by creating a lut of size out_length
    // and attempting to look up a value for each entry using lut_inverse_interp16
    let mut output = vec![0u16; out_length];
    let scale_value = 65535f64 / (out_length - 1) as f64;
    for (i, out) in output.iter_mut().enumerate() {
        let x: f64 = i as f64 * scale_value;
        let input: u16 = (x + 0.5f64).floor() as u16;
        *out = lut_inverse_interp16_boxed(input, table);
    }
    output
}

impl ToneReprCurve {
    pub(crate) fn to_clut(&self) -> Result<Vec<f32>, CmsError> {
        match self {
            ToneReprCurve::Lut(lut) => {
                if lut.is_empty() {
                    let passthrough_table = passthrough_table::<f32, 16384, 1>();
                    Ok(passthrough_table.to_vec())
                } else {
                    Ok(lut
                        .iter()
                        .map(|&x| x as f32 * (1. / 65535.))
                        .collect::<Vec<_>>())
                }
            }
            ToneReprCurve::Parametric(_) => {
                let curve = self
                    .build_linearize_table::<f32, 65535, 1>()
                    .ok_or(CmsError::InvalidTrcCurve)?;
                let max_value = f32::NOT_FINITE_LINEAR_TABLE_SIZE - 1;
                let sliced = &curve[..max_value];
                Ok(sliced.to_vec())
            }
        }
    }

    pub(crate) fn build_linearize_table<
        T: PointeeSizeExpressible,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
    ) -> Option<Box<[f32; N]>> {
        match self {
            ToneReprCurve::Parametric(params) => linear_curve_parametric::<T, N, BIT_DEPTH>(params),
            ToneReprCurve::Lut(data) => match data.len() {
                0 => Some(passthrough_table::<T, N, BIT_DEPTH>()),
                1 => Some(linear_forward_table::<T, N, BIT_DEPTH>(data[0])),
                _ => Some(linear_lut_interpolate::<T, N, BIT_DEPTH>(data)),
            },
        }
    }

    pub(crate) fn build_gamma_table<
        T: Default + Copy + 'static + PointeeSizeExpressible + GammaLutInterpolate,
        const BUCKET: usize,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
    ) -> Option<Box<[T; BUCKET]>>
    where
        f32: AsPrimitive<T>,
        u32: AsPrimitive<T>,
    {
        match self {
            ToneReprCurve::Parametric(params) => {
                if params.len() == 5 {
                    let srgb_params = vec![2.4, 1. / 1.055, 0.055 / 1.055, 1. / 12.92, 0.04045];
                    let rec709_params = create_rec709_parametric();

                    let mut lc_params: [f32; 5] = [0.; 5];
                    for (dst, src) in lc_params.iter_mut().zip(params.iter()) {
                        *dst = *src;
                    }

                    if compare_parametric(lc_params.as_slice(), srgb_params.as_slice()) {
                        return Some(
                            TransferCharacteristics::Srgb
                                .make_gamma_table::<T, BUCKET, N, BIT_DEPTH>(),
                        );
                    }

                    if compare_parametric(lc_params.as_slice(), rec709_params.as_slice()) {
                        return Some(
                            TransferCharacteristics::Bt709
                                .make_gamma_table::<T, BUCKET, N, BIT_DEPTH>(),
                        );
                    }
                }

                let parametric_curve = ParametricCurve::new(params);
                if let Some(v) = parametric_curve?
                    .invert()
                    .map(|x| make_gamma_parametric_table::<T, BUCKET, N, BIT_DEPTH>(x))
                {
                    return Some(v);
                }

                let mut gamma_table_uint = Box::new([0; N]);

                let inverted_size: usize = N;
                let gamma_table = linear_curve_parametric_s::<N>(params)?;
                for (&src, dst) in gamma_table.iter().zip(gamma_table_uint.iter_mut()) {
                    *dst = (src * 65535f32) as u16;
                }
                let inverted = invert_lut_boxed(&gamma_table_uint, inverted_size);
                Some(make_gamma_lut::<T, BUCKET, N, BIT_DEPTH>(&inverted))
            }
            ToneReprCurve::Lut(data) => match data.len() {
                0 => Some(make_gamma_linear_table::<T, BUCKET, N, BIT_DEPTH>()),
                1 => Some(make_gamma_pow_table::<T, BUCKET, N, BIT_DEPTH>(
                    1. / u8_fixed_8number_to_float(data[0]),
                )),
                _ => {
                    let mut inverted_size = data.len();
                    if inverted_size < 256 {
                        inverted_size = 256
                    }
                    let inverted = invert_lut(data, inverted_size);
                    Some(make_gamma_lut::<T, BUCKET, N, BIT_DEPTH>(&inverted))
                }
            },
        }
    }
}

impl ColorProfile {
    /// Produces LUT for 8 bit tone linearization
    pub fn build_8bit_lin_table(
        &self,
        trc: &Option<ToneReprCurve>,
    ) -> Result<Box<[f32; 256]>, CmsError> {
        trc.as_ref()
            .and_then(|trc| trc.build_linearize_table::<u8, 256, 8>())
            .ok_or(CmsError::BuildTransferFunction)
    }

    /// Produces LUT for Gray transfer curve with N depth
    pub fn build_gray_linearize_table<
        T: PointeeSizeExpressible,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
    ) -> Result<Box<[f32; N]>, CmsError> {
        self.gray_trc
            .as_ref()
            .and_then(|trc| trc.build_linearize_table::<T, N, BIT_DEPTH>())
            .ok_or(CmsError::BuildTransferFunction)
    }

    /// Produces LUT for Red transfer curve with N depth
    pub fn build_r_linearize_table<
        T: PointeeSizeExpressible,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
        use_cicp: bool,
    ) -> Result<Box<[f32; N]>, CmsError> {
        if use_cicp {
            if let Some(tc) = self.cicp.as_ref().map(|c| c.transfer_characteristics) {
                if tc.has_transfer_curve() {
                    return Ok(tc.make_linear_table::<T, N, BIT_DEPTH>());
                }
            }
        }
        self.red_trc
            .as_ref()
            .and_then(|trc| trc.build_linearize_table::<T, N, BIT_DEPTH>())
            .ok_or(CmsError::BuildTransferFunction)
    }

    /// Produces LUT for Green transfer curve with N depth
    pub fn build_g_linearize_table<
        T: PointeeSizeExpressible,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
        use_cicp: bool,
    ) -> Result<Box<[f32; N]>, CmsError> {
        if use_cicp {
            if let Some(tc) = self.cicp.as_ref().map(|c| c.transfer_characteristics) {
                if tc.has_transfer_curve() {
                    return Ok(tc.make_linear_table::<T, N, BIT_DEPTH>());
                }
            }
        }
        self.green_trc
            .as_ref()
            .and_then(|trc| trc.build_linearize_table::<T, N, BIT_DEPTH>())
            .ok_or(CmsError::BuildTransferFunction)
    }

    /// Produces LUT for Blue transfer curve with N depth
    pub fn build_b_linearize_table<
        T: PointeeSizeExpressible,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
        use_cicp: bool,
    ) -> Result<Box<[f32; N]>, CmsError> {
        if use_cicp {
            if let Some(tc) = self.cicp.as_ref().map(|c| c.transfer_characteristics) {
                if tc.has_transfer_curve() {
                    return Ok(tc.make_linear_table::<T, N, BIT_DEPTH>());
                }
            }
        }
        self.blue_trc
            .as_ref()
            .and_then(|trc| trc.build_linearize_table::<T, N, BIT_DEPTH>())
            .ok_or(CmsError::BuildTransferFunction)
    }

    /// Build gamma table for 8 bit depth
    /// Only 4092 first bins are used and values scaled in 0..255
    pub fn build_8bit_gamma_table(
        &self,
        trc: &Option<ToneReprCurve>,
        use_cicp: bool,
    ) -> Result<Box<[u16; 65536]>, CmsError> {
        self.build_gamma_table::<u16, 65536, 4092, 8>(trc, use_cicp)
    }

    /// Build gamma table for 10 bit depth
    /// Only 8192 first bins are used and values scaled in 0..1023
    pub fn build_10bit_gamma_table(
        &self,
        trc: &Option<ToneReprCurve>,
        use_cicp: bool,
    ) -> Result<Box<[u16; 65536]>, CmsError> {
        self.build_gamma_table::<u16, 65536, 8192, 10>(trc, use_cicp)
    }

    /// Build gamma table for 12 bit depth
    /// Only 16384 first bins are used and values scaled in 0..4095
    pub fn build_12bit_gamma_table(
        &self,
        trc: &Option<ToneReprCurve>,
        use_cicp: bool,
    ) -> Result<Box<[u16; 65536]>, CmsError> {
        self.build_gamma_table::<u16, 65536, 16384, 12>(trc, use_cicp)
    }

    /// Build gamma table for 16 bit depth
    /// Only 16384 first bins are used and values scaled in 0..65535
    pub fn build_16bit_gamma_table(
        &self,
        trc: &Option<ToneReprCurve>,
        use_cicp: bool,
    ) -> Result<Box<[u16; 65536]>, CmsError> {
        self.build_gamma_table::<u16, 65536, 65536, 16>(trc, use_cicp)
    }

    /// Builds gamma table checking CICP for Transfer characteristics first.
    pub fn build_gamma_table<
        T: Default + Copy + 'static + PointeeSizeExpressible + GammaLutInterpolate,
        const BUCKET: usize,
        const N: usize,
        const BIT_DEPTH: usize,
    >(
        &self,
        trc: &Option<ToneReprCurve>,
        use_cicp: bool,
    ) -> Result<Box<[T; BUCKET]>, CmsError>
    where
        f32: AsPrimitive<T>,
        u32: AsPrimitive<T>,
    {
        if use_cicp {
            if let Some(tc) = self.cicp.as_ref().map(|c| c.transfer_characteristics) {
                if tc.has_transfer_curve() {
                    return Ok(tc.make_gamma_table::<T, BUCKET, N, BIT_DEPTH>());
                }
            }
        }
        trc.as_ref()
            .and_then(|trc| trc.build_gamma_table::<T, BUCKET, N, BIT_DEPTH>())
            .ok_or(CmsError::BuildTransferFunction)
    }

    /// Checks if profile gamma can work in extended precision and we have implementation for this
    pub(crate) fn try_extended_gamma_evaluator(
        &self,
    ) -> Option<Box<dyn ToneCurveEvaluator + Send + Sync>> {
        if let Some(tc) = self.cicp.as_ref().map(|c| c.transfer_characteristics) {
            if tc.has_transfer_curve() {
                return Some(Box::new(ToneCurveCicpEvaluator {
                    rgb_trc: tc.extended_gamma_tristimulus(),
                    trc: tc.extended_gamma_single(),
                }));
            }
        }
        if !self.are_all_trc_the_same() {
            return None;
        }
        let reference_trc = if self.color_space == DataColorSpace::Gray {
            self.gray_trc.as_ref()
        } else {
            self.red_trc.as_ref()
        };
        if let Some(red_trc) = reference_trc {
            return Self::make_gamma_evaluator_all_the_same(red_trc);
        }
        None
    }

    fn make_gamma_evaluator_all_the_same(
        red_trc: &ToneReprCurve,
    ) -> Option<Box<dyn ToneCurveEvaluator + Send + Sync>> {
        match red_trc {
            ToneReprCurve::Lut(lut) => {
                if lut.is_empty() {
                    return Some(Box::new(ToneCurveEvaluatorLinear {}));
                }
                if lut.len() == 1 {
                    let gamma = 1. / u8_fixed_8number_to_float(lut[0]);
                    return Some(Box::new(ToneCurveEvaluatorPureGamma { gamma }));
                }
                None
            }
            ToneReprCurve::Parametric(params) => {
                if params.len() == 5 {
                    let srgb_params = vec![2.4, 1. / 1.055, 0.055 / 1.055, 1. / 12.92, 0.04045];
                    let rec709_params = create_rec709_parametric();

                    let mut lc_params: [f32; 5] = [0.; 5];
                    for (dst, src) in lc_params.iter_mut().zip(params.iter()) {
                        *dst = *src;
                    }

                    if compare_parametric(lc_params.as_slice(), srgb_params.as_slice()) {
                        return Some(Box::new(ToneCurveCicpEvaluator {
                            rgb_trc: TransferCharacteristics::Srgb.extended_gamma_tristimulus(),
                            trc: TransferCharacteristics::Srgb.extended_gamma_single(),
                        }));
                    }

                    if compare_parametric(lc_params.as_slice(), rec709_params.as_slice()) {
                        return Some(Box::new(ToneCurveCicpEvaluator {
                            rgb_trc: TransferCharacteristics::Bt709.extended_gamma_tristimulus(),
                            trc: TransferCharacteristics::Bt709.extended_gamma_single(),
                        }));
                    }
                }

                let parametric_curve = ParametricCurve::new(params);
                if let Some(v) = parametric_curve?.invert() {
                    return Some(Box::new(ToneCurveParametricEvaluator { parametric: v }));
                }
                None
            }
        }
    }

    /// Check if all TRC are the same
    pub(crate) fn are_all_trc_the_same(&self) -> bool {
        if self.color_space == DataColorSpace::Gray {
            return true;
        }
        if let (Some(red_trc), Some(green_trc), Some(blue_trc)) =
            (&self.red_trc, &self.green_trc, &self.blue_trc)
        {
            if !matches!(
                (red_trc, green_trc, blue_trc),
                (
                    ToneReprCurve::Lut(_),
                    ToneReprCurve::Lut(_),
                    ToneReprCurve::Lut(_),
                ) | (
                    ToneReprCurve::Parametric(_),
                    ToneReprCurve::Parametric(_),
                    ToneReprCurve::Parametric(_)
                )
            ) {
                return false;
            }
            if let (ToneReprCurve::Lut(lut0), ToneReprCurve::Lut(lut1), ToneReprCurve::Lut(lut2)) =
                (red_trc, green_trc, blue_trc)
            {
                if lut0 == lut1 || lut1 == lut2 {
                    return true;
                }
            }
            if let (
                ToneReprCurve::Parametric(lut0),
                ToneReprCurve::Parametric(lut1),
                ToneReprCurve::Parametric(lut2),
            ) = (red_trc, green_trc, blue_trc)
            {
                if lut0 == lut1 || lut1 == lut2 {
                    return true;
                }
            }
        }
        false
    }

    /// Checks if profile is matrix shaper, have same TRC and TRC is linear.
    pub(crate) fn is_linear_matrix_shaper(&self) -> bool {
        if !self.is_matrix_shaper() {
            return false;
        }
        if !self.are_all_trc_the_same() {
            return false;
        }
        if let Some(red_trc) = &self.red_trc {
            return match red_trc {
                ToneReprCurve::Lut(lut) => {
                    if lut.is_empty() {
                        return true;
                    }
                    if is_curve_linear16(lut) {
                        return true;
                    }
                    false
                }
                ToneReprCurve::Parametric(params) => {
                    if let Some(curve) = ParametricCurve::new(params) {
                        return curve.is_linear();
                    }
                    false
                }
            };
        }
        false
    }

    /// Checks if profile linearization can work in extended precision and we have implementation for this
    pub(crate) fn try_extended_linearizing_evaluator(
        &self,
    ) -> Option<Box<dyn ToneCurveEvaluator + Send + Sync>> {
        if let Some(tc) = self.cicp.as_ref().map(|c| c.transfer_characteristics) {
            if tc.has_transfer_curve() {
                return Some(Box::new(ToneCurveCicpEvaluator {
                    rgb_trc: tc.extended_linear_tristimulus(),
                    trc: tc.extended_linear_single(),
                }));
            }
        }
        if !self.are_all_trc_the_same() {
            return None;
        }
        let reference_trc = if self.color_space == DataColorSpace::Gray {
            self.gray_trc.as_ref()
        } else {
            self.red_trc.as_ref()
        };
        if let Some(red_trc) = reference_trc {
            if let Some(value) = Self::make_linear_curve_evaluator_all_the_same(red_trc) {
                return value;
            }
        }
        None
    }

    fn make_linear_curve_evaluator_all_the_same(
        evaluator_curve: &ToneReprCurve,
    ) -> Option<Option<Box<dyn ToneCurveEvaluator + Send + Sync>>> {
        match evaluator_curve {
            ToneReprCurve::Lut(lut) => {
                if lut.is_empty() {
                    return Some(Some(Box::new(ToneCurveEvaluatorLinear {})));
                }
                if lut.len() == 1 {
                    let gamma = u8_fixed_8number_to_float(lut[0]);
                    return Some(Some(Box::new(ToneCurveEvaluatorPureGamma { gamma })));
                }
            }
            ToneReprCurve::Parametric(params) => {
                if params.len() == 5 {
                    let srgb_params = vec![2.4, 1. / 1.055, 0.055 / 1.055, 1. / 12.92, 0.04045];
                    let rec709_params = create_rec709_parametric();

                    let mut lc_params: [f32; 5] = [0.; 5];
                    for (dst, src) in lc_params.iter_mut().zip(params.iter()) {
                        *dst = *src;
                    }

                    if compare_parametric(lc_params.as_slice(), srgb_params.as_slice()) {
                        return Some(Some(Box::new(ToneCurveCicpEvaluator {
                            rgb_trc: TransferCharacteristics::Srgb.extended_linear_tristimulus(),
                            trc: TransferCharacteristics::Srgb.extended_linear_single(),
                        })));
                    }

                    if compare_parametric(lc_params.as_slice(), rec709_params.as_slice()) {
                        return Some(Some(Box::new(ToneCurveCicpEvaluator {
                            rgb_trc: TransferCharacteristics::Bt709.extended_linear_tristimulus(),
                            trc: TransferCharacteristics::Bt709.extended_linear_single(),
                        })));
                    }
                }

                let parametric_curve = ParametricCurve::new(params);
                if let Some(v) = parametric_curve {
                    return Some(Some(Box::new(ToneCurveParametricEvaluator {
                        parametric: v,
                    })));
                }
            }
        }
        None
    }
}

pub(crate) struct ToneCurveCicpEvaluator {
    rgb_trc: fn(Rgb<f32>) -> Rgb<f32>,
    trc: fn(f32) -> f32,
}

pub(crate) struct ToneCurveParametricEvaluator {
    parametric: ParametricCurve,
}

pub(crate) struct ToneCurveEvaluatorPureGamma {
    gamma: f32,
}

pub(crate) struct ToneCurveEvaluatorLinear {}

impl ToneCurveEvaluator for ToneCurveCicpEvaluator {
    fn evaluate_tristimulus(&self, rgb: Rgb<f32>) -> Rgb<f32> {
        (self.rgb_trc)(rgb)
    }

    fn evaluate_value(&self, value: f32) -> f32 {
        (self.trc)(value)
    }
}

impl ToneCurveEvaluator for ToneCurveParametricEvaluator {
    fn evaluate_tristimulus(&self, rgb: Rgb<f32>) -> Rgb<f32> {
        Rgb::new(
            self.parametric.eval(rgb.r),
            self.parametric.eval(rgb.g),
            self.parametric.eval(rgb.b),
        )
    }

    fn evaluate_value(&self, value: f32) -> f32 {
        self.parametric.eval(value)
    }
}

impl ToneCurveEvaluator for ToneCurveEvaluatorPureGamma {
    fn evaluate_tristimulus(&self, rgb: Rgb<f32>) -> Rgb<f32> {
        Rgb::new(
            dirty_powf(rgb.r, self.gamma),
            dirty_powf(rgb.g, self.gamma),
            dirty_powf(rgb.b, self.gamma),
        )
    }

    fn evaluate_value(&self, value: f32) -> f32 {
        dirty_powf(value, self.gamma)
    }
}

impl ToneCurveEvaluator for ToneCurveEvaluatorLinear {
    fn evaluate_tristimulus(&self, rgb: Rgb<f32>) -> Rgb<f32> {
        rgb
    }

    fn evaluate_value(&self, value: f32) -> f32 {
        value
    }
}

pub trait ToneCurveEvaluator {
    fn evaluate_tristimulus(&self, rgb: Rgb<f32>) -> Rgb<f32>;
    fn evaluate_value(&self, value: f32) -> f32;
}
