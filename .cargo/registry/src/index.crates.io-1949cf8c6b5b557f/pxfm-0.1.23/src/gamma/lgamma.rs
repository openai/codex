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
use crate::f_log;
use crate::logs::{fast_log_d_to_dd, fast_log_dd, log_dd};
use crate::polyeval::{f_polyeval4, f_polyeval5, f_polyeval6, f_polyeval10};
use crate::sincospi::f_fast_sinpi_dd;

#[inline]
fn apply_sign_and_sum(z: DoubleDouble, parity: f64, s: DoubleDouble) -> DoubleDouble {
    if parity >= 0. {
        z
    } else {
        DoubleDouble::full_dd_sub(s, z)
    }
}

#[inline]
fn apply_sign_and_sum_quick(z: DoubleDouble, parity: f64, s: DoubleDouble) -> DoubleDouble {
    if parity >= 0. {
        z
    } else {
        DoubleDouble::quick_dd_sub(s, z)
    }
}

#[cold]
fn lgamma_0p5(dx: f64, v_log: DoubleDouble, f_res: DoubleDouble, sum_parity: f64) -> f64 {
    // Log[Gamma[x+1]]
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=LogGamma[x+1]
    // {err0,approx}=MiniMaxApproximation[f[z],{z,{0.0000000000000001,0.5},8,7},WorkingPrecision->90]
    // num=Numerator[approx][[1]];
    // den=Denominator[approx][[1]];
    // poly=den;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const P: [(u64, u64); 9] = [
        (0x349d90fba23c9118, 0xb7f552eee31fa8d2),
        (0x3c56cb95ec26b8b0, 0xbfe2788cfc6fb619),
        (0xbc98554b6e8ebe5c, 0xbff15a4f25fcc40b),
        (0x3c51b37126e8f9ee, 0xbfc2d93ef9720645),
        (0xbc85a532dd358f5d, 0x3fec506397ef590a),
        (0x3c7dc535e77ac796, 0x3fe674f6812154ca),
        (0x3c4ff02dbae30f4d, 0x3fc9aacc2b0173a0),
        (0xbc3aa29e4d4e6c9d, 0x3f95d8cbe81572b6),
        (0x3bed03960179ad28, 0x3f429e0ecfd47eb2),
    ];

    let mut p_num = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(P[8]),
        dx,
        DoubleDouble::from_bit_pair(P[7]),
    );
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[6]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[5]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[4]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[3]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[2]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[1]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[0]));

    const Q: [(u64, u64); 8] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0xbc93d5d3e154eb7a, 0x400a6e37d475364e),
        (0xbcb465441d96e6c2, 0x401112f3f3b083a7),
        (0x3ca1b893e8188325, 0x4005cbfc6085847f),
        (0xbc8608a5840fb86b, 0x3fec920358d35f3a),
        (0x3c5dc5b89a3624bd, 0x3fc21011cbbc6923),
        (0x3bfbe999bea0b965, 0x3f822ae49ffa14ce),
        (0x3b90cb3bd523bf32, 0x3f20d6565fe86116),
    ];

    let mut p_den = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(Q[7]),
        dx,
        DoubleDouble::from_bit_pair(Q[6]),
    );
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[5]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[4]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[3]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[2]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[1]));
    p_den = DoubleDouble::mul_f64_add_f64(p_den, dx, f64::from_bits(0x3ff0000000000000));
    let v0 = DoubleDouble::full_dd_sub(DoubleDouble::div(p_num, p_den), v_log);
    let v_res = apply_sign_and_sum(v0, sum_parity, f_res);
    v_res.to_f64()
}

#[cold]
fn lgamma_0p5_to_1(dx: f64, v_log: DoubleDouble, f_res: DoubleDouble, sum_parity: f64) -> f64 {
    // Log[Gamma[x+1]]
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=LogGamma[x+1]
    // {err0,approx}=MiniMaxApproximation[f[z],{z,{0.5,0.99999999999999999},9,9},WorkingPrecision->90]
    // num=Numerator[approx][[1]];
    // den=Denominator[approx][[1]];
    // poly=den;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const P: [(u64, u64); 10] = [
        (0xb967217bfcc647c2, 0xbcc9b2c47481ff4f),
        (0xbc7eff78623354d7, 0xbfe2788cfc6fb552),
        (0xbc9886c6f42886e0, 0xbff3c6a7f676cf4c),
        (0x3c77ed956ff8e661, 0xbfdaf57ee2a64253),
        (0x3c8723f3a5de4fd5, 0x3feb961a3d8bbe89),
        (0x3c8848ddf2e2726f, 0x3fedc9f11015d4ca),
        (0xbc799f3b76da571b, 0x3fd7ac6e82c07787),
        (0xbc5cb131b054a5f5, 0x3fb103e7e288f4da),
        (0x3bfe93ab961d39a4, 0x3f74789410ab2cf5),
        (0x3babcb1e8a573475, 0x3f1d74e78621d316),
    ];

    let mut p_num = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(P[9]),
        dx,
        DoubleDouble::from_bit_pair(P[8]),
    );
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[7]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[6]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[5]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[4]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[3]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[2]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[1]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[0]));

    const Q: [(u64, u64); 10] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0xbca99871cf68ff41, 0x400c87945e972c56),
        (0xbc8292764aa01c02, 0x401477d734273be4),
        (0xbcaf0f1758e16cb3, 0x400e53c84c87f686),
        (0xbc901825b170e576, 0x3ff8c99df9c66865),
        (0x3c78af0564323160, 0x3fd629441413e902),
        (0x3c3293dd176164f3, 0x3fa42e466fd1464e),
        (0xbbf3fbc18666280b, 0x3f5f9cd4c60c4e7d),
        (0xbb4036ba7be37458, 0x3efb2d3ed43aab6f),
        (0xbaf7a3a7d5321f53, 0xbe665e3fadf143a6),
    ];

    let mut p_den = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(Q[9]),
        dx,
        DoubleDouble::from_bit_pair(Q[8]),
    );
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[7]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[6]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[5]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[4]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[3]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[2]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[1]));
    p_den = DoubleDouble::mul_f64_add_f64(p_den, dx, f64::from_bits(0x3ff0000000000000));
    let v0 = DoubleDouble::full_dd_sub(DoubleDouble::div(p_num, p_den), v_log);
    let v_res = apply_sign_and_sum(v0, sum_parity, f_res);
    v_res.to_f64()
}

