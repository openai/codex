/*
 * // Copyright 2024 (c) the Radzivon Bartoshyk. All rights reserved.
 * //
 * // Use of this source code is governed by a BSD-style
 * // license that can be found in the LICENSE file.
 */
use crate::{Oklab, Rgb};
use num_traits::Pow;
use pxfm::{f_atan2f, f_cbrtf, f_hypotf, f_powf, f_sincosf};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Represents *Oklch* colorspace
#[repr(C)]
#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub struct Oklch {
    /// Lightness
    pub l: f32,
    /// Chroma
    pub c: f32,
    /// Hue
    pub h: f32,
}

impl Oklch {
    /// Creates new instance
    #[inline]
    pub const fn new(l: f32, c: f32, h: f32) -> Oklch {
        Oklch { l, c, h }
    }

    /// Converts Linear [Rgb] into [Oklch]
    ///
    /// # Arguments
    /// `transfer_function` - Transfer function into linear colorspace and its inverse
    #[inline]
    pub fn from_linear_rgb(rgb: Rgb<f32>) -> Oklch {
        let oklab = Oklab::from_linear_rgb(rgb);
        Oklch::from_oklab(oklab)
    }

    /// Converts [Oklch] into linear [Rgb]
    #[inline]
    pub fn to_linear_rgb(&self) -> Rgb<f32> {
        let oklab = self.to_oklab();
        oklab.to_linear_rgb()
    }

    /// Converts *Oklab* to *Oklch*
    #[inline]
    pub fn from_oklab(oklab: Oklab) -> Oklch {
        let chroma = f_hypotf(oklab.b, oklab.a);
        let hue = f_atan2f(oklab.b, oklab.a);
        Oklch::new(oklab.l, chroma, hue)
    }

    /// Converts *Oklch* to *Oklab*
    #[inline]
    pub fn to_oklab(&self) -> Oklab {
        let l = self.l;
        let sincos = f_sincosf(self.h);
        let a = self.c * sincos.1;
        let b = self.c * sincos.0;
        Oklab::new(l, a, b)
    }
}

impl Oklch {
    #[inline]
    pub fn euclidean_distance(&self, other: Self) -> f32 {
        let dl = self.l - other.l;
        let dc = self.c - other.c;
        let dh = self.h - other.h;
        (dl * dl + dc * dc + dh * dh).sqrt()
    }
}

impl Oklch {
    #[inline]
    pub fn taxicab_distance(&self, other: Self) -> f32 {
        let dl = self.l - other.l;
        let dc = self.c - other.c;
        let dh = self.h - other.h;
        dl.abs() + dc.abs() + dh.abs()
    }
}

impl Add<Oklch> for Oklch {
    type Output = Oklch;

    #[inline]
    fn add(self, rhs: Self) -> Oklch {
        Oklch::new(self.l + rhs.l, self.c + rhs.c, self.h + rhs.h)
    }
}

impl Add<f32> for Oklch {
    type Output = Oklch;

    #[inline]
    fn add(self, rhs: f32) -> Oklch {
        Oklch::new(self.l + rhs, self.c + rhs, self.h + rhs)
    }
}

impl AddAssign<Oklch> for Oklch {
    #[inline]
    fn add_assign(&mut self, rhs: Oklch) {
        self.l += rhs.l;
        self.c += rhs.c;
        self.h += rhs.h;
    }
}

impl AddAssign<f32> for Oklch {
    #[inline]
    fn add_assign(&mut self, rhs: f32) {
        self.l += rhs;
        self.c += rhs;
        self.h += rhs;
    }
}

impl Mul<f32> for Oklch {
    type Output = Oklch;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Oklch::new(self.l * rhs, self.c * rhs, self.h * rhs)
    }
}

impl Mul<Oklch> for Oklch {
    type Output = Oklch;

    #[inline]
    fn mul(self, rhs: Oklch) -> Self::Output {
        Oklch::new(self.l * rhs.l, self.c * rhs.c, self.h * rhs.h)
    }
}

