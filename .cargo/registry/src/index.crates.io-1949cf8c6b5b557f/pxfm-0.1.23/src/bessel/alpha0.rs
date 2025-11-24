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
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};
use crate::polyeval::f_polyeval9;

//
/// See [bessel_0_asympt_alpha] for the info
pub(crate) fn bessel_0_asympt_alpha_hard(reciprocal: DyadicFloat128) -> DyadicFloat128 {
    static C: [DyadicFloat128; 18] = [
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0x80000000_00000000_00000000_00000000_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -131,
            mantissa: 0x85555555_55555555_55555555_55555555_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -130,
            mantissa: 0xd6999999_99999999_99999999_9999999a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -127,
            mantissa: 0xd1ac2492_49249249_24924924_92492492_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -123,
            mantissa: 0xbbcd0fc7_1c71c71c_71c71c71_c71c71c7_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -118,
            mantissa: 0x85e8fe45_8ba2e8ba_2e8ba2e8_ba2e8ba3_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -113,
            mantissa: 0x8b5a8f33_63c4ec4e_c4ec4ec4_ec4ec4ec_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -108,
            mantissa: 0xc7661d79_9d59b555_55555555_55555555_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -102,
            mantissa: 0xbbced715_c2897a28_78787878_78787878_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -96,
            mantissa: 0xe14b19b4_aae3f7fe_be1af286_bca1af28_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -89,
            mantissa: 0xa7af7341_db2192db_975e0c30_c30c30c3_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -82,
            mantissa: 0x97a8f676_b349f6fc_5cefd338_590b2164_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -75,
            mantissa: 0xa3d299fb_6f304d73_86e15f12_0fd70a3d_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -68,
            mantissa: 0xd050b737_cbc044ef_e8807e3c_87f43da1_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -60,
            mantissa: 0x9a02379b_daa7e492_854f42de_6d3dffe6_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -52,
            mantissa: 0x83011a39_380e467d_de6b70ec_b92ce0cc_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Pos,
            exponent: -45,
            mantissa: 0xfe16521f_c79e5d9a_a5bed653_e3844e9a_u128,
        },
        DyadicFloat128 {
            sign: DyadicSign::Neg,
            exponent: -36,
            mantissa: 0x8b54b13d_3fb3e1c4_15dbb880_0bb32218_u128,
        },
    ];

    let x2 = reciprocal * reciprocal;

    let mut p = C[17];
    for i in (0..17).rev() {
        p = x2 * p + C[i];
    }

    p * reciprocal
}

/**
Note expansion generation below: this is negative series expressed in Sage as positive,
so before any real evaluation `x=1/x` should be applied.

Generated by SageMath:
```python
def binomial_like(n, m):
    prod = QQ(1)
    z = QQ(4)*(n**2)
    for k in range(1,m + 1):
        prod *= (z - (2*k - 1)**2)
    return prod / (QQ(2)**(2*m) * (ZZ(m).factorial()))

R = LaurentSeriesRing(RealField(300), 'x',default_prec=300)
x = R.gen()

def Pn_asymptotic(n, y, terms=10):
    # now y = 1/x
    return sum( (-1)**m * binomial_like(n, 2*m) / (QQ(2)**(2*m)) * y**(QQ(2)*m) for m in range(terms) )

def Qn_asymptotic(n, y, terms=10):
    return sum( (-1)**m * binomial_like(n, 2*m + 1) / (QQ(2)**(2*m + 1)) * y**(QQ(2)*m + 1) for m in range(terms) )

P = Pn_asymptotic(0, x, 50)
Q = Qn_asymptotic(0, x, 50)

R_series = (-Q/P)

# alpha is atan(R_series) so we're doing Taylor series atan expansion on R_series

arctan_series_Z = sum([QQ(-1)**k * x**(QQ(2)*k+1) / RealField(700)(RealField(700)(2)*k+1) for k in range(25)])
alpha_series = arctan_series_Z(R_series)

# see the series
print(alpha_series)
```
**/
#[inline]
pub(crate) fn bessel_0_asympt_alpha(recip: DoubleDouble) -> DoubleDouble {
    const C: [(u64, u64); 12] = [
        (0x0000000000000000, 0x3fc0000000000000),
        (0x3c55555555555555, 0xbfb0aaaaaaaaaaab),
        (0x3c5999999999999a, 0x3fcad33333333333),
        (0xbc92492492492492, 0xbffa358492492492),
        (0xbcbc71c71c71c71c, 0x403779a1f8e38e39),
        (0xbd0745d1745d1746, 0xc080bd1fc8b1745d),
        (0xbd7d89d89d89d89e, 0x40d16b51e66c789e),
        (0x3dc5555555555555, 0xc128ecc3af33ab37),
        (0x3e2143c3c3c3c3c4, 0x418779dae2b8512f),
        (0x3df41e50d79435e5, 0xc1ec296336955c7f),
        (0x3ef6dcbaf0618618, 0x4254f5ee683b6432),
        (0x3f503a3102cc7a6f, 0xc2c2f51eced6693f),
    ];

    // Doing (1/x)*(1/x) instead (1/(x*x)) to avoid spurious overflow/underflow
    let x2 = DoubleDouble::quick_mult(recip, recip);

    let mut p = DoubleDouble::mul_add(
        x2,
        DoubleDouble::from_bit_pair(C[11]),
        DoubleDouble::from_bit_pair(C[10]),
    );

    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[9]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[8]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[7]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[6]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[5]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[4]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[3]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[2]));
    p = DoubleDouble::mul_add(x2, p, DoubleDouble::from_bit_pair(C[1]));
    p = DoubleDouble::mul_add_f64(x2, p, f64::from_bits(C[0].1));

    let z = DoubleDouble::quick_mult(p, recip);

    DoubleDouble::from_exact_add(z.hi, z.lo)
}

