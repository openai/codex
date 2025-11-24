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
use crate::profile::{LutDataType, ProfileHeader};
use crate::tag::{TAG_SIZE, Tag, TagTypeDefinition};
use crate::trc::ToneReprCurve;
use crate::{
    CicpProfile, CmsError, ColorDateTime, ColorProfile, DataColorSpace, LocalizableString,
    LutMultidimensionalType, LutStore, LutType, LutWarehouse, Matrix3d, ProfileClass,
    ProfileSignature, ProfileText, ProfileVersion, Vector3d, Xyzd,
};

pub(crate) trait FloatToFixedS15Fixed16 {
    fn to_s15_fixed16(self) -> i32;
}

pub(crate) trait FloatToFixedU8Fixed8 {
    fn to_u8_fixed8(self) -> u16;
}

// pub(crate) trait FloatToFixedU16 {
//     fn to_fixed_u16(self) -> u16;
// }

// impl FloatToFixedU16 for f32 {
//     #[inline]
//     fn to_fixed_u16(self) -> u16 {
//         const SCALE: f64 = (1 << 16) as f64;
//         (self as f64 * SCALE + 0.5)
//             .floor()
//             .clamp(u16::MIN as f64, u16::MAX as f64) as u16
//     }
// }

impl FloatToFixedS15Fixed16 for f32 {
    #[inline]
    fn to_s15_fixed16(self) -> i32 {
        const SCALE: f64 = (1 << 16) as f64;
        (self as f64 * SCALE + 0.5)
            .floor()
            .clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }
}

impl FloatToFixedS15Fixed16 for f64 {
    #[inline]
    fn to_s15_fixed16(self) -> i32 {
        const SCALE: f64 = (1 << 16) as f64;
        (self * SCALE + 0.5)
            .floor()
            .clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }
}

#[inline]
fn write_u32_be(into: &mut Vec<u8>, value: u32) {
    let bytes = value.to_be_bytes();
    into.push(bytes[0]);
    into.push(bytes[1]);
    into.push(bytes[2]);
    into.push(bytes[3]);
}

#[inline]
pub(crate) fn write_u16_be(into: &mut Vec<u8>, value: u16) {
    let bytes = value.to_be_bytes();
    into.push(bytes[0]);
    into.push(bytes[1]);
}

#[inline]
fn write_i32_be(into: &mut Vec<u8>, value: i32) {
    let bytes = value.to_be_bytes();
    into.push(bytes[0]);
    into.push(bytes[1]);
    into.push(bytes[2]);
    into.push(bytes[3]);
}

fn first_two_ascii_bytes(s: &String) -> [u8; 2] {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        bytes[0..2].try_into().unwrap()
    } else if bytes.len() == 1 {
        let vec = vec![bytes[0], 0u8];
        vec.try_into().unwrap()
    } else {
        let vec = vec![0u8, 0u8];
        vec.try_into().unwrap()
    }
}

/// Writes Multi Localized Unicode
#[inline]
fn write_mluc(into: &mut Vec<u8>, strings: &[LocalizableString]) -> usize {
    assert!(!strings.is_empty());
    let start = into.len();
    let tag_def: u32 = TagTypeDefinition::MultiLocalizedUnicode.into();
    write_u32_be(into, tag_def);
    write_u32_be(into, 0);
    let number_of_records = strings.len();
    write_u32_be(into, number_of_records as u32);
    write_u32_be(into, 12); // Record size, must be 12
    let lang = first_two_ascii_bytes(&strings[0].language);
    into.extend_from_slice(&lang);
    let country = first_two_ascii_bytes(&strings[0].country);
    into.extend_from_slice(&country);
    let first_string_len = strings[0].value.len() * 2;
    write_u32_be(into, first_string_len as u32);
    let mut first_string_offset = 16 + 12 * strings.len();
    write_u32_be(into, first_string_offset as u32);
    first_string_offset += first_string_len;
    for record in strings.iter().skip(1) {
        let lang = first_two_ascii_bytes(&record.language);
        into.extend_from_slice(&lang);
        let country = first_two_ascii_bytes(&record.country);
        into.extend_from_slice(&country);
        let first_string_len = record.value.len() * 2;
        write_u32_be(into, first_string_len as u32);
        write_u32_be(into, first_string_offset as u32);
        first_string_offset += first_string_len;
    }
    for record in strings.iter() {
        for chunk in record.value.encode_utf16() {
            write_u16_be(into, chunk);
        }
    }
    let end = into.len();
    end - start
}

