extern crate tiff;

use tiff::decoder::{ifd, Decoder, DecodingResult};
use tiff::ColorType;

use std::fs::File;
use std::io::{Read, Seek};
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/";

macro_rules! test_image_sum {
    ($name:ident, $buffer:ident, $sum_ty:ty) => {
        fn $name(file: &str, expected_type: ColorType, expected_sum: $sum_ty) {
            let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
            let img_file = File::open(path).expect("Cannot find test image!");
            let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
            assert_eq!(decoder.colortype().unwrap(), expected_type);
            let img_res = decoder.read_image().unwrap();

            match img_res {
                DecodingResult::$buffer(res) => {
                    let sum: $sum_ty = res.into_iter().map(<$sum_ty>::from).sum();
                    assert_eq!(sum, expected_sum);
                }
                _ => panic!("Wrong bit depth"),
            }
        }
    };
}

test_image_sum!(test_image_sum_u8, U8, u64);
test_image_sum!(test_image_sum_i8, I8, i64);
test_image_sum!(test_image_sum_u16, U16, u64);
test_image_sum!(test_image_sum_i16, I16, i64);
test_image_sum!(test_image_sum_u32, U32, u64);
test_image_sum!(test_image_sum_u64, U64, u64);
test_image_sum!(test_image_sum_f32, F32, f32);
test_image_sum!(test_image_sum_f64, F64, f64);

/// Tests that a decoder can be constructed for an image and the color type
/// read from the IFD and is of the appropriate type, but the type is
/// unsupported.
fn test_image_color_type_unsupported(file: &str, expected_type: ColorType) {
    let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
    let img_file = File::open(path).expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), expected_type);
    assert!(matches!(
        decoder.read_image(),
        Err(tiff::TiffError::UnsupportedError(
            tiff::TiffUnsupportedError::UnsupportedColorType(_),
        ))
    ));
}

#[test]
fn test_cmyk_u8() {
    test_image_sum_u8("cmyk-3c-8b.tiff", ColorType::CMYK(8), 8522658);
}

#[test]
fn test_cmyk_u16() {
    test_image_sum_u16("cmyk-3c-16b.tiff", ColorType::CMYK(16), 2181426827);
}

#[test]
fn test_cmyk_f32() {
    test_image_sum_f32("cmyk-3c-32b-float.tiff", ColorType::CMYK(32), 496.0405);
}

#[test]
fn test_gray_u8() {
    test_image_sum_u8("minisblack-1c-8b.tiff", ColorType::Gray(8), 2840893);
}

#[test]
fn test_gray_u12() {
    test_image_color_type_unsupported("12bit.cropped.tiff", ColorType::Gray(12));
}

#[test]
fn test_gray_u16() {
    test_image_sum_u16("minisblack-1c-16b.tiff", ColorType::Gray(16), 733126239);
}

#[test]
fn test_gray_u32() {
    test_image_sum_u32("gradient-1c-32b.tiff", ColorType::Gray(32), 549892913787);
}

#[test]
fn test_gray_u64() {
    test_image_sum_u64("gradient-1c-64b.tiff", ColorType::Gray(64), 549892913787);
}

#[test]
fn test_gray_f32() {
    test_image_sum_f32("gradient-1c-32b-float.tiff", ColorType::Gray(32), 128.03194);
}

#[test]
fn test_gray_f64() {
    test_image_sum_f64(
        "gradient-1c-64b-float.tiff",
        ColorType::Gray(64),
        128.0319210877642,
    );
}

#[test]
fn test_rgb_u8() {
    test_image_sum_u8("rgb-3c-8b.tiff", ColorType::RGB(8), 7842108);
}

#[test]
fn test_rgb_u12() {
    test_image_color_type_unsupported("12bit.cropped.rgb.tiff", ColorType::RGB(12));
}

#[test]
fn test_rgb_u16() {
    test_image_sum_u16("rgb-3c-16b.tiff", ColorType::RGB(16), 2024349944);
}

