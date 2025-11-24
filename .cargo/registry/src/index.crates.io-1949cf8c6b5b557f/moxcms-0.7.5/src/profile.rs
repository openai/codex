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
use crate::chad::BRADFORD_D;
use crate::cicp::{
    CicpColorPrimaries, ColorPrimaries, MatrixCoefficients, TransferCharacteristics,
};
use crate::dat::ColorDateTime;
use crate::err::CmsError;
use crate::matrix::{Matrix3f, Xyz};
use crate::reader::s15_fixed16_number_to_float;
use crate::safe_math::{SafeAdd, SafeMul};
use crate::tag::{TAG_SIZE, Tag};
use crate::trc::ToneReprCurve;
use crate::{Chromaticity, Layout, Matrix3d, Vector3d, XyY, Xyzd, adapt_to_d50_d};
use std::io::Read;

const MAX_PROFILE_SIZE: usize = 1024 * 1024 * 10; // 10 MB max, for Fogra39 etc

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSignature {
    Acsp,
}

impl TryFrom<u32> for ProfileSignature {
    type Error = CmsError;
    #[inline]
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == u32::from_ne_bytes(*b"acsp").to_be() {
            return Ok(ProfileSignature::Acsp);
        }
        Err(CmsError::InvalidProfile)
    }
}

impl From<ProfileSignature> for u32 {
    #[inline]
    fn from(value: ProfileSignature) -> Self {
        match value {
            ProfileSignature::Acsp => u32::from_ne_bytes(*b"acsp").to_be(),
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Ord, PartialOrd)]
pub enum ProfileVersion {
    V2_0 = 0x02000000,
    V2_1 = 0x02100000,
    V2_2 = 0x02200000,
    V2_3 = 0x02300000,
    V2_4 = 0x02400000,
    V4_0 = 0x04000000,
    V4_1 = 0x04100000,
    V4_2 = 0x04200000,
    V4_3 = 0x04300000,
    #[default]
    V4_4 = 0x04400000,
    Unknown,
}

impl TryFrom<u32> for ProfileVersion {
    type Error = CmsError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0x02000000 => Ok(ProfileVersion::V2_0),
            0x02100000 => Ok(ProfileVersion::V2_1),
            0x02200000 => Ok(ProfileVersion::V2_2),
            0x02300000 => Ok(ProfileVersion::V2_3),
            0x02400000 => Ok(ProfileVersion::V2_4),
            0x04000000 => Ok(ProfileVersion::V4_0),
            0x04100000 => Ok(ProfileVersion::V4_1),
            0x04200000 => Ok(ProfileVersion::V4_2),
            0x04300000 => Ok(ProfileVersion::V4_3),
            0x04400000 => Ok(ProfileVersion::V4_3),
            _ => Err(CmsError::InvalidProfile),
        }
    }
}

impl From<ProfileVersion> for u32 {
    fn from(value: ProfileVersion) -> Self {
        match value {
            ProfileVersion::V2_0 => 0x02000000,
            ProfileVersion::V2_1 => 0x02100000,
            ProfileVersion::V2_2 => 0x02200000,
            ProfileVersion::V2_3 => 0x02300000,
            ProfileVersion::V2_4 => 0x02400000,
            ProfileVersion::V4_0 => 0x04000000,
            ProfileVersion::V4_1 => 0x04100000,
            ProfileVersion::V4_2 => 0x04200000,
            ProfileVersion::V4_3 => 0x04300000,
            ProfileVersion::V4_4 => 0x04400000,
            ProfileVersion::Unknown => 0x02000000,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Default, Hash)]
pub enum DataColorSpace {
    #[default]
    Xyz,
    Lab,
    Luv,
    YCbr,
    Yxy,
    Rgb,
    Gray,
    Hsv,
    Hls,
    Cmyk,
    Cmy,
    Color2,
    Color3,
    Color4,
    Color5,
    Color6,
    Color7,
    Color8,
    Color9,
    Color10,
    Color11,
    Color12,
    Color13,
    Color14,
    Color15,
}

impl DataColorSpace {
    #[inline]
    pub fn check_layout(self, layout: Layout) -> Result<(), CmsError> {
        let unsupported: bool = match self {
            DataColorSpace::Xyz => layout != Layout::Rgb,
            DataColorSpace::Lab => layout != Layout::Rgb,
            DataColorSpace::Luv => layout != Layout::Rgb,
            DataColorSpace::YCbr => layout != Layout::Rgb,
            DataColorSpace::Yxy => layout != Layout::Rgb,
            DataColorSpace::Rgb => layout != Layout::Rgb && layout != Layout::Rgba,
            DataColorSpace::Gray => layout != Layout::Gray && layout != Layout::GrayAlpha,
            DataColorSpace::Hsv => layout != Layout::Rgb,
            DataColorSpace::Hls => layout != Layout::Rgb,
            DataColorSpace::Cmyk => layout != Layout::Rgba,
            DataColorSpace::Cmy => layout != Layout::Rgb,
            DataColorSpace::Color2 => layout != Layout::GrayAlpha,
            DataColorSpace::Color3 => layout != Layout::Rgb,
            DataColorSpace::Color4 => layout != Layout::Rgba,
            DataColorSpace::Color5 => layout != Layout::Inks5,
            DataColorSpace::Color6 => layout != Layout::Inks6,
            DataColorSpace::Color7 => layout != Layout::Inks7,
            DataColorSpace::Color8 => layout != Layout::Inks8,
            DataColorSpace::Color9 => layout != Layout::Inks9,
            DataColorSpace::Color10 => layout != Layout::Inks10,
            DataColorSpace::Color11 => layout != Layout::Inks11,
            DataColorSpace::Color12 => layout != Layout::Inks12,
            DataColorSpace::Color13 => layout != Layout::Inks13,
            DataColorSpace::Color14 => layout != Layout::Inks14,
            DataColorSpace::Color15 => layout != Layout::Inks15,
        };
        if unsupported {
            Err(CmsError::InvalidLayout)
        } else {
            Ok(())
        }
    }

    pub(crate) fn is_three_channels(self) -> bool {
        matches!(
            self,
            DataColorSpace::Xyz
                | DataColorSpace::Lab
                | DataColorSpace::Luv
                | DataColorSpace::YCbr
                | DataColorSpace::Yxy
                | DataColorSpace::Rgb
                | DataColorSpace::Hsv
                | DataColorSpace::Hls
                | DataColorSpace::Cmy
                | DataColorSpace::Color3
        )
    }
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Default)]
pub enum ProfileClass {
    InputDevice,
    #[default]
    DisplayDevice,
    OutputDevice,
    DeviceLink,
    ColorSpace,
    Abstract,
    Named,
}

