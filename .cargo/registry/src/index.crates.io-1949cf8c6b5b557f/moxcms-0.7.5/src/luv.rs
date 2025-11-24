/*
 * // Copyright 2024 (c) the Radzivon Bartoshyk. All rights reserved.
 * //
 * // Use of this source code is governed by a BSD-style
 * // license that can be found in the LICENSE file.
 */

//! # Luv
/// Struct representing a color in CIE LUV, a.k.a. L\*u\*v\*, color space
#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialOrd)]
pub struct Luv {
    /// The L\* value (achromatic luminance) of the colour in 0–100 range.
    pub l: f32,
    /// The u\* value of the colour.
    ///
    /// Together with v\* value, it defines chromaticity of the colour.  The u\*
    /// coordinate represents colour’s position on red-green axis with negative
    /// values indicating more red and positive more green colour.  Typical
    /// values are in -134–220 range (but exact range for ‘valid’ colours
    /// depends on luminance and v\* value).
    pub u: f32,
    /// The u\* value of the colour.
    ///
    /// Together with u\* value, it defines chromaticity of the colour.  The v\*
    /// coordinate represents colour’s position on blue-yellow axis with
    /// negative values indicating more blue and positive more yellow colour.
    /// Typical values are in -140–122 range (but exact range for ‘valid’
    /// colours depends on luminance and u\* value).
    pub v: f32,
}

/// Representing a color in cylindrical CIE LCh(uv) color space
#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialOrd)]
pub struct LCh {
    /// The L\* value (achromatic luminance) of the colour in 0–100 range.
    ///
    /// This is the same value as in the [`Luv`] object.
    pub l: f32,
    /// The C\*_uv value (chroma) of the colour.
    ///
    /// Together with h_uv, it defines chromaticity of the colour.  The typical
    /// values of the coordinate go from zero up to around 150 (but exact range
    /// for ‘valid’ colours depends on luminance and hue).  Zero represents
    /// shade of grey.
    pub c: f32,
    /// The h_uv value (hue) of the colour measured in radians.
    ///
    /// Together with C\*_uv, it defines chromaticity of the colour.  The value
    /// represents an angle thus it wraps around τ.  Typically, the value will
    /// be in the -π–π range.  The value is undefined if C\*_uv is zero.
    pub h: f32,
}

use crate::mlaf::mlaf;
use crate::{Chromaticity, Lab, Xyz};
use num_traits::Pow;
use pxfm::{f_atan2f, f_cbrtf, f_hypotf, f_powf, f_sincosf};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub(crate) const LUV_WHITE_U_PRIME: f32 = 4.0f32 * Chromaticity::D50.to_xyz().y
    / (Chromaticity::D50.to_xyz().x
        + 15.0 * Chromaticity::D50.to_xyz().y
        + 3.0 * Chromaticity::D50.to_xyz().z);
pub(crate) const LUV_WHITE_V_PRIME: f32 = 9.0f32 * Chromaticity::D50.to_xyz().y
    / (Chromaticity::D50.to_xyz().x
        + 15.0 * Chromaticity::D50.to_xyz().y
        + 3.0 * Chromaticity::D50.to_xyz().z);

pub(crate) const LUV_CUTOFF_FORWARD_Y: f32 = (6f32 / 29f32) * (6f32 / 29f32) * (6f32 / 29f32);
pub(crate) const LUV_MULTIPLIER_FORWARD_Y: f32 = (29f32 / 3f32) * (29f32 / 3f32) * (29f32 / 3f32);
pub(crate) const LUV_MULTIPLIER_INVERSE_Y: f32 = (3f32 / 29f32) * (3f32 / 29f32) * (3f32 / 29f32);
impl Luv {
    /// Converts CIE XYZ to CIE Luv using D50 white point
    #[inline]
    #[allow(clippy::manual_clamp)]
    pub fn from_xyz(xyz: Xyz) -> Self {
        let [x, y, z] = [xyz.x, xyz.y, xyz.z];
        let den = mlaf(mlaf(x, 15.0, y), 3.0, z);

        let l = (if y < LUV_CUTOFF_FORWARD_Y {
            LUV_MULTIPLIER_FORWARD_Y * y
        } else {
            116. * f_cbrtf(y) - 16.
        })
        .min(100.)
        .max(0.);
        let (u, v);
        if den != 0f32 {
            let u_prime = 4. * x / den;
            let v_prime = 9. * y / den;
            u = 13. * l * (u_prime - LUV_WHITE_U_PRIME);
            v = 13. * l * (v_prime - LUV_WHITE_V_PRIME);
        } else {
            u = 0.;
            v = 0.;
        }

        Luv { l, u, v }
    }

