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

/* LOG10P1_EXACT_INT_TABLE[e] is zero if there is no value of the form 10^n-1 in the range
[2^e, 2^(e+1)), otherwise it is this (unique) value. */
pub(crate) static LOG10P1_EXACT_INT_TABLE: [u64; 50] = [
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x4022000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x4058c00000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x408f380000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x40c3878000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x40f869f000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x412e847e00000000,
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x416312cfe0000000,
    0x0000000000000000,
    0x0000000000000000,
    0x4197d783fc000000,
    0x0000000000000000,
    0x0000000000000000,
    0x41cdcd64ff800000,
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x4202a05f1ff80000,
    0x0000000000000000,
    0x0000000000000000,
    0x42374876e7ff0000,
    0x0000000000000000,
    0x0000000000000000,
    0x426d1a94a1ffe000,
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
    0x42a2309ce53ffe00,
    0x0000000000000000,
    0x0000000000000000,
    0x42d6bcc41e8fffc0,
    0x0000000000000000,
    0x0000000000000000,
    0x430c6bf52633fff8,
];

// LOG10P1_EXACT_INT_S_TABLE[e] is the integer n such that LOG10P1_EXACT_INT_TABLE[e] = 10^n-1 when LOG10P1_EXACT_INT_TABLE[e] is not zero
pub(crate) static LOG10P1_EXACT_INT_S_TABLE: [u32; 50] = [
    0, 0, 0, 1, 0, 0, 2, 0, 0, 3, 0, 0, 0, 4, 0, 0, 5, 0, 0, 6, 0, 0, 0, 7, 0, 0, 8, 0, 0, 9, 0, 0,
    0, 10, 0, 0, 11, 0, 0, 12, 0, 0, 0, 13, 0, 0, 14, 0, 0, 15,
];
