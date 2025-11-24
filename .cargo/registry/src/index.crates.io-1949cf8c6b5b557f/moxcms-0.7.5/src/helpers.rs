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
use crate::matan::{
    does_curve_have_discontinuity, is_curve_ascending, is_curve_degenerated, is_curve_descending,
    is_curve_linear8, is_curve_linear16, is_curve_monotonic,
};
use crate::reader::{
    s15_fixed16_number_to_double, uint8_number_to_float_fast, uint16_number_to_float_fast,
};
use crate::{CmsError, LutStore, Matrix3d, ToneReprCurve, Vector3d};

impl LutStore {
    pub fn to_clut_f32(&self) -> Vec<f32> {
        match self {
            LutStore::Store8(store) => store
                .iter()
                .map(|x| uint8_number_to_float_fast(*x))
                .collect(),
            LutStore::Store16(store) => store
                .iter()
                .map(|x| uint16_number_to_float_fast(*x as u32))
                .collect(),
        }
    }

    pub(crate) fn is_degenerated(&self, entries: usize, channel: usize) -> bool {
        let start = entries * channel;
        let end = start + entries;

        match &self {
            LutStore::Store8(v) => is_curve_degenerated(&v[start..end]),
            LutStore::Store16(v) => is_curve_degenerated(&v[start..end]),
        }
    }

    pub(crate) fn is_monotonic(&self, entries: usize, channel: usize) -> bool {
        let start = entries * channel;
        let end = start + entries;

        match &self {
            LutStore::Store8(v) => is_curve_monotonic(&v[start..end]),
            LutStore::Store16(v) => is_curve_monotonic(&v[start..end]),
        }
    }

    pub(crate) fn have_discontinuities(&self, entries: usize, channel: usize) -> bool {
        let start = entries * channel;
        let end = start + entries;

        match &self {
            LutStore::Store8(v) => does_curve_have_discontinuity(&v[start..end]),
            LutStore::Store16(v) => does_curve_have_discontinuity(&v[start..end]),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_linear(&self, entries: usize, channel: usize) -> bool {
        let start = entries * channel;
        let end = start + entries;

        match &self {
            LutStore::Store8(v) => is_curve_linear8(&v[start..end]),
            LutStore::Store16(v) => is_curve_linear16(&v[start..end]),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_descending(&self, entries: usize, channel: usize) -> bool {
        let start = entries * channel;
        let end = start + entries;

        match &self {
            LutStore::Store8(v) => is_curve_descending(&v[start..end]),
            LutStore::Store16(v) => is_curve_descending(&v[start..end]),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_ascending(&self, entries: usize, channel: usize) -> bool {
        let start = entries * channel;
        let end = start + entries;

        match &self {
            LutStore::Store8(v) => is_curve_ascending(&v[start..end]),
            LutStore::Store16(v) => is_curve_ascending(&v[start..end]),
        }
    }
}

impl ToneReprCurve {
    pub(crate) fn is_linear(&self) -> bool {
        match &self {
            ToneReprCurve::Lut(lut) => {
                if lut.is_empty() {
                    return true;
                }
                if lut.len() == 1 {
                    let gamma = 1. / crate::trc::u8_fixed_8number_to_float(lut[0]);
                    if (gamma - 1.).abs() < 1e-4 {
                        return true;
                    }
                }
                is_curve_linear16(lut)
            }
            ToneReprCurve::Parametric(parametric) => {
                if parametric.is_empty() {
                    return true;
                }
                if parametric.len() == 1 && parametric[0] == 1. {
                    return true;
                }
                false
            }
        }
    }

    pub(crate) fn is_monotonic(&self) -> bool {
        match &self {
            ToneReprCurve::Lut(lut) => is_curve_monotonic(lut),
            ToneReprCurve::Parametric(_) => true,
        }
    }

    pub(crate) fn is_degenerated(&self) -> bool {
        match &self {
            ToneReprCurve::Lut(lut) => is_curve_degenerated(lut),
            ToneReprCurve::Parametric(_) => false,
        }
    }

    pub(crate) fn have_discontinuities(&self) -> bool {
        match &self {
            ToneReprCurve::Lut(lut) => does_curve_have_discontinuity(lut),
            ToneReprCurve::Parametric(_) => false,
        }
    }
}

pub(crate) fn read_matrix_3d(arr: &[u8]) -> Result<Matrix3d, CmsError> {
    if arr.len() < 36 {
        return Err(CmsError::InvalidProfile);
    }

    let m_tag = &arr[..36];

    let e00 = i32::from_be_bytes([m_tag[0], m_tag[1], m_tag[2], m_tag[3]]);
    let e01 = i32::from_be_bytes([m_tag[4], m_tag[5], m_tag[6], m_tag[7]]);
    let e02 = i32::from_be_bytes([m_tag[8], m_tag[9], m_tag[10], m_tag[11]]);

    let e10 = i32::from_be_bytes([m_tag[12], m_tag[13], m_tag[14], m_tag[15]]);
    let e11 = i32::from_be_bytes([m_tag[16], m_tag[17], m_tag[18], m_tag[19]]);
    let e12 = i32::from_be_bytes([m_tag[20], m_tag[21], m_tag[22], m_tag[23]]);

    let e20 = i32::from_be_bytes([m_tag[24], m_tag[25], m_tag[26], m_tag[27]]);
    let e21 = i32::from_be_bytes([m_tag[28], m_tag[29], m_tag[30], m_tag[31]]);
    let e22 = i32::from_be_bytes([m_tag[32], m_tag[33], m_tag[34], m_tag[35]]);

    Ok(Matrix3d {
        v: [
            [
                s15_fixed16_number_to_double(e00),
                s15_fixed16_number_to_double(e01),
                s15_fixed16_number_to_double(e02),
            ],
            [
                s15_fixed16_number_to_double(e10),
                s15_fixed16_number_to_double(e11),
                s15_fixed16_number_to_double(e12),
            ],
            [
                s15_fixed16_number_to_double(e20),
                s15_fixed16_number_to_double(e21),
                s15_fixed16_number_to_double(e22),
            ],
        ],
    })
}

pub(crate) fn read_vector_3d(arr: &[u8]) -> Result<Vector3d, CmsError> {
    if arr.len() < 12 {
        return Err(CmsError::InvalidProfile);
    }

    let m_tag = &arr[..12];

    let b0 = i32::from_be_bytes([m_tag[0], m_tag[1], m_tag[2], m_tag[3]]);
    let b1 = i32::from_be_bytes([m_tag[4], m_tag[5], m_tag[6], m_tag[7]]);
    let b2 = i32::from_be_bytes([m_tag[8], m_tag[9], m_tag[10], m_tag[11]]);

    Ok(Vector3d {
        v: [
            s15_fixed16_number_to_double(b0),
            s15_fixed16_number_to_double(b1),
            s15_fixed16_number_to_double(b2),
        ],
    })
}