#[inline]
fn write_string_value(into: &mut Vec<u8>, text: &ProfileText) -> usize {
    match text {
        ProfileText::PlainString(text) => {
            let vec = vec![LocalizableString {
                language: "en".to_string(),
                country: "US".to_string(),
                value: text.clone(),
            }];
            write_mluc(into, &vec)
        }
        ProfileText::Localizable(localizable) => {
            if localizable.is_empty() {
                return 0;
            }
            write_mluc(into, localizable)
        }
        ProfileText::Description(description) => {
            let vec = vec![LocalizableString {
                language: "en".to_string(),
                country: "US".to_string(),
                value: description.unicode_string.clone(),
            }];
            write_mluc(into, &vec)
        }
    }
}

#[inline]
fn write_xyz_tag_value(into: &mut Vec<u8>, xyz: Xyzd) {
    let tag_definition: u32 = TagTypeDefinition::Xyz.into();
    write_u32_be(into, tag_definition);
    write_u32_be(into, 0);
    let x_fixed = xyz.x.to_s15_fixed16();
    write_i32_be(into, x_fixed);
    let y_fixed = xyz.y.to_s15_fixed16();
    write_i32_be(into, y_fixed);
    let z_fixed = xyz.z.to_s15_fixed16();
    write_i32_be(into, z_fixed);
}

#[inline]
fn write_tag_entry(into: &mut Vec<u8>, tag: Tag, tag_entry: usize, tag_size: usize) {
    let tag_value: u32 = tag.into();
    write_u32_be(into, tag_value);
    write_u32_be(into, tag_entry as u32);
    write_u32_be(into, tag_size as u32);
}

fn write_trc_entry(into: &mut Vec<u8>, trc: &ToneReprCurve) -> Result<usize, CmsError> {
    match trc {
        ToneReprCurve::Lut(lut) => {
            let curv: u32 = TagTypeDefinition::LutToneCurve.into();
            write_u32_be(into, curv);
            write_u32_be(into, 0);
            write_u32_be(into, lut.len() as u32);
            for item in lut.iter() {
                write_u16_be(into, *item);
            }
            Ok(12 + lut.len() * 2)
        }
        ToneReprCurve::Parametric(parametric_curve) => {
            if parametric_curve.len() > 7
                || parametric_curve.len() == 6
                || parametric_curve.len() == 2
            {
                return Err(CmsError::InvalidProfile);
            }
            let para: u32 = TagTypeDefinition::ParametricToneCurve.into();
            write_u32_be(into, para);
            write_u32_be(into, 0);
            if parametric_curve.len() == 1 {
                write_u16_be(into, 0);
            } else if parametric_curve.len() == 3 {
                write_u16_be(into, 1);
            } else if parametric_curve.len() == 4 {
                write_u16_be(into, 2);
            } else if parametric_curve.len() == 5 {
                write_u16_be(into, 3);
            } else if parametric_curve.len() == 7 {
                write_u16_be(into, 4);
            }
            write_u16_be(into, 0);
            for item in parametric_curve.iter() {
                write_i32_be(into, item.to_s15_fixed16());
            }
            Ok(12 + 4 * parametric_curve.len())
        }
    }
}

