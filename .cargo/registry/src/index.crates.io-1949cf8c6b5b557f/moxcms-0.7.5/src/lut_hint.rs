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
use crate::LutWarehouse;

impl LutWarehouse {
    /// Method tests if mathematical fusion on LUT table is allowed.
    /// If it's not, full brute-force pass in [Katana] is required.
    pub(crate) fn is_katana_required(&self) -> bool {
        match self {
            LutWarehouse::Lut(lut) => {
                let input_entries = lut.num_input_channels as usize;
                let output_entries = lut.num_output_channels as usize;
                for i in 0..input_entries {
                    if lut.input_table.is_degenerated(input_entries, i) {
                        return true;
                    }
                    if !lut.input_table.is_monotonic(input_entries, i) {
                        return true;
                    }
                    if lut.input_table.have_discontinuities(input_entries, i) {
                        return true;
                    }
                }

                for i in 0..output_entries {
                    if lut.output_table.is_degenerated(output_entries, i) {
                        return true;
                    }
                    if !lut.output_table.is_monotonic(output_entries, i) {
                        return true;
                    }
                    if lut.output_table.have_discontinuities(output_entries, i) {
                        return true;
                    }
                }

                false
            }
            LutWarehouse::Multidimensional(mab) => {
                for curve in mab.a_curves.iter() {
                    if curve.is_degenerated() {
                        return true;
                    }
                    if !curve.is_monotonic() {
                        return true;
                    }
                    if curve.have_discontinuities() {
                        return true;
                    }
                }

                for curve in mab.m_curves.iter() {
                    if curve.is_degenerated() {
                        return true;
                    }
                    if !curve.is_monotonic() {
                        return true;
                    }
                    if curve.have_discontinuities() {
                        return true;
                    }
                }

                for curve in mab.b_curves.iter() {
                    if curve.is_degenerated() {
                        return true;
                    }
                    if !curve.is_monotonic() {
                        return true;
                    }
                    if curve.have_discontinuities() {
                        return true;
                    }
                }

                false
            }
        }
    }
}
