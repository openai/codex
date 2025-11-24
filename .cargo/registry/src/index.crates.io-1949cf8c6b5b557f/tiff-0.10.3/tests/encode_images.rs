extern crate tiff;

use tiff::decoder::{ifd, Decoder, DecodingResult};
use tiff::encoder::{colortype, Ifd, Ifd8, SRational, TiffEncoder};
use tiff::tags::Tag;
use tiff::ColorType;

use std::fs::File;
use std::io::{Cursor, Seek, SeekFrom};
use std::path::PathBuf;

#[test]
fn encode_decode() {
    let mut image_data = Vec::new();
    for x in 0..100 {
        for y in 0..100u8 {
            let val = x + y;
            image_data.push(val);
            image_data.push(val);
            image_data.push(val);
        }
    }
    let mut file = Cursor::new(Vec::new());
    {
        let mut tiff = TiffEncoder::new(&mut file).unwrap();

        let mut image = tiff.new_image::<colortype::RGB8>(100, 100).unwrap();
        image
            .encoder()
            .write_tag(Tag::Artist, "Image-tiff")
            .unwrap();
        image.write_data(&image_data).unwrap();
    }
    {
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut decoder = Decoder::new(&mut file).unwrap();
        assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
        assert_eq!(decoder.dimensions().unwrap(), (100, 100));

        let mut all_tags = decoder
            .tag_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        all_tags.sort_by_key(|(t, _)| t.to_u16());
        assert_eq!(
            all_tags,
            vec![
                (Tag::ImageWidth, ifd::Value::Unsigned(100)),
                (Tag::ImageLength, ifd::Value::Unsigned(100)),
                (
                    Tag::BitsPerSample,
                    ifd::Value::List(vec![
                        ifd::Value::Short(8),
                        ifd::Value::Short(8),
                        ifd::Value::Short(8)
                    ])
                ),
                (Tag::Compression, ifd::Value::Short(1)),
                (Tag::PhotometricInterpretation, ifd::Value::Short(2)),
                (Tag::StripOffsets, ifd::Value::Unsigned(47)),
                (Tag::SamplesPerPixel, ifd::Value::Short(3)),
                (Tag::RowsPerStrip, ifd::Value::Unsigned(3334)),
                (Tag::StripByteCounts, ifd::Value::Unsigned(30000)),
                (Tag::XResolution, ifd::Value::Rational(1, 1)),
                (Tag::YResolution, ifd::Value::Rational(1, 1)),
                (Tag::ResolutionUnit, ifd::Value::Short(1)),
                (Tag::Artist, ifd::Value::Ascii("Image-tiff".into())),
                (Tag::Predictor, ifd::Value::Short(1)),
                (
                    Tag::SampleFormat,
                    ifd::Value::List(vec![
                        ifd::Value::Short(1),
                        ifd::Value::Short(1),
                        ifd::Value::Short(1)
                    ])
                ),
            ]
        );

        assert_eq!(
            decoder.get_tag(Tag::Artist).unwrap(),
            ifd::Value::Ascii("Image-tiff".into())
        );
        if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
            assert_eq!(image_data, img_res);
        } else {
            panic!("Wrong data type");
        }
    }
}

