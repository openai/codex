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
use crate::common::f_fmla;
use crate::logs::simple_fast_log;
use crate::polyeval::{
    f_estrin_polyeval7, f_estrin_polyeval8, f_estrin_polyeval9, f_polyeval4, f_polyeval6,
    f_polyeval10,
};
use crate::tangent::cotpif_core;

#[inline]
fn approx_digamma(x: f64) -> f64 {
    if x <= 1. {
        // Generated in Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[x + 1]
        // {err0,approx, err1}=MiniMaxApproximation[f[z],{z,{-0.99999999,0},9,8},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx];
        // den=Denominator[approx];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_polyeval10(
            x,
            f64::from_bits(0xbfe2788cfc6fb619),
            f64::from_bits(0x3fca347925788707),
            f64::from_bits(0x3ff887e0f068df69),
            f64::from_bits(0x3ff543446198b4d2),
            f64::from_bits(0x3fe03e4455fbad95),
            f64::from_bits(0x3fb994be8389e4f6),
            f64::from_bits(0x3f84eb98b830c9b1),
            f64::from_bits(0x3f4025193ac4ad97),
            f64::from_bits(0x3ee18c1d683d866a),
            f64::from_bits(0x3e457cb5b4a07c95),
        );
        let p_den = f_estrin_polyeval9(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x4003f5f42d95aca8),
            f64::from_bits(0x4002f96e541d0513),
            f64::from_bits(0x3ff22c34843313fa),
            f64::from_bits(0x3fd33574180109bf),
            f64::from_bits(0x3fa6c07b99ebb277),
            f64::from_bits(0x3f6cdd7b8fa68926),
            f64::from_bits(0x3f212b74d39e287f),
            f64::from_bits(0x3ebabd247f366583),
        );
        return p_num / p_den - 1. / x;
    } else if x < 1.461632144 {
        // exception
        if x == 1.4616321325302124 {
            return -1.2036052e-8;
        }
        // Generated in Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[x+1]
        // {err0,approx,err1}=MiniMaxApproximation[f[z],{z,{0,0.461632144},8,8},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx];
        // den=Denominator[approx];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_estrin_polyeval9(
            x,
            f64::from_bits(0xbfe2788cfc6fb619),
            f64::from_bits(0x3fd0ad221e8c3b8b),
            f64::from_bits(0x3ff813be4dee2e90),
            f64::from_bits(0x3ff2f64cbfa7d1a4),
            f64::from_bits(0x3fd9a8c4798f426c),
            f64::from_bits(0x3fb111a34898f6bf),
            f64::from_bits(0x3f75dd3efac1e579),
            f64::from_bits(0x3f272596b2582f0d),
            f64::from_bits(0x3eb9b074f4ca6263),
        );
        let p_den = f_estrin_polyeval9(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x40032fd3a1fe3a25),
            f64::from_bits(0x40012969bcd7fef3),
            f64::from_bits(0x3fee1a267ee7a97a),
            f64::from_bits(0x3fcc1522178a69a6),
            f64::from_bits(0x3f9bd89421334af0),
            f64::from_bits(0x3f5b40bc3203df4c),
            f64::from_bits(0x3f05ac6be0b79fac),
            f64::from_bits(0x3e9047c3d8071f18),
        );
        return p_num / p_den - 1. / x;
    } else if x <= 2. {
        // Generated in Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[x+1]
        // {err0,approx,err1}=MiniMaxApproximation[f[z],{z,{0.461632144,1},7,6},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx];
        // den=Denominator[approx];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_estrin_polyeval8(
            x,
            f64::from_bits(0xbfe2788cfc6fb613),
            f64::from_bits(0x3fd690553caa1c6b),
            f64::from_bits(0x3ff721cf4d9e008f),
            f64::from_bits(0x3fee9f4096f34b09),
            f64::from_bits(0x3fd055e88830fc71),
            f64::from_bits(0x3f9e66bceee16960),
            f64::from_bits(0x3f55d436778b3403),
            f64::from_bits(0x3eeef6bc214819b3),
        );
        let p_den = f_estrin_polyeval8(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x4001e96eaab05729),
            f64::from_bits(0x3ffcb1aa289077da),
            f64::from_bits(0x3fe5499e89b757b6),
            f64::from_bits(0x3fbee531a912bca9),
            f64::from_bits(0x3f84d46f2121ceb7),
            f64::from_bits(0x3f35abd7eb7440e6),
            f64::from_bits(0x3ec43bf7c110aad1),
        );
        return p_num / p_den - 1. / x;
    } else if x <= 3. {
        // Generated in Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[x+1]
        // {err0,approx}=MiniMaxApproximation[f[z],{z,{1,2},7,6},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_estrin_polyeval8(
            x,
            f64::from_bits(0xbfe2788cfc63695f),
            f64::from_bits(0x3fdb63791eb688ea),
            f64::from_bits(0x3ff625ed84968583),
            f64::from_bits(0x3fe900ea36e59e02),
            f64::from_bits(0x3fc5319409f4fec6),
            f64::from_bits(0x3f8b6e7cacff2a59),
            f64::from_bits(0x3f34a7e591bf2af3),
            f64::from_bits(0x3e9c323866d138db),
        );
        let p_den = f_estrin_polyeval7(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x4000ddf448e34181),
            f64::from_bits(0x3ff87188f2414f79),
            f64::from_bits(0x3fdeff74d18f811a),
            f64::from_bits(0x3fb1a0cddeb3a320),
            f64::from_bits(0x3f701050c1344800),
            f64::from_bits(0x3f10480a4ea8cf57),
        );
        return p_num / p_den - 1. / x;
    } else if x <= 8. {
        // Generated in Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[x + 1]
        // {err0,approx}=MiniMaxApproximation[f[z],{z,{2,7},7,7},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_estrin_polyeval8(
            x,
            f64::from_bits(0xbfe2788c3725fd5e),
            f64::from_bits(0x3fde39f54e5a651a),
            f64::from_bits(0x3ff56983b839b94f),
            f64::from_bits(0x3fe60d6118d8fc08),
            f64::from_bits(0x3fc0e889ace69a30),
            f64::from_bits(0x3f844e10e399bd93),
            f64::from_bits(0x3f3099741afda7cb),
            f64::from_bits(0x3eb74a15144af8e9),
        );
        let p_den = f_estrin_polyeval8(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x4000409ed08c0553),
            f64::from_bits(0x3ff63746cb6183e3),
            f64::from_bits(0x3fda1196b1a75351),
            f64::from_bits(0x3fab4ba9fad2d187),
            f64::from_bits(0x3f67de6e6987e3a3),
            f64::from_bits(0x3f0c9d85ca31182e),
            f64::from_bits(0x3e8b269f154c8f12),
        );
        return p_num / p_den - 1. / x;
    } else if x <= 15. {
        // Generated in Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[x + 1]
        // {err0,approx}=MiniMaxApproximation[f[z],{z,{7,14},7,7},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_estrin_polyeval8(
            x,
            f64::from_bits(0xbfe272e75f131710),
            f64::from_bits(0x3fe53fce507de081),
            f64::from_bits(0x3ff182957d4f961a),
            f64::from_bits(0x3fd6d1652dea00e9),
            f64::from_bits(0x3fa45e16488abe0f),
            f64::from_bits(0x3f5a52a8f3f3663f),
            f64::from_bits(0x3ef5b767554d208e),
            f64::from_bits(0x3e6d2393b100353d),
        );
        let p_den = f_estrin_polyeval8(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x3ffb1f295c6e5fc5),
            f64::from_bits(0x3feb88eb913eb117),
            f64::from_bits(0x3fc570f3aed83ff7),
            f64::from_bits(0x3f8afe819fdfa5a5),
            f64::from_bits(0x3f3a2cec9041f361),
            f64::from_bits(0x3ed0549335964bb9),
            f64::from_bits(0x3e3ebdcb0002d63e),
        );
        return p_num / p_den - 1. / x;
    }
    // digamma asymptotic expansion
    // digamma(x) ~ ln(z)+1/(2z)-sum_(n=1)^(infty)(Bernoulli(2n))/(2nz^(2n))
    // Generated in SageMath:
    // var('x')
    //
    // def bernoulli_terms(x, N):
    //     S = 0
    //     S += QQ(1)/QQ(2)/x
    //     for k in range(1, N+1):
    //         B = bernoulli(2*k)
    //         term = (B / QQ(2*k))*x**(-2*k)
    //         S += term
    //     return S
    //
    // terms = bernoulli_terms(x, 5)
    //
    // coeffs = [RealField(150)(terms.coefficient(x, n)) for n in range(0, terms.degree(x)+1, 1)]
    // for k in range(1, 13):
    //     c = terms.coefficient(x, -k)  # coefficient of x^(-k)
    //     if c == 0:
    //         continue
    //     print("f64::from_bits(" + double_to_hex(c) + "),")
    let rcp = 1. / x;
    let rcp_sqr = rcp * rcp;
    let p = f_polyeval6(
        rcp_sqr,
        f64::from_bits(0x3fb5555555555555),
        f64::from_bits(0xbf81111111111111),
        f64::from_bits(0x3f70410410410410),
        f64::from_bits(0xbf71111111111111),
        f64::from_bits(0x3f7f07c1f07c1f08),
        f64::from_bits(0xbf95995995995996),
    );
    let v_log = simple_fast_log(x);
    v_log - f_fmla(rcp, f64::from_bits(0x3fe0000000000000), p * rcp_sqr)
}