#[inline]
fn write_cicp_entry(into: &mut Vec<u8>, cicp: &CicpProfile) {
    let cicp_tag: u32 = TagTypeDefinition::Cicp.into();
    write_u32_be(into, cicp_tag);
    write_u32_be(into, 0);
    into.push(cicp.color_primaries as u8);
    into.push(cicp.transfer_characteristics as u8);
    into.push(cicp.matrix_coefficients as u8);
    into.push(if cicp.full_range { 1 } else { 0 });
}

fn write_chad(into: &mut Vec<u8>, matrix: Matrix3d) {
    let arr_type: u32 = TagTypeDefinition::S15Fixed16Array.into();
    write_u32_be(into, arr_type);
    write_u32_be(into, 0);
    write_matrix3d(into, matrix);
}

#[inline]
fn write_matrix3d(into: &mut Vec<u8>, v: Matrix3d) {
    write_i32_be(into, v.v[0][0].to_s15_fixed16());
    write_i32_be(into, v.v[0][1].to_s15_fixed16());
    write_i32_be(into, v.v[0][2].to_s15_fixed16());

    write_i32_be(into, v.v[1][0].to_s15_fixed16());
    write_i32_be(into, v.v[1][1].to_s15_fixed16());
    write_i32_be(into, v.v[1][2].to_s15_fixed16());

    write_i32_be(into, v.v[2][0].to_s15_fixed16());
    write_i32_be(into, v.v[2][1].to_s15_fixed16());
    write_i32_be(into, v.v[2][2].to_s15_fixed16());
}

#[inline]
fn write_vector3d(into: &mut Vec<u8>, v: Vector3d) {
    write_i32_be(into, v.v[0].to_s15_fixed16());
    write_i32_be(into, v.v[1].to_s15_fixed16());
    write_i32_be(into, v.v[2].to_s15_fixed16());
}

#[inline]
fn write_lut_entry(into: &mut Vec<u8>, lut: &LutDataType) -> Result<usize, CmsError> {
    if !lut.has_same_kind() {
        return Err(CmsError::InvalidProfile);
    }
    let start = into.len();
    let lut16_tag: u32 = match &lut.input_table {
        LutStore::Store8(_) => LutType::Lut8.into(),
        LutStore::Store16(_) => LutType::Lut16.into(),
    };
    write_u32_be(into, lut16_tag);
    write_u32_be(into, 0);
    into.push(lut.num_input_channels);
    into.push(lut.num_output_channels);
    into.push(lut.num_clut_grid_points);
    into.push(0);
    write_matrix3d(into, lut.matrix);
    write_u16_be(into, lut.num_input_table_entries);
    write_u16_be(into, lut.num_output_table_entries);
    match &lut.input_table {
        LutStore::Store8(input_table) => {
            for &item in input_table.iter() {
                into.push(item);
            }
        }
        LutStore::Store16(input_table) => {
            for &item in input_table.iter() {
                write_u16_be(into, item);
            }
        }
    }
    match &lut.clut_table {
        LutStore::Store8(input_table) => {
            for &item in input_table.iter() {
                into.push(item);
            }
        }
        LutStore::Store16(input_table) => {
            for &item in input_table.iter() {
                write_u16_be(into, item);
            }
        }
    }
    match &lut.output_table {
        LutStore::Store8(input_table) => {
            for &item in input_table.iter() {
                into.push(item);
            }
        }
        LutStore::Store16(input_table) => {
            for &item in input_table.iter() {
                write_u16_be(into, item);
            }
        }
    }
    let end = into.len();
    Ok(end - start)
}

