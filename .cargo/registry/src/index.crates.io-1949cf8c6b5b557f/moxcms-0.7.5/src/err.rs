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
use crate::RenderingIntent;
use std::error::Error;
use std::fmt::Display;

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub struct MalformedSize {
    pub size: usize,
    pub expected: usize,
}

#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub enum CmsError {
    LaneSizeMismatch,
    LaneMultipleOfChannels,
    InvalidProfile,
    InvalidTrcCurve,
    InvalidCicp,
    CurveLutIsTooLarge,
    ParametricCurveZeroDivision,
    InvalidRenderingIntent,
    DivisionByZero,
    UnsupportedColorPrimaries(u8),
    UnsupportedTrc(u8),
    InvalidLayout,
    UnsupportedProfileConnection,
    BuildTransferFunction,
    UnsupportedChannelConfiguration,
    UnknownTag(u32),
    UnknownTagTypeDefinition(u32),
    UnsupportedLutRenderingIntent(RenderingIntent),
    InvalidAtoBLut,
    OverflowingError,
    LUTTablesInvalidKind,
    MalformedClut(MalformedSize),
    MalformedCurveLutTable(MalformedSize),
    InvalidInksCountForProfile,
    MalformedTrcCurve(String),
}

impl Display for CmsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmsError::LaneSizeMismatch => f.write_str("Lanes length must match"),
            CmsError::LaneMultipleOfChannels => {
                f.write_str("Lane length must not be multiple of channel count")
            }
            CmsError::InvalidProfile => f.write_str("Invalid ICC profile"),
            CmsError::InvalidCicp => {
                f.write_str("Invalid Code Independent point (CICP) in ICC profile")
            }
            CmsError::InvalidTrcCurve => f.write_str("Invalid TRC curve"),
            CmsError::CurveLutIsTooLarge => f.write_str("Curve Lut is too large"),
            CmsError::ParametricCurveZeroDivision => {
                f.write_str("Parametric Curve definition causes division by zero")
            }
            CmsError::InvalidRenderingIntent => f.write_str("Invalid rendering intent"),
            CmsError::DivisionByZero => f.write_str("Division by zero"),
            CmsError::UnsupportedColorPrimaries(value) => {
                f.write_fmt(format_args!("Unsupported color primaries, {value}"))
            }
            CmsError::UnsupportedTrc(value) => f.write_fmt(format_args!("Unsupported TRC {value}")),
            CmsError::InvalidLayout => f.write_str("Invalid layout"),
            CmsError::UnsupportedProfileConnection => f.write_str("Unsupported profile connection"),
            CmsError::BuildTransferFunction => f.write_str("Can't reconstruct transfer function"),
            CmsError::UnsupportedChannelConfiguration => {
                f.write_str("Can't reconstruct channel configuration")
            }
            CmsError::UnknownTag(t) => f.write_fmt(format_args!("Unknown tag: {t}")),
            CmsError::UnknownTagTypeDefinition(t) => {
                f.write_fmt(format_args!("Unknown tag type definition: {t}"))
            }
            CmsError::UnsupportedLutRenderingIntent(intent) => f.write_fmt(format_args!(
                "Can't find LUT for rendering intent: {intent:?}"
            )),
            CmsError::InvalidAtoBLut => f.write_str("Invalid A to B Lut"),
            CmsError::OverflowingError => {
                f.write_str("Overflowing was happen, that is not allowed")
            }
            CmsError::LUTTablesInvalidKind => f.write_str("All LUT curves must have same kind"),
            CmsError::MalformedClut(size) => {
                f.write_fmt(format_args!("Invalid CLUT size: {size:?}"))
            }
            CmsError::MalformedCurveLutTable(size) => {
                f.write_fmt(format_args!("Malformed curve LUT size: {size:?}"))
            }
            CmsError::InvalidInksCountForProfile => {
                f.write_str("Invalid inks count for profile was provided")
            }
            CmsError::MalformedTrcCurve(str) => f.write_str(str),
        }
    }
}

impl Error for CmsError {}
