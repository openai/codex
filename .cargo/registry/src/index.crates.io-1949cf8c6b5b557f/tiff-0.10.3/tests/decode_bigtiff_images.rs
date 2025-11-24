extern crate tiff;

use tiff::decoder::Decoder;
use tiff::tags::Tag;
use tiff::ColorType;

use std::fs::File;
use std::path::PathBuf;

const TEST_IMAGE_DIR: &str = "./tests/images/bigtiff";

#[test]
fn test_big_tiff() {
    let filenames = ["BigTIFF.tif", "BigTIFFMotorola.tif", "BigTIFFLong.tif"];
    for filename in filenames.iter() {
        let path = PathBuf::from(TEST_IMAGE_DIR).join(filename);
        let img_file = File::open(path).expect("Cannot find test image!");
        let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");
        assert_eq!(
            decoder.dimensions().expect("Cannot get dimensions"),
            (64, 64)
        );
        assert_eq!(
            decoder.colortype().expect("Cannot get colortype"),
            ColorType::RGB(8)
        );
        assert_eq!(
            decoder
                .get_tag_u64(Tag::StripOffsets)
                .expect("Cannot get StripOffsets"),
            16
        );
        assert_eq!(
            decoder
                .get_tag_u64(Tag::RowsPerStrip)
                .expect("Cannot get RowsPerStrip"),
            64
        );
        assert_eq!(
            decoder
                .get_tag_u64(Tag::StripByteCounts)
                .expect("Cannot get StripByteCounts"),
            12288
        )
    }
}
