/*
 * // Copyright (c) Radzivon Bartoshyk 7/2025. All rights reserved.
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
use crate::bessel::j1_coeffs::{J1_ZEROS, J1_ZEROS_VALUE};
use crate::bessel::j1f::{j1f_asympt_alpha, j1f_asympt_beta};
use crate::bessel::j1f_coeffs::J1F_COEFFS;
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::polyeval::{f_polyeval6, f_polyeval14};
use crate::sin_helper::sin_small;

/// Normalized jinc 2*J1(PI\*x)/(pi\*x)
///
/// ULP 0.5
pub fn f_jincpif(x: f32) -> f32 {
    if (x.to_bits() & 0x0007_ffff) == 0 {
        if x == 0. {
            return 1.;
        }
        if x.is_infinite() {
            return 0.;
        }
        if x.is_nan() {
            return x + x;
        }
    }

    let ax = x.to_bits() & 0x7fff_ffff;

    if ax <= 0x34000000u32 {
        // |x| <= f32::EPSILON
        // use series here j1(x*Pi)/(x*Pi) ~ 1 - Pi^2*x^2/8 + O(x^4)
        let dx = x as f64;
        let x2 = dx * dx;
        const MSQR_PI_OVER_16: f64 = f64::from_bits(0xbff3bd3cc9be45de);
        return f_fmla(MSQR_PI_OVER_16, x2, 1.) as f32;
    }

    if ax < 0x429533c2u32 {
        // 74.60109
        if ax <= 0x3e800000u32 {
            // 0.25
            return jincf_near_zero(f32::from_bits(ax));
        }
        let scaled_pix = f32::from_bits(ax) * std::f32::consts::PI; // just test boundaries
        if scaled_pix < 74.60109 {
            return jincpif_small_argument(f32::from_bits(ax));
        }
    }

    jincpif_asympt(f32::from_bits(ax)) as f32
}

#[inline]
fn j1f_rsqrt(x: f64) -> f64 {
    (1. / x) * x.sqrt()
}

#[inline]
fn jincf_near_zero(x: f32) -> f32 {
    let dx = x as f64;
    // Generated in Wolfram Mathematica:
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=BesselJ[1,x*Pi]/(x*Pi)
    // {err,approx}=MiniMaxApproximation[f[z],{z,{2^-23,0.3},6,0},WorkingPrecision->60]
    // poly=Numerator[approx][[1]];
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    let p_num = f_polyeval6(
        dx,
        f64::from_bits(0x3fe0000000000002),
        f64::from_bits(0xbfd46cd1822a5aa0),
        f64::from_bits(0xbfde583c923dc6f4),
        f64::from_bits(0x3fd3834f47496519),
        f64::from_bits(0x3fb8118468756e6f),
        f64::from_bits(0xbfafaff09f13df88),
    );
    let p_den = f_polyeval6(
        dx,
        f64::from_bits(0x3ff0000000000000),
        f64::from_bits(0xbfe46cd1822a4cb0),
        f64::from_bits(0x3fd2447a026f477a),
        f64::from_bits(0xbfc6bdf2192404e5),
        f64::from_bits(0x3fa0cf182218e448),
        f64::from_bits(0xbf939ab46c3f7a7d),
    );
    (p_num / p_den * 2.) as f32
}

/// This method on small range searches for nearest zero or extremum.
/// Then picks stored series expansion at the point end evaluates the poly at the point.
#[inline]
fn jincpif_small_argument(ox: f32) -> f32 {
    const PI: f64 = f64::from_bits(0x400921fb54442d18);
    let x = ox as f64 * PI;
    let x_abs = f64::from_bits(x.to_bits() & 0x7fff_ffff_ffff_ffff);

    // let avg_step = 74.60109 / 47.0;
    // let inv_step = 1.0 / avg_step;

    const INV_STEP: f64 = 0.6300176043004198;

    let inv_scale = x;

    let fx = x_abs * INV_STEP;
    const J1_ZEROS_COUNT: f64 = (J1_ZEROS.len() - 1) as f64;
    let idx0 = fx.min(J1_ZEROS_COUNT) as usize;
    let idx1 = fx.ceil().min(J1_ZEROS_COUNT) as usize;

    let found_zero0 = DoubleDouble::from_bit_pair(J1_ZEROS[idx0]);
    let found_zero1 = DoubleDouble::from_bit_pair(J1_ZEROS[idx1]);

    let dist0 = (found_zero0.hi - x_abs).abs();
    let dist1 = (found_zero1.hi - x_abs).abs();

    let (found_zero, idx, dist) = if dist0 < dist1 {
        (found_zero0, idx0, dist0)
    } else {
        (found_zero1, idx1, dist1)
    };

    if idx == 0 {
        return jincf_near_zero(ox);
    }

    // We hit exact zero, value, better to return it directly
    if dist == 0. {
        return (f64::from_bits(J1_ZEROS_VALUE[idx]) / inv_scale * 2.) as f32;
    }

    let c = &J1F_COEFFS[idx - 1];

    let r = (x_abs - found_zero.hi) - found_zero.lo;

    let p = f_polyeval14(
        r,
        f64::from_bits(c[0]),
        f64::from_bits(c[1]),
        f64::from_bits(c[2]),
        f64::from_bits(c[3]),
        f64::from_bits(c[4]),
        f64::from_bits(c[5]),
        f64::from_bits(c[6]),
        f64::from_bits(c[7]),
        f64::from_bits(c[8]),
        f64::from_bits(c[9]),
        f64::from_bits(c[10]),
        f64::from_bits(c[11]),
        f64::from_bits(c[12]),
        f64::from_bits(c[13]),
    );

    (p / inv_scale * 2.) as f32
}

/*
   Evaluates:
   J1 = sqrt(2/(PI*x)) * beta(x) * cos(x - 3*PI/4 - alpha(x))
   discarding 1*PI/2 using identities gives:
   J1 = sqrt(2/(PI*x)) * beta(x) * sin(x - PI/4 - alpha(x))

   to avoid squashing small (-PI/4 - alpha(x)) into a large x actual expansion is:

   J1 = sqrt(2/(PI*x)) * beta(x) * sin((x mod 2*PI) - PI/4 - alpha(x))
*/
#[inline]
pub(crate) fn jincpif_asympt(x: f32) -> f64 {
    const PI: f64 = f64::from_bits(0x400921fb54442d18);

    let dox = x as f64;
    let dx = dox * PI;

    let inv_scale = dx;

    let alpha = j1f_asympt_alpha(dx);
    let beta = j1f_asympt_beta(dx);

    // argument reduction assuming x here value is already multiple of PI.
    // k = round((x*Pi) / (pi*2))
    let kd = (dox * 0.5).round();
    //  y = (x * Pi) - k * 2
    let angle = f_fmla(kd, -2., dox) * PI;

    const SQRT_2_OVER_PI: f64 = f64::from_bits(0x3fe9884533d43651);
    const MPI_OVER_4: f64 = f64::from_bits(0xbfe921fb54442d18);

    let x0pi34 = MPI_OVER_4 - alpha;
    let r0 = angle + x0pi34;

    let m_sin = sin_small(r0);

    let z0 = beta * m_sin;
    let scale = SQRT_2_OVER_PI * j1f_rsqrt(dx);

    let j1pix = scale * z0;
    (j1pix / inv_scale) * 2.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jincpif() {
        assert_eq!(f_jincpif(102.59484), 0.00024380769);
        assert_eq!(f_jincpif(100.08199), -0.00014386141);
        assert_eq!(f_jincpif(0.27715185), 0.9081822);
        assert_eq!(f_jincpif(0.007638072), 0.99992806);
        assert_eq!(
            f_jincpif(0.000000000000000000000000000000000000008827127),
            1.0
        );
        assert_eq!(f_jincpif(5.4), -0.010821743);
        assert_eq!(
            f_jincpif(77.743162408196766932633181568235159),
            -0.00041799102
        );
        assert_eq!(
            f_jincpif(84.027189586293545175976760219782591),
            -0.00023927793
        );
        assert_eq!(f_jincpif(f32::INFINITY), 0.);
        assert_eq!(f_jincpif(f32::NEG_INFINITY), 0.);
        assert!(f_jincpif(f32::NAN).is_nan());
        assert_eq!(f_jincpif(-1.7014118e38), -0.0);
    }
}