    /// To [Xyz] using D50 colorimetry
    #[inline]
    pub fn to_xyz(&self) -> Xyz {
        if self.l <= 0. {
            return Xyz::new(0., 0., 0.);
        }
        let l13 = 1. / (13. * self.l);
        let u = mlaf(LUV_WHITE_U_PRIME, self.u, l13);
        let v = mlaf(LUV_WHITE_V_PRIME, self.v, l13);
        let y = if self.l > 8. {
            let jx = (self.l + 16.) / 116.;
            jx * jx * jx
        } else {
            self.l * LUV_MULTIPLIER_INVERSE_Y
        };
        let (x, z);
        if v != 0. {
            let den = 1. / (4. * v);
            x = y * 9. * u * den;
            z = y * mlaf(mlaf(12.0, -3.0, u), -20., v) * den;
        } else {
            x = 0.;
            z = 0.;
        }

        Xyz::new(x, y, z)
    }

    #[inline]
    pub const fn new(l: f32, u: f32, v: f32) -> Luv {
        Luv { l, u, v }
    }
}

impl LCh {
    #[inline]
    pub const fn new(l: f32, c: f32, h: f32) -> Self {
        LCh { l, c, h }
    }

    /// Converts Lab to LCh(uv)
    #[inline]
    pub fn from_luv(luv: Luv) -> Self {
        LCh {
            l: luv.l,
            c: f_hypotf(luv.u, luv.v),
            h: f_atan2f(luv.v, luv.u),
        }
    }

    /// Converts Lab to LCh(ab)
    #[inline]
    pub fn from_lab(lab: Lab) -> Self {
        LCh {
            l: lab.l,
            c: f_hypotf(lab.a, lab.b),
            h: f_atan2f(lab.b, lab.a),
        }
    }

    /// Computes LCh(uv)
    #[inline]
    pub fn from_xyz(xyz: Xyz) -> Self {
        Self::from_luv(Luv::from_xyz(xyz))
    }

    /// Computes LCh(ab)
    #[inline]
    pub fn from_xyz_lab(xyz: Xyz) -> Self {
        Self::from_lab(Lab::from_xyz(xyz))
    }

    /// Converts LCh(uv) to Luv
    #[inline]
    pub fn to_xyz(&self) -> Xyz {
        self.to_luv().to_xyz()
    }

    /// Converts LCh(ab) to Lab
    #[inline]
    pub fn to_xyz_lab(&self) -> Xyz {
        self.to_lab().to_xyz()
    }

    #[inline]
    pub fn to_luv(&self) -> Luv {
        let sincos = f_sincosf(self.h);
        Luv {
            l: self.l,
            u: self.c * sincos.1,
            v: self.c * sincos.0,
        }
    }

    #[inline]
    pub fn to_lab(&self) -> Lab {
        let sincos = f_sincosf(self.h);
        Lab {
            l: self.l,
            a: self.c * sincos.1,
            b: self.c * sincos.0,
        }
    }
}

impl PartialEq<Luv> for Luv {
    /// Compares two colours ignoring chromaticity if L\* is zero.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if self.l != other.l {
            false
        } else if self.l == 0.0 {
            true
        } else {
            self.u == other.u && self.v == other.v
        }
    }
}

