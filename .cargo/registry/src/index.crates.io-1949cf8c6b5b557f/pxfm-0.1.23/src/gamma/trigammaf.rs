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
use crate::polyeval::{f_polyeval6, f_polyeval10};
use crate::sin_cosf::fast_sinpif;

#[inline]
fn approx_trigamma(x: f64) -> f64 {
    if x <= 1. {
        // Polynomial for Trigamma[x+1]
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[1, x+1]
        // {err0,approx,err1}=MiniMaxApproximation[f[z],{z,{-0.99999999,0},9,9},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx];
        // den=Denominator[approx];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_polyeval10(
            x,
            f64::from_bits(0x3ffa51a6625307d3),
            f64::from_bits(0x40142fc9c4f02b3f),
            f64::from_bits(0x401b30a762805b44),
            f64::from_bits(0x4014dc84c95656bd),
            f64::from_bits(0x4003e44f4c820b4c),
            f64::from_bits(0x3fe81f37523197d3),
            f64::from_bits(0x3fc22bffe2490221),
            f64::from_bits(0x3f8f221a6329ea36),
            f64::from_bits(0x3f47406930b9563c),
            f64::from_bits(0xbd99cd44c6ad497a),
        );
        let p_den = f_polyeval10(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x40121e3db4e0a2f3),
            f64::from_bits(0x40218e97a5430c4f),
            f64::from_bits(0x402329897737b159),
            f64::from_bits(0x401a0fdc27807c2d),
            f64::from_bits(0x4006ff242e1f3a51),
            f64::from_bits(0x3fea6eda129c4e85),
            f64::from_bits(0x3fc32700b2ae2e88),
            f64::from_bits(0x3f8fdc1dc6116d41),
            f64::from_bits(0x3f4740690261cfbc),
        );
        return p_num / p_den + 1. / (x * x);
    } else if x <= 4. {
        // Polynomial for Trigamma[x+1]
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[1, x+1]
        // {err0,approx,err1}=MiniMaxApproximation[f[z],{z,{0,3},9,9},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx];
        // den=Denominator[approx];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_polyeval10(
            x,
            f64::from_bits(0x3ffa51a6625307d3),
            f64::from_bits(0x4015167fa2d2b5a8),
            f64::from_bits(0x401dd40865d5985e),
            f64::from_bits(0x4018353d9425fb58),
            f64::from_bits(0x4008a12aa45851fa),
            f64::from_bits(0x3ff018736d0c5dbe),
            f64::from_bits(0x3fca715702bdb519),
            f64::from_bits(0x3f9908a9d73d983c),
            f64::from_bits(0x3f54fd9cbbb46314),
            f64::from_bits(0xbd00b8a28c578ab5),
        );
        let p_den = f_polyeval10(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x4012aa7f041a768b),
            f64::from_bits(0x4022c2604e5f9c7a),
            f64::from_bits(0x4025655b63c2db22),
            f64::from_bits(0x401eaa8e59c8295d),
            f64::from_bits(0x400cc8724a58809c),
            f64::from_bits(0x3ff1c7a91c8e3c40),
            f64::from_bits(0x3fcc05613a11183e),
            f64::from_bits(0x3f99b096bd3ce542),
            f64::from_bits(0x3f54fd9cbb9c6167),
        );
        return p_num / p_den + 1. / (x * x);
    } else if x <= 10. {
        // Polynomial for Trigamma[x+1]
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=PolyGamma[1, x+1]
        // {err0,approx,err1}=MiniMaxApproximation[f[z],{z,{3,9},9,9},WorkingPrecision->75,MaxIterations->100]
        // num=Numerator[approx];
        // den=Denominator[approx];
        // poly=num;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let p_num = f_polyeval10(
            x,
            f64::from_bits(0x3ffa51a664b1b211),
            f64::from_bits(0x4016d7f75881312a),
            f64::from_bits(0x4021b10defb47bcc),
            f64::from_bits(0x401fe9633665e1bf),
            f64::from_bits(0x4012601cce6766d7),
            f64::from_bits(0x3ffbd0ece1c435f1),
            f64::from_bits(0x3fdb3fd0e233c485),
            f64::from_bits(0x3faffdedea90b870),
            f64::from_bits(0x3f71a4bbf0d00147),
            f64::from_bits(0xbc0aae286498a357),
        );
        let p_den = f_polyeval10(
            x,
            f64::from_bits(0x3ff0000000000000),
            f64::from_bits(0x4013bbbd604d685d),
            f64::from_bits(0x40253a4fca05438e),
            f64::from_bits(0x402a4aba4634f14f),
            f64::from_bits(0x4024cdd23bd6284a),
            f64::from_bits(0x4015fbe371275c3f),
            f64::from_bits(0x3fff4d7ebf7d1ed0),
            f64::from_bits(0x3fdd459154d7bc72),
            f64::from_bits(0x3fb08c1cd4cedca3),
            f64::from_bits(0x3f71a4bbf0d0012d),
        );
        return p_num / p_den + 1. / (x * x);
    }
    // asymptotic expansion Trigamma[x] = 1/x + 1/x^2 + sum(Bernoulli(2*k)/x^(2*k + 1))
    // Generated in SageMath:
    // var('x')
    // def bernoulli_terms(x, N):
    //     S = 0
    //     for k in range(1, N+1):
    //         B = bernoulli(2*k)
    //         term = B*x**(-(2*k+1))
    //         S += term
    //     return S
    //
    // terms = bernoulli_terms(x, 7)
    // coeffs = [RealField(150)(terms.coefficient(x, n)) for n in range(0, terms.degree(x)+1, 1)]
    // for k in range(0, 14):
    //     c = terms.coefficient(x, -k)  # coefficient of x^(-k)
    //     if c == 0:
    //         continue
    //     print("f64::from_bits(" + double_to_hex(c) + "),")
    let r = 1. / x;
    let r2 = r * r;
    let p = f_polyeval6(
        r2,
        f64::from_bits(0x3fc5555555555555),
        f64::from_bits(0xbfa1111111111111),
        f64::from_bits(0x3f98618618618618),
        f64::from_bits(0xbfa1111111111111),
        f64::from_bits(0x3fb364d9364d9365),
        f64::from_bits(0xbfd0330330330330),
    );
    f_fmla(p, r2 * r, f_fmla(r2, 0.5, r))
}