#[test]
fn test_rgb_u32() {
    test_image_sum_u32("gradient-3c-32b.tiff", ColorType::RGB(32), 2030834111716);
}

#[test]
fn test_rgb_u64() {
    test_image_sum_u64("gradient-3c-64b.tiff", ColorType::RGB(64), 2030834111716);
}

#[test]
fn test_rgb_f32() {
    test_image_sum_f32("gradient-3c-32b-float.tiff", ColorType::RGB(32), 472.8405);
}

#[test]
fn test_int8() {
    test_image_sum_i8("int8.tif", ColorType::Gray(8), 3111)
}

#[test]
fn test_int8_rgb() {
    test_image_sum_i8("int8_rgb.tif", ColorType::RGB(8), -10344)
}

#[test]
fn test_int16() {
    test_image_sum_i16("int16.tif", ColorType::Gray(16), 354396);
}

#[test]
fn test_int16_rgb() {
    test_image_sum_i16("int16_rgb.tif", ColorType::RGB(16), 1063188);
}

#[test]
fn test_string_tags() {
    // these files have null-terminated strings for their Software tag. One has extra bytes after
    // the null byte, so we check both to ensure that we're truncating properly
    let filenames = ["minisblack-1c-16b.tiff", "rgb-3c-16b.tiff"];
    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        let software = decoder.get_tag(tiff::tags::Tag::Software).unwrap();
        match software {
            ifd::Value::Ascii(s) => assert_eq!(
                &s,
                "GraphicsMagick 1.2 unreleased Q16 http://www.GraphicsMagick.org/"
            ),
            other => unreachable!("Expected Ascii tag from file, got {other:?}"),
        };
    }
}

#[test]
fn test_decode_data() {
    let mut image_data = Vec::new();
    for x in 0..100 {
        for y in 0..100u8 {
            let val = x + y;
            image_data.push(val);
            image_data.push(val);
            image_data.push(val);
        }
    }
    let file = File::open("./tests/decodedata-rgb-3c-8b.tiff").unwrap();
    let mut decoder = Decoder::new(file).unwrap();
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));
    assert_eq!(decoder.dimensions().unwrap(), (100, 100));
    if let DecodingResult::U8(img_res) = decoder.read_image().unwrap() {
        assert_eq!(image_data, img_res);
    } else {
        panic!("Wrong data type");
    }
}

#[test]
fn issue_69() {
    test_image_sum_u16("issue_69_lzw.tiff", ColorType::Gray(16), 1015486);
    test_image_sum_u16("issue_69_packbits.tiff", ColorType::Gray(16), 1015486);
}

// TODO: GrayA support
//#[test]
//fn test_gray_alpha_u8()
//{
//let img_file = File::open("./tests/images/minisblack-2c-8b-alpha.tiff").expect("Cannot find test image!");
//let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
//assert_eq!(decoder.colortype().unwrap(), ColorType::GrayA(8));
//let img_res = decoder.read_image();
//assert!(img_res.is_ok());
//}

#[test]
fn test_tiled_gray_i1() {
    test_image_sum_u8("tiled-gray-i1.tif", ColorType::Gray(1), 30531);
}

#[test]
fn test_tiled_rgb_u8() {
    test_image_sum_u8("tiled-rgb-u8.tif", ColorType::RGB(8), 39528948);
}

#[test]
fn test_tiled_rect_rgb_u8() {
    test_image_sum_u8("tiled-rect-rgb-u8.tif", ColorType::RGB(8), 62081032);
}

