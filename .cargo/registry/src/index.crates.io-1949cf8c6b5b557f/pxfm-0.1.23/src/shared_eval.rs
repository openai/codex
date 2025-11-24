/*
 * // Copyright (c) Radzivon Bartoshyk 7/2025. All rights reserved.
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

#[inline]
pub(crate) fn poly_dd_3(x: DoubleDouble, poly: [(u64, u64); 3], l: f64) -> DoubleDouble {
    let zch = poly[2];
    let ach = f64::from_bits(zch.0) + l;
    let acl = (f64::from_bits(zch.0) - ach) + l + f64::from_bits(zch.1);
    let mut ch = DoubleDouble::new(acl, ach);

    let zch = poly[1];
    ch = DoubleDouble::quick_mult(ch, x);
    let th = ch.hi + f64::from_bits(zch.0);
    let tl = (f64::from_bits(zch.0) - th) + ch.hi;
    ch.hi = th;
    ch.lo += tl + f64::from_bits(zch.1);

    let zch = poly[0];
    ch = DoubleDouble::quick_mult(ch, x);
    let th = ch.hi + f64::from_bits(zch.0);
    let tl = (f64::from_bits(zch.0) - th) + ch.hi;
    ch.hi = th;
    ch.lo += tl + f64::from_bits(zch.1);

    ch
}

#[inline]
pub(crate) fn poly_dekker_generic<const N: usize>(
    x: DoubleDouble,
    poly: [(u64, u64); N],
) -> DoubleDouble {
    let zch = poly.last().unwrap();
    let ach = f64::from_bits(zch.0);
    let acl = f64::from_bits(zch.1);
    let mut ch = DoubleDouble::new(acl, ach);

    for zch in poly.iter().rev().skip(1) {
        ch = DoubleDouble::quick_mult(ch, x);
        let th = ch.hi + f64::from_bits(zch.0);
        let tl = (f64::from_bits(zch.0) - th) + ch.hi;
        ch.hi = th;
        ch.lo += tl + f64::from_bits(zch.1);
    }

    ch
}