#[cold]
fn lgamma_1_to_4(dx: f64, v_log: DoubleDouble, f_res: DoubleDouble, sum_parity: f64) -> f64 {
    // Log[Gamma[x+1]]
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=LogGamma[x+1]
    // {err0,approx}=MiniMaxApproximation[f[z],{z,{1.0000000000000000001,4},13,13},WorkingPrecision->90]
    // num=Numerator[approx][[1]];
    // den=Denominator[approx][[1]];
    // poly=den;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const P: [(u64, u64); 14] = [
        (0x398e8eb32d541d21, 0xbcf55335b06a5df1),
        (0xbc60fc919ad95ffb, 0xbfe2788cfc6fb2c4),
        (0x3c98d8e5e64ea307, 0xbffb5e5c82450af5),
        (0x3c778a3bdabcdb1f, 0xbff824ecfcecbcd5),
        (0x3c77768d59db11a0, 0x3fd6a097c5fa0036),
        (0x3c9f4a74ad888f08, 0x3ff9297ba86cc14e),
        (0xbc97e1e1819d78bd, 0x3ff3dd0025109756),
        (0xbc6f1b00b6ce8a2a, 0x3fdfded97a03f2f3),
        (0xbc55487a80e322fe, 0x3fbd5639f2258856),
        (0x3bede86c7e0323ce, 0x3f8f7261ccd00da5),
        (0x3bcde1ee8e81b7b5, 0x3f52fbfb5a8c7221),
        (0x3ba8ed54c3db7fde, 0x3f07d48361a072b6),
        (0xbb4c30e2cdaa48f2, 0x3eaa8661f0313183),
        (0xbac575d303dd9b93, 0x3e3205d52df415e6),
    ];

    let mut p_num = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(P[13]),
        dx,
        DoubleDouble::from_bit_pair(P[12]),
    );
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[11]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[10]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[9]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[8]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[7]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[6]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[5]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[4]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[3]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[2]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[1]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[0]));

    const Q: [(u64, u64); 14] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0x3cbde8d771be8aba, 0x40118da297250deb),
        (0xbccdbba67547f5dc, 0x4020589156c4058c),
        (0x3caca498a3ea822e, 0x4020e9442d441e22),
        (0xbca6a7d3651d1b42, 0x40156480b1dc22fe),
        (0x3ca79ba727e70ad6, 0x40013006d1e5d08c),
        (0x3c8c2b824cfce390, 0x3fe1b0eb2f75de3f),
        (0xbc5208ab1ad4d8e0, 0x3fb709e145993376),
        (0xbc21d64934aab809, 0x3f825ee5a8799658),
        (0x3bc29531f5a4518d, 0x3f40ed532b6bffdd),
        (0x3b994fb11dace77e, 0x3ef052da92364c6d),
        (0xbb1f0fb3869f715e, 0x3e8b6bbbe7aae5eb),
        (0x3a81d8373548a8a8, 0x3e09b5304c6d77ba),
        (0x3992e1e6b163e1f7, 0xbd50eee2d9d4e7b9),
    ];

    let mut p_den = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(Q[13]),
        dx,
        DoubleDouble::from_bit_pair(Q[12]),
    );
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[11]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[10]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[9]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[8]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[7]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[6]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[5]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[4]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[3]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[2]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[1]));
    p_den = DoubleDouble::mul_f64_add_f64(p_den, dx, f64::from_bits(0x3ff0000000000000));
    let mut k = DoubleDouble::div(p_num, p_den);
    k = DoubleDouble::from_exact_add(k.hi, k.lo);
    let v0 = DoubleDouble::full_dd_sub(k, v_log);
    let v_res = apply_sign_and_sum(v0, sum_parity, f_res);
    v_res.to_f64()
}