#[test]
fn test_inner_access() {
    let path = PathBuf::from(TEST_IMAGE_DIR).join("tiled-rect-rgb-u8.tif");
    let img_file = File::open(path).expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), ColorType::RGB(8));

    let c = decoder
        .get_tag(tiff::tags::Tag::Compression)
        .unwrap()
        .into_u16()
        .unwrap();
    assert_eq!(c, tiff::tags::CompressionMethod::None.to_u16());

    // Because the image is uncompressed, reading the first tile directly with the inner reader
    // should yield the same result as reading it with the decoder's read_chunk method.
    let first_offset = decoder
        .get_tag_u32_vec(tiff::tags::Tag::TileOffsets)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let first_byte_count = decoder
        .get_tag_u32_vec(tiff::tags::Tag::TileByteCounts)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    decoder
        .inner()
        .seek(std::io::SeekFrom::Start(first_offset as u64))
        .expect("Cannot seek to tile offset");
    let mut buf = vec![0; first_byte_count as usize];
    decoder
        .inner()
        .read_exact(&mut buf)
        .expect("Cannot read tile data");
    let raw_sum: u64 = buf.into_iter().map(<u64>::from).sum();

    match decoder.read_chunk(0).unwrap() {
        DecodingResult::U8(chunk) => {
            let sum: u64 = chunk.into_iter().map(<u64>::from).sum();
            assert_eq!(sum, raw_sum);
        }
        _ => panic!("Wrong bit depth"),
    }
}

/* #[test]
fn test_tiled_jpeg_rgb_u8() {
    test_image_sum_u8("tiled-jpeg-rgb-u8.tif", ColorType::RGB(8), 93031606);
} */

#[test]
fn test_tiled_oversize_gray_i8() {
    test_image_sum_i8("tiled-oversize-gray-i8.tif", ColorType::Gray(8), 1214996);
}

#[test]
fn test_tiled_cmyk_i8() {
    test_image_sum_i8("tiled-cmyk-i8.tif", ColorType::CMYK(8), 1759101);
}

#[test]
fn test_tiled_incremental() {
    let file = "tiled-rgb-u8.tif";
    let expected_type = ColorType::RGB(8);
    let sums = [
        188760, 195639, 108148, 81986, 665088, 366140, 705317, 423366, 172033, 324455, 244102,
        81853, 181258, 247971, 129486, 55600, 565625, 422102, 730888, 379271, 232142, 292549,
        244045, 86866, 188141, 115036, 150785, 84389, 353170, 459325, 719619, 329594, 278663,
        220474, 243048, 113563, 189152, 109684, 179391, 122188, 279651, 622093, 724682, 302459,
        268428, 204499, 224255, 124674, 170668, 121868, 192768, 183367, 378029, 585651, 657712,
        296790, 241444, 197083, 198429, 134869, 182318, 86034, 203655, 182338, 297255, 601284,
        633813, 242531, 228578, 206441, 193552, 125412, 181527, 165439, 202531, 159538, 268388,
        565790, 611382, 272967, 236497, 215154, 158881, 90806, 106114, 182342, 191824, 186138,
        215174, 393193, 701228, 198866, 227944, 193830, 166330, 49008, 55719, 122820, 197316,
        161969, 203152, 170986, 624427, 188605, 186187, 111064, 115192, 39538, 48626, 163929,
        144682, 135796, 194141, 154198, 584125, 180255, 153524, 121433, 132641, 35743, 47798,
        152343, 162874, 167664, 160175, 133038, 659882, 138339, 166470, 124173, 118929, 51317,
        45267, 155776, 161331, 161006, 130052, 137618, 337291, 106481, 161999, 127343, 87724,
        59540, 63907, 155677, 140668, 141523, 108061, 168657, 186482, 98599, 147614, 139963, 90444,
        56602, 92547, 125644, 134212, 126569, 144153, 179800, 174516, 133969, 129399, 117681,
        83305, 55075, 110737, 115108, 128572, 128911, 130922, 179986, 143288, 145884, 155856,
        96683, 94057, 56238, 79649, 71651, 70182, 75010, 77009, 98855, 78979, 74341, 83482, 53403,
        59842, 30305,
    ];

    let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
    let img_file = File::open(path).expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), expected_type);

    let tiles = decoder.tile_count().unwrap();
    assert_eq!(tiles as usize, sums.len());

    for tile in 0..tiles {
        match decoder.read_chunk(tile).unwrap() {
            DecodingResult::U8(res) => {
                let sum: u64 = res.into_iter().map(<u64>::from).sum();
                assert_eq!(sum, sums[tile as usize]);
            }
            _ => panic!("Wrong bit depth"),
        }
    }
}

