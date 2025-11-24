//! Decoding of netpbm image formats (pbm, pgm, ppm and pam).
//!
//! The formats pbm, pgm and ppm are fully supported. Only the official subformats
//! (`BLACKANDWHITE`, `GRAYSCALE`, `RGB`, `BLACKANDWHITE_ALPHA`, `GRAYSCALE_ALPHA`,
//! and `RGB_ALPHA`) of pam are supported; custom tuple types have no clear
//! interpretation as an image and will be rejected.
use self::autobreak::AutoBreak;
pub use self::decoder::PnmDecoder;
pub use self::encoder::PnmEncoder;
use self::header::HeaderRecord;
pub use self::header::{
    ArbitraryHeader, ArbitraryTuplType, BitmapHeader, GraymapHeader, PixmapHeader,
};
pub use self::header::{PnmHeader, PnmSubtype, SampleEncoding};

mod autobreak;
mod decoder;
mod encoder;
mod header;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExtendedColorType;
    use crate::ImageDecoder as _;
    use byteorder_lite::{ByteOrder, NativeEndian};

    fn execute_roundtrip_default(buffer: &[u8], width: u32, height: u32, color: ExtendedColorType) {
        let mut encoded_buffer = Vec::new();

        {
            let mut encoder = PnmEncoder::new(&mut encoded_buffer);
            encoder
                .encode(buffer, width, height, color)
                .expect("Failed to encode the image buffer");
        }

        let (header, loaded_color, loaded_image) = {
            let decoder = PnmDecoder::new(&encoded_buffer[..]).unwrap();
            let color_type = decoder.color_type();
            let mut image = vec![0; decoder.total_bytes() as usize];
            decoder
                .read_image(&mut image)
                .expect("Failed to decode the image");
            let (_, header) = PnmDecoder::new(&encoded_buffer[..]).unwrap().into_inner();
            (header, color_type, image)
        };

        assert_eq!(header.width(), width);
        assert_eq!(header.height(), height);
        assert_eq!(ExtendedColorType::from(loaded_color), color);
        assert_eq!(loaded_image.as_slice(), buffer);
    }

    fn execute_roundtrip_with_subtype(
        buffer: &[u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
        subtype: PnmSubtype,
    ) {
        let mut encoded_buffer = Vec::new();

        {
            let mut encoder = PnmEncoder::new(&mut encoded_buffer).with_subtype(subtype);
            encoder
                .encode(buffer, width, height, color)
                .expect("Failed to encode the image buffer");
        }

        let (header, loaded_color, loaded_image) = {
            let decoder = PnmDecoder::new(&encoded_buffer[..]).unwrap();
            let color_type = decoder.color_type();
            let mut image = vec![0; decoder.total_bytes() as usize];
            decoder
                .read_image(&mut image)
                .expect("Failed to decode the image");
            let (_, header) = PnmDecoder::new(&encoded_buffer[..]).unwrap().into_inner();
            (header, color_type, image)
        };

        assert_eq!(header.width(), width);
        assert_eq!(header.height(), height);
        assert_eq!(header.subtype(), subtype);
        assert_eq!(ExtendedColorType::from(loaded_color), color);
        assert_eq!(loaded_image.as_slice(), buffer);
    }

    fn execute_roundtrip_u16(buffer: &[u16], width: u32, height: u32, color: ExtendedColorType) {
        let mut encoded_buffer = Vec::new();

        {
            let mut encoder = PnmEncoder::new(&mut encoded_buffer);
            encoder
                .encode(buffer, width, height, color)
                .expect("Failed to encode the image buffer");
        }

        let (header, loaded_color, loaded_image) = {
            let decoder = PnmDecoder::new(&encoded_buffer[..]).unwrap();
            let color_type = decoder.color_type();
            let mut image = vec![0; decoder.total_bytes() as usize];
            decoder
                .read_image(&mut image)
                .expect("Failed to decode the image");
            let (_, header) = PnmDecoder::new(&encoded_buffer[..]).unwrap().into_inner();
            (header, color_type, image)
        };

        let mut buffer_u8 = vec![0; buffer.len() * 2];
        NativeEndian::write_u16_into(buffer, &mut buffer_u8[..]);

        assert_eq!(header.width(), width);
        assert_eq!(header.height(), height);
        assert_eq!(ExtendedColorType::from(loaded_color), color);
        assert_eq!(loaded_image, buffer_u8);
    }

    #[test]
    fn roundtrip_gray() {
        #[rustfmt::skip]
        let buf: [u8; 16] = [
            0, 0, 0, 255,
            255, 255, 255, 255,
            255, 0, 255, 0,
            255, 0, 0, 0,
        ];

        execute_roundtrip_default(&buf, 4, 4, ExtendedColorType::L8);
        execute_roundtrip_with_subtype(&buf, 4, 4, ExtendedColorType::L8, PnmSubtype::ArbitraryMap);
        execute_roundtrip_with_subtype(
            &buf,
            4,
            4,
            ExtendedColorType::L8,
            PnmSubtype::Graymap(SampleEncoding::Ascii),
        );
        execute_roundtrip_with_subtype(
            &buf,
            4,
            4,
            ExtendedColorType::L8,
            PnmSubtype::Graymap(SampleEncoding::Binary),
        );
    }

    #[test]
    fn roundtrip_rgb() {
        #[rustfmt::skip]
        let buf: [u8; 27] = [
              0,   0,   0,
              0,   0, 255,
              0, 255,   0,
              0, 255, 255,
            255,   0,   0,
            255,   0, 255,
            255, 255,   0,
            255, 255, 255,
            255, 255, 255,
        ];
        execute_roundtrip_default(&buf, 3, 3, ExtendedColorType::Rgb8);
        execute_roundtrip_with_subtype(
            &buf,
            3,
            3,
            ExtendedColorType::Rgb8,
            PnmSubtype::ArbitraryMap,
        );
        execute_roundtrip_with_subtype(
            &buf,
            3,
            3,
            ExtendedColorType::Rgb8,
            PnmSubtype::Pixmap(SampleEncoding::Binary),
        );
        execute_roundtrip_with_subtype(
            &buf,
            3,
            3,
            ExtendedColorType::Rgb8,
            PnmSubtype::Pixmap(SampleEncoding::Ascii),
        );
    }

    #[test]
    fn roundtrip_u16() {
        let buf: [u16; 6] = [0, 1, 0xFFFF, 0x1234, 0x3412, 0xBEAF];

        execute_roundtrip_u16(&buf, 6, 1, ExtendedColorType::L16);
    }
}