#[cold]
fn lgamma_4_to_12(dx: f64, f_res: DoubleDouble, sum_parity: f64) -> f64 {
    // Log[Gamma[x+1]]
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=LogGamma[x]
    // {err0,approx}=MiniMaxApproximation[f[z],{z,{4,12},10,10},WorkingPrecision->90]
    // num=Numerator[approx][[1]];
    // den=Denominator[approx][[1]];
    // poly=num;
    // coeffs=CoefficientList[poly,z];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const P: [(u64, u64); 11] = [
        (0x3c9a767426b2d1e8, 0x400e45c3573057bb),
        (0xbcccc22bbae40ac4, 0x403247d3a1b9b0e9),
        (0xbc82c1d4989e7813, 0x3ffe955db10dd744),
        (0x3cb4669cf9238f48, 0xc036a67a52498757),
        (0x3cb4c4039d4da434, 0xc01a8d26ec4b2f7b),
        (0x3ca43834ea54b437, 0x400c92b79f85b787),
        (0x3c930746944a1bc1, 0x3ff8b3afe3e0525c),
        (0xbc68499f7a8b0f87, 0x3fc8059ff9cb3e10),
        (0x3c0e16d9f08b7e18, 0x3f81c282c77a3862),
        (0xbbcec78600b42cd0, 0x3f22fe2577cf1017),
        (0x3b1c222faf24d4a1, 0x3ea5511d126ad883),
    ];

    let mut p_num = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(P[10]),
        dx,
        DoubleDouble::from_bit_pair(P[9]),
    );
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[8]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[7]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[6]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[5]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[4]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[3]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[2]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[1]));
    p_num = DoubleDouble::mul_f64_add(p_num, dx, DoubleDouble::from_bit_pair(P[0]));

    const Q: [(u64, u64); 11] = [
        (0x0000000000000000, 0x3ff0000000000000),
        (0x3ccaa60b201a29f2, 0x40288b76f18d51d8),
        (0xbcd831f5042551f9, 0x403d383173e1839d),
        (0x3cb2b77730673e36, 0x4037e8a6dda469df),
        (0x3cb8c229012ae276, 0x40207ddd53aa3098),
        (0xbc8aba961299355d, 0x3ff4c90761849336),
        (0xbc3e844b2b1f0edb, 0x3fb832c6fba5ff26),
        (0xbbc70261cd94cb90, 0x3f68b185e16f05c1),
        (0xbb6c1c2dfe90e592, 0x3f0303509bbb9cf8),
        (0xbb0f160d6b147ae5, 0x3e7d1bd86b15f52e),
        (0x3a5714448b9c17d2, 0xbdba82d4d2ccf533),
    ];

    let mut p_den = DoubleDouble::mul_f64_add(
        DoubleDouble::from_bit_pair(Q[10]),
        dx,
        DoubleDouble::from_bit_pair(Q[9]),
    );
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[8]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[7]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[6]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[5]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[4]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[3]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[2]));
    p_den = DoubleDouble::mul_f64_add(p_den, dx, DoubleDouble::from_bit_pair(Q[1]));
    p_den = DoubleDouble::mul_f64_add_f64(p_den, dx, f64::from_bits(0x3ff0000000000000));
    let v_res = apply_sign_and_sum(DoubleDouble::div(p_num, p_den), sum_parity, f_res);
    v_res.to_f64()
}

#[cold]
fn stirling_accurate(dx: f64, parity: f64, f_res: DoubleDouble) -> f64 {
    let y_recip = DoubleDouble::from_quick_recip(dx);
    let y_sqr = DoubleDouble::mult(y_recip, y_recip);
    // Bernoulli coefficients generated by SageMath:
    // var('x')
    // def bernoulli_terms(x, N):
    //     S = 0
    //     for k in range(1, N+1):
    //         B = bernoulli(2*k)
    //         term = (B / (2*k*(2*k-1))) * x**((2*k-1))
    //         S += term
    //     return S
    //
    // terms = bernoulli_terms(x, 10)
    const BERNOULLI_C: [(u64, u64); 10] = [
        (0x3c55555555555555, 0x3fb5555555555555),
        (0x3bff49f49f49f49f, 0xbf66c16c16c16c17),
        (0x3b8a01a01a01a01a, 0x3f4a01a01a01a01a),
        (0x3befb1fb1fb1fb20, 0xbf43813813813814),
        (0x3be5c3a9ce01b952, 0x3f4b951e2b18ff23),
        (0x3bff82553c999b0e, 0xbf5f6ab0d9993c7d),
        (0x3c10690690690690, 0x3f7a41a41a41a41a),
        (0x3c21efcdab896745, 0xbf9e4286cb0f5398),
        (0xbc279e2405a71f88, 0x3fc6fe96381e0680),
        (0x3c724246319da678, 0xbff6476701181f3a),
    ];
    let bernoulli_poly = f_polyeval10(
        y_sqr,
        DoubleDouble::from_bit_pair(BERNOULLI_C[0]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[1]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[2]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[3]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[4]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[5]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[6]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[7]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[8]),
        DoubleDouble::from_bit_pair(BERNOULLI_C[9]),
    );

    // Log[Gamma(x)] = x*log(x) - x + 1/2*Log(2*PI/x) + bernoulli_terms
    const LOG2_PI_OVER_2: DoubleDouble =
        DoubleDouble::from_bit_pair((0xbc865b5a1b7ff5df, 0x3fed67f1c864beb5));
    let mut log_gamma = DoubleDouble::add(
        DoubleDouble::mul_add_f64(bernoulli_poly, y_recip, -dx),
        LOG2_PI_OVER_2,
    );
    let dy_log = log_dd(dx);
    log_gamma = DoubleDouble::mul_add(
        DoubleDouble::from_exact_add(dy_log.hi, dy_log.lo),
        DoubleDouble::from_full_exact_add(dx, -0.5),
        log_gamma,
    );
    let res = apply_sign_and_sum(log_gamma, parity, f_res);
    res.to_f64()
}