#[test]
fn encode_decode_big() {
    let mut image_data = Vec::new();
    for x in 0..100 {
        for y in 0..100u8 {
            let val = x + y;
            image_data.push(val);
            image_data.push(val);
            image_data.push(val);
        }
    }
    let mut file = Cursor::new(Vec::new());
    {
        let mut tiff = TiffEncoder::new_big(&mut file).unwrap();

        let mut image = tiff.new_image::<colortype::RGB8>(100, 100).unwrap();
        image
            .encoder()
            .write_tag(Tag::Artist, "Image-tiff")
            .unwrap();
        image.write_data(&image_data).unwrap();
    }
    {
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut decoder = Decoder::new(&mut file).unwrap();
        assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
        assert_eq!(decoder.dimensions().unwrap(), (100, 100));

        let mut all_tags = decoder
            .tag_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        all_tags.sort_by_key(|(t, _)| t.to_u16());
        assert_eq!(
            all_tags,
            vec![
                (Tag::ImageWidth, ifd::Value::Unsigned(100)),
                (Tag::ImageLength, ifd::Value::Unsigned(100)),
                (
                    Tag::BitsPerSample,
                    ifd::Value::List(vec![
                        ifd::Value::Short(8),
                        ifd::Value::Short(8),
                        ifd::Value::Short(8)
                    ])
                ),
                (Tag::Compression, ifd::Value::Short(1)),
                (Tag::PhotometricInterpretation, ifd::Value::Short(2)),
                (Tag::StripOffsets, ifd::Value::UnsignedBig(27)),
                (Tag::SamplesPerPixel, ifd::Value::Short(3)),
                (Tag::RowsPerStrip, ifd::Value::Unsigned(3334)),
                (Tag::StripByteCounts, ifd::Value::UnsignedBig(30000)),
                (Tag::XResolution, ifd::Value::Rational(1, 1)),
                (Tag::YResolution, ifd::Value::Rational(1, 1)),
                (Tag::ResolutionUnit, ifd::Value::Short(1)),
                (Tag::Artist, ifd::Value::Ascii("Image-tiff".into())),
                (Tag::Predictor, ifd::Value::Short(1)),
                (
                    Tag::SampleFormat,
                    ifd::Value::List(vec![
                        ifd::Value::Short(1),
                        ifd::Value::Short(1),
                        ifd::Value::Short(1)
                    ])
                ),
            ]
        );

        assert_eq!(
            decoder.get_tag(Tag::Artist).unwrap(),
            ifd::Value::Ascii("Image-tiff".into())
        );
        if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
            assert_eq!(image_data, img_res);
        } else {
            panic!("Wrong data type");
        }
    }
}

#[test]
fn test_encode_ifd() {
    let mut data = Cursor::new(Vec::new());

    {
        let mut tiff = TiffEncoder::new(&mut data).unwrap();
        let mut image_encoder = tiff.new_image::<colortype::Gray8>(1, 1).unwrap();
        image_encoder.write_strip(&[1]).unwrap();
        let encoder = image_encoder.encoder();

        // Use the "reusable" tags section as per the TIFF6 spec
        encoder.write_tag(Tag::Unknown(65000), Ifd(42u32)).unwrap();
        encoder
            .write_tag(Tag::Unknown(65001), &[Ifd(100u32)][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65002), &[Ifd(1u32), Ifd(2u32), Ifd(3u32)][..])
            .unwrap();

        encoder.write_tag(Tag::Unknown(65010), Ifd8(43u64)).unwrap();
        encoder
            .write_tag(Tag::Unknown(65011), &[Ifd8(100u64)][..])
            .unwrap();
        encoder
            .write_tag(
                Tag::Unknown(65012),
                &[Ifd8(1u64), Ifd8(2u64), Ifd8(3u64)][..],
            )
            .unwrap();
    }

    // Rewind the cursor for reading
    data.set_position(0);
    {
        let mut decoder = Decoder::new(&mut data).unwrap();

        assert_eq!(decoder.assert_tag_u32(65000), 42);
        assert_eq!(decoder.assert_tag_u32_vec(65000), [42]);
        assert_eq!(decoder.assert_tag_u32_vec(65001), [100]);
        assert_eq!(decoder.assert_tag_u32_vec(65002), [1, 2, 3]);

        assert_eq!(decoder.assert_tag_u64(65010), 43);
        assert_eq!(decoder.assert_tag_u64_vec(65010), [43]);
        assert_eq!(decoder.assert_tag_u64_vec(65011), [100]);
        assert_eq!(decoder.assert_tag_u64_vec(65012), [1, 2, 3]);
    }
}

#[test]
/// Test that attempting to encode when the input buffer is undersized returns
/// an error rather than panicking.
/// See: https://github.com/PistonDevelopers/image-tiff/issues/35
fn test_encode_undersized_buffer() {
    let input_data = vec![1, 2, 3];
    let output = Vec::new();
    let mut output_stream = Cursor::new(output);
    if let Ok(mut tiff) = TiffEncoder::new(&mut output_stream) {
        let res = tiff.write_image::<colortype::RGB8>(50, 50, &input_data);
        assert!(res.is_err());
    }
}

const TEST_IMAGE_DIR: &str = "./tests/images/";