#[inline]
fn write_mab_entry(
    into: &mut Vec<u8>,
    lut: &LutMultidimensionalType,
    is_a_to_b: bool,
) -> Result<usize, CmsError> {
    let start = into.len();
    let lut16_tag: u32 = if is_a_to_b {
        LutType::LutMab.into()
    } else {
        LutType::LutMba.into()
    };
    write_u32_be(into, lut16_tag);
    write_u32_be(into, 0);
    into.push(lut.num_input_channels);
    into.push(lut.num_output_channels);
    write_u16_be(into, 0);
    let mut working_offset = 32usize;

    let mut data = Vec::new();

    // Offset to "B curves"
    if !lut.b_curves.is_empty() {
        while working_offset % 4 != 0 {
            data.push(0);
            working_offset += 1;
        }

        write_u32_be(into, working_offset as u32);

        for trc in lut.b_curves.iter() {
            let curve_size = write_trc_entry(&mut data, trc)?;
            working_offset += curve_size;
            while working_offset % 4 != 0 {
                data.push(0);
                working_offset += 1;
            }
        }
    } else {
        write_u32_be(into, 0);
    }

    // Offset to matrix
    if !lut.m_curves.is_empty() {
        while working_offset % 4 != 0 {
            data.push(0);
            working_offset += 1;
        }

        write_u32_be(into, working_offset as u32);
        write_matrix3d(&mut data, lut.matrix);
        write_vector3d(&mut data, lut.bias);
        working_offset += 9 * 4 + 3 * 4;
        // Offset to "M curves"
        write_u32_be(into, working_offset as u32);
        for trc in lut.m_curves.iter() {
            let curve_size = write_trc_entry(&mut data, trc)?;
            working_offset += curve_size;
            while working_offset % 4 != 0 {
                data.push(0);
                working_offset += 1;
            }
        }
    } else {
        // Offset to matrix
        write_u32_be(into, 0);
        // Offset to "M curves"
        write_u32_be(into, 0);
    }

    let mut clut_start = data.len();

    // Offset to CLUT
    if let Some(clut) = &lut.clut {
        while working_offset % 4 != 0 {
            data.push(0);
            working_offset += 1;
        }

        clut_start = data.len();

        write_u32_be(into, working_offset as u32);

        // Writing CLUT
        for &pt in lut.grid_points.iter() {
            data.push(pt);
        }
        data.push(match clut {
            LutStore::Store8(_) => 1,
            LutStore::Store16(_) => 2,
        }); // Entry size
        data.push(0);
        data.push(0);
        data.push(0);
        match clut {
            LutStore::Store8(store) => {
                for &element in store.iter() {
                    data.push(element)
                }
            }
            LutStore::Store16(store) => {
                for &element in store.iter() {
                    write_u16_be(&mut data, element);
                }
            }
        }
    } else {
        write_u32_be(into, 0);
    }

    let clut_size = data.len() - clut_start;
    working_offset += clut_size;

    // Offset to "A curves"
    if !lut.a_curves.is_empty() {
        while working_offset % 4 != 0 {
            data.push(0);
            working_offset += 1;
        }

        write_u32_be(into, working_offset as u32);

        for trc in lut.a_curves.iter() {
            let curve_size = write_trc_entry(&mut data, trc)?;
            working_offset += curve_size;
            while working_offset % 4 != 0 {
                data.push(0);
                working_offset += 1;
            }
        }
    } else {
        write_u32_be(into, 0);
    }

    into.extend(data);

    let end = into.len();
    Ok(end - start)
}

fn write_lut(into: &mut Vec<u8>, lut: &LutWarehouse, is_a_to_b: bool) -> Result<usize, CmsError> {
    match lut {
        LutWarehouse::Lut(lut) => Ok(write_lut_entry(into, lut)?),
        LutWarehouse::Multidimensional(mab) => write_mab_entry(into, mab, is_a_to_b),
    }
}

