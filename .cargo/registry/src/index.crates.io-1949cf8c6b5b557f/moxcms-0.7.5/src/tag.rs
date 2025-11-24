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
use crate::CmsError;

pub(crate) const TAG_SIZE: usize = 12;

#[derive(Debug, Copy, Clone, PartialEq, Ord, PartialOrd, Eq, Hash)]
pub(crate) enum Tag {
    RedXyz,
    GreenXyz,
    BlueXyz,
    RedToneReproduction,
    GreenToneReproduction,
    BlueToneReproduction,
    GreyToneReproduction,
    MediaWhitePoint,
    CodeIndependentPoints,
    ChromaticAdaptation,
    BlackPoint,
    DeviceToPcsLutPerceptual,
    DeviceToPcsLutColorimetric,
    DeviceToPcsLutSaturation,
    PcsToDeviceLutPerceptual,
    PcsToDeviceLutColorimetric,
    PcsToDeviceLutSaturation,
    ProfileDescription,
    Copyright,
    ViewingConditionsDescription,
    DeviceManufacturer,
    DeviceModel,
    Gamut,
    Luminance,
    Measurement,
    Chromaticity,
    ObserverConditions,
    CharTarget,
    Technology,
    CalibrationDateTime,
}

impl TryFrom<u32> for Tag {
    type Error = CmsError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == u32::from_ne_bytes(*b"rXYZ").to_be() {
            return Ok(Self::RedXyz);
        } else if value == u32::from_ne_bytes(*b"gXYZ").to_be() {
            return Ok(Self::GreenXyz);
        } else if value == u32::from_ne_bytes(*b"bXYZ").to_be() {
            return Ok(Self::BlueXyz);
        } else if value == u32::from_ne_bytes(*b"rTRC").to_be() {
            return Ok(Self::RedToneReproduction);
        } else if value == u32::from_ne_bytes(*b"gTRC").to_be() {
            return Ok(Self::GreenToneReproduction);
        } else if value == u32::from_ne_bytes(*b"bTRC").to_be() {
            return Ok(Self::BlueToneReproduction);
        } else if value == u32::from_ne_bytes(*b"kTRC").to_be() {
            return Ok(Self::GreyToneReproduction);
        } else if value == u32::from_ne_bytes(*b"wtpt").to_be() {
            return Ok(Self::MediaWhitePoint);
        } else if value == u32::from_ne_bytes(*b"cicp").to_be() {
            return Ok(Self::CodeIndependentPoints);
        } else if value == u32::from_ne_bytes(*b"chad").to_be() {
            return Ok(Self::ChromaticAdaptation);
        } else if value == u32::from_ne_bytes(*b"bkpt").to_be() {
            return Ok(Self::BlackPoint);
        } else if value == u32::from_ne_bytes(*b"A2B0").to_be() {
            return Ok(Self::DeviceToPcsLutPerceptual);
        } else if value == u32::from_ne_bytes(*b"A2B1").to_be() {
            return Ok(Self::DeviceToPcsLutColorimetric);
        } else if value == u32::from_ne_bytes(*b"A2B2").to_be() {
            return Ok(Self::DeviceToPcsLutSaturation);
        } else if value == u32::from_ne_bytes(*b"B2A0").to_be() {
            return Ok(Self::PcsToDeviceLutPerceptual);
        } else if value == u32::from_ne_bytes(*b"B2A1").to_be() {
            return Ok(Self::PcsToDeviceLutColorimetric);
        } else if value == u32::from_ne_bytes(*b"B2A2").to_be() {
            return Ok(Self::PcsToDeviceLutSaturation);
        } else if value == u32::from_ne_bytes(*b"desc").to_be() {
            return Ok(Self::ProfileDescription);
        } else if value == u32::from_ne_bytes(*b"cprt").to_be() {
            return Ok(Self::Copyright);
        } else if value == u32::from_ne_bytes(*b"vued").to_be() {
            return Ok(Self::ViewingConditionsDescription);
        } else if value == u32::from_ne_bytes(*b"dmnd").to_be() {
            return Ok(Self::DeviceManufacturer);
        } else if value == u32::from_ne_bytes(*b"dmdd").to_be() {
            return Ok(Self::DeviceModel);
        } else if value == u32::from_ne_bytes(*b"gamt").to_be() {
            return Ok(Self::Gamut);
        } else if value == u32::from_ne_bytes(*b"lumi").to_be() {
            return Ok(Self::Luminance);
        } else if value == u32::from_ne_bytes(*b"meas").to_be() {
            return Ok(Self::Measurement);
        } else if value == u32::from_ne_bytes(*b"chrm").to_be() {
            return Ok(Self::Chromaticity);
        } else if value == u32::from_ne_bytes(*b"view").to_be() {
            return Ok(Self::ObserverConditions);
        } else if value == u32::from_ne_bytes(*b"targ").to_be() {
            return Ok(Self::CharTarget);
        } else if value == u32::from_ne_bytes(*b"tech").to_be() {
            return Ok(Self::Technology);
        } else if value == u32::from_ne_bytes(*b"calt").to_be() {
            return Ok(Self::CalibrationDateTime);
        }
        Err(CmsError::UnknownTag(value))
    }
}