#[test]
fn test_planar_rgb_u8() {
    test_image_sum_u8("planar-rgb-u8.tif", ColorType::RGB(8), 15417630);
}

#[test]
fn test_read_planar_bands() {
    // gdal_translate tiled-rgb-u8.tif planar-rgb-u8.tif -co INTERLEAVE=BAND -co COMPRESS=LZW -co PROFILE=BASELINE
    let file = "planar-rgb-u8.tif";
    let expected_type = ColorType::RGB(8);

    let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
    let img_file = File::open(path).expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
    assert_eq!(decoder.colortype().unwrap(), expected_type);

    let chunks = decoder.strip_count().unwrap();
    assert_eq!(chunks as usize, 72);

    // convert -quiet planar-rgb-u8.tif[0] -crop 1x1+0+0 txt:
    // 0,0: (73,51,30)  #49331E  srgb(73,51,30)

    // 1st band (red)
    match decoder.read_chunk(0).unwrap() {
        DecodingResult::U8(chunk) => {
            assert_eq!(chunk[0], 73);
        }
        _ => panic!("Wrong bit depth"),
    }
    // 2nd band (green)
    match decoder.read_chunk(chunks / 3).unwrap() {
        DecodingResult::U8(chunk) => {
            assert_eq!(chunk[0], 51);
        }
        _ => panic!("Wrong bit depth"),
    }
    // 3rd band (blue)
    match decoder.read_chunk(chunks / 3 * 2).unwrap() {
        DecodingResult::U8(chunk) => {
            assert_eq!(chunk[0], 30);
        }
        _ => panic!("Wrong bit depth"),
    }
}

#[test]
fn test_div_zero() {
    use tiff::{TiffError, TiffFormatError};

    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 40, 1, 0, 0,
        0, 158, 0, 0, 251, 3, 1, 3, 0, 1, 0, 0, 0, 1, 0, 0, 39, 6, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0,
        17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 3, 0, 1, 0, 0, 0, 158, 0, 0, 251, 67, 1, 3, 0,
        1, 0, 0, 0, 40, 0, 0, 0, 66, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 48, 178, 178, 178, 178,
        178, 178, 178,
    ];

    let err = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();

    match err {
        TiffError::FormatError(TiffFormatError::StripTileTagConflict) => {}
        unexpected => panic!("Unexpected error {unexpected}"),
    }
}

#[test]
fn test_too_many_value_bytes() {
    let image = [
        73, 73, 43, 0, 8, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 8, 0, 0, 0,
        23, 0, 12, 0, 0, 65, 4, 0, 1, 6, 0, 0, 1, 16, 0, 1, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0,
        0, 0, 3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 59, 73, 84, 186, 202, 83, 240, 66, 1, 53, 22, 56, 47,
        0, 0, 0, 0, 0, 0, 1, 222, 4, 0, 58, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 4, 0, 0, 100, 0,
        0, 89, 89, 89, 89, 89, 89, 89, 89, 96, 1, 20, 89, 89, 89, 89, 18,
    ];

    let error = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();

    match error {
        tiff::TiffError::LimitsExceeded => {}
        unexpected => panic!("Unexpected error {unexpected}"),
    }
}

#[test]
fn fuzzer_testcase5() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 100, 0, 0, 0, 1, 1, 4, 0, 1, 0, 0,
        0, 158, 0, 0, 251, 3, 1, 3, 0, 1, 0, 0, 0, 1, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0,
        17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 0, 0, 0, 0, 0, 246, 16, 0, 0, 22, 1, 4, 0, 1,
        0, 0, 0, 40, 0, 251, 255, 23, 1, 4, 0, 1, 0, 0, 0, 48, 178, 178, 178, 178, 178, 178, 178,
        178, 178, 178,
    ];

    let _ = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();
}

