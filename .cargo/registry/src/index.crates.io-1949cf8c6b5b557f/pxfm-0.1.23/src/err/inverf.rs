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
use crate::double_double::DoubleDouble;
use crate::logs::fast_log_dd;
use crate::polyeval::{f_polyeval4, f_polyeval5};

#[cold]
fn inverf_0p06_to_0p75(x: f64) -> f64 {
    // First step rational approximant is generated, but it's ill-conditioned, thus
    // we're using taylor expansion to create Newton form at the point.
    // Generated in Wolfram Mathematica:
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=InverseErf[x]/x
    // g[x_] =f[Sqrt[x]];
    // {err0,approx}=MiniMaxApproximation[g[z],{z,{0.06,0.75},9,9},WorkingPrecision->75, MaxIterations->100]
    // num=Numerator[approx][[1]];
    // den=Denominator[approx][[1]];
    // poly=den;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    // x0=SetPrecision[0.5625,75];
    // NumberForm[Series[num[x],{x,x0,50}], ExponentFunction->(Null&)]
    // coeffs=Table[SeriesCoefficient[num[x],{x,x0,k}],{k,0,9}];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]];
    const P: [(u64, u64); 10] = [
        (0xbc3e06eda42202a0, 0x3f93c2fc5d00e0c8),
        (0xbc6eb374406b33b4, 0xbfc76fcfd022e3ff),
        (0xbc857822d7ffd282, 0x3fe6f8443546010a),
        (0x3c68269c66dfb28a, 0xbff80996754ceb79),
        (0x3c543dce8990a9f9, 0x3ffcf778d5ef0504),
        (0xbc72fc55f73765f6, 0xbff433be821423d0),
        (0xbc66d05fb37c8592, 0x3fdf15f19e9d8da4),
        (0x3c56dfb85e83a2c5, 0xbfb770b6827e0829),
        (0x3bff1472ecdfa403, 0x3f7a98a2980282bb),
        (0x3baffb33d69d6276, 0xbf142a246fd2c07c),
    ];
    let x2 = DoubleDouble::from_exact_mult(x, x);
    let vz = DoubleDouble::full_add_f64(x2, -0.5625);

    let vx2 = vz * vz;
    let vx4 = vx2 * vx2;
    let vx8 = vx4 * vx4;

    let p0 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(P[1]),
        DoubleDouble::from_bit_pair(P[0]),
    );
    let p1 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(P[3]),
        DoubleDouble::from_bit_pair(P[2]),
    );
    let p2 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(P[5]),
        DoubleDouble::from_bit_pair(P[4]),
    );
    let p3 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(P[7]),
        DoubleDouble::from_bit_pair(P[6]),
    );
    let p4 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(P[9]),
        DoubleDouble::from_bit_pair(P[8]),
    );

    let q0 = DoubleDouble::mul_add(vx2, p1, p0);
    let q1 = DoubleDouble::mul_add(vx2, p3, p2);

    let r0 = DoubleDouble::mul_add(vx4, q1, q0);
    let num = DoubleDouble::mul_add(vx8, p4, r0);
    // Generated in Wolfram Mathematica:
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=InverseErf[x]/x
    // g[x_] =f[Sqrt[x]];
    // {err0,approx}=MiniMaxApproximation[g[z],{z,{0.06,0.75},9,9},WorkingPrecision->75, MaxIterations->100]
    // num=Numerator[approx][[1]];
    // den=Denominator[approx][[1]];
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    // x0=SetPrecision[0.5625,75];
    // NumberForm[Series[den[x],{x,x0,50}], ExponentFunction->(Null&)]
    // coeffs=Table[SeriesCoefficient[den[x],{x,x0,k}],{k,0,9}];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]];
    const Q: [(u64, u64); 10] = [
        (0xbc36337f24e57cb9, 0x3f92388d5d757e3a),
        (0xbc63dfae43d60e0b, 0xbfc6ca7da581358c),
        (0xbc77656389bd0e62, 0x3fe7c82ce417b4e0),
        (0xbc93679667bef2f0, 0xbffad58651fd1a51),
        (0x3ca2c6cb9eb17fb4, 0x4001bdb67e93a242),
        (0xbc9b58961ba253bc, 0xbffbdaeff6fbb81c),
        (0x3c7861f549c6aa61, 0x3fe91b12cf47da3a),
        (0xbc696dfd665b2f5e, 0xbfc7c5d0ffb7f1da),
        (0x3c1552b0ec0ba7b3, 0x3f939ada247f7609),
        (0xbbcaa226fb7b30a8, 0xbf41be65038ccfe6),
    ];

    let p0 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(Q[1]),
        DoubleDouble::from_bit_pair(Q[0]),
    );
    let p1 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(Q[3]),
        DoubleDouble::from_bit_pair(Q[2]),
    );
    let p2 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(Q[5]),
        DoubleDouble::from_bit_pair(Q[4]),
    );
    let p3 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(Q[7]),
        DoubleDouble::from_bit_pair(Q[6]),
    );
    let p4 = DoubleDouble::mul_add(
        vz,
        DoubleDouble::from_bit_pair(Q[9]),
        DoubleDouble::from_bit_pair(Q[8]),
    );

    let q0 = DoubleDouble::mul_add(vx2, p1, p0);
    let q1 = DoubleDouble::mul_add(vx2, p3, p2);

    let r0 = DoubleDouble::mul_add(vx4, q1, q0);
    let den = DoubleDouble::mul_add(vx8, p4, r0);

    let r = DoubleDouble::div(num, den);
    let k = DoubleDouble::quick_mult_f64(r, x);
    k.to_f64()
}