/// Computes the trigamma function ψ₁(x).
///
/// The trigamma function is the second derivative of the logarithm of the gamma function.
pub fn f_trigammaf(x: f32) -> f32 {
    let xb = x.to_bits();
    if !x.is_normal() {
        if x.is_infinite() {
            return if x.is_sign_negative() {
                f32::NEG_INFINITY
            } else {
                0.
            };
        }
        if x.is_nan() {
            return f32::NAN;
        }
        if xb == 0 {
            return f32::INFINITY;
        }
    }

    let ax = x.to_bits() & 0x7fff_ffff;

    if ax <= 0x34000000u32 {
        // |x| < f32::EPSILON
        let dx = x as f64;
        return (1. / (dx * dx)) as f32;
    }

    let mut dx = x as f64;

    let mut result = 0.;
    let mut sum_parity: f64 = 1.0;

    if x < 0. {
        // singularity at negative integers
        if x.floor() == x {
            return f32::INFINITY;
        }
        // reflection formula
        // Trigamma[1-x] + Trigamma[x] = PI^2 / sinpi^2(x)
        const SQR_PI: f64 = f64::from_bits(0x4023bd3cc9be45de); // pi^2
        let sinpi_ax = fast_sinpif(-x);
        dx = 1. - dx;
        result = SQR_PI / (sinpi_ax * sinpi_ax);
        sum_parity = -1.;
    }

    let r = approx_trigamma(dx) * sum_parity;
    result += r;
    result as f32
}

#[cfg(test)]
mod tests {
    use crate::f_trigammaf;

    #[test]
    fn test_trigamma() {
        assert_eq!(f_trigammaf(-27.058018), 300.35904);
        assert_eq!(f_trigammaf(27.058018), 0.037648965);
        assert_eq!(f_trigammaf(8.058018), 0.13211797);
        assert_eq!(f_trigammaf(-8.058018), 300.27863);
        assert_eq!(f_trigammaf(2.23432), 0.56213206);
        assert_eq!(f_trigammaf(-2.4653), 9.653673);
        assert_eq!(f_trigammaf(0.123541), 66.911285);
        assert_eq!(f_trigammaf(-0.54331), 9.154416);
        assert_eq!(f_trigammaf(-5.), f32::INFINITY);
        assert_eq!(f_trigammaf(f32::INFINITY), 0.0);
        assert_eq!(f_trigammaf(f32::NEG_INFINITY), f32::NEG_INFINITY);
        assert!(f_trigammaf(f32::NAN).is_nan());
    }
}
