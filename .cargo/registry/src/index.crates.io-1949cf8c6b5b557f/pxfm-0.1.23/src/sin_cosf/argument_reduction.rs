/*
 * // Copyright (c) Radzivon Bartoshyk 8/2025. All rights reserved.
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
use crate::bits::get_exponent_f32;
use crate::common::f_fmla;

#[derive(Debug, Copy, Clone)]
pub(crate) struct ArgumentReducer {
    pub(crate) x: f64,
    pub(crate) x_abs: u32,
}

impl ArgumentReducer {
    // Return k and y, where
    // k = round(x * 32 / pi) and y = (x * 32 / pi) - k.
    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    #[inline]
    pub(crate) fn reduce_small(self) -> (f64, i64) {
        /*
           Generated in SageMath:
           z = RealField(300)(32) / RealField(300).pi()
           n = 53
           x_hi = RealField(n)(z)  # convert to f64
           x_mid = RealField(n)(z - RealField(300)(x_hi))
           x_lo = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid))
           print(double_to_hex(x_hi), ",")
           print(double_to_hex(x_mid), ",")
           print(double_to_hex(x_lo), ",")
        */
        const THIRTYTWO_OVER_PI: [u64; 3] =
            [0x40245f306dc9c883, 0xbcc6b01ec5417056, 0xb966447e493ad4ce];

        let kd = (self.x * f64::from_bits(THIRTYTWO_OVER_PI[0])).round();
        let mut y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[0]), -kd);
        y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[1]), y);
        (y, kd as i64)
    }

    // Return k and y, where
    // k = round(x * 32 / pi) and y = (x * 32 / pi) - k.
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    #[inline]
    pub(crate) fn reduce_small(self) -> (f64, i64) {
        /*
           Generated in SageMath:
           z = RealField(300)(32) / RealField(300).pi()
           n = 28
           x_hi = RealField(n)(z)  # convert to f64
           x_mid = RealField(n)(z - RealField(300)(x_hi))
           x_lo = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid))
           print(double_to_hex(x_hi), ",")
           print(double_to_hex(x_mid), ",")
           print(double_to_hex(x_lo), ",")
        */
        const THIRTYTWO_OVER_PI: [u64; 3] =
            [0x40245f306e000000, 0xbe3b1bbeae000000, 0x3c63f84eb0000000];
        let prod = self.x * f64::from_bits(THIRTYTWO_OVER_PI[0]);
        let kd = prod.round();
        let mut y = prod - kd;
        y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[1]), y);
        y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[2]), y);
        (y, kd as i64)
    }

    // Return k and y, where
    // k = round(x * 32 / pi) and y = (x * 32 / pi) - k.
    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    #[inline]
    pub(crate) fn reduce_large(self, exponent: i32) -> (f64, i64) {
        /*
        Generated in SageMath:
        z = RealField(300)(32) / RealField(300).pi()
        n = 53
        x_hi = RealField(n)(z)  # convert to f64
        x_mid = RealField(n)(z - RealField(300)(x_hi))
        x_lo = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid))
        x_lo_2 = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid) - RealField(300)(x_lo))
        x_lo_3 = z - RealField(300)(x_hi) - RealField(300)(x_mid) - RealField(300)(x_lo) - RealField(300)(x_lo_2)
        print(double_to_hex(x_hi), ",")
        print(double_to_hex(x_mid), ",")
        print(double_to_hex(x_lo), ",")
        print(double_to_hex(x_lo_2), ",")
        print(double_to_hex(x_lo_3), ",")
         */
        const THIRTYTWO_OVER_PI: [u64; 5] = [
            0x40245f306dc9c883,
            0xbcc6b01ec5417056,
            0xb966447e493ad4ce,
            0x360e21c820ff28b2,
            0xb29508510ea79237,
        ];

        // 2^45 <= |x| < 2^99
        if exponent < 99 {
            // - When x < 2^99, the full exact product of x * THIRTYTWO_OVER_PI[0]
            // contains at least one integral bit <= 2^5.
            // - When 2^45 <= |x| < 2^55, the lowest 6 unit bits are contained
            // in the last 12 bits of double(x * THIRTYTWO_OVER_PI[0]).
            // - When |x| >= 2^55, the LSB of double(x * THIRTYTWO_OVER_PI[0]) is at
            // least 2^6.
            let mut prod_hi = self.x * f64::from_bits(THIRTYTWO_OVER_PI[0]);
            prod_hi = f64::from_bits(
                prod_hi.to_bits()
                    & (if exponent < 55 {
                        0xfffffffffffff000
                    } else {
                        0xffffffffffffffff
                    }),
            ); // |x| < 2^55
            let k_hi = prod_hi.round();
            let truncated_prod = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[0]), -k_hi);
            let prod_lo = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[1]), truncated_prod);
            let k_lo = prod_lo.round();
            let mut y = f_fmla(
                self.x,
                f64::from_bits(THIRTYTWO_OVER_PI[1]),
                truncated_prod - k_lo,
            );
            y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[2]), y);
            y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[3]), y);

            return (y, k_lo as i64);
        }

        // - When x >= 2^110, the full exact product of x * THIRTYTWO_OVER_PI[0] does
        // not contain any of the lowest 6 unit bits, so we can ignore it completely.
        // - When 2^99 <= |x| < 2^110, the lowest 6 unit bits are contained
        // in the last 12 bits of double(x * THIRTYTWO_OVER_PI[1]).
        // - When |x| >= 2^110, the LSB of double(x * THIRTYTWO_OVER_PI[1]) is at
        // least 64.
        let mut prod_hi = self.x * f64::from_bits(THIRTYTWO_OVER_PI[1]);
        prod_hi = f64::from_bits(
            prod_hi.to_bits()
                & (if exponent < 110 {
                    0xfffffffffffff000
                } else {
                    0xffffffffffffffff
                }),
        ); // |x| < 2^110
        let k_hi = prod_hi.round();
        let truncated_prod = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[1]), -k_hi);
        let prod_lo = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[2]), truncated_prod);
        let k_lo = prod_lo.round();
        let mut y = f_fmla(
            self.x,
            f64::from_bits(THIRTYTWO_OVER_PI[2]),
            truncated_prod - k_lo,
        );
        y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[3]), y);
        y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[4]), y);

        (y, k_lo as i64)
    }

    // Return k and y, where
    // k = round(x * 32 / pi) and y = (x * 32 / pi) - k.
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    #[inline]
    pub(crate) fn reduce_large(self, exponent: i32) -> (f64, i64) {
        // For large range, there are at most 2 parts of THIRTYTWO_OVER_PI_28
        // contributing to the lowest 6 binary digits (k & 63).  If the least
        // significant bit of x * the least significant bit of THIRTYTWO_OVER_PI_28[i]
        // >= 64, we can completely ignore THIRTYTWO_OVER_PI_28[i].

        // Generated by SageMath:
        // z = RealField(300)(32) / RealField(300).pi()
        // n = 28
        // x_hi = RealField(n)(z)  # convert to f64
        // x_mid = RealField(n)(z - RealField(300)(x_hi))
        // x_lo = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid))
        // x_lo_2 = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid) - RealField(300)(x_lo))
        // x_lo_3 = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid) - RealField(300)(x_lo) - RealField(300)(x_lo_2))
        // x_lo_4 = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid) - RealField(300)(x_lo) - RealField(300)(x_lo_2) - RealField(300)(x_lo_3))
        // x_lo_5 = RealField(n)(z - RealField(300)(x_hi) - RealField(300)(x_mid) - RealField(300)(x_lo) - RealField(300)(x_lo_2) - RealField(300)(x_lo_3) - RealField(300)(x_lo_4))
        // x_lo_6 = (z - RealField(300)(x_hi) - RealField(300)(x_mid) - RealField(300)(x_lo) - RealField(300)(x_lo_2) - RealField(300)(x_lo_3) - RealField(300)(x_lo_4) - RealField(300)(x_lo_5))
        //
        //
        // print(double_to_hex(x_hi), ",")
        // print(double_to_hex(x_mid), ",")
        // print(double_to_hex(x_lo), ",")
        // print(double_to_hex(x_lo_2), ",")
        // print(double_to_hex(x_lo_3), ",")
        // print(double_to_hex(x_lo_4), ",")
        // print(double_to_hex(x_lo_5), ",")
        // print(double_to_hex(x_lo_6), ",")
        static THIRTYTWO_OVER_PI: [u64; 8] = [
            0x40245f306e000000,
            0xbe3b1bbeae000000,
            0x3c63f84eb0000000,
            0xba87056592000000,
            0x38bc0db62a000000,
            0xb6e4cd8778000000,
            0xb51bef806c000000,
            0x33363abdebbc561b,
        ];

        let mut idx = 0;
        const FRACTION_LEN: i32 = 23;
        let x_lsb_exp_m4 = exponent - FRACTION_LEN;

        // Exponents of the least significant bits of the corresponding entries in
        // THIRTYTWO_OVER_PI_28.
        static THIRTYTWO_OVER_PI_28_LSB_EXP: [i32; 8] =
            [-24, -55, -81, -114, -143, -170, -200, -230];

        // Skipping the first parts of 32/pi such that:
        //   LSB of x * LSB of THIRTYTWO_OVER_PI_28[i] >= 32.
        while x_lsb_exp_m4 + THIRTYTWO_OVER_PI_28_LSB_EXP[idx] > 5 {
            idx += 1;
        }

        let prod_hi = self.x * f64::from_bits(THIRTYTWO_OVER_PI[idx]);
        // Get the integral part of x * THIRTYTWO_OVER_PI_28[idx]
        let k_hi = prod_hi.round();
        // Get the fractional part of x * THIRTYTWO_OVER_PI_28[idx]
        let frac = prod_hi - k_hi;
        let prod_lo = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[idx + 1]), frac);
        let k_lo = prod_lo.round();

        // Now y is the fractional parts.
        let mut y = prod_lo - k_lo;
        y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[idx + 2]), y);
        y = f_fmla(self.x, f64::from_bits(THIRTYTWO_OVER_PI[idx + 3]), y);

        (y, (k_hi as i64).wrapping_add(k_lo as i64))
    }

    #[inline]
    pub(crate) fn reduce(self) -> (f64, i64) {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        const SMALL_PASS_BOUND: u32 = 0x5600_0000u32;
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        const SMALL_PASS_BOUND: u32 = 0x4a80_0000u32;
        if self.x_abs < SMALL_PASS_BOUND {
            // 2^45
            self.reduce_small()
        } else {
            self.reduce_large(get_exponent_f32(f32::from_bits(self.x_abs)))
        }
    }
}
