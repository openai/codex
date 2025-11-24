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
use crate::acosf::poly12;
use crate::common::{dd_fmlaf, f_fmla};

#[cold]
fn as_special(x: f32) -> f32 {
    let t = x.to_bits();
    let ax = t.wrapping_shl(1);
    if ax > (0xffu32 << 24) {
        return x + x;
    } // nan
    f32::NAN
}

/// Computes asin
///
/// Max found ULP 0.49999928
#[inline]
pub fn f_asinf(x: f32) -> f32 {
    const PI2: f64 = f64::from_bits(0x3ff921fb54442d18);
    let xs = x as f64;
    let mut r;
    let t = x.to_bits();
    let ax = t.wrapping_shl(1);
    if ax > 0x7f << 24 {
        return as_special(x);
    }
    if ax < 0x7ec29000u32 {
        // |x| < 1.49029
        if ax < 115 << 24 {
            // |x| < 0.000244141
            return dd_fmlaf(x, f64::from_bits(0x3e60000000000000) as f32, x);
        }
        const B: [u64; 16] = [
            0x3ff0000000000005,
            0x3fc55557aeca105d,
            0x3fb3314ec3db7d12,
            0x3fa775738a5a6f92,
            0x3f75d5f7ce1c8538,
            0x3fd605c6d58740f0,
            0xc005728b732d73c6,
            0x402f152170f151eb,
            0xc04f962ea3ca992e,
            0x40671971e17375a0,
            0xc07860512b4ba230,
            0x40826a3b8d4bdb14,
            0xc0836f2ea5698b51,
            0x407b3d722aebfa2e,
            0xc066cf89703b1289,
            0x4041518af6a65e2d,
        ];
        let z = xs;
        let z2 = z * z;
        let w0 = f_fmla(z2, f64::from_bits(B[1]), f64::from_bits(B[0]));
        let w1 = f_fmla(z2, f64::from_bits(B[3]), f64::from_bits(B[2]));
        let w2 = f_fmla(z2, f64::from_bits(B[5]), f64::from_bits(B[4]));
        let w3 = f_fmla(z2, f64::from_bits(B[7]), f64::from_bits(B[6]));
        let w4 = f_fmla(z2, f64::from_bits(B[9]), f64::from_bits(B[8]));
        let w5 = f_fmla(z2, f64::from_bits(B[11]), f64::from_bits(B[10]));
        let w6 = f_fmla(z2, f64::from_bits(B[13]), f64::from_bits(B[12]));
        let w7 = f_fmla(z2, f64::from_bits(B[15]), f64::from_bits(B[14]));

        let z4 = z2 * z2;
        let z8 = z4 * z4;
        let z16 = z8 * z8;

        r = z
            * ((f_fmla(z4, w1, w0) + z8 * f_fmla(z4, w3, w2))
                + z16 * (f_fmla(z4, w5, w4) + z8 * f_fmla(z4, w7, w6)));
        let ub = r;
        let lb = r - z * f64::from_bits(0x3e0efa8eb0000000);
        // Ziv's accuracy test
        if ub == lb {
            return ub as f32;
        }
    }
    if ax < (0x7eu32 << 24) {
        const C: [u64; 12] = [
            0x3fc555555555529c,
            0x3fb333333337e0dd,
            0x3fa6db6db3b4465e,
            0x3f9f1c72e13ac306,
            0x3f96e89cebe06bc4,
            0x3f91c6dcf5289094,
            0x3f8c6dbbcc7c6315,
            0x3f88f8dc2615e996,
            0x3f7a5833b7bf15e8,
            0x3f943f44ace1665c,
            0xbf90fb17df881c73,
            0x3fa07520c026b2d6,
        ];
        let z = xs;
        let z2 = z * z;
        let c0 = poly12(z2, C);
        r = z + (z * z2) * c0;
    } else {
        if ax == 0x7e55688au32 {
            return f32::copysign(f64::from_bits(0x3fe75b8a20000000) as f32, x)
                + f32::copysign(f64::from_bits(0x3e50000000000000) as f32, x);
        }
        if ax == 0x7e107434u32 {
            return f32::copysign(f64::from_bits(0x3fe1f4b640000000) as f32, x)
                + f32::copysign(f64::from_bits(0x3e50000000000000) as f32, x);
        }
        let bx = xs.abs();
        let z = 1.0 - bx;
        let s = z.sqrt();
        const C: [u64; 12] = [
            0x3ff6a09e667f3bcb,
            0x3fbe2b7dddff2db9,
            0x3f9b27247ab42dbc,
            0x3f802995cc4e0744,
            0x3f65ffb0276ec8ea,
            0x3f5033885a928dec,
            0x3f3911f2be23f8c7,
            0x3f24c3c55d2437fd,
            0x3f0af477e1d7b461,
            0x3f0abd6bdff67dcb,
            0xbef1717e86d0fa28,
            0x3ef6ff526de46023,
        ];
        r = PI2 - s * poly12(z, C);
        r = f64::copysign(r, xs);
    }
    r as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asinf() {
        assert_eq!(f_asinf(-0.5), -std::f32::consts::FRAC_PI_6);
        assert_eq!(f_asinf(0.5), std::f32::consts::FRAC_PI_6);
        assert!(f_asinf(7.).is_nan());
    }
}