#[inline]
fn inverf_asympt_small(z: DoubleDouble, zeta_sqrt: DoubleDouble, x: f64) -> f64 {
    // Generated in Wolfram Mathematica:
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=InverseErf[Exp[-1/(x^2)]*(-1+Exp[1/(x^2)])]/(Sqrt[-Log[1-(Exp[-1/(x^2)]*(-1+Exp[1/(x^2)]))]] )
    // {err0, approx,err1}=MiniMaxApproximation[f[z],{z,{0.2,0.9999999},10,10},WorkingPrecision->90]
    // num=Numerator[approx];
    // den=Denominator[approx];
    // poly=num;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const P: [(u64, u64); 11] = [
        (0x3c936555853a8b2c, 0x3ff0001df06a2515),
        (0x3cea488e802db3c3, 0x404406ba373221da),
        (0xbce27d42419754e3, 0x407b0442e38a9597),
        (0xbd224a407624cbdf, 0x409c9277e31ef446),
        (0x3d4f16ce65d6fea0, 0x40aec3ec005b1d8a),
        (0x3d105bc37bc61b58, 0x40b46be8f860f4d9),
        (0x3d5ca133dcdecaa0, 0x40b3826e6a32dad7),
        (0x3d1d52013ba8aa38, 0x40aae93a603cf3ea),
        (0xbd07a75306df0fc3, 0x4098ab8357dc2e51),
        (0x3d1bb6770bb7a27e, 0x407ebead00879010),
        (0xbbfcbff4a9737936, 0x3f8936117ccbff83),
    ];

    let z2 = DoubleDouble::quick_mult(z, z);
    let z4 = DoubleDouble::quick_mult(z2, z2);
    let z8 = DoubleDouble::quick_mult(z4, z4);

    let q0 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(P[1]),
        z,
        DoubleDouble::from_bit_pair(P[0]),
    );
    let q1 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(P[3]),
        z,
        DoubleDouble::from_bit_pair(P[2]),
    );
    let q2 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(P[5]),
        z,
        DoubleDouble::from_bit_pair(P[4]),
    );
    let q3 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(P[7]),
        z,
        DoubleDouble::from_bit_pair(P[6]),
    );
    let q4 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(P[9]),
        z,
        DoubleDouble::from_bit_pair(P[8]),
    );

    let r0 = DoubleDouble::mul_add(z2, q1, q0);
    let r1 = DoubleDouble::mul_add(z2, q3, q2);

    let s0 = DoubleDouble::mul_add(z4, r1, r0);
    let s1 = DoubleDouble::mul_add(z2, DoubleDouble::from_bit_pair(P[10]), q4);
    let num = DoubleDouble::mul_add(z8, s1, s0);

    // See numerator generation above:
    // poly=den;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const Q: [(u64, u64); 11] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0xbc75b1109d4a3262, 0x40440782efaab17f),
        (0x3d1f7775b207d84f, 0x407b2da74b0d39f2),
        (0xbd3291fdbab49501, 0x409dac8d9e7c90b2),
        (0xbd58d8fdd27707a9, 0x40b178dfeffa3192),
        (0xbd57fc74ad705ce0, 0x40bad19b686f219f),
        (0x3d4075510031f2cd, 0x40be70a598208cea),
        (0xbd5442e109152efb, 0x40b9683ef36ae330),
        (0x3d5398192933962e, 0x40b04b7c4c3ca8ee),
        (0x3d2d04d03598e303, 0x409bd0080799fbf1),
        (0x3d2a988eb552ef44, 0x40815a46f12bafe3),
    ];

    let q0 = DoubleDouble::mul_add_f64(
        DoubleDouble::from_bit_pair(Q[1]),
        z,
        f64::from_bits(0x3ff0000000000000),
    );
    let q1 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(Q[3]),
        z,
        DoubleDouble::from_bit_pair(Q[2]),
    );
    let q2 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(Q[5]),
        z,
        DoubleDouble::from_bit_pair(Q[4]),
    );
    let q3 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(Q[7]),
        z,
        DoubleDouble::from_bit_pair(Q[6]),
    );
    let q4 = DoubleDouble::mul_add(
        DoubleDouble::from_bit_pair(Q[9]),
        z,
        DoubleDouble::from_bit_pair(Q[8]),
    );

    let r0 = DoubleDouble::mul_add(z2, q1, q0);
    let r1 = DoubleDouble::mul_add(z2, q3, q2);

    let s0 = DoubleDouble::mul_add(z4, r1, r0);
    let s1 = DoubleDouble::mul_add(z2, DoubleDouble::from_bit_pair(Q[10]), q4);
    let den = DoubleDouble::mul_add(z8, s1, s0);
    let r = DoubleDouble::div(num, den);
    let k = DoubleDouble::quick_mult(r, zeta_sqrt);
    f64::copysign(k.to_f64(), x)
}