#[test]
fn fuzzer_testcase1() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 99, 255, 255, 254, 1, 1, 4, 0, 1,
        0, 0, 0, 158, 0, 0, 251, 3, 1, 3, 255, 254, 255, 255, 0, 1, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0,
        0, 0, 0, 0, 0, 17, 1, 4, 0, 9, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 0, 2, 0, 0, 0, 63, 0, 0, 0,
        22, 1, 4, 0, 1, 0, 0, 0, 44, 0, 0, 0, 23, 1, 4, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 1, 0, 178,
        178,
    ];

    let _ = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();
}

#[test]
fn fuzzer_testcase6() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 100, 0, 0, 148, 1, 1, 4, 0, 1, 0,
        0, 0, 158, 0, 0, 251, 3, 1, 3, 255, 254, 255, 255, 0, 1, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0,
        0, 0, 0, 0, 17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 0, 2, 0, 0, 0, 63, 0, 0, 0, 22,
        1, 4, 0, 1, 0, 0, 0, 44, 0, 248, 255, 23, 1, 4, 0, 1, 0, 0, 0, 178, 178, 178, 0, 1, 178,
        178, 178,
    ];

    let _ = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();
}

#[test]
fn oom() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 40, 1, 0, 0,
        0, 158, 0, 0, 251, 3, 1, 3, 0, 1, 0, 0, 0, 7, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 2, 0, 0, 0,
        17, 1, 4, 0, 1, 0, 0, 0, 3, 77, 0, 0, 1, 1, 3, 0, 1, 0, 0, 0, 3, 128, 0, 0, 22, 1, 4, 0, 1,
        0, 0, 0, 40, 0, 0, 0, 23, 1, 4, 0, 1, 0, 0, 0, 178, 48, 178, 178, 178, 178, 162, 178,
    ];

    let _ = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();
}

#[test]
fn fuzzer_testcase4() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 40, 1, 0, 0,
        0, 158, 0, 0, 251, 3, 1, 3, 0, 1, 0, 0, 0, 5, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0,
        17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 3, 0, 1, 0, 0, 0, 3, 128, 0, 0, 22, 1, 4, 0, 1,
        0, 0, 0, 40, 0, 0, 0, 23, 1, 4, 0, 1, 0, 0, 0, 48, 178, 178, 178, 0, 1, 0, 13, 13,
    ];

    let _ = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();
}

#[test]
fn fuzzer_testcase2() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 15, 0, 0, 254, 44, 1, 0, 0, 0, 0, 0, 32, 0, 0, 0, 1, 4, 0, 1, 0,
        0, 0, 0, 1, 0, 0, 91, 1, 1, 0, 0, 0, 0, 0, 242, 4, 0, 0, 0, 22, 0, 56, 77, 0, 77, 1, 0, 0,
        73, 42, 0, 1, 4, 0, 1, 0, 0, 0, 4, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 158, 0, 0, 251, 3, 1,
        3, 0, 1, 0, 0, 0, 7, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 2, 0, 0, 0, 17, 1, 4, 0, 1, 0, 0, 0,
        0, 0, 0, 0, 1, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 4, 61, 1, 18, 0, 1, 0, 0, 0, 202, 0, 0, 0, 17,
        1, 100, 0, 129, 0, 0, 0, 0, 0, 0, 0, 232, 254, 252, 255, 254, 255, 255, 255, 1, 29, 0, 0,
        22, 1, 3, 0, 1, 0, 0, 0, 16, 0, 0, 0, 23, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 123, 73, 254, 0,
        73,
    ];

    let _ = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();
}