impl PartialEq<LCh> for LCh {
    /// Compares two colours ignoring chromaticity if L\* is zero and hue if C\*
    /// is zero.  Hues which are τ apart are compared equal.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if self.l != other.l {
            false
        } else if self.l == 0.0 {
            true
        } else if self.c != other.c {
            false
        } else if self.c == 0.0 {
            true
        } else {
            use std::f32::consts::TAU;
            self.h.rem_euclid(TAU) == other.h.rem_euclid(TAU)
        }
    }
}

impl Luv {
    #[inline]
    pub fn euclidean_distance(&self, other: Luv) -> f32 {
        let dl = self.l - other.l;
        let du = self.u - other.u;
        let dv = self.v - other.v;
        (dl * dl + du * du + dv * dv).sqrt()
    }
}

impl LCh {
    #[inline]
    pub fn euclidean_distance(&self, other: LCh) -> f32 {
        let dl = self.l - other.l;
        let dc = self.c - other.c;
        let dh = self.h - other.h;
        (dl * dl + dc * dc + dh * dh).sqrt()
    }
}

impl Luv {
    #[inline]
    pub const fn taxicab_distance(&self, other: Self) -> f32 {
        let dl = self.l - other.l;
        let du = self.u - other.u;
        let dv = self.v - other.v;
        dl.abs() + du.abs() + dv.abs()
    }
}

impl LCh {
    #[inline]
    pub const fn taxicab_distance(&self, other: Self) -> f32 {
        let dl = self.l - other.l;
        let dc = self.c - other.c;
        let dh = self.h - other.h;
        dl.abs() + dc.abs() + dh.abs()
    }
}

impl Add<Luv> for Luv {
    type Output = Luv;

    #[inline]
    fn add(self, rhs: Luv) -> Luv {
        Luv::new(self.l + rhs.l, self.u + rhs.u, self.v + rhs.v)
    }
}

impl Add<LCh> for LCh {
    type Output = LCh;

    #[inline]
    fn add(self, rhs: LCh) -> LCh {
        LCh::new(self.l + rhs.l, self.c + rhs.c, self.h + rhs.h)
    }
}

impl Sub<Luv> for Luv {
    type Output = Luv;

    #[inline]
    fn sub(self, rhs: Luv) -> Luv {
        Luv::new(self.l - rhs.l, self.u - rhs.u, self.v - rhs.v)
    }
}

impl Sub<LCh> for LCh {
    type Output = LCh;

    #[inline]
    fn sub(self, rhs: LCh) -> LCh {
        LCh::new(self.l - rhs.l, self.c - rhs.c, self.h - rhs.h)
    }
}

impl Mul<Luv> for Luv {
    type Output = Luv;

    #[inline]
    fn mul(self, rhs: Luv) -> Luv {
        Luv::new(self.l * rhs.l, self.u * rhs.u, self.v * rhs.v)
    }
}

impl Mul<LCh> for LCh {
    type Output = LCh;

    #[inline]
    fn mul(self, rhs: LCh) -> LCh {
        LCh::new(self.l * rhs.l, self.c * rhs.c, self.h * rhs.h)
    }
}

impl Div<Luv> for Luv {
    type Output = Luv;

    #[inline]
    fn div(self, rhs: Luv) -> Luv {
        Luv::new(self.l / rhs.l, self.u / rhs.u, self.v / rhs.v)
    }
}

impl Div<LCh> for LCh {
    type Output = LCh;

    #[inline]
    fn div(self, rhs: LCh) -> LCh {
        LCh::new(self.l / rhs.l, self.c / rhs.c, self.h / rhs.h)
    }
}

impl Add<f32> for Luv {
    type Output = Luv;

    #[inline]
    fn add(self, rhs: f32) -> Self::Output {
        Luv::new(self.l + rhs, self.u + rhs, self.v + rhs)
    }
}

impl Add<f32> for LCh {
    type Output = LCh;

