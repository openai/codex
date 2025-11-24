/*
 * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
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
use crate::mlaf::mlaf;
use crate::{Matrix3f, Vector3f, Xyz};
use pxfm::{f_atan2f, f_hypotf, f_sincosf};

/// Structure for Yrg colorspace
///
/// Kirk Yrg 2021.
#[repr(C)]
#[derive(Default, Debug, PartialOrd, PartialEq, Copy, Clone)]
pub struct Yrg {
    pub y: f32,
    pub r: f32,
    pub g: f32,
}

/// Structure for cone form of Yrg colorspace
#[repr(C)]
#[derive(Default, Debug, PartialOrd, PartialEq, Copy, Clone)]
pub struct Ych {
    pub y: f32,
    pub c: f32,
    pub h: f32,
}

const LMS_TO_XYZ: Matrix3f = Matrix3f {
    v: [
        [1.8079466, -1.2997167, 0.34785876],
        [0.61783963, 0.39595452, -0.041046873],
        [-0.12546961, 0.20478038, 1.7427418],
    ],
};
const XYZ_TO_LMS: Matrix3f = Matrix3f {
    v: [
        [0.257085, 0.859943, -0.031061],
        [-0.394427, 1.175800, 0.106423],
        [0.064856, -0.076250, 0.559067],
    ],
};

impl Yrg {
    #[inline]
    pub const fn new(y: f32, r: f32, g: f32) -> Yrg {
        Yrg { y, r, g }
    }

    /// Convert [Xyz] D65 to [Yrg]
    ///
    /// Yrg defined in D65 white point. Ensure Xyz values is adapted.
    /// Yrg use CIE XYZ 2006, adapt CIE XYZ 1931 by using [cie_y_1931_to_cie_y_2006] at first.
    #[inline]
    pub fn from_xyz(xyz: Xyz) -> Self {
        let lms = XYZ_TO_LMS.f_mul_vector(Vector3f {
            v: [xyz.x, xyz.y, xyz.z],
        });
        let y = mlaf(0.68990272 * lms.v[0], 0.34832189, lms.v[1]);

        let a = lms.v[0] + lms.v[1] + lms.v[2];
        let l = if a == 0. { 0. } else { lms.v[0] / a };
        let m = if a == 0. { 0. } else { lms.v[1] / a };
        let r = mlaf(mlaf(0.02062, -0.6873, m), 1.0671, l);
        let g = mlaf(mlaf(-0.05155, -0.0362, l), 1.7182, m);
        Yrg { y, r, g }
    }

    #[inline]
    pub fn to_xyz(&self) -> Xyz {
        let l = mlaf(0.95 * self.r, 0.38, self.g);
        let m = mlaf(mlaf(0.03, 0.59, self.g), 0.02, self.r);
        let den = mlaf(0.68990272 * l, 0.34832189, m);
        let a = if den == 0. { 0. } else { self.y / den };
        let l0 = l * a;
        let m0 = m * a;
        let s0 = (1f32 - l - m) * a;
        let v = Vector3f { v: [l0, m0, s0] };
        let x = LMS_TO_XYZ.f_mul_vector(v);
        Xyz {
            x: x.v[0],
            y: x.v[1],
            z: x.v[2],
        }
    }
}

impl Ych {
    #[inline]
    pub const fn new(y: f32, c: f32, h: f32) -> Self {
        Ych { y, c, h }
    }

    #[inline]
    pub fn from_yrg(yrg: Yrg) -> Self {
        let y = yrg.y;
        // Subtract white point. These are the r, g coordinates of
        // sRGB (D50 adapted) (1, 1, 1) taken through
        // XYZ D50 -> CAT16 D50->D65 adaptation -> LMS 2006
        // -> grading RGB conversion.
        let r = yrg.r - 0.21902143;
        let g = yrg.g - 0.54371398;
        let c = f_hypotf(g, r);
        let h = f_atan2f(g, r);
        Self { y, c, h }
    }

    #[inline]
    pub fn to_yrg(&self) -> Yrg {
        let y = self.y;
        let c = self.c;
        let h = self.h;
        let sincos = f_sincosf(h);
        let r = mlaf(0.21902143, c, sincos.1);
        let g = mlaf(0.54371398, c, sincos.0);
        Yrg { y, r, g }
    }
}

// Pipeline and ICC luminance is CIE Y 1931
// Kirk Ych/Yrg uses CIE Y 2006
// 1 CIE Y 1931 = 1.05785528 CIE Y 2006, so we need to adjust that.
// This also accounts for the CAT16 D50->D65 adaptation that has to be done
// to go from RGB to CIE LMS 2006.
// Warning: only applies to achromatic pixels.
pub const fn cie_y_1931_to_cie_y_2006(x: f32) -> f32 {
    1.05785528 * (x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yrg() {
        let xyz = Xyz::new(0.95, 1.0, 1.08);
        let yrg = Yrg::from_xyz(xyz);
        let yrg_to_xyz = yrg.to_xyz();
        assert!((xyz.x - yrg_to_xyz.x) < 1e-5);
        assert!((xyz.y - yrg_to_xyz.y) < 1e-5);
        assert!((xyz.z - yrg_to_xyz.z) < 1e-5);
    }

    #[test]
    fn test_ych() {
        let xyz = Yrg::new(0.5, 0.4, 0.3);
        let yrg = Ych::from_yrg(xyz);
        let yrg_to_xyz = yrg.to_yrg();
        assert!((xyz.y - yrg_to_xyz.y) < 1e-5);
        assert!((xyz.r - yrg_to_xyz.r) < 1e-5);
        assert!((xyz.g - yrg_to_xyz.g) < 1e-5);
    }
}
