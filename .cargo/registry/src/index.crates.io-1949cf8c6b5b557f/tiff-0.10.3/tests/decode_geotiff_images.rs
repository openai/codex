extern crate tiff;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;
use tiff::ColorType;

use std::fs::File;
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images";

#[test]
fn test_geo_tiff() {
    let filenames = ["geo-5b.tif"];
    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        decoder = decoder.with_limits(tiff::decoder::Limits::unlimited());

        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (10, 10)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::Multiband {
                bit_depth: 16,
                num_samples: 5
            }
        );
        assert_eq!(
            decoder
                .get_tag_u64(Tag::StripOffsets)
                .expect("Cannot get StripOffsets"),
            418
        );
        assert_eq!(
            decoder
                .get_tag_u64(Tag::RowsPerStrip)
                .expect("Cannot get RowsPerStrip"),
            10
        );
        assert_eq!(
            decoder
                .get_tag_u64(Tag::StripByteCounts)
                .expect("Cannot get StripByteCounts"),
            1000
        );
        assert_eq!(
            decoder
                .get_tag(Tag::ModelPixelScaleTag)
                .expect("Cannot get pixel scale")
                .into_f64_vec()
                .expect("Cannot get pixel scale"),
            vec![60.0, 60.0, 0.0]
        );
        let DecodingResult::I16(data) = decoder.read_image().unwrap() else {
            panic!("Cannot read band data")
        };
        assert_eq!(data.len(), 500);
    }
}
