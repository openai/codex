extern crate tiff;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::ColorType;

use std::fs::File;
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/";

/// Test a basic all white image
#[test]
fn test_white_ieee_fp16() {
    let filenames = ["white-fp16.tiff"];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (256, 256)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            for p in img {
                assert!(p == half::f16::from_f32_const(1.0));
            }
        } else {
            panic!("Wrong data type");
        }
    }
}

/// Test a single black pixel, to make sure scaling is ok
#[test]
fn test_one_black_pixel_ieee_fp16() {
    let filenames = ["single-black-fp16.tiff"];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (256, 256)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            for (i, p) in img.iter().enumerate() {
                if i == 0 {
                    assert!(p < &half::f16::from_f32_const(0.001));
                } else {
                    assert!(p == &half::f16::from_f32_const(1.0));
                }
            }
        } else {
            panic!("Wrong data type");
        }
    }
}

/// Test white with horizontal differencing predictor
#[test]
fn test_pattern_horizontal_differencing_ieee_fp16() {
    let filenames = ["white-fp16-pred2.tiff"];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (256, 256)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            // 0, 2, 5, 8, 12, 16, 255 are black
            let black = [0, 2, 5, 8, 12, 16, 255];
            for (i, p) in img.iter().enumerate() {
                if black.contains(&i) {
                    assert!(p < &half::f16::from_f32_const(0.001));
                } else {
                    assert!(p == &half::f16::from_f32_const(1.0));
                }
            }
        } else {
            panic!("Wrong data type");
        }
    }
}

/// Test white with floating point predictor
#[test]
fn test_pattern_predictor_ieee_fp16() {
    let filenames = ["white-fp16-pred3.tiff"];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (256, 256)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            // 0, 2, 5, 8, 12, 16, 255 are black
            let black = [0, 2, 5, 8, 12, 16, 255];
            for (i, p) in img.iter().enumerate() {
                if black.contains(&i) {
                    assert!(p < &half::f16::from_f32_const(0.001));
                } else {
                    assert!(p == &half::f16::from_f32_const(1.0));
                }
            }
        } else {
            panic!("Wrong data type");
        }
    }
}

/// Test several random images
/// we'rell compare against a pnm file, that scales from 0 (for 0.0) to 65767 (for 1.0)
#[test]
fn test_predictor_ieee_fp16() {
    // first parse pnm, skip the first 4 \n
    let pnm_path = PathBuf::from(TEST_IMAGE_DIR).join("random-fp16.pgm");
    let pnm_bytes = std::fs::read(pnm_path).expect("Failed to read expected PNM file");

    // PGM looks like this:
    // ---
    // P5
    // #Created with GIMP
    // 16 16
    // 65535
    // ... <big-endian bytes>
    // ---
    // get index of 4th \n
    let byte_start = pnm_bytes
        .iter()
        .enumerate()
        .filter(|(_, &v)| v == b'\n')
        .map(|(i, _)| i)
        .nth(3)
        .expect("Must be 4 \\n's");

    let pnm_values: Vec<f32> = pnm_bytes[(byte_start + 1)..]
        .chunks(2)
        .map(|slice| {
            let bts = [slice[0], slice[1]];
            (u16::from_be_bytes(bts) as f32) / (u16::MAX as f32)
        })
        .collect();
    assert!(pnm_values.len() == 256);

    let filenames = [
        "random-fp16-pred2.tiff",
        "random-fp16-pred3.tiff",
        "random-fp16.tiff",
    ];

    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (16, 16)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Gray(16)
        );
        if let DecodingResult::F16(img) = decoder.read_image().unwrap() {
            for (exp, found) in std::iter::zip(pnm_values.iter(), img.iter()) {
                assert!((exp - found.to_f32()).abs() < 0.0001);
            }
        } else {
            panic!("Wrong data type");
        }
    }
}