macro_rules! test_roundtrip {
    ($name:ident, $buffer:ident, $buffer_ty:ty) => {
        fn $name<C: colortype::ColorType<Inner = $buffer_ty>>(
            file: &str,
            expected_type: ColorType,
        ) {
            let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
            let img_file = File::open(path).expect("Cannot find test image!");
            let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
            assert_eq!(decoder.colortype().unwrap(), expected_type);

            let image_data = match decoder.read_image().unwrap() {
                DecodingResult::$buffer(res) => res,
                _ => panic!("Wrong data type"),
            };

            let mut file = Cursor::new(Vec::new());
            {
                let mut tiff = TiffEncoder::new(&mut file).unwrap();

                let (width, height) = decoder.dimensions().unwrap();
                tiff.write_image::<C>(width, height, &image_data).unwrap();
            }
            file.seek(SeekFrom::Start(0)).unwrap();
            {
                let mut decoder = Decoder::new(&mut file).unwrap();
                if let DecodingResult::$buffer(img_res) = decoder.read_image().unwrap() {
                    assert_eq!(image_data, img_res);
                } else {
                    panic!("Wrong data type");
                }
            }
        }
    };
}

test_roundtrip!(test_u8_roundtrip, U8, u8);
test_roundtrip!(test_i8_roundtrip, I8, i8);
test_roundtrip!(test_u16_roundtrip, U16, u16);
test_roundtrip!(test_i16_roundtrip, I16, i16);
test_roundtrip!(test_u32_roundtrip, U32, u32);
test_roundtrip!(test_u64_roundtrip, U64, u64);
test_roundtrip!(test_f32_roundtrip, F32, f32);
test_roundtrip!(test_f64_roundtrip, F64, f64);