impl MulAssign<f32> for Oklch {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.l *= rhs;
        self.c *= rhs;
        self.h *= rhs;
    }
}

impl MulAssign<Oklch> for Oklch {
    #[inline]
    fn mul_assign(&mut self, rhs: Oklch) {
        self.l *= rhs.l;
        self.c *= rhs.c;
        self.h *= rhs.h;
    }
}

impl Sub<f32> for Oklch {
    type Output = Oklch;

    #[inline]
    fn sub(self, rhs: f32) -> Self::Output {
        Oklch::new(self.l - rhs, self.c - rhs, self.h - rhs)
    }
}

impl Sub<Oklch> for Oklch {
    type Output = Oklch;

    #[inline]
    fn sub(self, rhs: Oklch) -> Self::Output {
        Oklch::new(self.l - rhs.l, self.c - rhs.c, self.h - rhs.h)
    }
}

impl SubAssign<f32> for Oklch {
    #[inline]
    fn sub_assign(&mut self, rhs: f32) {
        self.l -= rhs;
        self.c -= rhs;
        self.h -= rhs;
    }
}

impl SubAssign<Oklch> for Oklch {
    #[inline]
    fn sub_assign(&mut self, rhs: Oklch) {
        self.l -= rhs.l;
        self.c -= rhs.c;
        self.h -= rhs.h;
    }
}

impl Div<f32> for Oklch {
    type Output = Oklch;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Oklch::new(self.l / rhs, self.c / rhs, self.h / rhs)
    }
}

impl Div<Oklch> for Oklch {
    type Output = Oklch;

    #[inline]
    fn div(self, rhs: Oklch) -> Self::Output {
        Oklch::new(self.l / rhs.l, self.c / rhs.c, self.h / rhs.h)
    }
}

impl DivAssign<f32> for Oklch {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.l /= rhs;
        self.c /= rhs;
        self.h /= rhs;
    }
}

impl DivAssign<Oklch> for Oklch {
    #[inline]
    fn div_assign(&mut self, rhs: Oklch) {
        self.l /= rhs.l;
        self.c /= rhs.c;
        self.h /= rhs.h;
    }
}

impl Neg for Oklch {
    type Output = Oklch;

    #[inline]
    fn neg(self) -> Self::Output {
        Oklch::new(-self.l, -self.c, -self.h)
    }
}

impl Pow<f32> for Oklch {
    type Output = Oklch;

    #[inline]
    fn pow(self, rhs: f32) -> Self::Output {
        Oklch::new(
            f_powf(self.l, rhs),
            f_powf(self.c, rhs),
            f_powf(self.h, rhs),
        )
    }
}

impl Pow<Oklch> for Oklch {
    type Output = Oklch;

    #[inline]
    fn pow(self, rhs: Oklch) -> Self::Output {
        Oklch::new(
            f_powf(self.l, rhs.l),
            f_powf(self.c, rhs.c),
            f_powf(self.h, rhs.h),
        )
    }
}

impl Oklch {
    #[inline]
    pub fn sqrt(&self) -> Oklch {
        Oklch::new(self.l.sqrt(), self.c.sqrt(), self.h.sqrt())
    }

    #[inline]
    pub fn cbrt(&self) -> Oklch {
        Oklch::new(f_cbrtf(self.l), f_cbrtf(self.c), f_cbrtf(self.h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let xyz = Rgb::new(0.1, 0.2, 0.3);
        let lab = Oklch::from_linear_rgb(xyz);
        let rolled_back = lab.to_linear_rgb();
        let dx = (xyz.r - rolled_back.r).abs();
        let dy = (xyz.g - rolled_back.g).abs();
        let dz = (xyz.b - rolled_back.b).abs();
        assert!(dx < 1e-5);
        assert!(dy < 1e-5);
        assert!(dz < 1e-5);
    }
}