impl TryFrom<u32> for ProfileClass {
    type Error = CmsError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == u32::from_ne_bytes(*b"scnr").to_be() {
            return Ok(ProfileClass::InputDevice);
        } else if value == u32::from_ne_bytes(*b"mntr").to_be() {
            return Ok(ProfileClass::DisplayDevice);
        } else if value == u32::from_ne_bytes(*b"prtr").to_be() {
            return Ok(ProfileClass::OutputDevice);
        } else if value == u32::from_ne_bytes(*b"link").to_be() {
            return Ok(ProfileClass::DeviceLink);
        } else if value == u32::from_ne_bytes(*b"spac").to_be() {
            return Ok(ProfileClass::ColorSpace);
        } else if value == u32::from_ne_bytes(*b"abst").to_be() {
            return Ok(ProfileClass::Abstract);
        } else if value == u32::from_ne_bytes(*b"nmcl").to_be() {
            return Ok(ProfileClass::Named);
        }
        Err(CmsError::InvalidProfile)
    }
}

impl From<ProfileClass> for u32 {
    fn from(val: ProfileClass) -> Self {
        match val {
            ProfileClass::InputDevice => u32::from_ne_bytes(*b"scnr").to_be(),
            ProfileClass::DisplayDevice => u32::from_ne_bytes(*b"mntr").to_be(),
            ProfileClass::OutputDevice => u32::from_ne_bytes(*b"prtr").to_be(),
            ProfileClass::DeviceLink => u32::from_ne_bytes(*b"link").to_be(),
            ProfileClass::ColorSpace => u32::from_ne_bytes(*b"spac").to_be(),
            ProfileClass::Abstract => u32::from_ne_bytes(*b"abst").to_be(),
            ProfileClass::Named => u32::from_ne_bytes(*b"nmcl").to_be(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LutStore {
    Store8(Vec<u8>),
    Store16(Vec<u16>),
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum LutType {
    Lut8,
    Lut16,
    LutMab,
    LutMba,
}

impl TryFrom<u32> for LutType {
    type Error = CmsError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == u32::from_ne_bytes(*b"mft1").to_be() {
            return Ok(LutType::Lut8);
        } else if value == u32::from_ne_bytes(*b"mft2").to_be() {
            return Ok(LutType::Lut16);
        } else if value == u32::from_ne_bytes(*b"mAB ").to_be() {
            return Ok(LutType::LutMab);
        } else if value == u32::from_ne_bytes(*b"mBA ").to_be() {
            return Ok(LutType::LutMba);
        }
        Err(CmsError::InvalidProfile)
    }
}

impl From<LutType> for u32 {
    fn from(val: LutType) -> Self {
        match val {
            LutType::Lut8 => u32::from_ne_bytes(*b"mft1").to_be(),
            LutType::Lut16 => u32::from_ne_bytes(*b"mft2").to_be(),
            LutType::LutMab => u32::from_ne_bytes(*b"mAB ").to_be(),
            LutType::LutMba => u32::from_ne_bytes(*b"mBA ").to_be(),
        }
    }
}

impl TryFrom<u32> for DataColorSpace {
    type Error = CmsError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == u32::from_ne_bytes(*b"XYZ ").to_be() {
            return Ok(DataColorSpace::Xyz);
        } else if value == u32::from_ne_bytes(*b"Lab ").to_be() {
            return Ok(DataColorSpace::Lab);
        } else if value == u32::from_ne_bytes(*b"Luv ").to_be() {
            return Ok(DataColorSpace::Luv);
        } else if value == u32::from_ne_bytes(*b"YCbr").to_be() {
            return Ok(DataColorSpace::YCbr);
        } else if value == u32::from_ne_bytes(*b"Yxy ").to_be() {
            return Ok(DataColorSpace::Yxy);
        } else if value == u32::from_ne_bytes(*b"RGB ").to_be() {
            return Ok(DataColorSpace::Rgb);
        } else if value == u32::from_ne_bytes(*b"GRAY").to_be() {
            return Ok(DataColorSpace::Gray);
        } else if value == u32::from_ne_bytes(*b"HSV ").to_be() {
            return Ok(DataColorSpace::Hsv);
        } else if value == u32::from_ne_bytes(*b"HLS ").to_be() {
            return Ok(DataColorSpace::Hls);
        } else if value == u32::from_ne_bytes(*b"CMYK").to_be() {
            return Ok(DataColorSpace::Cmyk);
        } else if value == u32::from_ne_bytes(*b"CMY ").to_be() {
            return Ok(DataColorSpace::Cmy);
        } else if value == u32::from_ne_bytes(*b"2CLR").to_be() {
            return Ok(DataColorSpace::Color2);
        } else if value == u32::from_ne_bytes(*b"3CLR").to_be() {
            return Ok(DataColorSpace::Color3);
        } else if value == u32::from_ne_bytes(*b"4CLR").to_be() {
            return Ok(DataColorSpace::Color4);
        } else if value == u32::from_ne_bytes(*b"5CLR").to_be() {
            return Ok(DataColorSpace::Color5);
        } else if value == u32::from_ne_bytes(*b"6CLR").to_be() {
            return Ok(DataColorSpace::Color6);
        } else if value == u32::from_ne_bytes(*b"7CLR").to_be() {
            return Ok(DataColorSpace::Color7);
        } else if value == u32::from_ne_bytes(*b"8CLR").to_be() {
            return Ok(DataColorSpace::Color8);
        } else if value == u32::from_ne_bytes(*b"9CLR").to_be() {
            return Ok(DataColorSpace::Color9);
        } else if value == u32::from_ne_bytes(*b"ACLR").to_be() {
            return Ok(DataColorSpace::Color10);
        } else if value == u32::from_ne_bytes(*b"BCLR").to_be() {
            return Ok(DataColorSpace::Color11);
        } else if value == u32::from_ne_bytes(*b"CCLR").to_be() {
            return Ok(DataColorSpace::Color12);
        } else if value == u32::from_ne_bytes(*b"DCLR").to_be() {
            return Ok(DataColorSpace::Color13);
        } else if value == u32::from_ne_bytes(*b"ECLR").to_be() {
            return Ok(DataColorSpace::Color14);
        } else if value == u32::from_ne_bytes(*b"FCLR").to_be() {
            return Ok(DataColorSpace::Color15);
        }
        Err(CmsError::InvalidProfile)
    }
}

impl From<DataColorSpace> for u32 {
    fn from(val: DataColorSpace) -> Self {
        match val {
            DataColorSpace::Xyz => u32::from_ne_bytes(*b"XYZ ").to_be(),
            DataColorSpace::Lab => u32::from_ne_bytes(*b"Lab ").to_be(),
            DataColorSpace::Luv => u32::from_ne_bytes(*b"Luv ").to_be(),
            DataColorSpace::YCbr => u32::from_ne_bytes(*b"YCbr").to_be(),
            DataColorSpace::Yxy => u32::from_ne_bytes(*b"Yxy ").to_be(),
            DataColorSpace::Rgb => u32::from_ne_bytes(*b"RGB ").to_be(),
            DataColorSpace::Gray => u32::from_ne_bytes(*b"GRAY").to_be(),
            DataColorSpace::Hsv => u32::from_ne_bytes(*b"HSV ").to_be(),
            DataColorSpace::Hls => u32::from_ne_bytes(*b"HLS ").to_be(),
            DataColorSpace::Cmyk => u32::from_ne_bytes(*b"CMYK").to_be(),
            DataColorSpace::Cmy => u32::from_ne_bytes(*b"CMY ").to_be(),
            DataColorSpace::Color2 => u32::from_ne_bytes(*b"2CLR").to_be(),
            DataColorSpace::Color3 => u32::from_ne_bytes(*b"3CLR").to_be(),
            DataColorSpace::Color4 => u32::from_ne_bytes(*b"4CLR").to_be(),
            DataColorSpace::Color5 => u32::from_ne_bytes(*b"5CLR").to_be(),
            DataColorSpace::Color6 => u32::from_ne_bytes(*b"6CLR").to_be(),
            DataColorSpace::Color7 => u32::from_ne_bytes(*b"7CLR").to_be(),
            DataColorSpace::Color8 => u32::from_ne_bytes(*b"8CLR").to_be(),
            DataColorSpace::Color9 => u32::from_ne_bytes(*b"9CLR").to_be(),
            DataColorSpace::Color10 => u32::from_ne_bytes(*b"ACLR").to_be(),
            DataColorSpace::Color11 => u32::from_ne_bytes(*b"BCLR").to_be(),
            DataColorSpace::Color12 => u32::from_ne_bytes(*b"CCLR").to_be(),
            DataColorSpace::Color13 => u32::from_ne_bytes(*b"DCLR").to_be(),
            DataColorSpace::Color14 => u32::from_ne_bytes(*b"ECLR").to_be(),
            DataColorSpace::Color15 => u32::from_ne_bytes(*b"FCLR").to_be(),
        }
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum TechnologySignatures {
    FilmScanner,
    DigitalCamera,
    ReflectiveScanner,
    InkJetPrinter,
    ThermalWaxPrinter,
    ElectrophotographicPrinter,
    ElectrostaticPrinter,
    DyeSublimationPrinter,
    PhotographicPaperPrinter,
    FilmWriter,
    VideoMonitor,
    VideoCamera,
    ProjectionTelevision,
    CathodeRayTubeDisplay,
    PassiveMatrixDisplay,
    ActiveMatrixDisplay,
    LiquidCrystalDisplay,
    OrganicLedDisplay,
    PhotoCd,
    PhotographicImageSetter,
    Gravure,
    OffsetLithography,
    Silkscreen,
    Flexography,
    MotionPictureFilmScanner,
    MotionPictureFilmRecorder,
    DigitalMotionPictureCamera,
    DigitalCinemaProjector,
    Unknown(u32),
}

impl From<u32> for TechnologySignatures {
    fn from(value: u32) -> Self {
        if value == u32::from_ne_bytes(*b"fscn").to_be() {
            return TechnologySignatures::FilmScanner;
        } else if value == u32::from_ne_bytes(*b"dcam").to_be() {
            return TechnologySignatures::DigitalCamera;
        } else if value == u32::from_ne_bytes(*b"rscn").to_be() {
            return TechnologySignatures::ReflectiveScanner;
        } else if value == u32::from_ne_bytes(*b"ijet").to_be() {
            return TechnologySignatures::InkJetPrinter;
        } else if value == u32::from_ne_bytes(*b"twax").to_be() {
            return TechnologySignatures::ThermalWaxPrinter;
        } else if value == u32::from_ne_bytes(*b"epho").to_be() {
            return TechnologySignatures::ElectrophotographicPrinter;
        } else if value == u32::from_ne_bytes(*b"esta").to_be() {
            return TechnologySignatures::ElectrostaticPrinter;
        } else if value == u32::from_ne_bytes(*b"dsub").to_be() {
            return TechnologySignatures::DyeSublimationPrinter;
        } else if value == u32::from_ne_bytes(*b"rpho").to_be() {
            return TechnologySignatures::PhotographicPaperPrinter;
        } else if value == u32::from_ne_bytes(*b"fprn").to_be() {
            return TechnologySignatures::FilmWriter;
        } else if value == u32::from_ne_bytes(*b"vidm").to_be() {
            return TechnologySignatures::VideoMonitor;
        } else if value == u32::from_ne_bytes(*b"vidc").to_be() {
            return TechnologySignatures::VideoCamera;
        } else if value == u32::from_ne_bytes(*b"pjtv").to_be() {
            return TechnologySignatures::ProjectionTelevision;
        } else if value == u32::from_ne_bytes(*b"CRT ").to_be() {
            return TechnologySignatures::CathodeRayTubeDisplay;
        } else if value == u32::from_ne_bytes(*b"PMD ").to_be() {
            return TechnologySignatures::PassiveMatrixDisplay;
        } else if value == u32::from_ne_bytes(*b"AMD ").to_be() {
            return TechnologySignatures::ActiveMatrixDisplay;
        } else if value == u32::from_ne_bytes(*b"LCD ").to_be() {
            return TechnologySignatures::LiquidCrystalDisplay;
        } else if value == u32::from_ne_bytes(*b"OLED").to_be() {
            return TechnologySignatures::OrganicLedDisplay;
        } else if value == u32::from_ne_bytes(*b"KPCD").to_be() {
            return TechnologySignatures::PhotoCd;
        } else if value == u32::from_ne_bytes(*b"imgs").to_be() {
            return TechnologySignatures::PhotographicImageSetter;
        } else if value == u32::from_ne_bytes(*b"grav").to_be() {
            return TechnologySignatures::Gravure;
        } else if value == u32::from_ne_bytes(*b"offs").to_be() {
            return TechnologySignatures::OffsetLithography;
        } else if value == u32::from_ne_bytes(*b"silk").to_be() {
            return TechnologySignatures::Silkscreen;
        } else if value == u32::from_ne_bytes(*b"flex").to_be() {
            return TechnologySignatures::Flexography;
        } else if value == u32::from_ne_bytes(*b"mpfs").to_be() {
            return TechnologySignatures::MotionPictureFilmScanner;
        } else if value == u32::from_ne_bytes(*b"mpfr").to_be() {
            return TechnologySignatures::MotionPictureFilmRecorder;
        } else if value == u32::from_ne_bytes(*b"dmpc").to_be() {
            return TechnologySignatures::DigitalMotionPictureCamera;
        } else if value == u32::from_ne_bytes(*b"dcpj").to_be() {
            return TechnologySignatures::DigitalCinemaProjector;
        }
        TechnologySignatures::Unknown(value)
    }
}

#[derive(Debug, Clone)]
pub enum LutWarehouse {
    Lut(LutDataType),
    Multidimensional(LutMultidimensionalType),
}

#[derive(Debug, Clone)]
pub struct LutDataType {
    // used by lut8Type/lut16Type (mft2) only
    pub num_input_channels: u8,
    pub num_output_channels: u8,
    pub num_clut_grid_points: u8,
    pub matrix: Matrix3d,
    pub num_input_table_entries: u16,
    pub num_output_table_entries: u16,
    pub input_table: LutStore,
    pub clut_table: LutStore,
    pub output_table: LutStore,
    pub lut_type: LutType,
}

impl LutDataType {
    pub(crate) fn has_same_kind(&self) -> bool {
        matches!(
            (&self.input_table, &self.clut_table, &self.output_table),
            (
                LutStore::Store8(_),
                LutStore::Store8(_),
                LutStore::Store8(_)
            ) | (
                LutStore::Store16(_),
                LutStore::Store16(_),
                LutStore::Store16(_)
            )
        )
    }
}

#[derive(Debug, Clone)]
pub struct LutMultidimensionalType {
    pub num_input_channels: u8,
    pub num_output_channels: u8,
    pub grid_points: [u8; 16],
    pub clut: Option<LutStore>,
    pub a_curves: Vec<ToneReprCurve>,
    pub b_curves: Vec<ToneReprCurve>,
    pub m_curves: Vec<ToneReprCurve>,
    pub matrix: Matrix3d,
    pub bias: Vector3d,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum RenderingIntent {
    AbsoluteColorimetric = 3,
    Saturation = 2,
    RelativeColorimetric = 1,
    #[default]
    Perceptual = 0,
}

impl TryFrom<u32> for RenderingIntent {
    type Error = CmsError;

    #[inline]
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RenderingIntent::Perceptual),
            1 => Ok(RenderingIntent::RelativeColorimetric),
            2 => Ok(RenderingIntent::Saturation),
            3 => Ok(RenderingIntent::AbsoluteColorimetric),
            _ => Err(CmsError::InvalidRenderingIntent),
        }
    }
}

impl From<RenderingIntent> for u32 {
    #[inline]
    fn from(value: RenderingIntent) -> Self {
        match value {
            RenderingIntent::AbsoluteColorimetric => 3,
            RenderingIntent::Saturation => 2,
            RenderingIntent::RelativeColorimetric => 1,
            RenderingIntent::Perceptual => 0,
        }
    }
}

/// ICC Header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProfileHeader {
    pub size: u32,                         // Size of the profile (computed)
    pub cmm_type: u32,                     // Preferred CMM type (ignored)
    pub version: ProfileVersion,           // Version (4.3 or 4.4 if CICP is included)
    pub profile_class: ProfileClass,       // Display device profile
    pub data_color_space: DataColorSpace,  // RGB input color space
    pub pcs: DataColorSpace,               // Profile connection space
    pub creation_date_time: ColorDateTime, // Date and time
    pub signature: ProfileSignature,       // Profile signature
    pub platform: u32,                     // Platform target (ignored)
    pub flags: u32,                        // Flags (not embedded, can be used independently)
    pub device_manufacturer: u32,          // Device manufacturer (ignored)
    pub device_model: u32,                 // Device model (ignored)
    pub device_attributes: [u8; 8],        // Device attributes (ignored)
    pub rendering_intent: RenderingIntent, // Relative colorimetric rendering intent
    pub illuminant: Xyz,                   // D50 standard illuminant X
    pub creator: u32,                      // Profile creator (ignored)
    pub profile_id: [u8; 16],              // Profile id checksum (ignored)
    pub reserved: [u8; 28],                // Reserved (ignored)
    pub tag_count: u32,                    // Technically not part of header, but required
}

impl ProfileHeader {
    #[allow(dead_code)]
    pub(crate) fn new(size: u32) -> Self {
        Self {
            size,
            cmm_type: 0,
            version: ProfileVersion::V4_3,
            profile_class: ProfileClass::DisplayDevice,
            data_color_space: DataColorSpace::Rgb,
            pcs: DataColorSpace::Xyz,
            creation_date_time: ColorDateTime::default(),
            signature: ProfileSignature::Acsp,
            platform: 0,
            flags: 0x00000000,
            device_manufacturer: 0,
            device_model: 0,
            device_attributes: [0; 8],
            rendering_intent: RenderingIntent::Perceptual,
            illuminant: Chromaticity::D50.to_xyz(),
            creator: 0,
            profile_id: [0; 16],
            reserved: [0; 28],
            tag_count: 0,
        }
    }

    /// Creates profile from the buffer
    pub(crate) fn new_from_slice(slice: &[u8]) -> Result<Self, CmsError> {
        if slice.len() < size_of::<ProfileHeader>() {
            return Err(CmsError::InvalidProfile);
        }
        let mut cursor = std::io::Cursor::new(slice);
        let mut buffer = [0u8; size_of::<ProfileHeader>()];
        cursor
            .read_exact(&mut buffer)
            .map_err(|_| CmsError::InvalidProfile)?;

        let header = Self {
            size: u32::from_be_bytes(buffer[0..4].try_into().unwrap()),
            cmm_type: u32::from_be_bytes(buffer[4..8].try_into().unwrap()),
            version: ProfileVersion::try_from(u32::from_be_bytes(
                buffer[8..12].try_into().unwrap(),
            ))?,
            profile_class: ProfileClass::try_from(u32::from_be_bytes(
                buffer[12..16].try_into().unwrap(),
            ))?,
            data_color_space: DataColorSpace::try_from(u32::from_be_bytes(
                buffer[16..20].try_into().unwrap(),
            ))?,
            pcs: DataColorSpace::try_from(u32::from_be_bytes(buffer[20..24].try_into().unwrap()))?,
            creation_date_time: ColorDateTime::new_from_slice(buffer[24..36].try_into().unwrap())?,
            signature: ProfileSignature::try_from(u32::from_be_bytes(
                buffer[36..40].try_into().unwrap(),
            ))?,
            platform: u32::from_be_bytes(buffer[40..44].try_into().unwrap()),
            flags: u32::from_be_bytes(buffer[44..48].try_into().unwrap()),
            device_manufacturer: u32::from_be_bytes(buffer[48..52].try_into().unwrap()),
            device_model: u32::from_be_bytes(buffer[52..56].try_into().unwrap()),
            device_attributes: buffer[56..64].try_into().unwrap(),
            rendering_intent: RenderingIntent::try_from(u32::from_be_bytes(
                buffer[64..68].try_into().unwrap(),
            ))?,
            illuminant: Xyz::new(
                s15_fixed16_number_to_float(i32::from_be_bytes(buffer[68..72].try_into().unwrap())),
                s15_fixed16_number_to_float(i32::from_be_bytes(buffer[72..76].try_into().unwrap())),
                s15_fixed16_number_to_float(i32::from_be_bytes(buffer[76..80].try_into().unwrap())),
            ),
            creator: u32::from_be_bytes(buffer[80..84].try_into().unwrap()),
            profile_id: buffer[84..100].try_into().unwrap(),
            reserved: buffer[100..128].try_into().unwrap(),
            tag_count: u32::from_be_bytes(buffer[128..132].try_into().unwrap()),
        };
        Ok(header)
    }
}

/// A [Coding Independent Code Point](https://en.wikipedia.org/wiki/Coding-independent_code_points).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CicpProfile {
    pub color_primaries: CicpColorPrimaries,
    pub transfer_characteristics: TransferCharacteristics,
    pub matrix_coefficients: MatrixCoefficients,
    pub full_range: bool,
}

#[derive(Debug, Clone)]
pub struct LocalizableString {
    /// An ISO 639-1 value is expected; any text w. more than two symbols will be truncated
    pub language: String,
    /// An ISO 3166-1 value is expected; any text w. more than two symbols will be truncated
    pub country: String,
    pub value: String,
}

impl LocalizableString {
    /// Creates new localizable string
    ///
    /// # Arguments
    ///
    /// * `language`: an ISO 639-1 value is expected, any text more than 2 symbols will be truncated
    /// * `country`: an ISO 3166-1 value is expected, any text more than 2 symbols will be truncated
    /// * `value`: String value
    ///
    pub fn new(language: String, country: String, value: String) -> Self {
        Self {
            language,
            country,
            value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DescriptionString {
    pub ascii_string: String,
    pub unicode_language_code: u32,
    pub unicode_string: String,
    pub script_code_code: i8,
    pub mac_string: String,
}

#[derive(Debug, Clone)]
pub enum ProfileText {
    PlainString(String),
    Localizable(Vec<LocalizableString>),
    Description(DescriptionString),
}

impl ProfileText {
    pub(crate) fn has_values(&self) -> bool {
        match self {
            ProfileText::PlainString(_) => true,
            ProfileText::Localizable(lc) => !lc.is_empty(),
            ProfileText::Description(_) => true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StandardObserver {
    D50,
    D65,
    Unknown,
}

impl From<u32> for StandardObserver {
    fn from(value: u32) -> Self {
        if value == 1 {
            return StandardObserver::D50;
        } else if value == 2 {
            return StandardObserver::D65;
        }
        StandardObserver::Unknown
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ViewingConditions {
    pub illuminant: Xyz,
    pub surround: Xyz,
    pub observer: StandardObserver,
}

#[derive(Debug, Clone, Copy)]
pub enum MeasurementGeometry {
    Unknown,
    /// 0°:45° or 45°:0°
    D45to45,
    /// 0°:d or d:0°
    D0to0,
}

impl From<u32> for MeasurementGeometry {
    fn from(value: u32) -> Self {
        if value == 1 {
            Self::D45to45
        } else if value == 2 {
            Self::D0to0
        } else {
            Self::Unknown
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StandardIlluminant {
    Unknown,
    D50,
    D65,
    D93,
    F2,
    D55,
    A,
    EquiPower,
    F8,
}

impl From<u32> for StandardIlluminant {
    fn from(value: u32) -> Self {
        match value {
            1 => StandardIlluminant::D50,
            2 => StandardIlluminant::D65,
            3 => StandardIlluminant::D93,
            4 => StandardIlluminant::F2,
            5 => StandardIlluminant::D55,
            6 => StandardIlluminant::A,
            7 => StandardIlluminant::EquiPower,
            8 => StandardIlluminant::F8,
            _ => Self::Unknown,
        }
    }
}

impl From<StandardIlluminant> for u32 {
    fn from(value: StandardIlluminant) -> Self {
        match value {
            StandardIlluminant::Unknown => 0u32,
            StandardIlluminant::D50 => 1u32,
            StandardIlluminant::D65 => 2u32,
            StandardIlluminant::D93 => 3,
            StandardIlluminant::F2 => 4,
            StandardIlluminant::D55 => 5,
            StandardIlluminant::A => 6,
            StandardIlluminant::EquiPower => 7,
            StandardIlluminant::F8 => 8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Measurement {
    pub observer: StandardObserver,
    pub backing: Xyz,
    pub geometry: MeasurementGeometry,
    pub flare: f32,
    pub illuminant: StandardIlluminant,
}

/// ICC Profile representation
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct ColorProfile {
    pub pcs: DataColorSpace,
    pub color_space: DataColorSpace,
    pub profile_class: ProfileClass,
    pub rendering_intent: RenderingIntent,
    pub red_colorant: Xyzd,
    pub green_colorant: Xyzd,
    pub blue_colorant: Xyzd,
    pub white_point: Xyzd,
    pub black_point: Option<Xyzd>,
    pub media_white_point: Option<Xyzd>,
    pub luminance: Option<Xyzd>,
    pub measurement: Option<Measurement>,
    pub red_trc: Option<ToneReprCurve>,
    pub green_trc: Option<ToneReprCurve>,
    pub blue_trc: Option<ToneReprCurve>,
    pub gray_trc: Option<ToneReprCurve>,
    pub cicp: Option<CicpProfile>,
    pub chromatic_adaptation: Option<Matrix3d>,
    pub lut_a_to_b_perceptual: Option<LutWarehouse>,
    pub lut_a_to_b_colorimetric: Option<LutWarehouse>,
    pub lut_a_to_b_saturation: Option<LutWarehouse>,
    pub lut_b_to_a_perceptual: Option<LutWarehouse>,
    pub lut_b_to_a_colorimetric: Option<LutWarehouse>,
    pub lut_b_to_a_saturation: Option<LutWarehouse>,
    pub gamut: Option<LutWarehouse>,
    pub copyright: Option<ProfileText>,
    pub description: Option<ProfileText>,
    pub device_manufacturer: Option<ProfileText>,
    pub device_model: Option<ProfileText>,
    pub char_target: Option<ProfileText>,
    pub viewing_conditions: Option<ViewingConditions>,
    pub viewing_conditions_description: Option<ProfileText>,
    pub technology: Option<TechnologySignatures>,
    pub calibration_date: Option<ColorDateTime>,
    /// Version for internal and viewing purposes only.
    /// On encoding added value to profile will always be V4.
    pub(crate) version_internal: ProfileVersion,
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Hash)]
pub struct ParsingOptions {
    // Maximum allowed profile size in bytes
    pub max_profile_size: usize,
    // Maximum allowed CLUT size in bytes
    pub max_allowed_clut_size: usize,
    // Maximum allowed TRC size in elements count
    pub max_allowed_trc_size: usize,
}

impl Default for ParsingOptions {
    fn default() -> Self {
        Self {
            max_profile_size: MAX_PROFILE_SIZE,
            max_allowed_clut_size: 10_000_000,
            max_allowed_trc_size: 40_000,
        }
    }
}

impl ColorProfile {
    /// Returns profile version
    pub fn version(&self) -> ProfileVersion {
        self.version_internal
    }

    pub fn new_from_slice(slice: &[u8]) -> Result<Self, CmsError> {
        Self::new_from_slice_with_options(slice, Default::default())
    }

    pub fn new_from_slice_with_options(
        slice: &[u8],
        options: ParsingOptions,
    ) -> Result<Self, CmsError> {
        let header = ProfileHeader::new_from_slice(slice)?;
        let tags_count = header.tag_count as usize;
        if slice.len() >= options.max_profile_size {
            return Err(CmsError::InvalidProfile);
        }
        let tags_end = tags_count
            .safe_mul(TAG_SIZE)?
            .safe_add(size_of::<ProfileHeader>())?;
        if slice.len() < tags_end {
            return Err(CmsError::InvalidProfile);
        }
        let tags_slice = &slice[size_of::<ProfileHeader>()..tags_end];
        let mut profile = ColorProfile {
            rendering_intent: header.rendering_intent,
            pcs: header.pcs,
            profile_class: header.profile_class,
            color_space: header.data_color_space,
            white_point: header.illuminant.to_xyzd(),
            version_internal: header.version,
            ..Default::default()
        };
        let color_space = profile.color_space;
        for tag in tags_slice.chunks_exact(TAG_SIZE) {
            let tag_value = u32::from_be_bytes([tag[0], tag[1], tag[2], tag[3]]);
            let tag_entry = u32::from_be_bytes([tag[4], tag[5], tag[6], tag[7]]);
            let tag_size = u32::from_be_bytes([tag[8], tag[9], tag[10], tag[11]]) as usize;
            // Just ignore unknown tags
            if let Ok(tag) = Tag::try_from(tag_value) {
                match tag {
                    Tag::RedXyz => {
                        if color_space == DataColorSpace::Rgb {
                            profile.red_colorant =
                                Self::read_xyz_tag(slice, tag_entry as usize, tag_size)?;
                        }
                    }
                    Tag::GreenXyz => {
                        if color_space == DataColorSpace::Rgb {
                            profile.green_colorant =
                                Self::read_xyz_tag(slice, tag_entry as usize, tag_size)?;
                        }
                    }
                    Tag::BlueXyz => {
                        if color_space == DataColorSpace::Rgb {
                            profile.blue_colorant =
                                Self::read_xyz_tag(slice, tag_entry as usize, tag_size)?;
                        }
                    }
                    Tag::RedToneReproduction => {
                        if color_space == DataColorSpace::Rgb {
                            profile.red_trc = Self::read_trc_tag_s(
                                slice,
                                tag_entry as usize,
                                tag_size,
                                &options,
                            )?;
                        }
                    }
                    Tag::GreenToneReproduction => {
                        if color_space == DataColorSpace::Rgb {
                            profile.green_trc = Self::read_trc_tag_s(
                                slice,
                                tag_entry as usize,
                                tag_size,
                                &options,
                            )?;
                        }
                    }
                    Tag::BlueToneReproduction => {
                        if color_space == DataColorSpace::Rgb {
                            profile.blue_trc = Self::read_trc_tag_s(
                                slice,
                                tag_entry as usize,
                                tag_size,
                                &options,
                            )?;
                        }
                    }
                    Tag::GreyToneReproduction => {
                        if color_space == DataColorSpace::Gray {
                            profile.gray_trc = Self::read_trc_tag_s(
                                slice,
                                tag_entry as usize,
                                tag_size,
                                &options,
                            )?;
                        }
                    }
                    Tag::MediaWhitePoint => {
                        profile.media_white_point =
                            Self::read_xyz_tag(slice, tag_entry as usize, tag_size).map(Some)?;
                    }
                    Tag::Luminance => {
                        profile.luminance =
                            Self::read_xyz_tag(slice, tag_entry as usize, tag_size).map(Some)?;
                    }
                    Tag::Measurement => {
                        profile.measurement =
                            Self::read_meas_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::CodeIndependentPoints => {
                        // This tag may be present when the data colour space in the profile header is RGB, YCbCr, or XYZ, and the
                        // profile class in the profile header is Input or Display. The tag shall not be present for other data colour spaces
                        // or profile classes indicated in the profile header.
                        if (profile.profile_class == ProfileClass::InputDevice
                            || profile.profile_class == ProfileClass::DisplayDevice)
                            && (profile.color_space == DataColorSpace::Rgb
                                || profile.color_space == DataColorSpace::YCbr
                                || profile.color_space == DataColorSpace::Xyz)
                        {
                            profile.cicp =
                                Self::read_cicp_tag(slice, tag_entry as usize, tag_size)?;
                        }
                    }
                    Tag::ChromaticAdaptation => {
                        profile.chromatic_adaptation =
                            Self::read_chad_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::BlackPoint => {
                        profile.black_point =
                            Self::read_xyz_tag(slice, tag_entry as usize, tag_size).map(Some)?
                    }
                    Tag::DeviceToPcsLutPerceptual => {
                        profile.lut_a_to_b_perceptual =
                            Self::read_lut_tag(slice, tag_entry, tag_size, &options)?;
                    }
                    Tag::DeviceToPcsLutColorimetric => {
                        profile.lut_a_to_b_colorimetric =
                            Self::read_lut_tag(slice, tag_entry, tag_size, &options)?;
                    }
                    Tag::DeviceToPcsLutSaturation => {
                        profile.lut_a_to_b_saturation =
                            Self::read_lut_tag(slice, tag_entry, tag_size, &options)?;
                    }
                    Tag::PcsToDeviceLutPerceptual => {
                        profile.lut_b_to_a_perceptual =
                            Self::read_lut_tag(slice, tag_entry, tag_size, &options)?;
                    }
                    Tag::PcsToDeviceLutColorimetric => {
                        profile.lut_b_to_a_colorimetric =
                            Self::read_lut_tag(slice, tag_entry, tag_size, &options)?;
                    }
                    Tag::PcsToDeviceLutSaturation => {
                        profile.lut_b_to_a_saturation =
                            Self::read_lut_tag(slice, tag_entry, tag_size, &options)?;
                    }
                    Tag::Gamut => {
                        profile.gamut = Self::read_lut_tag(slice, tag_entry, tag_size, &options)?;
                    }
                    Tag::Copyright => {
                        profile.copyright =
                            Self::read_string_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::ProfileDescription => {
                        profile.description =
                            Self::read_string_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::ViewingConditionsDescription => {
                        profile.viewing_conditions_description =
                            Self::read_string_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::DeviceModel => {
                        profile.device_model =
                            Self::read_string_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::DeviceManufacturer => {
                        profile.device_manufacturer =
                            Self::read_string_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::CharTarget => {
                        profile.char_target =
                            Self::read_string_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::Chromaticity => {}
                    Tag::ObserverConditions => {
                        profile.viewing_conditions =
                            Self::read_viewing_conditions(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::Technology => {
                        profile.technology =
                            Self::read_tech_tag(slice, tag_entry as usize, tag_size)?;
                    }
                    Tag::CalibrationDateTime => {
                        profile.calibration_date =
                            Self::read_date_time_tag(slice, tag_entry as usize, tag_size)?;
                    }
                }
            }
        }

        Ok(profile)
    }
}

impl ColorProfile {
    #[inline]
    pub fn colorant_matrix(&self) -> Matrix3d {
        Matrix3d {
            v: [
                [
                    self.red_colorant.x,
                    self.green_colorant.x,
                    self.blue_colorant.x,
                ],
                [
                    self.red_colorant.y,
                    self.green_colorant.y,
                    self.blue_colorant.y,
                ],
                [
                    self.red_colorant.z,
                    self.green_colorant.z,
                    self.blue_colorant.z,
                ],
            ],
        }
    }

    /// Computes colorants matrix. Returns not transposed matrix.
    ///
    /// To work on `const` context this method does have restrictions.
    /// If invalid values were provided it may return invalid matrix or NaNs.
    pub const fn colorants_matrix(white_point: XyY, primaries: ColorPrimaries) -> Matrix3d {
        let red_xyz = primaries.red.to_xyzd();
        let green_xyz = primaries.green.to_xyzd();
        let blue_xyz = primaries.blue.to_xyzd();

        let xyz_matrix = Matrix3d {
            v: [
                [red_xyz.x, green_xyz.x, blue_xyz.x],
                [red_xyz.y, green_xyz.y, blue_xyz.y],
                [red_xyz.z, green_xyz.z, blue_xyz.z],
            ],
        };
        let colorants = ColorProfile::rgb_to_xyz_d(xyz_matrix, white_point.to_xyzd());
        adapt_to_d50_d(colorants, white_point)
    }

    /// Updates RGB triple colorimetry from 3 [Chromaticity] and white point
    pub const fn update_rgb_colorimetry(&mut self, white_point: XyY, primaries: ColorPrimaries) {
        let red_xyz = primaries.red.to_xyzd();
        let green_xyz = primaries.green.to_xyzd();
        let blue_xyz = primaries.blue.to_xyzd();

        self.chromatic_adaptation = Some(BRADFORD_D);
        self.update_rgb_colorimetry_triplet(white_point, red_xyz, green_xyz, blue_xyz)
    }

    /// Updates RGB triple colorimetry from 3 [Xyzd] and white point
    ///
    /// To work on `const` context this method does have restrictions.
    /// If invalid values were provided it may return invalid matrix or NaNs.
    pub const fn update_rgb_colorimetry_triplet(
        &mut self,
        white_point: XyY,
        red_xyz: Xyzd,
        green_xyz: Xyzd,
        blue_xyz: Xyzd,
    ) {
        let xyz_matrix = Matrix3d {
            v: [
                [red_xyz.x, green_xyz.x, blue_xyz.x],
                [red_xyz.y, green_xyz.y, blue_xyz.y],
                [red_xyz.z, green_xyz.z, blue_xyz.z],
            ],
        };
        let colorants = ColorProfile::rgb_to_xyz_d(xyz_matrix, white_point.to_xyzd());
        let colorants = adapt_to_d50_d(colorants, white_point);

        self.update_colorants(colorants);
    }

    pub(crate) const fn update_colorants(&mut self, colorants: Matrix3d) {
        // note: there's a transpose type of operation going on here
        self.red_colorant.x = colorants.v[0][0];
        self.red_colorant.y = colorants.v[1][0];
        self.red_colorant.z = colorants.v[2][0];
        self.green_colorant.x = colorants.v[0][1];
        self.green_colorant.y = colorants.v[1][1];
        self.green_colorant.z = colorants.v[2][1];
        self.blue_colorant.x = colorants.v[0][2];
        self.blue_colorant.y = colorants.v[1][2];
        self.blue_colorant.z = colorants.v[2][2];
    }

    /// Updates RGB triple colorimetry from CICP
    pub fn update_rgb_colorimetry_from_cicp(&mut self, cicp: CicpProfile) -> bool {
        self.cicp = Some(cicp);
        if !cicp.color_primaries.has_chromaticity()
            || !cicp.transfer_characteristics.has_transfer_curve()
        {
            return false;
        }
        let primaries_xy: ColorPrimaries = match cicp.color_primaries.try_into() {
            Ok(primaries) => primaries,
            Err(_) => return false,
        };
        let white_point: Chromaticity = match cicp.color_primaries.white_point() {
            Ok(v) => v,
            Err(_) => return false,
        };
        self.update_rgb_colorimetry(white_point.to_xyyb(), primaries_xy);

        let red_trc: ToneReprCurve = match cicp.transfer_characteristics.try_into() {
            Ok(trc) => trc,
            Err(_) => return false,
        };
        self.green_trc = Some(red_trc.clone());
        self.blue_trc = Some(red_trc.clone());
        self.red_trc = Some(red_trc);
        false
    }

    pub const fn rgb_to_xyz(&self, xyz_matrix: Matrix3f, wp: Xyz) -> Matrix3f {
        let xyz_inverse = xyz_matrix.inverse();
        let s = xyz_inverse.mul_vector(wp.to_vector());
        let mut v = xyz_matrix.mul_row_vector::<0>(s);
        v = v.mul_row_vector::<1>(s);
        v.mul_row_vector::<2>(s)
    }

    ///TODO: make primary instead of [rgb_to_xyz] in the next major version
    pub(crate) const fn rgb_to_xyz_static(xyz_matrix: Matrix3f, wp: Xyz) -> Matrix3f {
        let xyz_inverse = xyz_matrix.inverse();
        let s = xyz_inverse.mul_vector(wp.to_vector());
        let mut v = xyz_matrix.mul_row_vector::<0>(s);
        v = v.mul_row_vector::<1>(s);
        v.mul_row_vector::<2>(s)
    }

    /// If Primaries is invalid will return invalid matrix on const context.
    /// This assumes not transposed matrix and returns not transposed matrix.
    pub const fn rgb_to_xyz_d(xyz_matrix: Matrix3d, wp: Xyzd) -> Matrix3d {
        let xyz_inverse = xyz_matrix.inverse();
        let s = xyz_inverse.mul_vector(wp.to_vector_d());
        let mut v = xyz_matrix.mul_row_vector::<0>(s);
        v = v.mul_row_vector::<1>(s);
        v = v.mul_row_vector::<2>(s);
        v
    }

    pub fn rgb_to_xyz_matrix(&self) -> Matrix3d {
        let xyz_matrix = self.colorant_matrix();
        let white_point = Chromaticity::D50.to_xyzd();
        ColorProfile::rgb_to_xyz_d(xyz_matrix, white_point)
    }

    /// Computes transform matrix RGB -> XYZ -> RGB
    /// Current profile is used as source, other as destination
    pub fn transform_matrix(&self, dest: &ColorProfile) -> Matrix3d {
        let source = self.rgb_to_xyz_matrix();
        let dst = dest.rgb_to_xyz_matrix();
        let dest_inverse = dst.inverse();
        dest_inverse.mat_mul(source)
    }

    /// Returns volume of colors stored in profile
    pub fn profile_volume(&self) -> Option<f32> {
        let red_prim = self.red_colorant;
        let green_prim = self.green_colorant;
        let blue_prim = self.blue_colorant;
        let tetrahedral_vertices = Matrix3d {
            v: [
                [red_prim.x, red_prim.y, red_prim.z],
                [green_prim.x, green_prim.y, green_prim.z],
                [blue_prim.x, blue_prim.y, blue_prim.z],
            ],
        };
        let det = tetrahedral_vertices.determinant()?;
        Some((det / 6.0f64) as f32)
    }

    pub(crate) fn has_device_to_pcs_lut(&self) -> bool {
        self.lut_a_to_b_perceptual.is_some()
            || self.lut_a_to_b_saturation.is_some()
            || self.lut_a_to_b_colorimetric.is_some()
    }

    pub(crate) fn has_pcs_to_device_lut(&self) -> bool {
        self.lut_b_to_a_perceptual.is_some()
            || self.lut_b_to_a_saturation.is_some()
            || self.lut_b_to_a_colorimetric.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_gray() {
        if let Ok(gray_icc) = fs::read("./assets/Generic Gray Gamma 2.2 Profile.icc") {
            let f_p = ColorProfile::new_from_slice(&gray_icc).unwrap();
            assert!(f_p.gray_trc.is_some());
        }
    }

    #[test]
    fn test_perceptual() {
        if let Ok(srgb_perceptual_icc) = fs::read("./assets/srgb_perceptual.icc") {
            let f_p = ColorProfile::new_from_slice(&srgb_perceptual_icc).unwrap();
            assert_eq!(f_p.pcs, DataColorSpace::Lab);
            assert_eq!(f_p.color_space, DataColorSpace::Rgb);
            assert_eq!(f_p.version(), ProfileVersion::V4_2);
            assert!(f_p.lut_a_to_b_perceptual.is_some());
            assert!(f_p.lut_b_to_a_perceptual.is_some());
        }
    }

    #[test]
    fn test_us_swop_coated() {
        if let Ok(us_swop_coated) = fs::read("./assets/us_swop_coated.icc") {
            let f_p = ColorProfile::new_from_slice(&us_swop_coated).unwrap();
            assert_eq!(f_p.pcs, DataColorSpace::Lab);
            assert_eq!(f_p.color_space, DataColorSpace::Cmyk);
            assert_eq!(f_p.version(), ProfileVersion::V2_0);

            assert!(f_p.lut_a_to_b_perceptual.is_some());
            assert!(f_p.lut_b_to_a_perceptual.is_some());

            assert!(f_p.lut_a_to_b_colorimetric.is_some());
            assert!(f_p.lut_b_to_a_colorimetric.is_some());

            assert!(f_p.gamut.is_some());

            assert!(f_p.copyright.is_some());
            assert!(f_p.description.is_some());
        }
    }

    #[test]
    fn test_matrix_shaper() {
        if let Ok(matrix_shaper) = fs::read("./assets/Display P3.icc") {
            let f_p = ColorProfile::new_from_slice(&matrix_shaper).unwrap();
            assert_eq!(f_p.pcs, DataColorSpace::Xyz);
            assert_eq!(f_p.color_space, DataColorSpace::Rgb);
            assert_eq!(f_p.version(), ProfileVersion::V4_0);

            assert!(f_p.red_trc.is_some());
            assert!(f_p.blue_trc.is_some());
            assert!(f_p.green_trc.is_some());

            assert_ne!(f_p.red_colorant, Xyzd::default());
            assert_ne!(f_p.blue_colorant, Xyzd::default());
            assert_ne!(f_p.green_colorant, Xyzd::default());

            assert!(f_p.copyright.is_some());
            assert!(f_p.description.is_some());
        }
    }
}
