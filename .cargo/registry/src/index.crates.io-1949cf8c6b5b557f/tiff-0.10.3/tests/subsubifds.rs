use tiff::decoder::Decoder;

// This file has the following IFD structure:
//
// ```
// IFD -> IFD -> IFD
//        |-> IFD
//        |-> IFD -> IFD
//        |          \-> IFD -> IFD
//        \-> IFD
// ```
//
// where the main chain of IFD has 3 images, the second of which has several sub-IFDs and some have
// more nested IFDs.
const TEST_IMAGE_SUBIFD: &str = "./tests/subsubifds.tif";
use std::fs::File;

#[test]
fn decode_seek_chain() {
    let file = File::open(TEST_IMAGE_SUBIFD).expect("Cannot open test image");
    let mut decoder = Decoder::new(file).expect("Invalid format to create decoder");

    // Remember the offsets in the first iteration.
    let offset0 = decoder.ifd_pointer().expect("First IFD pointer not found");
    decoder.next_image().unwrap();
    let offset1 = decoder.ifd_pointer().expect("Second IFD pointer not found");
    decoder.next_image().unwrap();
    let offset2 = decoder.ifd_pointer().expect("Third IFD pointer not found");
    assert!(!decoder.more_images());

    // Ensure we can seek, to do this again.
    decoder
        .seek_to_image(0)
        .expect("Failed to seek to first image");
    assert_eq!(Some(offset0), decoder.ifd_pointer());
    decoder.next_image().unwrap();
    assert_eq!(Some(offset1), decoder.ifd_pointer());
    decoder.next_image().unwrap();
    assert_eq!(Some(offset2), decoder.ifd_pointer());
    assert!(!decoder.more_images());
}