#[test]
fn test_gray_u8_roundtrip() {
    test_u8_roundtrip::<colortype::Gray8>("minisblack-1c-8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_gray_i8_roundtrip() {
    test_i8_roundtrip::<colortype::GrayI8>("minisblack-1c-i8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_rgb_u8_roundtrip() {
    test_u8_roundtrip::<colortype::RGB8>("rgb-3c-8b.tiff", ColorType::RGB(8));
}

#[test]
fn test_cmyk_u8_roundtrip() {
    test_u8_roundtrip::<colortype::CMYK8>("cmyk-3c-8b.tiff", ColorType::CMYK(8));
}

#[test]
fn test_cmyka_u8_roundtrip() {
    test_u8_roundtrip::<colortype::CMYKA8>("cmyk-4c-8b.tiff", ColorType::CMYKA(8));
}

#[test]
fn test_gray_u16_roundtrip() {
    test_u16_roundtrip::<colortype::Gray16>("minisblack-1c-16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_gray_i16_roundtrip() {
    test_i16_roundtrip::<colortype::GrayI16>("minisblack-1c-i16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_rgb_u16_roundtrip() {
    test_u16_roundtrip::<colortype::RGB16>("rgb-3c-16b.tiff", ColorType::RGB(16));
}

#[test]
fn test_cmyk_u16_roundtrip() {
    test_u16_roundtrip::<colortype::CMYK16>("cmyk-3c-16b.tiff", ColorType::CMYK(16));
}

#[test]
fn test_gray_u32_roundtrip() {
    test_u32_roundtrip::<colortype::Gray32>("gradient-1c-32b.tiff", ColorType::Gray(32));
}

#[test]
fn test_rgb_u32_roundtrip() {
    test_u32_roundtrip::<colortype::RGB32>("gradient-3c-32b.tiff", ColorType::RGB(32));
}

#[test]
fn test_gray_u64_roundtrip() {
    test_u64_roundtrip::<colortype::Gray64>("gradient-1c-64b.tiff", ColorType::Gray(64));
}

#[test]
fn test_rgb_u64_roundtrip() {
    test_u64_roundtrip::<colortype::RGB64>("gradient-3c-64b.tiff", ColorType::RGB(64));
}

#[test]
fn test_gray_f32_roundtrip() {
    test_f32_roundtrip::<colortype::Gray32Float>("gradient-1c-32b-float.tiff", ColorType::Gray(32));
}

#[test]
fn test_rgb_f32_roundtrip() {
    test_f32_roundtrip::<colortype::RGB32Float>("gradient-3c-32b-float.tiff", ColorType::RGB(32));
}

#[test]
fn test_cmyk_f32_roundtrip() {
    test_f32_roundtrip::<colortype::CMYK32Float>("cmyk-3c-32b-float.tiff", ColorType::CMYK(32));
}

#[test]
fn test_gray_f64_roundtrip() {
    test_f64_roundtrip::<colortype::Gray64Float>("gradient-1c-64b-float.tiff", ColorType::Gray(64));
}

#[test]
fn test_ycbcr_u8_roundtrip() {
    test_u8_roundtrip::<colortype::YCbCr8>("tiled-jpeg-ycbcr.tif", ColorType::YCbCr(8));
}

trait AssertDecode {
    fn assert_tag_u32(&mut self, tag: u16) -> u32;
    fn assert_tag_u32_vec(&mut self, tag: u16) -> Vec<u32>;
    fn assert_tag_i32(&mut self, tag: u16) -> i32;
    fn assert_tag_i32_vec(&mut self, tag: u16) -> Vec<i32>;
    fn assert_tag_u64(&mut self, tag: u16) -> u64;
    fn assert_tag_u64_vec(&mut self, tag: u16) -> Vec<u64>;
    fn assert_tag_i64(&mut self, tag: u16) -> i64;
    fn assert_tag_i64_vec(&mut self, tag: u16) -> Vec<i64>;
}

impl<R: std::io::Read + std::io::Seek> AssertDecode for Decoder<R> {
    fn assert_tag_u32(&mut self, tag: u16) -> u32 {
        self.get_tag(Tag::Unknown(tag)).unwrap().into_u32().unwrap()
    }
    fn assert_tag_u32_vec(&mut self, tag: u16) -> Vec<u32> {
        self.get_tag(Tag::Unknown(tag))
            .unwrap()
            .into_u32_vec()
            .unwrap()
    }
    fn assert_tag_i32(&mut self, tag: u16) -> i32 {
        self.get_tag(Tag::Unknown(tag)).unwrap().into_i32().unwrap()
    }
    fn assert_tag_i32_vec(&mut self, tag: u16) -> Vec<i32> {
        self.get_tag(Tag::Unknown(tag))
            .unwrap()
            .into_i32_vec()
            .unwrap()
    }
    fn assert_tag_u64(&mut self, tag: u16) -> u64 {
        self.get_tag(Tag::Unknown(tag)).unwrap().into_u64().unwrap()
    }
    fn assert_tag_u64_vec(&mut self, tag: u16) -> Vec<u64> {
        self.get_tag(Tag::Unknown(tag))
            .unwrap()
            .into_u64_vec()
            .unwrap()
    }
    fn assert_tag_i64(&mut self, tag: u16) -> i64 {
        self.get_tag(Tag::Unknown(tag)).unwrap().into_i64().unwrap()
    }
    fn assert_tag_i64_vec(&mut self, tag: u16) -> Vec<i64> {
        self.get_tag(Tag::Unknown(tag))
            .unwrap()
            .into_i64_vec()
            .unwrap()
    }
}

#[test]
fn test_multiple_byte() {
    let mut data = Cursor::new(Vec::new());

    {
        let mut tiff = TiffEncoder::new(&mut data).unwrap();
        let mut image_encoder = tiff.new_image::<colortype::Gray8>(1, 1).unwrap();
        image_encoder.write_strip(&[1]).unwrap();
        let encoder = image_encoder.encoder();

        encoder.write_tag(Tag::Unknown(65000), &[1_u8][..]).unwrap();
        encoder
            .write_tag(Tag::Unknown(65001), &[1_u8, 2][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65002), &[1_u8, 2, 3][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65003), &[1_u8, 2, 3, 4][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65004), &[1_u8, 2, 3, 4, 5][..])
            .unwrap();
    }

    data.set_position(0);
    {
        let mut decoder = Decoder::new(&mut data).unwrap();

        assert_eq!(decoder.assert_tag_u32_vec(65000), [1]);
        assert_eq!(decoder.assert_tag_u32_vec(65001), [1, 2]);
        assert_eq!(decoder.assert_tag_u32_vec(65002), [1, 2, 3]);
        assert_eq!(decoder.assert_tag_u32_vec(65003), [1, 2, 3, 4]);
        assert_eq!(decoder.assert_tag_u32_vec(65004), [1, 2, 3, 4, 5]);
    }
}

#[test]
/// Test writing signed tags from TIFF 6.0
fn test_signed() {
    let mut data = Cursor::new(Vec::new());
    fn make_srational(i: i32) -> SRational {
        SRational { n: i, d: 100 }
    }

    {
        let mut tiff = TiffEncoder::new(&mut data).unwrap();
        let mut image_encoder = tiff.new_image::<colortype::Gray8>(1, 1).unwrap();
        image_encoder.write_strip(&[1]).unwrap();
        let encoder = image_encoder.encoder();

        //Use the "reusable" tags section as per the TIFF6 spec
        encoder.write_tag(Tag::Unknown(65000), -1_i8).unwrap();
        encoder
            .write_tag(Tag::Unknown(65001), &[-1_i8][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65002), &[-1_i8, 2][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65003), &[-1_i8, 2, -3][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65004), &[-1_i8, 2, -3, 4][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65005), &[-1_i8, 2, -3, 4, -5][..])
            .unwrap();

        encoder.write_tag(Tag::Unknown(65010), -1_i16).unwrap();
        encoder.write_tag(Tag::Unknown(65011), -1_i16).unwrap();
        encoder
            .write_tag(Tag::Unknown(65012), &[-1_i16, 2][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65013), &[-1_i16, 2, -3][..])
            .unwrap();

        encoder.write_tag(Tag::Unknown(65020), -1_i32).unwrap();
        encoder
            .write_tag(Tag::Unknown(65021), &[-1_i32][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65022), &[-1_i32, 2][..])
            .unwrap();

        encoder.write_tag(Tag::Unknown(65030), -1_i64).unwrap();
        encoder
            .write_tag(Tag::Unknown(65031), &[-1_i64][..])
            .unwrap();
        encoder
            .write_tag(Tag::Unknown(65032), &[-1_i64, 2][..])
            .unwrap();

        encoder
            .write_tag(Tag::Unknown(65040), make_srational(-1))
            .unwrap();
        encoder
            .write_tag(
                Tag::Unknown(65041),
                &[make_srational(-1), make_srational(2)][..],
            )
            .unwrap();
    }

    //Rewind the cursor for reading
    data.set_position(0);
    {
        let mut decoder = Decoder::new(&mut data).unwrap();

        assert_eq!(decoder.assert_tag_i32(65000), -1);
        assert_eq!(decoder.assert_tag_i32_vec(65001), [-1]);
        assert_eq!(decoder.assert_tag_i32_vec(65002), [-1, 2]);
        assert_eq!(decoder.assert_tag_i32_vec(65003), [-1, 2, -3]);
        assert_eq!(decoder.assert_tag_i32_vec(65004), [-1, 2, -3, 4]);
        assert_eq!(decoder.assert_tag_i32_vec(65005), [-1, 2, -3, 4, -5],);

        assert_eq!(decoder.assert_tag_i32(65010), -1);
        assert_eq!(decoder.assert_tag_i32_vec(65011), [-1]);
        assert_eq!(decoder.assert_tag_i32_vec(65012), [-1, 2]);
        assert_eq!(decoder.assert_tag_i32_vec(65013), [-1, 2, -3]);

        assert_eq!(decoder.assert_tag_i32(65020), -1);
        assert_eq!(decoder.assert_tag_i32_vec(65021), [-1]);
        assert_eq!(decoder.assert_tag_i32_vec(65022), [-1, 2]);

        assert_eq!(decoder.assert_tag_i64(65030), -1);
        assert_eq!(decoder.assert_tag_i64_vec(65031), [-1]);
        assert_eq!(decoder.assert_tag_i64_vec(65032), [-1, 2]);

        assert_eq!(decoder.assert_tag_i32_vec(65040), [-1, 100]);
        assert_eq!(decoder.assert_tag_i32_vec(65041), [-1_i32, 100, 2, 100]);
    }
}

#[test]
/// check multipage image handling
fn test_multipage_image() {
    let mut img_file = Cursor::new(Vec::new());

    {
        // first create a multipage image with 2 images
        let mut img_encoder = TiffEncoder::new(&mut img_file).unwrap();

        // write first grayscale image (2x2 16-bit)
        let img1: Vec<u16> = [1, 2, 3, 4].to_vec();
        img_encoder
            .write_image::<colortype::Gray16>(2, 2, &img1[..])
            .unwrap();
        // write second grayscale image (3x3 8-bit)
        let img2: Vec<u8> = [9, 8, 7, 6, 5, 4, 3, 2, 1].to_vec();
        img_encoder
            .write_image::<colortype::Gray8>(3, 3, &img2[..])
            .unwrap();
    }

    // seek to the beginning of the file, so that it can be decoded
    img_file.seek(SeekFrom::Start(0)).unwrap();

    {
        let mut img_decoder = Decoder::new(&mut img_file).unwrap();

        // check the dimensions of the image in the first page
        assert_eq!(img_decoder.dimensions().unwrap(), (2, 2));
        img_decoder.next_image().unwrap();
        // check the dimensions of the image in the second page
        assert_eq!(img_decoder.dimensions().unwrap(), (3, 3));
    }
}

#[test]
/// verify rows per strip setting
fn test_rows_per_strip() {
    let mut file = Cursor::new(Vec::new());
    {
        let mut img_encoder = TiffEncoder::new(&mut file).unwrap();

        let mut image = img_encoder.new_image::<colortype::Gray8>(100, 100).unwrap();
        assert_eq!(image.next_strip_sample_count(), 100 * 100);
        image.rows_per_strip(2).unwrap();
        assert_eq!(image.next_strip_sample_count(), 2 * 100);

        let img2: Vec<u8> = vec![0; 2 * 100];
        image.write_strip(&img2[..]).unwrap();
        assert!(image.rows_per_strip(5).is_err());
        for i in 1..50 {
            let img2: Vec<u8> = vec![i; 2 * 100];
            image.write_strip(&img2[..]).unwrap();
        }
        assert!(image.write_strip(&img2[..]).is_err());
        image.finish().unwrap();
    }

    file.seek(SeekFrom::Start(0)).unwrap();
    {
        let mut decoder = Decoder::new(&mut file).unwrap();
        assert_eq!(decoder.get_tag_u64(Tag::RowsPerStrip).unwrap(), 2);
        assert_eq!(decoder.strip_count().unwrap(), 50);

        for i in 0..50 {
            let img2 = [i; 2 * 100];
            match decoder.read_chunk(i as u32).unwrap() {
                DecodingResult::U8(data) => assert_eq!(&img2[..], &data[..]),
                other => panic!("Incorrect strip type {other:?}"),
            }
        }
    }
}

#[test]
fn test_auxiliary_directory() {
    const ARTIST: &str = "Auxiliary-tiff";

    let mut data = Cursor::new(Vec::new());
    let mut tiff = TiffEncoder::new(&mut data).unwrap();

    {
        let mut image_encoder = tiff.new_image::<colortype::Gray8>(1, 1).unwrap();
        image_encoder.write_strip(&[1]).unwrap();
    }

    let exif = {
        let mut extra = tiff.extra_directory().unwrap();
        extra.write_tag(Tag::Artist, ARTIST).unwrap();
        extra.finish_with_offsets().unwrap()
    };

    {
        let mut image_encoder = tiff.new_image::<colortype::Gray8>(2, 1).unwrap();
        image_encoder.write_strip(&[1, 2]).unwrap();
        let encoder = image_encoder.encoder();
        encoder.write_tag(Tag::ExifDirectory, exif.offset).unwrap();
    }

    data.set_position(0);
    let mut decoder = Decoder::new(&mut data).unwrap();

    {
        assert_eq!(decoder.dimensions().unwrap(), (1, 1));
        let _ = decoder.read_image().unwrap();
    }

    {
        decoder.next_image().unwrap();
        assert_eq!(decoder.dimensions().unwrap(), (2, 1));
        let _ = decoder.read_image().unwrap();

        let exif_location = decoder
            .get_tag(Tag::ExifDirectory)
            .expect("second directory missing Exif")
            .into_ifd_pointer()
            .expect("exif directory is not an IFD pointer");

        let exif = decoder.read_directory(exif_location).unwrap();

        let artist = decoder
            .read_directory_tags(&exif)
            .get_tag_ascii_string(Tag::Artist)
            .expect("EXIF directory missing artist tag");
        assert_eq!(artist, ARTIST);
    }
}