/**
Note expansion generation below: this is negative series expressed in Sage as positive,
so before any real evaluation `x=1/x` should be applied.

Generated by SageMath:
```python
def binomial_like(n, m):
    prod = QQ(1)
    z = QQ(4)*(n**2)
    for k in range(1,m + 1):
        prod *= (z - (2*k - 1)**2)
    return prod / (QQ(2)**(2*m) * (ZZ(m).factorial()))

R = LaurentSeriesRing(RealField(300), 'x',default_prec=300)
x = R.gen()

def Pn_asymptotic(n, y, terms=10):
    # now y = 1/x
    return sum( (-1)**m * binomial_like(n, 2*m) / (QQ(2)**(2*m)) * y**(QQ(2)*m) for m in range(terms) )

def Qn_asymptotic(n, y, terms=10):
    return sum( (-1)**m * binomial_like(n, 2*m + 1) / (QQ(2)**(2*m + 1)) * y**(QQ(2)*m + 1) for m in range(terms) )

P = Pn_asymptotic(0, x, 50)
Q = Qn_asymptotic(0, x, 50)

R_series = (-Q/P)

# alpha is atan(R_series) so we're doing Taylor series atan expansion on R_series

arctan_series_Z = sum([QQ(-1)**k * x**(QQ(2)*k+1) / RealField(700)(RealField(700)(2)*k+1) for k in range(25)])
alpha_series = arctan_series_Z(R_series)

# see the series
print(alpha_series)
```
**/
#[inline]
pub(crate) fn bessel_0_asympt_alpha_fast(recip: DoubleDouble) -> DoubleDouble {
    const C: [u64; 12] = [
        0x3fc0000000000000,
        0xbfb0aaaaaaaaaaab,
        0x3fcad33333333333,
        0xbffa358492492492,
        0x403779a1f8e38e39,
        0xc080bd1fc8b1745d,
        0x40d16b51e66c789e,
        0xc128ecc3af33ab37,
        0x418779dae2b8512f,
        0xc1ec296336955c7f,
        0x4254f5ee683b6432,
        0xc2c2f51eced6693f,
    ];

    // Doing (1/x)*(1/x) instead (1/(x*x)) to avoid spurious overflow/underflow
    let x2 = DoubleDouble::quick_mult(recip, recip);

    let p = f_polyeval9(
        x2.hi,
        f64::from_bits(C[3]),
        f64::from_bits(C[4]),
        f64::from_bits(C[5]),
        f64::from_bits(C[6]),
        f64::from_bits(C[7]),
        f64::from_bits(C[8]),
        f64::from_bits(C[9]),
        f64::from_bits(C[10]),
        f64::from_bits(C[11]),
    );

    let mut z = DoubleDouble::mul_f64_add_f64(x2, p, f64::from_bits(C[2]));
    z = DoubleDouble::mul_add_f64(x2, z, f64::from_bits(C[1]));
    z = DoubleDouble::mul_add_f64(x2, z, f64::from_bits(C[0]));
    DoubleDouble::quick_mult(z, recip)
}
