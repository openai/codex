extern crate tiff;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::encoder::{colortype, Predictor, TiffEncoder};
use tiff::ColorType;

use std::fs::File;
use std::io::{Cursor, Seek, SeekFrom};
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/";

macro_rules! test_predict {
    ($name:ident, $buffer:ident, $buffer_ty:ty) => {
        fn $name<C: colortype::ColorType<Inner = $buffer_ty>>(
            file: &str,
            expected_type: ColorType,
        ) {
            let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
            let file = File::open(path).expect("Cannot find test image!");
            let mut decoder = Decoder::new(file).expect("Cannot create decoder!");

            assert_eq!(decoder.colortype().unwrap(), expected_type);
            let image_data = match decoder.read_image().unwrap() {
                DecodingResult::$buffer(res) => res,
                _ => panic!("Wrong data type"),
            };

            let mut predicted = Vec::with_capacity(image_data.len());
            C::horizontal_predict(&image_data, &mut predicted);

            let sample_size = C::SAMPLE_FORMAT.len();

            (0..sample_size).for_each(|i| {
                assert_eq!(predicted[i], image_data[i]);
            });

            (sample_size..image_data.len()).for_each(|i| {
                predicted[i] = predicted[i].wrapping_add(predicted[i - sample_size]);
                assert_eq!(predicted[i], image_data[i]);
            });
        }
    };
}

test_predict!(test_u8_predict, U8, u8);
test_predict!(test_i8_predict, I8, i8);
test_predict!(test_u16_predict, U16, u16);
test_predict!(test_i16_predict, I16, i16);
test_predict!(test_u32_predict, U32, u32);
test_predict!(test_u64_predict, U64, u64);

#[test]
fn test_gray_u8_predict() {
    test_u8_predict::<colortype::Gray8>("minisblack-1c-8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_gray_i8_predict() {
    test_i8_predict::<colortype::GrayI8>("minisblack-1c-i8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_rgb_u8_predict() {
    test_u8_predict::<colortype::RGB8>("rgb-3c-8b.tiff", ColorType::RGB(8));
}

#[test]
fn test_cmyk_u8_predict() {
    test_u8_predict::<colortype::CMYK8>("cmyk-3c-8b.tiff", ColorType::CMYK(8));
}

#[test]
fn test_gray_u16_predict() {
    test_u16_predict::<colortype::Gray16>("minisblack-1c-16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_gray_i16_predict() {
    test_i16_predict::<colortype::GrayI16>("minisblack-1c-i16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_rgb_u16_predict() {
    test_u16_predict::<colortype::RGB16>("rgb-3c-16b.tiff", ColorType::RGB(16));
}

#[test]
fn test_cmyk_u16_predict() {
    test_u16_predict::<colortype::CMYK16>("cmyk-3c-16b.tiff", ColorType::CMYK(16));
}

#[test]
fn test_gray_u32_predict() {
    test_u32_predict::<colortype::Gray32>("gradient-1c-32b.tiff", ColorType::Gray(32));
}

#[test]
fn test_rgb_u32_predict() {
    test_u32_predict::<colortype::RGB32>("gradient-3c-32b.tiff", ColorType::RGB(32));
}

#[test]
fn test_gray_u64_predict() {
    test_u64_predict::<colortype::Gray64>("gradient-1c-64b.tiff", ColorType::Gray(64));
}

#[test]
fn test_rgb_u64_predict() {
    test_u64_predict::<colortype::RGB64>("gradient-3c-64b.tiff", ColorType::RGB(64));
}

#[test]
fn test_ycbcr_u8_predict() {
    test_u8_predict::<colortype::YCbCr8>("tiled-jpeg-ycbcr.tif", ColorType::YCbCr(8));
}

macro_rules! test_predict_roundtrip {
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
                let mut tiff = TiffEncoder::new(&mut file)
                    .unwrap()
                    .with_predictor(Predictor::Horizontal);

                let (width, height) = decoder.dimensions().unwrap();
                tiff.write_image::<C>(width, height, &image_data).unwrap();
            }
            file.seek(SeekFrom::Start(0)).unwrap();
            {
                let mut decoder = Decoder::new(&mut file).unwrap();
                if let DecodingResult::$buffer(img_res) =
                    decoder.read_image().expect("Decoding image failed")
                {
                    assert_eq!(image_data, img_res);
                } else {
                    panic!("Wrong data type");
                }
            }
        }
    };
}

test_predict_roundtrip!(test_u8_predict_roundtrip, U8, u8);
test_predict_roundtrip!(test_i8_predict_roundtrip, I8, i8);
test_predict_roundtrip!(test_u16_predict_roundtrip, U16, u16);
test_predict_roundtrip!(test_i16_predict_roundtrip, I16, i16);
test_predict_roundtrip!(test_u32_predict_roundtrip, U32, u32);
test_predict_roundtrip!(test_u64_predict_roundtrip, U64, u64);

#[test]
fn test_gray_u8_predict_roundtrip() {
    test_u8_predict_roundtrip::<colortype::Gray8>("minisblack-1c-8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_gray_i8_predict_roundtrip() {
    test_i8_predict_roundtrip::<colortype::GrayI8>("minisblack-1c-i8b.tiff", ColorType::Gray(8));
}

#[test]
fn test_rgb_u8_predict_roundtrip() {
    test_u8_predict_roundtrip::<colortype::RGB8>("rgb-3c-8b.tiff", ColorType::RGB(8));
}

#[test]
fn test_cmyk_u8_predict_roundtrip() {
    test_u8_predict_roundtrip::<colortype::CMYK8>("cmyk-3c-8b.tiff", ColorType::CMYK(8));
}

#[test]
fn test_gray_u16_predict_roundtrip() {
    test_u16_predict_roundtrip::<colortype::Gray16>("minisblack-1c-16b.tiff", ColorType::Gray(16));
}

#[test]
fn test_gray_i16_predict_roundtrip() {
    test_i16_predict_roundtrip::<colortype::GrayI16>(
        "minisblack-1c-i16b.tiff",
        ColorType::Gray(16),
    );
}

#[test]
fn test_rgb_u16_predict_roundtrip() {
    test_u16_predict_roundtrip::<colortype::RGB16>("rgb-3c-16b.tiff", ColorType::RGB(16));
}

#[test]
fn test_cmyk_u16_predict_roundtrip() {
    test_u16_predict_roundtrip::<colortype::CMYK16>("cmyk-3c-16b.tiff", ColorType::CMYK(16));
}

#[test]
fn test_gray_u32_predict_roundtrip() {
    test_u32_predict_roundtrip::<colortype::Gray32>("gradient-1c-32b.tiff", ColorType::Gray(32));
}

#[test]
fn test_rgb_u32_predict_roundtrip() {
    test_u32_predict_roundtrip::<colortype::RGB32>("gradient-3c-32b.tiff", ColorType::RGB(32));
}

#[test]
fn test_gray_u64_predict_roundtrip() {
    test_u64_predict_roundtrip::<colortype::Gray64>("gradient-1c-64b.tiff", ColorType::Gray(64));
}

#[test]
fn test_rgb_u64_predict_roundtrip() {
    test_u64_predict_roundtrip::<colortype::RGB64>("gradient-3c-64b.tiff", ColorType::RGB(64));
}

#[test]
fn test_ycbcr_u8_predict_roundtrip() {
    test_u8_predict_roundtrip::<colortype::YCbCr8>("tiled-jpeg-ycbcr.tif", ColorType::YCbCr(8));
}