// branch for |x| > 0.9999 for extreme tail
#[cold]
fn inverf_asympt_long(z: DoubleDouble, zeta_sqrt: DoubleDouble, x: f64) -> f64 {
    // First step rational approximant is generated, but it's ill-conditioned, thus
    // we're using taylor expansion to create Newton form at the point.
    // Generated in Wolfram Mathematica:
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=InverseErf[Exp[-1/(x^2)]*(-1+Exp[1/(x^2)])]/(Sqrt[-Log[1-(Exp[-1/(x^2)]*(-1+Exp[1/(x^2)]))]] )
    // {err0, approx}=MiniMaxApproximation[f[z],{z,{0.2,0.9999999},13,13},WorkingPrecision->90]
    // num=Numerator[approx][[1]];
    // den=Denominator[approx][[1]];
    // poly=num;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const P: [(u64, u64); 14] = [
        (0x3c97612f9b24a614, 0x3ff0000ba84cc7a5),
        (0xbcee8fe2da463412, 0x40515246546f5d88),
        (0x3d2fa4a2b891b526, 0x40956b6837159b11),
        (0x3d5d673ffad4f817, 0x40c5a1aa3be58652),
        (0x3d8867a1e5506f88, 0x40e65ebb1e1e7c75),
        (0xbd9bbc0764ed8f5b, 0x40fd2064a652e5c2),
        (0xbda78e569c0d237f, 0x410a385c627c461c),
        (0xbdab3123ebc465d7, 0x4110f05ca2b65fe5),
        (0x3d960def35955192, 0x4110bb079af2fe08),
        (0xbd97904816054836, 0x410911c24610c11c),
        (0xbd937745e9192593, 0x40fc603244adca35),
        (0xbd65fbc476d63050, 0x40e6399103188c21),
        (0xbd61016ef381cce6, 0x40c6482b44995b89),
        (0x3c326105c49e5a1a, 0xbfab44bd8b4e3138),
    ];

    let z2 = z * z;
    let z4 = z2 * z2;
    let z8 = z4 * z4;

    let g0 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(P[1]),
        DoubleDouble::from_bit_pair(P[0]),
    );
    let g1 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(P[3]),
        DoubleDouble::from_bit_pair(P[2]),
    );
    let g2 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(P[5]),
        DoubleDouble::from_bit_pair(P[4]),
    );
    let g3 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(P[7]),
        DoubleDouble::from_bit_pair(P[6]),
    );
    let g4 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(P[9]),
        DoubleDouble::from_bit_pair(P[8]),
    );
    let g5 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(P[11]),
        DoubleDouble::from_bit_pair(P[10]),
    );
    let g6 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(P[13]),
        DoubleDouble::from_bit_pair(P[12]),
    );

    let h0 = DoubleDouble::mul_add(z2, g1, g0);
    let h1 = DoubleDouble::mul_add(z2, g3, g2);
    let h2 = DoubleDouble::mul_add(z2, g5, g4);

    let q0 = DoubleDouble::mul_add(z4, h1, h0);
    let q1 = DoubleDouble::mul_add(z4, g6, h2);

    let num = DoubleDouble::mul_add(z8, q1, q0);

    // See numerator generation above:
    // poly=den;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const Q: [(u64, u64); 14] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0xbcfc7b886ee61417, 0x405152838f711f3c),
        (0xbd33f933c14e831a, 0x409576cb78cab36e),
        (0x3d33fb09e2c4898a, 0x40c5e8a2c7602ced),
        (0x3d7be430c664bf7e, 0x40e766fdc8c7638c),
        (0x3dac662e74cdfc0e, 0x4100276b5f47b5f1),
        (0x3da67d06e82a8495, 0x410f843887f8a24a),
        (0x3dbbf2e22fc2550a, 0x4116d04271703e08),
        (0xbdb2fb3aed100853, 0x4119aff4ed32b74b),
        (0x3dba75e7b7171c3c, 0x4116b5eb8bf386bd),
        (0x3dab2d8b8c1937eb, 0x410f71c38e84cb34),
        (0xbda4e2e8a50b7370, 0x4100ca04b0f36b94),
        (0xbd86ed6df34fdaf9, 0x40e9151ded4cf4b7),
        (0x3d6938ea702c0328, 0x40c923ee1ab270c4),
    ];

    let g0 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(Q[1]),
        DoubleDouble::from_bit_pair(Q[0]),
    );
    let g1 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(Q[3]),
        DoubleDouble::from_bit_pair(Q[2]),
    );
    let g2 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(Q[5]),
        DoubleDouble::from_bit_pair(Q[4]),
    );
    let g3 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(Q[7]),
        DoubleDouble::from_bit_pair(Q[6]),
    );
    let g4 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(Q[9]),
        DoubleDouble::from_bit_pair(Q[8]),
    );
    let g5 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(Q[11]),
        DoubleDouble::from_bit_pair(Q[10]),
    );
    let g6 = DoubleDouble::mul_add(
        z,
        DoubleDouble::from_bit_pair(Q[13]),
        DoubleDouble::from_bit_pair(Q[12]),
    );

    let h0 = DoubleDouble::mul_add(z2, g1, g0);
    let h1 = DoubleDouble::mul_add(z2, g3, g2);
    let h2 = DoubleDouble::mul_add(z2, g5, g4);

    let q0 = DoubleDouble::mul_add(z4, h1, h0);
    let q1 = DoubleDouble::mul_add(z4, g6, h2);

    let den = DoubleDouble::mul_add(z8, q1, q0);
    let r = DoubleDouble::div(num, den);

    let k = DoubleDouble::quick_mult(r, zeta_sqrt);
    f64::copysign(k.to_f64(), x)
}