impl ProfileHeader {
    fn encode(&self) -> Vec<u8> {
        let mut encoder: Vec<u8> = Vec::with_capacity(size_of::<ProfileHeader>());
        write_u32_be(&mut encoder, self.size); // Size
        write_u32_be(&mut encoder, 0); // CMM Type
        write_u32_be(&mut encoder, self.version.into()); // Version Number Type
        write_u32_be(&mut encoder, self.profile_class.into()); // Profile class
        write_u32_be(&mut encoder, self.data_color_space.into()); // Data color space
        write_u32_be(&mut encoder, self.pcs.into()); // PCS
        self.creation_date_time.encode(&mut encoder); // Date time
        write_u32_be(&mut encoder, self.signature.into()); // Profile signature
        write_u32_be(&mut encoder, self.platform);
        write_u32_be(&mut encoder, self.flags);
        write_u32_be(&mut encoder, self.device_manufacturer);
        write_u32_be(&mut encoder, self.device_model);
        for &i in self.device_attributes.iter() {
            encoder.push(i);
        }
        write_u32_be(&mut encoder, self.rendering_intent.into());
        write_i32_be(&mut encoder, self.illuminant.x.to_s15_fixed16());
        write_i32_be(&mut encoder, self.illuminant.y.to_s15_fixed16());
        write_i32_be(&mut encoder, self.illuminant.z.to_s15_fixed16());
        write_u32_be(&mut encoder, self.creator);
        for &i in self.profile_id.iter() {
            encoder.push(i);
        }
        for &i in self.reserved.iter() {
            encoder.push(i);
        }
        write_u32_be(&mut encoder, self.tag_count);
        encoder
    }
}

impl ColorProfile {
    fn writable_tags_count(&self) -> usize {
        let mut tags_count = 0usize;
        if self.red_colorant != Xyzd::default() {
            tags_count += 1;
        }
        if self.green_colorant != Xyzd::default() {
            tags_count += 1;
        }
        if self.blue_colorant != Xyzd::default() {
            tags_count += 1;
        }
        if self.red_trc.is_some() {
            tags_count += 1;
        }
        if self.green_trc.is_some() {
            tags_count += 1;
        }
        if self.blue_trc.is_some() {
            tags_count += 1;
        }
        if self.gray_trc.is_some() {
            tags_count += 1;
        }
        if self.cicp.is_some() {
            tags_count += 1;
        }
        if self.media_white_point.is_some() {
            tags_count += 1;
        }
        if self.gamut.is_some() {
            tags_count += 1;
        }
        if self.chromatic_adaptation.is_some() {
            tags_count += 1;
        }
        if self.lut_a_to_b_perceptual.is_some() {
            tags_count += 1;
        }
        if self.lut_a_to_b_colorimetric.is_some() {
            tags_count += 1;
        }
        if self.lut_a_to_b_saturation.is_some() {
            tags_count += 1;
        }
        if self.lut_b_to_a_perceptual.is_some() {
            tags_count += 1;
        }
        if self.lut_b_to_a_colorimetric.is_some() {
            tags_count += 1;
        }
        if self.lut_b_to_a_saturation.is_some() {
            tags_count += 1;
        }
        if self.luminance.is_some() {
            tags_count += 1;
        }
        if let Some(description) = &self.description {
            if description.has_values() {
                tags_count += 1;
            }
        }
        if let Some(copyright) = &self.copyright {
            if copyright.has_values() {
                tags_count += 1;
            }
        }
        if let Some(vd) = &self.viewing_conditions_description {
            if vd.has_values() {
                tags_count += 1;
            }
        }
        if let Some(vd) = &self.device_model {
            if vd.has_values() {
                tags_count += 1;
            }
        }
        if let Some(vd) = &self.device_manufacturer {
            if vd.has_values() {
                tags_count += 1;
            }
        }
        tags_count
    }

