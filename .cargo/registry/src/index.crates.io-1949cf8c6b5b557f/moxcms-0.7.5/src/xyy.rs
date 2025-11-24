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
use crate::{Xyz, Xyzd};

/// Holds CIE XyY representation
#[derive(Clone, Debug, Copy, Default)]
pub struct XyY {
    pub x: f64,
    pub y: f64,
    pub yb: f64,
}

pub trait XyYRepresentable {
    fn to_xyy(self) -> XyY;
}

impl XyYRepresentable for XyY {
    #[inline]
    fn to_xyy(self) -> XyY {
        self
    }
}

impl XyY {
    #[inline]
    pub const fn new(x: f64, y: f64, yb: f64) -> Self {
        Self { x, y, yb }
    }

    #[inline]
    pub const fn to_xyz(self) -> Xyz {
        let reciprocal = if self.y != 0. {
            1. / self.y * self.yb
        } else {
            0.
        };
        Xyz {
            x: (self.x * reciprocal) as f32,
            y: self.yb as f32,
            z: ((1. - self.x - self.y) * reciprocal) as f32,
        }
    }

    #[inline]
    pub const fn to_xyzd(self) -> Xyzd {
        let reciprocal = if self.y != 0. {
            1. / self.y * self.yb
        } else {
            0.
        };
        Xyzd {
            x: self.x * reciprocal,
            y: self.yb,
            z: (1. - self.x - self.y) * reciprocal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xyzd_xyy() {
        let xyy = XyY::new(0.2, 0.4, 0.5);
        let xyy = xyy.to_xyzd();
        let r_xyy = xyy.to_xyzd();
        assert!((r_xyy.x - xyy.x).abs() < 1e-5);
        assert!((r_xyy.y - xyy.y).abs() < 1e-5);
        assert!((r_xyy.z - xyy.z).abs() < 1e-5);
    }
}
