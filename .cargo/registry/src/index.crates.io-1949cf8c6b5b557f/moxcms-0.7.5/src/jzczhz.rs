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
use crate::Xyz;
use crate::jzazbz::Jzazbz;
use num_traits::Pow;
use pxfm::{f_atan2f, f_cbrtf, f_hypot3f, f_hypotf, f_powf, f_sincosf, f_sinf};
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

/// Represents Jzazbz in polar coordinates as Jzczhz
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub struct Jzczhz {
    /// Jz(lightness) generally expects to be between `0.0..1.0`.
    pub jz: f32,
    /// Cz generally expects to be between `-1.0..1.0`.
    pub cz: f32,
    /// Hz generally expects to be between `-1.0..1.0`.
    pub hz: f32,
}

impl Jzczhz {
    /// Creates new instance of Jzczhz
    #[inline]
    pub fn new(jz: f32, cz: f32, hz: f32) -> Jzczhz {
        Jzczhz { jz, cz, hz }
    }

    /// Converts Jzazbz to polar coordinates Jzczhz
    #[inline]
    pub fn from_jzazbz(jzazbz: Jzazbz) -> Jzczhz {
        let cz = f_hypotf(jzazbz.az, jzazbz.bz);
        let hz = f_atan2f(jzazbz.bz, jzazbz.az);
        Jzczhz::new(jzazbz.jz, cz, hz)
    }

    /// Converts Jzczhz into Jzazbz
    #[inline]
    pub fn to_jzazbz(&self) -> Jzazbz {
        let sincos = f_sincosf(self.hz);
        let az = self.cz * sincos.1;
        let bz = self.cz * sincos.0;
        Jzazbz::new(self.jz, az, bz)
    }

    /// Converts Jzczhz into Jzazbz
    #[inline]
    pub fn to_jzazbz_with_luminance(&self) -> Jzazbz {
        let sincos = f_sincosf(self.hz);
        let az = self.cz * sincos.1;
        let bz = self.cz * sincos.0;
        Jzazbz::new(self.jz, az, bz)
    }

    /// Converts Jzczhz to *Xyz*
    #[inline]
    pub fn to_xyz(&self, display_luminance: f32) -> Xyz {
        let jzazbz = self.to_jzazbz();
        jzazbz.to_xyz(display_luminance)
    }

    /// Converts [Xyz] to [Jzczhz]
    #[inline]
    pub fn from_xyz(xyz: Xyz) -> Jzczhz {
        let jzazbz = Jzazbz::from_xyz(xyz);
        Jzczhz::from_jzazbz(jzazbz)
    }

    /// Converts [Xyz] to [Jzczhz]
    #[inline]
    pub fn from_xyz_with_display_luminance(xyz: Xyz, luminance: f32) -> Jzczhz {
        let jzazbz = Jzazbz::from_xyz_with_display_luminance(xyz, luminance);
        Jzczhz::from_jzazbz(jzazbz)
    }

    /// Computes distance for *Jzczhz*
    #[inline]
    pub fn distance(&self, other: Jzczhz) -> f32 {
        let djz = self.jz - other.jz;
        let dcz = self.cz - other.cz;
        let dhz = self.hz - other.hz;
        let dh = 2. * (self.cz * other.cz).sqrt() * f_sinf(dhz * 0.5);
        f_hypot3f(djz, dcz, dh)
    }

    #[inline]
    pub fn euclidean_distance(&self, other: Self) -> f32 {
        let djz = self.jz - other.jz;
        let dhz = self.hz - other.hz;
        let dcz = self.cz - other.cz;
        (djz * djz + dhz * dhz + dcz * dcz).sqrt()
    }

    #[inline]
    pub fn taxicab_distance(&self, other: Self) -> f32 {
        let djz = self.jz - other.jz;
        let dhz = self.hz - other.hz;
        let dcz = self.cz - other.cz;
        djz.abs() + dhz.abs() + dcz.abs()
    }
}

impl Index<usize> for Jzczhz {
    type Output = f32;

    #[inline]
    fn index(&self, index: usize) -> &f32 {
        match index {
            0 => &self.jz,
            1 => &self.cz,
            2 => &self.hz,
            _ => panic!("Index out of bounds for Jzczhz"),
        }
    }
}

impl IndexMut<usize> for Jzczhz {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut f32 {
        match index {
            0 => &mut self.jz,
            1 => &mut self.cz,
            2 => &mut self.hz,
            _ => panic!("Index out of bounds for Jzczhz"),
        }
    }
}

impl Add<f32> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn add(self, rhs: f32) -> Self::Output {
        Jzczhz::new(self.jz + rhs, self.cz + rhs, self.hz + rhs)
    }
}

impl Sub<f32> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn sub(self, rhs: f32) -> Self::Output {
        Jzczhz::new(self.jz - rhs, self.cz - rhs, self.hz - rhs)
    }
}

