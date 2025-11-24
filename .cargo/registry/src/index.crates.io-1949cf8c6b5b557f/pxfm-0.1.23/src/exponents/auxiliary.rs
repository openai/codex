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
use crate::dyadic_float::{DyadicSign, f64_from_parts};

#[inline]
pub(crate) fn ldexp(d: f64, i: i32) -> f64 {
    let mut n = i;
    let exp_max = 1023;
    let exp_min = -1022;

    const EXP_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;
    // 2 ^ Emax, maximum positive with null significand (0x1p1023 for f64)
    let f_exp_max = f64_from_parts(DyadicSign::Pos, EXP_BIAS << 1, 0);

    // 2 ^ Emin, minimum positive normal with null significand (0x1p-1022 for f64)
    let f_exp_min = f64_from_parts(DyadicSign::Pos, 1, 0);

    let mut x = d;

    if n < exp_min {
        // 2 ^ sig_total_bits, moltiplier to normalize subnormals (0x1p53 for f64)
        let f_pow_subnorm = f64_from_parts(DyadicSign::Pos, 52 + EXP_BIAS, 0);

        let mul = f_exp_min * f_pow_subnorm;
        let add = -exp_min - 52i32;

        // Worse case negative `n`: `x`  is the maximum positive value, the result is `F::MIN`.
        // This must be reachable by three scaling multiplications (two here and one final).
        debug_assert!(-exp_min + 52i32 + exp_max <= add * 2 + -exp_min);

        x *= mul;
        n += add;

        if n < exp_min {
            x *= mul;
            n += add;

            if n < exp_min {
                n = exp_min;
            }
        }
    } else if n > exp_max {
        x *= f_exp_max;
        n -= exp_max;
        if n > exp_max {
            x *= f_exp_max;
            n -= exp_max;
            if n > exp_max {
                n = exp_max;
            }
        }
    }

    let scale = f64_from_parts(DyadicSign::Pos, (EXP_BIAS as i32 + n) as u64, 0);
    x * scale
}

#[inline]
pub(crate) fn fast_ldexp(d: f64, i: i32) -> f64 {
    let mut u = d.to_bits();
    u = u.wrapping_add((i as u64).wrapping_shl(52));
    f64::from_bits(u)
}