/// Inverse error function
///
/// ulp 0.5
pub fn f_erfinv(x: f64) -> f64 {
    let ax = x.to_bits() & 0x7fff_ffff_ffff_ffff;

    if !x.is_normal() {
        if x.is_nan() || x.is_infinite() {
            return f64::NAN;
        }
        if ax == 0 {
            return 0.;
        }
    }

    if ax >= 0x3ff0000000000000u64 {
        // |x| > 1
        if ax == 0x3ff0000000000000u64 {
            return if x.is_sign_negative() {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        return f64::NAN;
    }

    let z = f64::from_bits(ax);

    if ax <= 0x3f8374bc6a7ef9db {
        // 0.0095
        // for small |x| using taylor series first 3 terms
        // Generated by SageMath:
        // from mpmath import mp, erf
        //
        // mp.prec = 100
        //
        // def inverf_series(n_terms):
        //     from mpmath import taylor
        //     series_erf = taylor(mp.erfinv, 0, n_terms)
        //     return series_erf
        //
        // ser = inverf_series(10)
        // for i in range(1, len(ser), 2):
        //     k = ser[i]
        //     print("f64::from_bits(" + double_to_hex(RealField(100)(k)) + "),")
        let z2 = DoubleDouble::from_exact_mult(z, z);
        let p = f_fmla(
            z2.hi,
            f64::from_bits(0x3fb62847c47dda48),
            f64::from_bits(0x3fc053c2c0ab91c5),
        );
        let mut r = DoubleDouble::mul_f64_add(
            z2,
            p,
            DoubleDouble::from_bit_pair((0xbc33ea2ef8dde075, 0x3fcdb29fb2fee5e4)),
        );
        r = DoubleDouble::mul_add(
            z2,
            r,
            DoubleDouble::from_bit_pair((0xbc8618f13eb7ca89, 0x3fec5bf891b4ef6b)),
        );
        // (rh + rl) * z = rh * z + rl*z
        let v = DoubleDouble::quick_mult_f64(r, z);
        return f64::copysign(v.to_f64(), x);
    } else if ax <= 0x3faeb851eb851eb8 {
        // 0.06
        // for |x| < 0.06 using taylor series first 5 terms
        // Generated by SageMath:
        // from mpmath import mp, erf
        //
        // mp.prec = 100
        //
        // def inverf_series(n_terms):
        //     from mpmath import taylor
        //     series_erf = taylor(mp.erfinv, 0, n_terms)
        //     return series_erf
        //
        // ser = inverf_series(10)
        // for i in range(1, len(ser), 2):
        //     k = ser[i]
        //     print("f64::from_bits(" + double_to_hex(RealField(100)(k)) + "),")
        let z2 = DoubleDouble::from_exact_mult(z, z);
        let p = f_polyeval4(
            z2.hi,
            f64::from_bits(0x3fb62847c47dda48),
            f64::from_bits(0x3fb0a13189c6ef7a),
            f64::from_bits(0x3faa7c85c89bb08b),
            f64::from_bits(0x3fa5eeb1d488e312),
        );
        let mut r = DoubleDouble::mul_f64_add(
            z2,
            p,
            DoubleDouble::from_bit_pair((0x3c2cec68daff0d80, 0x3fc053c2c0ab91c5)),
        );
        r = DoubleDouble::mul_add(
            z2,
            r,
            DoubleDouble::from_bit_pair((0xbc33ea2ef8dde075, 0x3fcdb29fb2fee5e4)),
        );
        r = DoubleDouble::mul_add(
            z2,
            r,
            DoubleDouble::from_bit_pair((0xbc8618f13eb7ca89, 0x3fec5bf891b4ef6b)),
        );
        // (rh + rl) * z = rh * z + rl*z
        let v = DoubleDouble::quick_mult_f64(r, z);
        return f64::copysign(v.to_f64(), x);
    }

    if ax <= 0x3fe8000000000000u64 {
        // |x| < 0.75

        // First step rational approximant is generated, but it's ill-conditioned, thus
        // we're using taylor expansion to create Newton form at the point.
        // Generated in Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=InverseErf[x]/x
        // g[x_] =f[Sqrt[x]];
        // {err0,approx}=MiniMaxApproximation[g[z],{z,{0.06,0.75},9,9},WorkingPrecision->75, MaxIterations->100]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // x0=SetPrecision[0.5625,75];
        // NumberForm[Series[num[x],{x,x0,50}], ExponentFunction->(Null&)]
        // coeffs=Table[SeriesCoefficient[num[x],{x,x0,k}],{k,0,9}];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]];
        const P: [(u64, u64); 5] = [
            (0xbc3e06eda42202a0, 0x3f93c2fc5d00e0c8),
            (0xbc6eb374406b33b4, 0xbfc76fcfd022e3ff),
            (0xbc857822d7ffd282, 0x3fe6f8443546010a),
            (0x3c68269c66dfb28a, 0xbff80996754ceb79),
            (0x3c543dce8990a9f9, 0x3ffcf778d5ef0504),
        ];
        let x2 = DoubleDouble::from_exact_mult(x, x);
        let vz = DoubleDouble::full_add_f64(x2, -0.5625);
        let ps_num = f_polyeval5(
            vz.hi,
            f64::from_bits(0xbff433be821423d0),
            f64::from_bits(0x3fdf15f19e9d8da4),
            f64::from_bits(0xbfb770b6827e0829),
            f64::from_bits(0x3f7a98a2980282bb),
            f64::from_bits(0xbf142a246fd2c07c),
        );
        let mut num = DoubleDouble::mul_f64_add(vz, ps_num, DoubleDouble::from_bit_pair(P[4]));
        num = DoubleDouble::mul_add(vz, num, DoubleDouble::from_bit_pair(P[3]));
        num = DoubleDouble::mul_add(vz, num, DoubleDouble::from_bit_pair(P[2]));
        num = DoubleDouble::mul_add(vz, num, DoubleDouble::from_bit_pair(P[1]));
        num = DoubleDouble::mul_add(vz, num, DoubleDouble::from_bit_pair(P[0]));

        // Generated in Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=InverseErf[x]/x
        // g[x_] =f[Sqrt[x]];
        // {err0,approx}=MiniMaxApproximation[g[z],{z,{0.06,0.75},9,9},WorkingPrecision->75, MaxIterations->100]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        // x0=SetPrecision[0.5625,75];
        // NumberForm[Series[den[x],{x,x0,50}], ExponentFunction->(Null&)]
        // coeffs=Table[SeriesCoefficient[den[x],{x,x0,k}],{k,0,9}];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]];
        const Q: [(u64, u64); 5] = [
            (0xbc36337f24e57cb9, 0x3f92388d5d757e3a),
            (0xbc63dfae43d60e0b, 0xbfc6ca7da581358c),
            (0xbc77656389bd0e62, 0x3fe7c82ce417b4e0),
            (0xbc93679667bef2f0, 0xbffad58651fd1a51),
            (0x3ca2c6cb9eb17fb4, 0x4001bdb67e93a242),
        ];

        let ps_den = f_polyeval5(
            vz.hi,
            f64::from_bits(0xbffbdaeff6fbb81c),
            f64::from_bits(0x3fe91b12cf47da3a),
            f64::from_bits(0xbfc7c5d0ffb7f1da),
            f64::from_bits(0x3f939ada247f7609),
            f64::from_bits(0xbf41be65038ccfe6),
        );

        let mut den = DoubleDouble::mul_f64_add(vz, ps_den, DoubleDouble::from_bit_pair(Q[4]));
        den = DoubleDouble::mul_add(vz, den, DoubleDouble::from_bit_pair(Q[3]));
        den = DoubleDouble::mul_add(vz, den, DoubleDouble::from_bit_pair(Q[2]));
        den = DoubleDouble::mul_add(vz, den, DoubleDouble::from_bit_pair(Q[1]));
        den = DoubleDouble::mul_add(vz, den, DoubleDouble::from_bit_pair(Q[0]));
        let r = DoubleDouble::div(num, den);
        let k = DoubleDouble::quick_mult_f64(r, z);
        let err = f_fmla(
            k.hi,
            f64::from_bits(0x3c70000000000000), // 2^-56
            f64::from_bits(0x3c40000000000000), // 2^-59
        );
        let ub = k.hi + (k.lo + err);
        let lb = k.hi + (k.lo - err);
        if ub == lb {
            return f64::copysign(k.to_f64(), x);
        }
        return inverf_0p06_to_0p75(x);
    }

    let q = DoubleDouble::from_full_exact_add(1.0, -z);

    let mut zeta = fast_log_dd(q);
    zeta = DoubleDouble::from_exact_add(zeta.hi, zeta.lo);
    zeta = -zeta;
    let zeta_sqrt = zeta.fast_sqrt();
    let rz = zeta_sqrt.recip();

    if z < 0.9999 {
        inverf_asympt_small(rz, zeta_sqrt, x)
    } else {
        inverf_asympt_long(rz, zeta_sqrt, x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erfinv() {
        assert!(f_erfinv(f64::NEG_INFINITY).is_nan());
        assert!(f_erfinv(f64::INFINITY).is_nan());
        assert!(f_erfinv(f64::NAN).is_nan());
        assert_eq!(f_erfinv(-0.5435340000000265), -0.5265673336010599);
        assert_eq!(f_erfinv(0.5435340000000265), 0.5265673336010599);
        assert_eq!(f_erfinv(0.001000000000084706), 0.0008862271575416209);
        assert_eq!(f_erfinv(-0.001000000000084706), -0.0008862271575416209);
        assert_eq!(f_erfinv(0.71), 0.7482049711849852);
        assert_eq!(f_erfinv(-0.71), -0.7482049711849852);
        assert_eq!(f_erfinv(0.41), 0.381014610957532);
        assert_eq!(f_erfinv(-0.41), -0.381014610957532);
        assert_eq!(f_erfinv(0.32), 0.29165547581744206);
        assert_eq!(f_erfinv(-0.32), -0.29165547581744206);
        assert_eq!(f_erfinv(0.82), 0.9480569762323499);
        assert_eq!(f_erfinv(-0.82), -0.9480569762323499);
        assert_eq!(f_erfinv(0.05), 0.044340387910005497);
        assert_eq!(f_erfinv(-0.05), -0.044340387910005497);
        assert_eq!(f_erfinv(0.99), 1.8213863677184494);
        assert_eq!(f_erfinv(-0.99), -1.8213863677184494);
        assert_eq!(f_erfinv(0.9900000000867389), 1.8213863698392927);
        assert_eq!(f_erfinv(-0.9900000000867389), -1.8213863698392927);
        assert_eq!(f_erfinv(0.99999), 3.123413274341571);
        assert_eq!(f_erfinv(-0.99999), -3.123413274341571);
    }
}