    /// Encodes profile
    pub fn encode(&self) -> Result<Vec<u8>, CmsError> {
        let mut entries = Vec::new();
        let tags_count = self.writable_tags_count();
        let mut tags = Vec::with_capacity(TAG_SIZE * tags_count);
        let mut base_offset = size_of::<ProfileHeader>() + TAG_SIZE * tags_count;
        if self.red_colorant != Xyzd::default() {
            write_tag_entry(&mut tags, Tag::RedXyz, base_offset, 20);
            write_xyz_tag_value(&mut entries, self.red_colorant);
            base_offset += 20;
        }
        if self.green_colorant != Xyzd::default() {
            write_tag_entry(&mut tags, Tag::GreenXyz, base_offset, 20);
            write_xyz_tag_value(&mut entries, self.green_colorant);
            base_offset += 20;
        }
        if self.blue_colorant != Xyzd::default() {
            write_tag_entry(&mut tags, Tag::BlueXyz, base_offset, 20);
            write_xyz_tag_value(&mut entries, self.blue_colorant);
            base_offset += 20;
        }
        if let Some(chad) = self.chromatic_adaptation {
            write_tag_entry(&mut tags, Tag::ChromaticAdaptation, base_offset, 8 + 9 * 4);
            write_chad(&mut entries, chad);
            base_offset += 8 + 9 * 4;
        }
        if let Some(trc) = &self.red_trc {
            let entry_size = write_trc_entry(&mut entries, trc)?;
            write_tag_entry(&mut tags, Tag::RedToneReproduction, base_offset, entry_size);
            base_offset += entry_size;
        }
        if let Some(trc) = &self.green_trc {
            let entry_size = write_trc_entry(&mut entries, trc)?;
            write_tag_entry(
                &mut tags,
                Tag::GreenToneReproduction,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }
        if let Some(trc) = &self.blue_trc {
            let entry_size = write_trc_entry(&mut entries, trc)?;
            write_tag_entry(
                &mut tags,
                Tag::BlueToneReproduction,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }
        if let Some(trc) = &self.gray_trc {
            let entry_size = write_trc_entry(&mut entries, trc)?;
            write_tag_entry(
                &mut tags,
                Tag::GreyToneReproduction,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }
        if self.white_point != Xyzd::default() {
            write_tag_entry(&mut tags, Tag::MediaWhitePoint, base_offset, 20);
            write_xyz_tag_value(&mut entries, self.white_point);
            base_offset += 20;
        }

        let has_cicp = self.cicp.is_some();

        // This tag may be present when the data colour space in the profile header is RGB, YCbCr, or XYZ, and the
        // profile class in the profile header is Input or Display. The tag shall not be present for other data colour spaces
        // or profile classes indicated in the profile header.

        if let Some(cicp) = &self.cicp {
            if (self.profile_class == ProfileClass::InputDevice
                || self.profile_class == ProfileClass::DisplayDevice)
                && (self.color_space == DataColorSpace::Rgb
                    || self.color_space == DataColorSpace::YCbr
                    || self.color_space == DataColorSpace::Xyz)
            {
                write_tag_entry(&mut tags, Tag::CodeIndependentPoints, base_offset, 12);
                write_cicp_entry(&mut entries, cicp);
                base_offset += 12;
            }
        }

        if let Some(lut) = &self.lut_a_to_b_perceptual {
            let entry_size = write_lut(&mut entries, lut, true)?;
            write_tag_entry(
                &mut tags,
                Tag::DeviceToPcsLutPerceptual,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }

        if let Some(lut) = &self.lut_a_to_b_colorimetric {
            let entry_size = write_lut(&mut entries, lut, true)?;
            write_tag_entry(
                &mut tags,
                Tag::DeviceToPcsLutColorimetric,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }

        if let Some(lut) = &self.lut_a_to_b_saturation {
            let entry_size = write_lut(&mut entries, lut, true)?;
            write_tag_entry(
                &mut tags,
                Tag::DeviceToPcsLutSaturation,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }

        if let Some(lut) = &self.lut_b_to_a_perceptual {
            let entry_size = write_lut(&mut entries, lut, false)?;
            write_tag_entry(
                &mut tags,
                Tag::PcsToDeviceLutPerceptual,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }

        if let Some(lut) = &self.lut_b_to_a_colorimetric {
            let entry_size = write_lut(&mut entries, lut, false)?;
            write_tag_entry(
                &mut tags,
                Tag::PcsToDeviceLutColorimetric,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }

        if let Some(lut) = &self.lut_b_to_a_saturation {
            let entry_size = write_lut(&mut entries, lut, false)?;
            write_tag_entry(
                &mut tags,
                Tag::PcsToDeviceLutSaturation,
                base_offset,
                entry_size,
            );
            base_offset += entry_size;
        }

        if let Some(lut) = &self.gamut {
            let entry_size = write_lut(&mut entries, lut, false)?;
            write_tag_entry(&mut tags, Tag::Gamut, base_offset, entry_size);
            base_offset += entry_size;
        }

        if let Some(luminance) = self.luminance {
            write_tag_entry(&mut tags, Tag::Luminance, base_offset, 20);
            write_xyz_tag_value(&mut entries, luminance);
            base_offset += 20;
        }

        if let Some(description) = &self.description {
            if description.has_values() {
                let entry_size = write_string_value(&mut entries, description);
                write_tag_entry(&mut tags, Tag::ProfileDescription, base_offset, entry_size);
                base_offset += entry_size;
            }
        }

        if let Some(copyright) = &self.copyright {
            if copyright.has_values() {
                let entry_size = write_string_value(&mut entries, copyright);
                write_tag_entry(&mut tags, Tag::Copyright, base_offset, entry_size);
                base_offset += entry_size;
            }
        }

        if let Some(vd) = &self.viewing_conditions_description {
            if vd.has_values() {
                let entry_size = write_string_value(&mut entries, vd);
                write_tag_entry(
                    &mut tags,
                    Tag::ViewingConditionsDescription,
                    base_offset,
                    entry_size,
                );
                base_offset += entry_size;
            }
        }

        if let Some(vd) = &self.device_model {
            if vd.has_values() {
                let entry_size = write_string_value(&mut entries, vd);
                write_tag_entry(&mut tags, Tag::DeviceModel, base_offset, entry_size);
                base_offset += entry_size;
            }
        }

        if let Some(vd) = &self.device_manufacturer {
            if vd.has_values() {
                let entry_size = write_string_value(&mut entries, vd);
                write_tag_entry(&mut tags, Tag::DeviceManufacturer, base_offset, entry_size);
                // base_offset += entry_size;
            }
        }

        tags.extend(entries);

        let profile_header = ProfileHeader {
            size: size_of::<ProfileHeader>() as u32 + tags.len() as u32,
            pcs: self.pcs,
            profile_class: self.profile_class,
            rendering_intent: self.rendering_intent,
            cmm_type: 0,
            version: if has_cicp {
                ProfileVersion::V4_3
            } else {
                ProfileVersion::V4_0
            },
            data_color_space: self.color_space,
            creation_date_time: ColorDateTime::now(),
            signature: ProfileSignature::Acsp,
            platform: 0u32,
            flags: 0u32,
            device_manufacturer: 0u32,
            device_model: 0u32,
            device_attributes: [0u8; 8],
            illuminant: self.white_point.to_xyz(),
            creator: 0u32,
            profile_id: [0u8; 16],
            reserved: [0u8; 28],
            tag_count: tags_count as u32,
        };
        let mut header = profile_header.encode();
        header.extend(tags);
        Ok(header)
    }
}

impl FloatToFixedU8Fixed8 for f32 {
    #[inline]
    fn to_u8_fixed8(self) -> u16 {
        if self > 255.0 + 255.0 / 256f32 {
            0xffffu16
        } else if self < 0.0 {
            0u16
        } else {
            (self * 256.0 + 0.5).floor() as u16
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_u8_fixed8() {
        assert_eq!(0, 0f32.to_u8_fixed8());
        assert_eq!(0x0100, 1f32.to_u8_fixed8());
        assert_eq!(u16::MAX, (255f32 + (255f32 / 256f32)).to_u8_fixed8());
    }

    #[test]
    fn to_s15_fixed16() {
        assert_eq!(0x80000000u32 as i32, (-32768f32).to_s15_fixed16());
        assert_eq!(0, 0f32.to_s15_fixed16());
        assert_eq!(0x10000, 1.0f32.to_s15_fixed16());
        assert_eq!(
            i32::MAX,
            (32767f32 + (65535f32 / 65536f32)).to_s15_fixed16()
        );
    }
}