/// Computes digamma(x)
pub fn f_digammaf(x: f32) -> f32 {
    let xb = x.to_bits();
    if (xb & 0x0007_ffff) == 0 {
        if x.is_infinite() {
            return if x.is_sign_negative() {
                f32::NAN
            } else {
                f32::INFINITY
            };
        }
        if x.is_nan() {
            return f32::NAN;
        }
        if xb == 0 {
            return 1. / x;
        }
    }

    let ax = x.to_bits() & 0x7fff_ffff;

    if ax <= 0x32abcc77u32 {
        // |x| < 2e-8
        // digamma near where x -> 1 ~ Digamma[x] = -euler + O(x)
        // considering identity Digamma[x+1] = Digamma[x] + 1/x,
        // hence x < ulp(1) then x+1 ~= 1 that gives
        // Digamma[x] = Digamma[x+1] - 1/x = -euler - 1/x
        const EULER: f64 = f64::from_bits(0x3fe2788cfc6fb619);
        return (-EULER - 1. / x as f64) as f32;
    } else if ax <= 0x377ba882u32 {
        // |x| <= 0.000015
        // Laurent series of digamma(x)
        // Generated by SageMath:
        // from mpmath import mp
        // mp.prec = 150
        // R = RealField(150)
        // var('x')
        // def laurent_terms(x, N):
        //     S = 0
        //     S += -1/x - R(mp.euler)
        //     S1 = 0
        //     for k in range(1, N+1):
        //         zet = R(mp.zeta(k + 1))
        //         term = zet*(-x)**k
        //         S1 += term
        //     return S - S1
        //
        // terms = laurent_terms(x, 4)
        //
        // coeffs = [RealField(150)(terms.coefficient(x, n)) for n in range(0, terms.degree(x)+1, 1)]
        // for k in range(1, 13):
        //     c = terms.coefficient(x, k)  # coefficient of x^(-k)
        //     if c == 0:
        //         continue
        //     print("f64::from_bits(" + double_to_hex(c) + "),")
        const EULER: f64 = f64::from_bits(0x3fe2788cfc6fb619);
        let dx = x as f64;
        let start = -1. / dx;
        let neg_dx = -dx;
        let z = f_polyeval4(
            neg_dx,
            f64::from_bits(0x3ffa51a6625307d3),
            f64::from_bits(0xbff33ba004f00621),
            f64::from_bits(0x3ff151322ac7d848),
            f64::from_bits(0xbff097418eca7cce),
        );
        let r = f_fmla(z, dx, -EULER) + start;
        return r as f32;
    }

    let mut dx = x as f64;
    let mut result: f64 = 0.;
    if x < 0. {
        // at negative integers it's inf
        if x.floor() == x {
            return f32::NAN;
        }

        // reflection Gamma(1-x) + Gamma(x) = Pi/tan(PI*x)
        const PI: f64 = f64::from_bits(0x400921fb54442d18);
        let cot_x_angle = -dx;
        dx = 1. - dx;
        result = PI * cotpif_core(cot_x_angle);
    }
    let approx = approx_digamma(dx);
    result += approx;
    result as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digamma() {
        assert_eq!(f_digammaf(-13.999000000012591), -996.9182);
        assert_eq!(f_digammaf(15.3796425), 2.700182);
        assert_eq!(f_digammaf(0.0005187988), -1928.1058);
        assert_eq!(f_digammaf(0.0019531252), -512.574);
        assert_eq!(f_digammaf(-96.353516), 6.1304626);
        assert_eq!(f_digammaf(-31.06964), 17.582127);
        assert_eq!(f_digammaf(-0.000000000000001191123), 839543830000000.);
        assert_eq!(f_digammaf(f32::INFINITY), f32::INFINITY);
        assert!(f_digammaf(f32::NEG_INFINITY).is_nan());
    }
}