/// due to log(x) leading terms cancellation happens around 2,
/// hence we're using different approximation around LogGamma(2).
/// Coefficients are ill-conditioned here so Minimal Newton form is used
#[cold]
fn lgamma_around_2(x: f64, parity: f64, f_res: DoubleDouble) -> f64 {
    let dx = DoubleDouble::from_full_exact_sub(x, 2.);

    // Generated by Wolfram Mathematica:
    // <<FunctionApproximations`
    // ClearAll["Global`*"]
    // f[x_]:=LogGamma[x]
    // {err0,approx,err1}=MiniMaxApproximation[f[x],{x,{1.95,2.05},11,11},WorkingPrecision->90]
    // num=Numerator[approx];
    // den=Denominator[approx];
    // poly=num;
    // x0=SetPrecision[2,90];
    // NumberForm[Series[num,{x,x0,9}],ExponentFunction->(Null&)]
    // coeffs=Table[SeriesCoefficient[num,{x,x0,k}],{k,0,10}];
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    // poly=den;
    // coeffs=Table[SeriesCoefficient[den,{x,x0,k}],{k,0,10}];
    // NumberForm[Series[den,{x,x0,9}],ExponentFunction->(Null&)]
    // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
    const P: [(u64, u64); 10] = [
        (0xbd8ef0f02676337a, 0x41000f48befb1b70),
        (0x3dae9e186ab39649, 0x411aeb5de0ba157d),
        (0xbdcfe4bca8f58298, 0x4122d673e1f69e2c),
        (0x3db1e3de8e53cfce, 0x411d0fdd168e56a8),
        (0x3d9114d199dacd01, 0x410b4a1ce996f910),
        (0x3d9172663132c0b6, 0x40f02d95ceca2c8a),
        (0x3d52988b7d30cfd3, 0x40c83a4c6e145abc),
        (0xbd14327346a3db7b, 0x409632fe8dc80419),
        (0x3cfe9f975e5e9984, 0x4057219509ba1d62),
        (0xbca0fdd3506bf429, 0x4007988259d795c4),
    ];

    let dx2 = dx * dx;
    let dx4 = dx2 * dx2;
    let dx8 = dx4 * dx4;

    let p0 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(P[1]),
        DoubleDouble::from_bit_pair(P[0]),
    );
    let p1 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(P[3]),
        DoubleDouble::from_bit_pair(P[2]),
    );
    let p2 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(P[5]),
        DoubleDouble::from_bit_pair(P[4]),
    );
    let p3 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(P[7]),
        DoubleDouble::from_bit_pair(P[6]),
    );
    let p4 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(P[9]),
        DoubleDouble::from_bit_pair(P[8]),
    );

    let q0 = DoubleDouble::quick_mul_add(dx2, p1, p0);
    let q1 = DoubleDouble::quick_mul_add(dx2, p3, p2);

    let r0 = DoubleDouble::quick_mul_add(dx4, q1, q0);
    let mut p_num = DoubleDouble::quick_mul_add(dx8, p4, r0);
    p_num = DoubleDouble::quick_mult(p_num, dx);

    const Q: [(u64, u64); 11] = [
        (0x3da0f8e36723f959, 0x4112fe27249fc6f1),
        (0xbdc289e4a571ddb8, 0x412897be1a39b97b),
        (0xbdb8d18ab489860f, 0x412b4fcbc6d9cb44),
        (0x3da0550c9f65a5ef, 0x4120fe765c0f6a79),
        (0xbd90a121e792bf7f, 0x4109fa9eaa0f816a),
        (0xbd7168de8e78812e, 0x40e9269d002372a5),
        (0x3d4e4d052cd6982a, 0x40beab7f948d82f0),
        (0xbd0cebe53b7e81bf, 0x4086a91c01d7241f),
        (0xbcd2edf097020841, 0x4042a780e9d1b74b),
        (0xbc73d1a40910845f, 0x3fecd007ff47224f),
        (0xbc06e4de631047f3, 0x3f7ad97267872491),
    ];

    let q0 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(Q[1]),
        DoubleDouble::from_bit_pair(Q[0]),
    );
    let q1 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(Q[3]),
        DoubleDouble::from_bit_pair(Q[2]),
    );
    let q2 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(Q[5]),
        DoubleDouble::from_bit_pair(Q[4]),
    );
    let q3 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(Q[7]),
        DoubleDouble::from_bit_pair(Q[6]),
    );
    let q4 = DoubleDouble::quick_mul_add(
        dx,
        DoubleDouble::from_bit_pair(Q[9]),
        DoubleDouble::from_bit_pair(Q[8]),
    );

    let r0 = DoubleDouble::quick_mul_add(dx2, q1, q0);
    let r1 = DoubleDouble::quick_mul_add(dx2, q3, q2);

    let s0 = DoubleDouble::quick_mul_add(dx4, r1, r0);
    let s1 = DoubleDouble::quick_mul_add(dx2, DoubleDouble::from_bit_pair(Q[10]), q4);
    let p_den = DoubleDouble::quick_mul_add(dx8, s1, s0);

    let v_res = apply_sign_and_sum_quick(DoubleDouble::div(p_num, p_den), parity, f_res);
    v_res.to_f64()
}