impl Mul<f32> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Jzczhz::new(self.jz * rhs, self.cz * rhs, self.hz * rhs)
    }
}

impl Div<f32> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Jzczhz::new(self.jz / rhs, self.cz / rhs, self.hz / rhs)
    }
}

impl Add<Jzczhz> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn add(self, rhs: Jzczhz) -> Self::Output {
        Jzczhz::new(self.jz + rhs.jz, self.cz + rhs.cz, self.hz + rhs.hz)
    }
}

impl Sub<Jzczhz> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn sub(self, rhs: Jzczhz) -> Self::Output {
        Jzczhz::new(self.jz - rhs.jz, self.cz - rhs.cz, self.hz - rhs.hz)
    }
}

impl Mul<Jzczhz> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn mul(self, rhs: Jzczhz) -> Self::Output {
        Jzczhz::new(self.jz * rhs.jz, self.cz * rhs.cz, self.hz * rhs.hz)
    }
}

impl Div<Jzczhz> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn div(self, rhs: Jzczhz) -> Self::Output {
        Jzczhz::new(self.jz / rhs.jz, self.cz / rhs.cz, self.hz / rhs.hz)
    }
}

impl AddAssign<Jzczhz> for Jzczhz {
    #[inline]
    fn add_assign(&mut self, rhs: Jzczhz) {
        self.jz += rhs.jz;
        self.cz += rhs.cz;
        self.hz += rhs.hz;
    }
}

impl SubAssign<Jzczhz> for Jzczhz {
    #[inline]
    fn sub_assign(&mut self, rhs: Jzczhz) {
        self.jz -= rhs.jz;
        self.cz -= rhs.cz;
        self.hz -= rhs.hz;
    }
}

impl MulAssign<Jzczhz> for Jzczhz {
    #[inline]
    fn mul_assign(&mut self, rhs: Jzczhz) {
        self.jz *= rhs.jz;
        self.cz *= rhs.cz;
        self.hz *= rhs.hz;
    }
}

impl DivAssign<Jzczhz> for Jzczhz {
    #[inline]
    fn div_assign(&mut self, rhs: Jzczhz) {
        self.jz /= rhs.jz;
        self.cz /= rhs.cz;
        self.hz /= rhs.hz;
    }
}

impl AddAssign<f32> for Jzczhz {
    #[inline]
    fn add_assign(&mut self, rhs: f32) {
        self.jz += rhs;
        self.cz += rhs;
        self.hz += rhs;
    }
}

impl SubAssign<f32> for Jzczhz {
    #[inline]
    fn sub_assign(&mut self, rhs: f32) {
        self.jz -= rhs;
        self.cz -= rhs;
        self.hz -= rhs;
    }
}

impl MulAssign<f32> for Jzczhz {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.jz *= rhs;
        self.cz *= rhs;
        self.hz *= rhs;
    }
}

impl DivAssign<f32> for Jzczhz {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.jz /= rhs;
        self.cz /= rhs;
        self.hz /= rhs;
    }
}

impl Jzczhz {
    #[inline]
    pub fn sqrt(&self) -> Jzczhz {
        Jzczhz::new(self.jz.sqrt(), self.cz.sqrt(), self.hz.sqrt())
    }

    #[inline]
    pub fn cbrt(&self) -> Jzczhz {
        Jzczhz::new(f_cbrtf(self.jz), f_cbrtf(self.cz), f_cbrtf(self.hz))
    }
}

impl Pow<f32> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn pow(self, rhs: f32) -> Self::Output {
        Jzczhz::new(
            f_powf(self.jz, rhs),
            f_powf(self.cz, rhs),
            f_powf(self.hz, rhs),
        )
    }
}

impl Pow<Jzczhz> for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn pow(self, rhs: Jzczhz) -> Self::Output {
        Jzczhz::new(
            f_powf(self.jz, rhs.jz),
            f_powf(self.cz, self.cz),
            f_powf(self.hz, self.hz),
        )
    }
}

impl Neg for Jzczhz {
    type Output = Jzczhz;

    #[inline]
    fn neg(self) -> Self::Output {
        Jzczhz::new(-self.jz, -self.cz, -self.hz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jzczhz_round() {
        let xyz = Xyz::new(0.5, 0.4, 0.3);
        let jzczhz = Jzczhz::from_xyz_with_display_luminance(xyz, 253.);
        let old_xyz = jzczhz.to_xyz(253f32);
        assert!(
            (xyz.x - old_xyz.x).abs() <= 1e-3,
            "{:?} != {:?}",
            xyz,
            old_xyz
        );
        assert!(
            (xyz.y - old_xyz.y).abs() <= 1e-3,
            "{:?} != {:?}",
            xyz,
            old_xyz
        );
        assert!(
            (xyz.z - old_xyz.z).abs() <= 1e-3,
            "{:?} != {:?}",
            xyz,
            old_xyz
        );
    }
}
