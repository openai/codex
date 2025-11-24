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

#[inline]
pub(crate) const fn get_exponent_f32(x: f32) -> i32 {
    let bits = x.to_bits();
    (((bits >> 23) & 0xFF) as i32).wrapping_sub(127)
}

#[inline]
pub(crate) const fn mantissa_f32(x: f32) -> u32 {
    x.to_bits() & ((1u32 << 23) - 1)
}

#[inline]
pub(crate) const fn mantissa_f64(x: f64) -> u64 {
    x.to_bits() & ((1u64 << 52) - 1)
}

#[inline]
pub(crate) const fn get_exponent_f64(x: f64) -> i64 {
    ((x.to_bits() as i64 & EXP_MASK as i64) >> 52).wrapping_sub(1023)
}

#[inline]
pub(crate) const fn biased_exponent_f64(x: f64) -> i64 {
    (x.to_bits() as i64 & EXP_MASK as i64) >> 52
}

#[inline]
pub(crate) const fn mask_trailing_ones(len: u64) -> u64 {
    if len >= 64 {
        u64::MAX
    } else {
        (1u64 << len).wrapping_sub(1)
    }
}

pub(crate) const EXP_MASK: u64 = mask_trailing_ones(11) << 52;

#[inline]
pub(crate) fn set_exponent_f64(x: u64, new_exp: u64) -> u64 {
    let encoded_mask = new_exp.wrapping_shl(52) & EXP_MASK;
    x ^ ((x ^ encoded_mask) & EXP_MASK)
}

#[inline]
pub(crate) const fn min_normal_f32(sign: bool) -> f32 {
    let sign_bit = if sign { 1u32 << 31 } else { 0 };
    let exponent = 1u32 << 23;
    f32::from_bits(sign_bit | exponent)
}