impl From<Tag> for u32 {
    fn from(value: Tag) -> Self {
        match value {
            Tag::RedXyz => u32::from_ne_bytes(*b"rXYZ").to_be(),
            Tag::GreenXyz => u32::from_ne_bytes(*b"gXYZ").to_be(),
            Tag::BlueXyz => u32::from_ne_bytes(*b"bXYZ").to_be(),
            Tag::RedToneReproduction => u32::from_ne_bytes(*b"rTRC").to_be(),
            Tag::GreenToneReproduction => u32::from_ne_bytes(*b"gTRC").to_be(),
            Tag::BlueToneReproduction => u32::from_ne_bytes(*b"bTRC").to_be(),
            Tag::GreyToneReproduction => u32::from_ne_bytes(*b"kTRC").to_be(),
            Tag::MediaWhitePoint => u32::from_ne_bytes(*b"wtpt").to_be(),
            Tag::CodeIndependentPoints => u32::from_ne_bytes(*b"cicp").to_be(),
            Tag::ChromaticAdaptation => u32::from_ne_bytes(*b"chad").to_be(),
            Tag::BlackPoint => u32::from_ne_bytes(*b"bkpt").to_be(),
            Tag::DeviceToPcsLutPerceptual => u32::from_ne_bytes(*b"A2B0").to_be(),
            Tag::DeviceToPcsLutColorimetric => u32::from_ne_bytes(*b"A2B1").to_be(),
            Tag::DeviceToPcsLutSaturation => u32::from_ne_bytes(*b"A2B2").to_be(),
            Tag::PcsToDeviceLutPerceptual => u32::from_ne_bytes(*b"B2A0").to_be(),
            Tag::PcsToDeviceLutColorimetric => u32::from_ne_bytes(*b"B2A1").to_be(),
            Tag::PcsToDeviceLutSaturation => u32::from_ne_bytes(*b"B2A2").to_be(),
            Tag::ProfileDescription => u32::from_ne_bytes(*b"desc").to_be(),
            Tag::Copyright => u32::from_ne_bytes(*b"cprt").to_be(),
            Tag::ViewingConditionsDescription => u32::from_ne_bytes(*b"vued").to_be(),
            Tag::DeviceManufacturer => u32::from_ne_bytes(*b"dmnd").to_be(),
            Tag::DeviceModel => u32::from_ne_bytes(*b"dmdd").to_be(),
            Tag::Gamut => u32::from_ne_bytes(*b"gamt").to_be(),
            Tag::Luminance => u32::from_ne_bytes(*b"lumi").to_be(),
            Tag::Measurement => u32::from_ne_bytes(*b"meas").to_be(),
            Tag::Chromaticity => u32::from_ne_bytes(*b"chrm").to_be(),
            Tag::ObserverConditions => u32::from_ne_bytes(*b"view").to_be(),
            Tag::CharTarget => u32::from_ne_bytes(*b"targ").to_be(),
            Tag::Technology => u32::from_ne_bytes(*b"tech").to_be(),
            Tag::CalibrationDateTime => u32::from_ne_bytes(*b"calt").to_be(),
        }
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub(crate) enum TagTypeDefinition {
    Text,
    MultiLocalizedUnicode,
    Description,
    MabLut,
    MbaLut,
    ParametricToneCurve,
    LutToneCurve,
    Xyz,
    MultiProcessElement,
    DefViewingConditions,
    Signature,
    Cicp,
    DateTime,
    S15Fixed16Array,
    U8Array,
    U16Fixed16Array,
    U16Array,
    U32Array,
    U64Array,
    Measurement,
    NotAllowed,
}

impl From<u32> for TagTypeDefinition {
    fn from(value: u32) -> Self {
        if value == u32::from_ne_bytes(*b"mluc").to_be() {
            return TagTypeDefinition::MultiLocalizedUnicode;
        } else if value == u32::from_ne_bytes(*b"desc").to_be() {
            return TagTypeDefinition::Description;
        } else if value == u32::from_ne_bytes(*b"text").to_be() {
            return TagTypeDefinition::Text;
        } else if value == u32::from_ne_bytes(*b"mAB ").to_be() {
            return TagTypeDefinition::MabLut;
        } else if value == u32::from_ne_bytes(*b"mBA ").to_be() {
            return TagTypeDefinition::MbaLut;
        } else if value == u32::from_ne_bytes(*b"para").to_be() {
            return TagTypeDefinition::ParametricToneCurve;
        } else if value == u32::from_ne_bytes(*b"curv").to_be() {
            return TagTypeDefinition::LutToneCurve;
        } else if value == u32::from_ne_bytes(*b"XYZ ").to_be() {
            return TagTypeDefinition::Xyz;
        } else if value == u32::from_ne_bytes(*b"mpet").to_be() {
            return TagTypeDefinition::MultiProcessElement;
        } else if value == u32::from_ne_bytes(*b"view").to_be() {
            return TagTypeDefinition::DefViewingConditions;
        } else if value == u32::from_ne_bytes(*b"sig ").to_be() {
            return TagTypeDefinition::Signature;
        } else if value == u32::from_ne_bytes(*b"cicp").to_be() {
            return TagTypeDefinition::Cicp;
        } else if value == u32::from_ne_bytes(*b"dtim").to_be() {
            return TagTypeDefinition::DateTime;
        } else if value == u32::from_ne_bytes(*b"meas").to_be() {
            return TagTypeDefinition::Measurement;
        } else if value == u32::from_ne_bytes(*b"sf32").to_be() {
            return TagTypeDefinition::S15Fixed16Array;
        } else if value == u32::from_ne_bytes(*b"uf32").to_be() {
            return TagTypeDefinition::U16Fixed16Array;
        } else if value == u32::from_ne_bytes(*b"ui16").to_be() {
            return TagTypeDefinition::U16Array;
        } else if value == u32::from_ne_bytes(*b"ui32").to_be() {
            return TagTypeDefinition::U32Array;
        } else if value == u32::from_ne_bytes(*b"ui64").to_be() {
            return TagTypeDefinition::U64Array;
        } else if value == u32::from_ne_bytes(*b"ui08").to_be() {
            return TagTypeDefinition::U8Array;
        }
        TagTypeDefinition::NotAllowed
    }
}

impl From<TagTypeDefinition> for u32 {
    fn from(value: TagTypeDefinition) -> Self {
        match value {
            TagTypeDefinition::MultiLocalizedUnicode => u32::from_ne_bytes(*b"mluc").to_be(),
            TagTypeDefinition::Description => u32::from_ne_bytes(*b"desc").to_be(),
            TagTypeDefinition::Text => u32::from_ne_bytes(*b"text").to_be(),
            TagTypeDefinition::MabLut => u32::from_ne_bytes(*b"mAB ").to_be(),
            TagTypeDefinition::MbaLut => u32::from_ne_bytes(*b"mBA ").to_be(),
            TagTypeDefinition::ParametricToneCurve => u32::from_ne_bytes(*b"para").to_be(),
            TagTypeDefinition::LutToneCurve => u32::from_ne_bytes(*b"curv").to_be(),
            TagTypeDefinition::Xyz => u32::from_ne_bytes(*b"XYZ ").to_be(),
            TagTypeDefinition::MultiProcessElement => u32::from_ne_bytes(*b"mpet").to_be(),
            TagTypeDefinition::DefViewingConditions => u32::from_ne_bytes(*b"view").to_be(),
            TagTypeDefinition::Signature => u32::from_ne_bytes(*b"sig ").to_be(),
            TagTypeDefinition::Cicp => u32::from_ne_bytes(*b"cicp").to_be(),
            TagTypeDefinition::DateTime => u32::from_ne_bytes(*b"dtim").to_be(),
            TagTypeDefinition::S15Fixed16Array => u32::from_ne_bytes(*b"sf32").to_be(),
            TagTypeDefinition::U16Fixed16Array => u32::from_ne_bytes(*b"uf32").to_be(),
            TagTypeDefinition::U8Array => u32::from_ne_bytes(*b"ui08").to_be(),
            TagTypeDefinition::U16Array => u32::from_ne_bytes(*b"ui16").to_be(),
            TagTypeDefinition::U32Array => u32::from_ne_bytes(*b"ui32").to_be(),
            TagTypeDefinition::U64Array => u32::from_ne_bytes(*b"ui64").to_be(),
            TagTypeDefinition::Measurement => u32::from_ne_bytes(*b"meas").to_be(),
            TagTypeDefinition::NotAllowed => 0,
        }
    }
}