    #[inline]
    fn add(self, rhs: f32) -> Self::Output {
        LCh::new(self.l + rhs, self.c + rhs, self.h + rhs)
    }
}

impl Sub<f32> for Luv {
    type Output = Luv;

    #[inline]
    fn sub(self, rhs: f32) -> Self::Output {
        Luv::new(self.l - rhs, self.u - rhs, self.v - rhs)
    }
}

impl Sub<f32> for LCh {
    type Output = LCh;

    #[inline]
    fn sub(self, rhs: f32) -> Self::Output {
        LCh::new(self.l - rhs, self.c - rhs, self.h - rhs)
    }
}

impl Mul<f32> for Luv {
    type Output = Luv;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Luv::new(self.l * rhs, self.u * rhs, self.v * rhs)
    }
}

impl Mul<f32> for LCh {
    type Output = LCh;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        LCh::new(self.l * rhs, self.c * rhs, self.h * rhs)
    }
}

impl Div<f32> for Luv {
    type Output = Luv;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Luv::new(self.l / rhs, self.u / rhs, self.v / rhs)
    }
}

impl Div<f32> for LCh {
    type Output = LCh;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        LCh::new(self.l / rhs, self.c / rhs, self.h / rhs)
    }
}

impl AddAssign<Luv> for Luv {
    #[inline]
    fn add_assign(&mut self, rhs: Luv) {
        self.l += rhs.l;
        self.u += rhs.u;
        self.v += rhs.v;
    }
}

impl AddAssign<LCh> for LCh {
    #[inline]
    fn add_assign(&mut self, rhs: LCh) {
        self.l += rhs.l;
        self.c += rhs.c;
        self.h += rhs.h;
    }
}

impl SubAssign<Luv> for Luv {
    #[inline]
    fn sub_assign(&mut self, rhs: Luv) {
        self.l -= rhs.l;
        self.u -= rhs.u;
        self.v -= rhs.v;
    }
}

impl SubAssign<LCh> for LCh {
    #[inline]
    fn sub_assign(&mut self, rhs: LCh) {
        self.l -= rhs.l;
        self.c -= rhs.c;
        self.h -= rhs.h;
    }
}

impl MulAssign<Luv> for Luv {
    #[inline]
    fn mul_assign(&mut self, rhs: Luv) {
        self.l *= rhs.l;
        self.u *= rhs.u;
        self.v *= rhs.v;
    }
}

impl MulAssign<LCh> for LCh {
    #[inline]
    fn mul_assign(&mut self, rhs: LCh) {
        self.l *= rhs.l;
        self.c *= rhs.c;
        self.h *= rhs.h;
    }
}

impl DivAssign<Luv> for Luv {
    #[inline]
    fn div_assign(&mut self, rhs: Luv) {
        self.l /= rhs.l;
        self.u /= rhs.u;
        self.v /= rhs.v;
    }
}

impl DivAssign<LCh> for LCh {
    #[inline]
    fn div_assign(&mut self, rhs: LCh) {
        self.l /= rhs.l;
        self.c /= rhs.c;
        self.h /= rhs.h;
    }
}

impl AddAssign<f32> for Luv {
    #[inline]
    fn add_assign(&mut self, rhs: f32) {
        self.l += rhs;
        self.u += rhs;
        self.v += rhs;
    }
}

impl AddAssign<f32> for LCh {
    #[inline]
    fn add_assign(&mut self, rhs: f32) {
        self.l += rhs;
        self.c += rhs;
        self.h += rhs;
    }
}

impl SubAssign<f32> for Luv {
    #[inline]
    fn sub_assign(&mut self, rhs: f32) {
        self.l -= rhs;
        self.u -= rhs;
        self.v -= rhs;
    }
}

impl SubAssign<f32> for LCh {
    #[inline]
    fn sub_assign(&mut self, rhs: f32) {
        self.l -= rhs;
        self.c -= rhs;
        self.h -= rhs;
    }
}

impl MulAssign<f32> for Luv {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.l *= rhs;
        self.u *= rhs;
        self.v *= rhs;
    }
}

