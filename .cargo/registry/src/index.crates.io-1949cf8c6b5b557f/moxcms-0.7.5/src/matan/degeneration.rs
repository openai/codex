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

#[derive(Copy, Clone, Default, Debug)]
struct DegenerationAmount {
    leading: usize,
    trailing: usize,
}

/// Counts amount of duplicates on each side of curve
fn count_leading_trailing_duplicated<T: PartialOrd>(lut: &[T]) -> DegenerationAmount {
    if lut.is_empty() {
        return DegenerationAmount::default();
    }
    let first = lut.first().unwrap();
    let last = lut.last().unwrap();
    let leading = lut.iter().take_while(|&v| v.eq(first)).count();
    let trailing = lut.iter().rev().take_while(|&v| v.eq(last)).count();
    DegenerationAmount { leading, trailing }
}

/// Finds out if curve is degenerated on the sides.
pub(crate) fn is_curve_degenerated<T: PartialOrd>(v: &[T]) -> bool {
    if v.is_empty() || v.len() < 2 {
        return false;
    }
    let degeneration_amount = count_leading_trailing_duplicated(v);
    if degeneration_amount.trailing <= 1 && degeneration_amount.leading <= 1 {
        return false;
    }
    let leading_percentage = degeneration_amount.leading;
    let trailing_percentage = degeneration_amount.trailing;
    ((leading_percentage / 20) > 0) || ((trailing_percentage / 20) > 0)
}