#[test]
fn invalid_jpeg_tag_2() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 16, 0, 254, 0, 4, 0, 1, 0, 0, 0, 0, 0, 0, 242, 0, 1, 4, 0, 1, 0,
        0, 0, 0, 129, 16, 0, 1, 1, 4, 0, 1, 0, 0, 0, 214, 0, 0, 248, 253, 1, 3, 0, 1, 0, 0, 0, 64,
        0, 0, 0, 3, 1, 3, 0, 1, 0, 0, 0, 7, 0, 0, 0, 6, 1, 3, 0, 1, 0, 0, 0, 1, 0, 0, 64, 14, 1, 0,
        2, 0, 0, 148, 0, 206, 0, 0, 0, 17, 1, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
        1, 0, 0, 0, 22, 1, 4, 0, 17, 0, 0, 201, 1, 0, 0, 0, 23, 1, 2, 0, 20, 0, 0, 0, 194, 0, 0, 0,
        91, 1, 7, 0, 5, 0, 0, 0, 64, 0, 0, 0, 237, 254, 65, 255, 255, 255, 255, 255, 1, 0, 0, 0,
        22, 1, 4, 0, 1, 0, 0, 0, 42, 0, 0, 0, 23, 1, 255, 255, 255, 255, 255, 36, 36, 0, 0, 0, 0,
        0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 36, 73, 73, 0, 42, 36, 36, 36, 36, 0, 0, 8, 0,
    ];

    let _ = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();
}

#[test]
fn fuzzer_testcase3() {
    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 2, 0, 0, 0, 61, 1, 9, 0, 46, 22,
        128, 0, 0, 0, 0, 1, 6, 1, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0, 17, 1, 4, 0, 27, 0, 0, 0, 0, 0, 0,
        0, 1, 1, 3, 0, 1, 0, 0, 0, 17, 1, 0, 231, 22, 1, 1, 0, 1, 0, 0, 0, 130, 0, 0, 0, 23, 1, 4,
        0, 14, 0, 0, 0, 0, 0, 0, 0, 133, 133, 133, 77, 77, 77, 0, 0, 22, 128, 0, 255, 255, 255,
        255, 255,
    ];

    let _ = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();
}

#[test]
fn timeout() {
    use tiff::{TiffError, TiffFormatError};

    let image = [
        73, 73, 42, 0, 8, 0, 0, 0, 16, 0, 254, 0, 4, 0, 1, 68, 0, 0, 0, 2, 0, 32, 254, 252, 0, 109,
        0, 129, 0, 0, 0, 32, 0, 58, 0, 1, 4, 0, 1, 0, 6, 0, 0, 0, 8, 0, 0, 1, 73, 73, 42, 0, 8, 0,
        0, 0, 8, 0, 0, 1, 4, 0, 1, 0, 0, 0, 21, 0, 0, 0, 61, 1, 255, 128, 9, 0, 0, 8, 0, 1, 113, 2,
        3, 1, 3, 0, 1, 0, 0, 0, 5, 0, 65, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 112, 0, 0, 36, 0, 0,
        0, 112, 56, 200, 0, 5, 0, 0, 64, 0, 0, 1, 0, 4, 0, 0, 0, 2, 0, 6, 1, 3, 0, 1, 0, 0, 0, 0,
        0, 0, 4, 17, 1, 1, 0, 93, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0, 0, 3, 6, 0, 231, 22, 1,
        1, 0, 1, 0, 0, 0, 2, 64, 118, 36, 23, 1, 1, 0, 43, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 4, 0, 8,
        0, 0, 73, 73, 42, 0, 8, 0, 0, 0, 0, 0, 32,
    ];

    let error = tiff::decoder::Decoder::new(std::io::Cursor::new(&image)).unwrap_err();

    match error {
        TiffError::FormatError(TiffFormatError::CycleInOffsets) => {}
        e => panic!("Unexpected error {e:?}"),
    }
}

#[test]
fn test_no_rows_per_strip() {
    test_image_sum_u8("no_rows_per_strip.tiff", ColorType::RGB(8), 99448840);
}

