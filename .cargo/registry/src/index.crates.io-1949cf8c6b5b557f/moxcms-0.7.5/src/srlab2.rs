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
use crate::Xyz;
use crate::mlaf::mlaf;
use pxfm::f_cbrtf;

#[inline]
fn srlab2_gamma(x: f32) -> f32 {
    if x <= 216. / 24389. {
        x * (24389. / 2700.)
    } else {
        1.16 * f_cbrtf(x) - 0.16
    }
}

#[inline]
fn srlab2_linearize(x: f32) -> f32 {
    if x <= 0.08 {
        x * (2700.0 / 24389.0)
    } else {
        let zx = (x + 0.16) / 1.16;
        zx * zx * zx
    }
}

#[derive(Copy, Clone, Debug, Default, PartialOrd, PartialEq)]
pub struct Srlab2 {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl Srlab2 {
    #[inline]
    pub const fn new(l: f32, a: f32, b: f32) -> Srlab2 {
        Srlab2 { l, a, b }
    }

    #[inline]
    pub fn from_xyz(xyz: Xyz) -> Srlab2 {
        let lx = srlab2_gamma(xyz.x);
        let ly = srlab2_gamma(xyz.y);
        let lz = srlab2_gamma(xyz.z);

        let l = mlaf(mlaf(0.629054 * ly, -0.000008, lz), 0.37095, lx);
        let a = mlaf(mlaf(6.634684 * lx, -7.505078, ly), 0.870328, lz);
        let b = mlaf(mlaf(0.639569 * lx, 1.084576, ly), -1.724152, lz);
        Srlab2 { l, a, b }
    }

    #[inline]
    pub fn to_xyz(&self) -> Xyz {
        let x = mlaf(mlaf(self.l, 0.09041272, self.a), 0.045634452, self.b);
        let y = mlaf(mlaf(self.l, -0.05331593, self.a), -0.026917785, self.b);
        let z = mlaf(self.l, -0.58, self.b);
        let lx = srlab2_linearize(x);
        let ly = srlab2_linearize(y);
        let lz = srlab2_linearize(z);
        Xyz::new(lx, ly, lz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srlab2() {
        let xyz = Xyz::new(0.3, 0.65, 0.66);
        let srlab2 = Srlab2::from_xyz(xyz);
        let r_xyz = srlab2.to_xyz();
        assert!((r_xyz.x - xyz.x).abs() < 1e-5);
        assert!((r_xyz.y - xyz.y).abs() < 1e-5);
        assert!((r_xyz.z - xyz.z).abs() < 1e-5);
    }
}
