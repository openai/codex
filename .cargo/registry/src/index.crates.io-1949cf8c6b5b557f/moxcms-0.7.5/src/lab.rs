/*
 * // Copyright (c) Radzivon Bartoshyk 2/2025. All rights reserved.
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
use crate::mlaf::{fmla, mlaf};
use crate::{Chromaticity, LCh, Xyz};
use pxfm::f_cbrtf;

/// Holds CIE LAB values
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialOrd, PartialEq)]
pub struct Lab {
    /// `l`: lightness component (0 to 100)
    pub l: f32,
    /// `a`: green (negative) and red (positive) component.
    pub a: f32,
    /// `b`: blue (negative) and yellow (positive) component
    pub b: f32,
}

impl Lab {
    /// Create a new CIELAB color.
    ///
    /// # Arguments
    ///
    /// * `l`: lightness component (0 to 100).
    /// * `a`: green (negative) and red (positive) component.
    /// * `b`: blue (negative) and yellow (positive) component.
    #[inline]
    pub const fn new(l: f32, a: f32, b: f32) -> Self {
        Self { l, a, b }
    }
}

#[inline(always)]
const fn f_1(t: f32) -> f32 {
    if t <= 24.0 / 116.0 {
        (108.0 / 841.0) * (t - 16.0 / 116.0)
    } else {
        t * t * t
    }
}

#[inline(always)]
fn f(t: f32) -> f32 {
    if t <= 24. / 116. * (24. / 116.) * (24. / 116.) {
        (841. / 108. * t) + 16. / 116.
    } else {
        f_cbrtf(t)
    }
}

impl Lab {
    /// Converts to CIE Lab from CIE XYZ for PCS encoding
    #[inline]
    pub fn from_pcs_xyz(xyz: Xyz) -> Self {
        const WP: Xyz = Chromaticity::D50.to_xyz();
        let device_x = (xyz.x as f64 * (1.0f64 + 32767.0f64 / 32768.0f64) / WP.x as f64) as f32;
        let device_y = (xyz.y as f64 * (1.0f64 + 32767.0f64 / 32768.0f64) / WP.y as f64) as f32;
        let device_z = (xyz.z as f64 * (1.0f64 + 32767.0f64 / 32768.0f64) / WP.z as f64) as f32;

        let fx = f(device_x);
        let fy = f(device_y);
        let fz = f(device_z);

        let lb = mlaf(-16.0, 116.0, fy);
        let a = 500.0 * (fx - fy);
        let b = 200.0 * (fy - fz);

        let l = lb / 100.0;
        let a = (a + 128.0) / 255.0;
        let b = (b + 128.0) / 255.0;
        Self::new(l, a, b)
    }

    /// Converts to CIE Lab from CIE XYZ
    #[inline]
    pub fn from_xyz(xyz: Xyz) -> Self {
        const WP: Xyz = Chromaticity::D50.to_xyz();
        let device_x = (xyz.x as f64 * (1.0f64 + 32767.0f64 / 32768.0f64) / WP.x as f64) as f32;
        let device_y = (xyz.y as f64 * (1.0f64 + 32767.0f64 / 32768.0f64) / WP.y as f64) as f32;
        let device_z = (xyz.z as f64 * (1.0f64 + 32767.0f64 / 32768.0f64) / WP.z as f64) as f32;

        let fx = f(device_x);
        let fy = f(device_y);
        let fz = f(device_z);

        let lb = mlaf(-16.0, 116.0, fy);
        let a = 500.0 * (fx - fy);
        let b = 200.0 * (fy - fz);

        Self::new(lb, a, b)
    }

    /// Converts CIE [Lab] into CIE [Xyz] for PCS encoding
    #[inline]
    pub fn to_pcs_xyz(self) -> Xyz {
        let device_l = self.l * 100.0;
        let device_a = fmla(self.a, 255.0, -128.0);
        let device_b = fmla(self.b, 255.0, -128.0);

        let y = (device_l + 16.0) / 116.0;

        const WP: Xyz = Chromaticity::D50.to_xyz();

        let x = f_1(mlaf(y, 0.002, device_a)) * WP.x;
        let y1 = f_1(y) * WP.y;
        let z = f_1(mlaf(y, -0.005, device_b)) * WP.z;

        let x = (x as f64 / (1.0f64 + 32767.0f64 / 32768.0f64)) as f32;
        let y = (y1 as f64 / (1.0f64 + 32767.0f64 / 32768.0f64)) as f32;
        let z = (z as f64 / (1.0f64 + 32767.0f64 / 32768.0f64)) as f32;
        Xyz::new(x, y, z)
    }

    /// Converts CIE [Lab] into CIE [Xyz]
    #[inline]
    pub fn to_xyz(self) -> Xyz {
        let device_l = self.l;
        let device_a = self.a;
        let device_b = self.b;

        let y = (device_l + 16.0) / 116.0;

        const WP: Xyz = Chromaticity::D50.to_xyz();

        let x = f_1(mlaf(y, 0.002, device_a)) * WP.x;
        let y1 = f_1(y) * WP.y;
        let z = f_1(mlaf(y, -0.005, device_b)) * WP.z;

        let x = (x as f64 / (1.0f64 + 32767.0f64 / 32768.0f64)) as f32;
        let y = (y1 as f64 / (1.0f64 + 32767.0f64 / 32768.0f64)) as f32;
        let z = (z as f64 / (1.0f64 + 32767.0f64 / 32768.0f64)) as f32;
        Xyz::new(x, y, z)
    }

    /// Desaturates out of gamut PCS encoded LAB
    pub fn desaturate_pcs(self) -> Lab {
        if self.l < 0. {
            return Lab::new(0., 0., 0.);
        }

        let mut new_lab = self;
        if new_lab.l > 1. {
            new_lab.l = 1.;
        }

        let amax = 1.0;
        let amin = 0.0;
        let bmin = 0.0;
        let bmax = 1.0;
        if self.a < amin || self.a > amax || self.b < bmin || self.b > bmax {
            if self.a == 0.0 {
                // Is hue exactly 90?

                // atan will not work, so clamp here
                new_lab.b = if new_lab.b < bmin { bmin } else { bmax };
                return Lab::new(self.l, self.a, self.b);
            }

            let lch = LCh::from_lab(new_lab);

            let slope = new_lab.b / new_lab.a;
            let h = lch.h * (180.0 / std::f32::consts::PI);

            // There are 4 zones
            if (0. ..45.).contains(&h) || (315. ..=360.).contains(&h) {
                // clip by amax
                new_lab.a = amax;
                new_lab.b = amax * slope;
            } else if (45. ..135.).contains(&h) {
                // clip by bmax
                new_lab.b = bmax;
                new_lab.a = bmax / slope;
            } else if (135. ..225.).contains(&h) {
                // clip by amin
                new_lab.a = amin;
                new_lab.b = amin * slope;
            } else if (225. ..315.).contains(&h) {
                // clip by bmin
                new_lab.b = bmin;
                new_lab.a = bmin / slope;
            }
        }
        new_lab
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let xyz = Xyz::new(0.1, 0.2, 0.3);
        let lab = Lab::from_xyz(xyz);
        let rolled_back = lab.to_xyz();
        let dx = (xyz.x - rolled_back.x).abs();
        let dy = (xyz.y - rolled_back.y).abs();
        let dz = (xyz.z - rolled_back.z).abs();
        assert!(dx < 1e-5);
        assert!(dy < 1e-5);
        assert!(dz < 1e-5);
    }

    #[test]
    fn round_pcs_trip() {
        let xyz = Xyz::new(0.1, 0.2, 0.3);
        let lab = Lab::from_pcs_xyz(xyz);
        let rolled_back = lab.to_pcs_xyz();
        let dx = (xyz.x - rolled_back.x).abs();
        let dy = (xyz.y - rolled_back.y).abs();
        let dz = (xyz.z - rolled_back.z).abs();
        assert!(dx < 1e-5);
        assert!(dy < 1e-5);
        assert!(dz < 1e-5);
    }
}
