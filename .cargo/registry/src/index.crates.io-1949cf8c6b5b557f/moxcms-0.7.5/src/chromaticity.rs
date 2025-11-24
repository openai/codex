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
use crate::{CmsError, XyY, XyYRepresentable, Xyz, Xyzd};

#[derive(Clone, Debug, Copy)]
#[repr(C)]
pub struct Chromaticity {
    pub x: f32,
    pub y: f32,
}

impl Chromaticity {
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Converts this chromaticity (`x`, `y`) to a tristimulus [`Xyz`] value,
    /// normalized such that `y = 1.0`.
    #[inline]
    pub const fn to_xyz(&self) -> Xyz {
        let reciprocal = if self.y != 0. { 1. / self.y } else { 0. };
        Xyz {
            x: self.x * reciprocal,
            y: 1f32,
            z: (1f32 - self.x - self.y) * reciprocal,
        }
    }

    /// Get the color representation with component sum `1`.
    ///
    /// In contrast to the XYZ representation defined through setting `Y` to a known
    /// value (such as `1` in [`Self::to_xyz`]) this representation can be uniquely
    /// derived from the `xy` coordinates with no ambiguities. It is scaled from the
    /// original XYZ color by diving by `X + Y + Z`. Note that, in particular, this
    /// method is well-defined even if the original color had pure chromamatic
    /// information with no luminance (Y = `0`) and will preserve that information,
    /// whereas [`Self::to_xyz`] is ill-defined and returns an incorrect value.
    #[inline]
    pub const fn to_scaled_xyzd(&self) -> Xyzd {
        let z = 1.0 - self.x as f64 - self.y as f64;
        Xyzd::new(self.x as f64, self.y as f64, z)
    }

    /// Get the color representation with component sum `1`.
    ///
    /// In contrast to the XYZ representation defined through setting `Y` to a known
    /// value (such as `1` in [`Self::to_xyz`]) this representation can be uniquely
    /// derived from the `xy` coordinates with no ambiguities. It is scaled from the
    /// original XYZ color by diving by `X + Y + Z`. Note that, in particular, this
    /// method is well-defined even if the original color had pure chromamatic
    /// information with no luminance (Y = `0`) and will preserve that information,
    /// whereas [`Self::to_xyz`] is ill-defined and returns an incorrect value.
    #[inline]
    pub const fn to_scaled_xyz(&self) -> Xyz {
        let z = 1.0 - self.x - self.y;
        Xyz::new(self.x, self.y, z)
    }

    #[inline]
    pub const fn to_xyzd(&self) -> Xyzd {
        let reciprocal = if self.y != 0. { 1. / self.y } else { 0. };
        Xyzd {
            x: self.x as f64 * reciprocal as f64,
            y: 1f64,
            z: (1f64 - self.x as f64 - self.y as f64) * reciprocal as f64,
        }
    }

    #[inline]
    pub const fn to_xyyb(&self) -> XyY {
        XyY {
            x: self.x as f64,
            y: self.y as f64,
            yb: 1.,
        }
    }

    pub const D65: Chromaticity = Chromaticity {
        x: 0.31272,
        y: 0.32903,
    };

    pub const D50: Chromaticity = Chromaticity {
        x: 0.34567,
        y: 0.35850,
    };
}

impl XyYRepresentable for Chromaticity {
    fn to_xyy(self) -> XyY {
        self.to_xyyb()
    }
}

impl TryFrom<Xyz> for Chromaticity {
    type Error = CmsError;

    #[inline]
    fn try_from(xyz: Xyz) -> Result<Self, Self::Error> {
        let sum = xyz.x + xyz.y + xyz.z;

        // Avoid division by zero or invalid XYZ values
        if sum == 0.0 {
            return Err(CmsError::DivisionByZero);
        }
        let rec = 1f32 / sum;

        let chromaticity_x = xyz.x * rec;
        let chromaticity_y = xyz.y * rec;

        Ok(Chromaticity {
            x: chromaticity_x,
            y: chromaticity_y,
        })
    }
}