impl MulAssign<f32> for LCh {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.l *= rhs;
        self.c *= rhs;
        self.h *= rhs;
    }
}

impl DivAssign<f32> for Luv {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.l /= rhs;
        self.u /= rhs;
        self.v /= rhs;
    }
}

impl DivAssign<f32> for LCh {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.l /= rhs;
        self.c /= rhs;
        self.h /= rhs;
    }
}

impl Neg for LCh {
    type Output = LCh;

    #[inline]
    fn neg(self) -> Self::Output {
        LCh::new(-self.l, -self.c, -self.h)
    }
}

impl Neg for Luv {
    type Output = Luv;

    #[inline]
    fn neg(self) -> Self::Output {
        Luv::new(-self.l, -self.u, -self.v)
    }
}

impl Pow<f32> for Luv {
    type Output = Luv;

    #[inline]
    fn pow(self, rhs: f32) -> Self::Output {
        Luv::new(
            f_powf(self.l, rhs),
            f_powf(self.u, rhs),
            f_powf(self.v, rhs),
        )
    }
}

impl Pow<f32> for LCh {
    type Output = LCh;

    #[inline]
    fn pow(self, rhs: f32) -> Self::Output {
        LCh::new(
            f_powf(self.l, rhs),
            f_powf(self.c, rhs),
            f_powf(self.h, rhs),
        )
    }
}

impl Pow<Luv> for Luv {
    type Output = Luv;

    #[inline]
    fn pow(self, rhs: Luv) -> Self::Output {
        Luv::new(
            f_powf(self.l, rhs.l),
            f_powf(self.u, rhs.u),
            f_powf(self.v, rhs.v),
        )
    }
}

impl Pow<LCh> for LCh {
    type Output = LCh;

    #[inline]
    fn pow(self, rhs: LCh) -> Self::Output {
        LCh::new(
            f_powf(self.l, rhs.l),
            f_powf(self.c, rhs.c),
            f_powf(self.h, rhs.h),
        )
    }
}

impl Luv {
    #[inline]
    pub fn sqrt(&self) -> Luv {
        Luv::new(self.l.sqrt(), self.u.sqrt(), self.v.sqrt())
    }

    #[inline]
    pub fn cbrt(&self) -> Luv {
        Luv::new(f_cbrtf(self.l), f_cbrtf(self.u), f_cbrtf(self.v))
    }
}

impl LCh {
    #[inline]
    pub fn sqrt(&self) -> LCh {
        LCh::new(
            if self.l < 0. { 0. } else { self.l.sqrt() },
            if self.c < 0. { 0. } else { self.c.sqrt() },
            if self.h < 0. { 0. } else { self.h.sqrt() },
        )
    }

    #[inline]
    pub fn cbrt(&self) -> LCh {
        LCh::new(f_cbrtf(self.l), f_cbrtf(self.c), f_cbrtf(self.h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_luv() {
        let xyz = Xyz::new(0.1, 0.2, 0.3);
        let lab = Luv::from_xyz(xyz);
        let rolled_back = lab.to_xyz();
        let dx = (xyz.x - rolled_back.x).abs();
        let dy = (xyz.y - rolled_back.y).abs();
        let dz = (xyz.z - rolled_back.z).abs();
        assert!(dx < 1e-5);
        assert!(dy < 1e-5);
        assert!(dz < 1e-5);
    }

    #[test]
    fn round_trip_lch() {
        let xyz = Xyz::new(0.1, 0.2, 0.3);
        let luv = Luv::from_xyz(xyz);
        let lab = LCh::from_luv(luv);
        let rolled_back = lab.to_luv();
        let dx = (luv.l - rolled_back.l).abs();
        let dy = (luv.u - rolled_back.u).abs();
        let dz = (luv.v - rolled_back.v).abs();
        assert!(dx < 1e-4);
        assert!(dy < 1e-4);
        assert!(dz < 1e-4);
    }
}