#[test]
fn test_predictor_3_rgb_f32() {
    test_image_sum_f32("predictor-3-rgb-f32.tif", ColorType::RGB(32), 54004.33);
}

#[test]
fn test_predictor_3_gray_f32() {
    test_image_sum_f32("predictor-3-gray-f32.tif", ColorType::Gray(32), 20008.275);
}

#[test]
#[cfg(feature = "zstd")]
fn test_zstd_compression() {
    // gdal_translate -co COMPRESS=ZSTD -co ZSTD_LEVEL=20 int16.tif int16_zstd.tif
    test_image_sum_i16("int16_zstd.tif", ColorType::Gray(16), 354396);
}

fn test_image_bytes(
    file: &str,
    expected: &[(ColorType, u32)],
    // Represent all samples as little endian, ensuring check sums on all platforms.
    normalize_byte_order: fn(&mut [u8]),
) {
    let path = PathBuf::from(TEST_IMAGE_DIR).join(file);
    let img_file = File::open(path).expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");

    let mut buffer = vec![0u8; 0];
    for (idx, &(expected_type, expected_crc)) in expected.iter().enumerate() {
        if idx > 0 {
            decoder
                .next_image()
                .expect("mismatched number of directories");
        }

        assert_eq!(decoder.colortype().unwrap(), expected_type);
        let layout = decoder
            .image_buffer_layout()
            .expect("Cannot describe buffer layout");

        buffer.resize(layout.len, 0u8);
        decoder.read_image_bytes(&mut buffer).unwrap();
        normalize_byte_order(&mut buffer);

        // Well almost a crc..
        assert_eq!(crc32fast::hash(&buffer), expected_crc);
    }
}

fn byte_order_u16(bytes: &mut [u8]) {
    for chunk in bytes.chunks_exact_mut(2) {
        let n = u16::from_ne_bytes(chunk.try_into().unwrap());
        chunk.copy_from_slice(&n.to_le_bytes());
    }
}

fn byte_order_u32(bytes: &mut [u8]) {
    for chunk in bytes.chunks_exact_mut(4) {
        let n = u32::from_ne_bytes(chunk.try_into().unwrap());
        chunk.copy_from_slice(&n.to_le_bytes());
    }
}

fn byte_order_u64(bytes: &mut [u8]) {
    for chunk in bytes.chunks_exact_mut(8) {
        let n = u64::from_ne_bytes(chunk.try_into().unwrap());
        chunk.copy_from_slice(&n.to_le_bytes());
    }
}

#[test]
fn bytes_cmyk_3c_8b() {
    test_image_bytes(
        "cmyk-3c-8b.tiff",
        &[(ColorType::CMYK(8), 2935032230)],
        |_| (),
    );
}

#[test]
fn bytes_gray_16b() {
    test_image_bytes(
        "minisblack-1c-16b.tiff",
        &[(ColorType::Gray(16), 3524597615)],
        byte_order_u16,
    );
}

#[test]
fn bytes_gray_32b() {
    test_image_bytes(
        "gradient-1c-32b.tiff",
        &[(ColorType::Gray(32), 2257036530)],
        byte_order_u32,
    );
}

#[test]
fn bytes_gray_u64() {
    test_image_bytes(
        "gradient-1c-64b.tiff",
        &[(ColorType::Gray(64), 2156547113)],
        byte_order_u64,
    );
}

#[test]
fn bytes_gray_f32() {
    test_image_bytes(
        "gradient-1c-32b-float.tiff",
        &[(ColorType::Gray(32), 3232356730)],
        byte_order_u32,
    );
}

#[test]
#[cfg(feature = "fax")]
fn test_fax4() {
    test_image_sum_u8("fax4.tiff", ColorType::Gray(1), 7802706);
}

#[test]
#[cfg(feature = "fax")]
fn test_fax4_white_is_min() {
    test_image_sum_u8("imagemagick_group4.tiff", ColorType::Gray(1), 3742820);
}