/// Computes log(gamma(x))
///
/// ulp 0.52
pub fn f_lgamma(x: f64) -> f64 {
    let nx = x.to_bits().wrapping_shl(1);
    if nx >= 0xfeaea9b24f16a34cu64 {
        // |x| >= 0x1.006df1bfac84ep+1015
        if x.is_infinite() {
            return f64::INFINITY;
        }
        if x.is_nan() {
            return f64::NAN;
        }
        return f64::INFINITY;
    }

    if x.floor() == x {
        if x == 2. || x == 1. {
            return 0.;
        }
        if x.is_sign_negative() {
            return f64::INFINITY;
        }
    }

    let ax = f64::from_bits(x.to_bits() & 0x7fff_ffff_ffff_ffff);
    let dx = ax;

    let is_positive = x.is_sign_positive();
    let mut sum_parity = 1f64;

    if ax < f64::EPSILON {
        return -f_log(dx);
    }

    let mut f_res = DoubleDouble::default();
    // For negative x, since (G is gamma function)
    // -x*G(-x)*G(x) = pi/sin(pi*x),
    // we have
    // G(x) = pi/(sin(pi*x)*(-x)*G(-x))
    // since G(-x) is positive, sign(G(x)) = sign(sin(pi*x)) for x<0
    // Hence, for x<0, signgam = sign(sin(pi*x)) and
    // lgamma(x) = log(|Gamma(x)|) = log(pi/(|x*sin(pi*x)|)) - lgamma(-x);
    if !is_positive {
        let y1 = ax.floor();
        let fraction = ax - y1; // excess over the boundary

        let a = f_fast_sinpi_dd(fraction);

        sum_parity = -1.;
        const LOG_PI: DoubleDouble =
            DoubleDouble::from_bit_pair((0x3c67abf2ad8d5088, 0x3ff250d048e7a1bd));
        let mut den = DoubleDouble::quick_mult_f64(a, dx);
        den = DoubleDouble::from_exact_add(den.hi, den.lo);
        f_res = fast_log_dd(den);
        f_res = DoubleDouble::from_exact_add(f_res.hi, f_res.lo);
        f_res = DoubleDouble::quick_dd_sub(LOG_PI, f_res);
    }

    if ax <= 0.5 {
        // Log[Gamma[x + 1]] poly generated by Wolfram
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=LogGamma[x+1]
        // {err0,approx}=MiniMaxApproximation[f[z],{z,{0.0000000000000001,0.5},7,7},WorkingPrecision->90]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let ps_num = f_polyeval4(
            dx,
            f64::from_bits(0x3fea8287cc94dc31),
            f64::from_bits(0x3fe000cbc75a2ab7),
            f64::from_bits(0x3fba69bac2495765),
            f64::from_bits(0x3f78eb3984eb55ee),
        );
        let ps_den = f_polyeval5(
            dx,
            f64::from_bits(0x3ffee073402b349e),
            f64::from_bits(0x3fe04c62232d12ec),
            f64::from_bits(0x3fad09ff23ffb930),
            f64::from_bits(0x3f5c253f6e8af7d2),
            f64::from_bits(0xbee18a68b4ed9516),
        );
        let mut p_num = DoubleDouble::f64_mul_f64_add(
            ps_num,
            dx,
            DoubleDouble::from_bit_pair((0x3c4d671927a50f5b, 0x3fb184cdffda39b0)),
        );
        p_num = DoubleDouble::mul_f64_add(
            p_num,
            dx,
            DoubleDouble::from_bit_pair((0xbc8055e20f9945f5, 0xbfedba6e22cced8a)),
        );
        p_num = DoubleDouble::mul_f64_add(
            p_num,
            dx,
            DoubleDouble::from_bit_pair((0x3c56cc8e006896be, 0xbfe2788cfc6fb619)),
        );
        p_num = DoubleDouble::mul_f64_add(
            p_num,
            dx,
            DoubleDouble::from_bit_pair((0x34c1e175ecd02e7e, 0xb84ed528d8df1c88)),
        );
        let mut p_den = DoubleDouble::f64_mul_f64_add(
            ps_den,
            dx,
            DoubleDouble::from_bit_pair((0xbc70ab0b3b299408, 0x400c16483bffaf57)),
        );
        p_den = DoubleDouble::mul_f64_add(
            p_den,
            dx,
            DoubleDouble::from_bit_pair((0xbcac74fbe90fa7cb, 0x400846598b0e4750)),
        );
        p_den = DoubleDouble::mul_f64_add_f64(p_den, dx, f64::from_bits(0x3ff0000000000000));
        let d_log = fast_log_d_to_dd(dx);
        let v0 = DoubleDouble::sub(DoubleDouble::div(p_num, p_den), d_log);
        let l_res = f_res;
        f_res = apply_sign_and_sum_quick(v0, sum_parity, f_res);
        let err = f_fmla(
            f_res.hi.abs(),
            f64::from_bits(0x3c56a09e667f3bcd), // 2^-57.5
            f64::from_bits(0x3c20000000000000), // 2^-61
        );
        let ub = f_res.hi + (f_res.lo + err);
        let lb = f_res.hi + (f_res.lo - err);
        if ub == lb {
            return ub;
        }
        lgamma_0p5(dx, d_log, l_res, sum_parity)
    } else if ax <= 1. {
        let distance_to_2 = ax - 2.;
        if distance_to_2.abs() < 0.05 {
            return lgamma_around_2(ax, sum_parity, f_res);
        }

        // Log[Gamma[x+1]]
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=LogGamma[x+1]
        // {err0,approx}=MiniMaxApproximation[f[z],{z,{0.0000000000000001,0.5},9,9},WorkingPrecision->90]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]

        const P: [(u64, u64); 10] = [
            (0xb967217bfcc647c2, 0xbcc9b2c47481ff4f),
            (0xbc7eff78623354d7, 0xbfe2788cfc6fb552),
            (0xbc9886c6f42886e0, 0xbff3c6a7f676cf4c),
            (0x3c77ed956ff8e661, 0xbfdaf57ee2a64253),
            (0x3c8723f3a5de4fd5, 0x3feb961a3d8bbe89),
            (0x3c8848ddf2e2726f, 0x3fedc9f11015d4ca),
            (0xbc799f3b76da571b, 0x3fd7ac6e82c07787),
            (0xbc5cb131b054a5f5, 0x3fb103e7e288f4da),
            (0x3bfe93ab961d39a4, 0x3f74789410ab2cf5),
            (0x3babcb1e8a573475, 0x3f1d74e78621d316),
        ];

        let ps_den = f_polyeval4(
            dx,
            f64::from_bits(0x3fa42e466fd1464e),
            f64::from_bits(0x3f5f9cd4c60c4e7d),
            f64::from_bits(0x3efb2d3ed43aab6f),
            f64::from_bits(0xbe665e3fadf143a6),
        );

        let dx2 = DoubleDouble::from_exact_mult(dx, dx);
        let dx4 = DoubleDouble::quick_mult(dx2, dx2);
        let dx8 = DoubleDouble::quick_mult(dx4, dx4);

        let p0 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[1]),
            dx,
            DoubleDouble::from_bit_pair(P[0]),
        );
        let p1 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[3]),
            dx,
            DoubleDouble::from_bit_pair(P[2]),
        );
        let p2 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[5]),
            dx,
            DoubleDouble::from_bit_pair(P[4]),
        );
        let p3 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[7]),
            dx,
            DoubleDouble::from_bit_pair(P[6]),
        );
        let p4 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[9]),
            dx,
            DoubleDouble::from_bit_pair(P[8]),
        );

        let q0 = DoubleDouble::mul_add(dx2, p1, p0);
        let q1 = DoubleDouble::mul_add(dx2, p3, p2);

        let r0 = DoubleDouble::mul_add(dx4, q1, q0);
        let p_num = DoubleDouble::mul_add(dx8, p4, r0);

        let mut p_den = DoubleDouble::f64_mul_f64_add(
            ps_den,
            dx,
            DoubleDouble::from_bit_pair((0x3c78af0564323160, 0x3fd629441413e902)),
        );
        p_den = DoubleDouble::mul_f64_add(
            p_den,
            dx,
            DoubleDouble::from_bit_pair((0xbc901825b170e576, 0x3ff8c99df9c66865)),
        );
        p_den = DoubleDouble::mul_f64_add(
            p_den,
            dx,
            DoubleDouble::from_bit_pair((0xbcaf0f1758e16cb3, 0x400e53c84c87f686)),
        );
        p_den = DoubleDouble::mul_f64_add(
            p_den,
            dx,
            DoubleDouble::from_bit_pair((0xbc8292764aa01c02, 0x401477d734273be4)),
        );
        p_den = DoubleDouble::mul_f64_add(
            p_den,
            dx,
            DoubleDouble::from_bit_pair((0xbca99871cf68ff41, 0x400c87945e972c56)),
        );
        p_den = DoubleDouble::mul_f64_add_f64(p_den, dx, f64::from_bits(0x3ff0000000000000));
        let d_log = fast_log_d_to_dd(dx);
        let v0 = DoubleDouble::sub(DoubleDouble::div(p_num, p_den), d_log);
        let l_res = f_res;
        f_res = apply_sign_and_sum_quick(v0, sum_parity, f_res);
        let err = f_fmla(
            f_res.hi.abs(),
            f64::from_bits(0x3c40000000000000), // 2^-59
            f64::from_bits(0x3c20000000000000), // 2^-61
        );
        let ub = f_res.hi + (f_res.lo + err);
        let lb = f_res.hi + (f_res.lo - err);
        if ub == lb {
            return f_res.to_f64();
        }
        lgamma_0p5_to_1(dx, d_log, l_res, sum_parity)
    } else if ax <= 4. {
        let distance_to_2 = ax - 2.;
        if distance_to_2.abs() < 0.05 {
            return lgamma_around_2(ax, sum_parity, f_res);
        }
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=LogGamma[x+1]
        // {err0,approx}=MiniMaxApproximation[f[z],{z,{1.0000000000000000001,4},9,9},WorkingPrecision->90]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]

        let x2 = DoubleDouble::from_exact_mult(x, x);
        let x4 = DoubleDouble::quick_mult(x2, x2);
        let x8 = DoubleDouble::quick_mult(x4, x4);

        const P: [(u64, u64); 10] = [
            (0x3a8ea8c71173ba1f, 0xbde8c6619bc06d43),
            (0x3c8f2502d288b7e1, 0xbfe2788cfb0f13f7),
            (0xbc873b33ddea3333, 0xbfebd290912f0200),
            (0x3c223d47fd7b2e30, 0x3fb52786e934492b),
            (0xbc8f4e91d7f48aa5, 0x3fe8204c68bc38f4),
            (0x3c5356ff82d857c6, 0x3fde676d587374a4),
            (0x3c5d8deef0e6c21f, 0x3fbef2e284faabe5),
            (0xbc24ea363b4779fb, 0x3f8bb45183525b51),
            (0xbbec808b7b332822, 0x3f43b11bd314773b),
            (0xbb777d551025e6da, 0x3edf7931f7cb9cd1),
        ];

        let p0 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[1]),
            dx,
            DoubleDouble::from_bit_pair(P[0]),
        );
        let p1 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[3]),
            dx,
            DoubleDouble::from_bit_pair(P[2]),
        );
        let p2 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[5]),
            dx,
            DoubleDouble::from_bit_pair(P[4]),
        );
        let p3 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[7]),
            dx,
            DoubleDouble::from_bit_pair(P[6]),
        );
        let p4 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[9]),
            dx,
            DoubleDouble::from_bit_pair(P[8]),
        );

        let q0 = DoubleDouble::mul_add(x2, p1, p0);
        let q1 = DoubleDouble::mul_add(x2, p3, p2);

        let r0 = DoubleDouble::mul_add(x4, q1, q0);
        let p_num = DoubleDouble::mul_add(x8, p4, r0);

        const Q: [(u64, u64); 10] = [
            (0x0000000000000000, 0x3ff0000000000000),
            (0xbc9ad7b53d7da072, 0x4007730c69fb4a20),
            (0xbc93d2a47740a995, 0x400ab6d03e2d6528),
            (0x3c9cd643c37f1205, 0x3ffe2cccacb6740b),
            (0xbc7b616646543538, 0x3fe1f36f793ad9c6),
            (0xbc483b2cb83a34ba, 0x3fb630083527c66f),
            (0x3c1089007cac404c, 0x3f7a6b85d9c297ea),
            (0xbbcfc269fc4a2c55, 0x3f298d01a660c3d9),
            (0xbb43342127aafe5a, 0x3eb9c8ba657b4b0a),
            (0xbaac5e3aa213d878, 0xbe14186c41f01fd1),
        ];

        let p0 = DoubleDouble::mul_f64_add_f64(
            DoubleDouble::from_bit_pair(Q[1]),
            dx,
            f64::from_bits(0x3ff0000000000000),
        );
        let p1 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(Q[3]),
            dx,
            DoubleDouble::from_bit_pair(Q[2]),
        );
        let p2 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(Q[5]),
            dx,
            DoubleDouble::from_bit_pair(Q[4]),
        );
        let p3 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(Q[7]),
            dx,
            DoubleDouble::from_bit_pair(Q[6]),
        );
        let p4 = DoubleDouble::f64_mul_f64_add(
            f64::from_bits(0xbe14186c41f01fd1), // Q[9].hi
            dx,
            DoubleDouble::from_bit_pair(Q[8]),
        );

        let q0 = DoubleDouble::mul_add(x2, p1, p0);
        let q1 = DoubleDouble::mul_add(x2, p3, p2);

        let r0 = DoubleDouble::mul_add(x4, q1, q0);
        let p_den = DoubleDouble::mul_add(x8, p4, r0);

        let d_log = fast_log_d_to_dd(dx);
        let prod = DoubleDouble::div(p_num, p_den);
        let v0 = DoubleDouble::sub(prod, d_log);
        let l_res = f_res;
        f_res = apply_sign_and_sum_quick(v0, sum_parity, f_res);
        let err = f_fmla(
            f_res.hi.abs(),
            f64::from_bits(0x3c40000000000000), // 2^-59
            f64::from_bits(0x3c00000000000000), // 2^-63
        );
        let ub = f_res.hi + (f_res.lo + err);
        let lb = f_res.hi + (f_res.lo - err);
        if ub == lb {
            return f_res.to_f64();
        }
        lgamma_1_to_4(dx, d_log, l_res, sum_parity)
    } else if ax <= 12. {
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=LogGamma[x]
        // {err0,approx}=MiniMaxApproximation[f[z],{z,{4,12},9,9},WorkingPrecision->90]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50},ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]

        let x2 = DoubleDouble::from_exact_mult(x, x);
        let x4 = DoubleDouble::quick_mult(x2, x2);
        let x8 = DoubleDouble::quick_mult(x4, x4);

        const P: [(u64, u64); 10] = [
            (0x3ca9a6c909e67304, 0x400c83f8e5e68934),
            (0xbcc3a71a296d1f00, 0x40278d8a2abd6aec),
            (0xbca623fa8857b35a, 0xc0144dd0190486d6),
            (0x3cc1845532bca122, 0xc02920aaae63c5a7),
            (0x3c57111721fd9df2, 0xbfc9a27952cac38f),
            (0xbca78a77f8acae38, 0x400043078ac20503),
            (0xbc721a88d770af7e, 0x3fdbeba4a1a95bfd),
            (0xbc09c9e5917a665e, 0x3f9e18ff2504fd11),
            (0xbbeb6e5c1cdf8c87, 0x3f45b0595f7eb903),
            (0xbaf149d407d419d3, 0x3ecf5336ddb96b5f),
        ];

        let p0 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[1]),
            dx,
            DoubleDouble::from_bit_pair(P[0]),
        );
        let p1 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[3]),
            dx,
            DoubleDouble::from_bit_pair(P[2]),
        );
        let p2 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[5]),
            dx,
            DoubleDouble::from_bit_pair(P[4]),
        );
        let p3 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[7]),
            dx,
            DoubleDouble::from_bit_pair(P[6]),
        );
        let p4 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(P[9]),
            dx,
            DoubleDouble::from_bit_pair(P[8]),
        );

        let q0 = DoubleDouble::mul_add(x2, p1, p0);
        let q1 = DoubleDouble::mul_add(x2, p3, p2);

        let r0 = DoubleDouble::mul_add(x4, q1, q0);
        let p_num = DoubleDouble::mul_add(x8, p4, r0);

        const Q: [(u64, u64); 10] = [
            (0x0000000000000000, 0x3ff0000000000000),
            (0x3cc5e0cfc2a3ed2f, 0x4023561fd4efbbb2),
            (0x3c829f67da778215, 0x403167ff0d04f99a),
            (0x3ca3c8e7b81b165f, 0x4024f2d3c7d9439f),
            (0xbca90199265d1bfc, 0x40045530db97bad5),
            (0xbc3e89169d10977f, 0x3fd0d9f46084a388),
            (0x3c1c2b7582566435, 0x3f872f7f248227bd),
            (0x3ba8f3f0294e144f, 0x3f271e198971c58e),
            (0x3b4d13ceca0a9bf7, 0x3ea62e85cd267c65),
            (0xba896f9fc0c4f644, 0xbde99e5ee24ff6ba),
        ];

        let p0 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(Q[1]),
            dx,
            DoubleDouble::from_bit_pair(Q[0]),
        );
        let p1 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(Q[3]),
            dx,
            DoubleDouble::from_bit_pair(Q[2]),
        );
        let p2 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(Q[5]),
            dx,
            DoubleDouble::from_bit_pair(Q[4]),
        );
        let p3 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(Q[7]),
            dx,
            DoubleDouble::from_bit_pair(Q[6]),
        );
        let p4 = DoubleDouble::f64_mul_f64_add(
            f64::from_bits(0xbde99e5ee24ff6ba), //Q[9].hi
            dx,
            DoubleDouble::from_bit_pair(Q[8]),
        );

        let q0 = DoubleDouble::mul_add(x2, p1, p0);
        let q1 = DoubleDouble::mul_add(x2, p3, p2);

        let r0 = DoubleDouble::mul_add(x4, q1, q0);
        let p_den = DoubleDouble::mul_add(x8, p4, r0);

        let l_res = f_res;
        f_res = apply_sign_and_sum_quick(DoubleDouble::div(p_num, p_den), sum_parity, f_res);
        let err = f_fmla(
            f_res.hi.abs(),
            f64::from_bits(0x3c50000000000000), // 2^-58
            f64::from_bits(0x3bd0000000000000), // 2^-66
        );
        let ub = f_res.hi + (f_res.lo + err);
        let lb = f_res.hi + (f_res.lo - err);
        if ub == lb {
            return f_res.to_f64();
        }
        lgamma_4_to_12(dx, l_res, sum_parity)
    } else {
        // Stirling's approximation of Log(Gamma) and then Exp[Log[Gamma]]
        let y_recip = DoubleDouble::from_quick_recip(dx);
        let y_sqr = DoubleDouble::mult(y_recip, y_recip);
        // Bernoulli coefficients generated by SageMath:
        // var('x')
        // def bernoulli_terms(x, N):
        //     S = 0
        //     for k in range(1, N+1):
        //         B = bernoulli(2*k)
        //         term = (B / (2*k*(2*k-1))) * x**((2*k-1))
        //         S += term
        //     return S
        //
        // terms = bernoulli_terms(x, 7)
        let bernoulli_poly_s = f_polyeval6(
            y_sqr.hi,
            f64::from_bits(0xbf66c16c16c16c17),
            f64::from_bits(0x3f4a01a01a01a01a),
            f64::from_bits(0xbf43813813813814),
            f64::from_bits(0x3f4b951e2b18ff23),
            f64::from_bits(0xbf5f6ab0d9993c7d),
            f64::from_bits(0x3f7a41a41a41a41a),
        );
        let bernoulli_poly = DoubleDouble::mul_f64_add(
            y_sqr,
            bernoulli_poly_s,
            DoubleDouble::from_bit_pair((0x3c55555555555555, 0x3fb5555555555555)),
        );
        // Log[Gamma(x)] = x*log(x) - x + 1/2*Log(2*PI/x) + bernoulli_terms
        const LOG2_PI_OVER_2: DoubleDouble =
            DoubleDouble::from_bit_pair((0xbc865b5a1b7ff5df, 0x3fed67f1c864beb5));
        let mut log_gamma = DoubleDouble::add(
            DoubleDouble::mul_add_f64(bernoulli_poly, y_recip, -dx),
            LOG2_PI_OVER_2,
        );
        let dy_log = fast_log_d_to_dd(dx);
        log_gamma = DoubleDouble::mul_add(
            DoubleDouble::from_exact_add(dy_log.hi, dy_log.lo),
            DoubleDouble::from_full_exact_add(dx, -0.5),
            log_gamma,
        );
        let l_res = f_res;
        f_res = apply_sign_and_sum_quick(log_gamma, sum_parity, f_res);
        let err = f_fmla(
            f_res.hi.abs(),
            f64::from_bits(0x3c30000000000000), // 2^-60
            f64::from_bits(0x3bc0000000000000), // 2^-67
        );
        let ub = f_res.hi + (f_res.lo + err);
        let lb = f_res.hi + (f_res.lo - err);
        if ub == lb {
            return f_res.to_f64();
        }
        stirling_accurate(dx, sum_parity, l_res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lgamma() {
        assert_eq!(f_lgamma(-4.039410591125488), -0.001305303022594149);
        assert_eq!(f_lgamma(-2.000001907348633), 12.47664749001284);
        assert_eq!(
            f_lgamma(0.0000000000000006939032951805219),
            34.90419906721559
        );
        assert_eq!(f_lgamma(0.9743590354901843), 0.01534797880086699);
        assert_eq!(f_lgamma(1.9533844296518055), -0.019000687007583488);
        assert_eq!(f_lgamma(1.9614259600725743), -0.015824770893504085);
        assert_eq!(f_lgamma(1.961426168688831), -0.015824687947423532);
        assert_eq!(f_lgamma(2.0000000026484486), 0.0000000011197225706325235);
        assert_eq!(f_lgamma(f64::INFINITY), f64::INFINITY);
        assert!(f_lgamma(f64::NAN).is_nan());
        // assert_eq!(f_lgamma(1.2902249255008019e301),
        //            8932652024571557000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.);
        assert_eq!(
            f_lgamma(2.000000000000014),
            0.000000000000006008126761947661
        );
        assert_eq!(f_lgamma(-2.74999999558122), 0.004487888879321723);
        assert_eq!(
            f_lgamma(0.00000000000001872488985570349),
            31.608922747730112
        );
        assert_eq!(f_lgamma(-2.7484742253727745), 0.0015213685011468314);
        assert_eq!(f_lgamma(0.9759521409866919), 0.014362097695996162);
    }
}
