extern crate tiff;

use std::io::{Cursor, Seek, Write};
use tiff::{
    decoder::{Decoder, DecodingResult},
    encoder::{
        colortype::{self, ColorType},
        compression::*,
        Compression, TiffEncoder, TiffValue,
    },
};

trait TestImage<const NUM_CHANNELS: usize>: From<Vec<<Self::Color as ColorType>::Inner>> {
    const WIDTH: u32;
    const HEIGHT: u32;
    type Color: ColorType;

    fn reference_data(&self) -> &[<Self::Color as ColorType>::Inner];
    fn generate_pixel(x: u32, y: u32) -> [<Self::Color as ColorType>::Inner; NUM_CHANNELS];

    fn compress<W: Write + Seek>(&self, encoder: &mut TiffEncoder<W>)
    where
        [<Self::Color as ColorType>::Inner]: TiffValue,
    {
        let image = encoder
            .new_image::<Self::Color>(Self::WIDTH, Self::HEIGHT)
            .unwrap();
        image.write_data(self.reference_data()).unwrap();
    }

    fn generate() -> Self {
        assert_eq!(
            Self::Color::BITS_PER_SAMPLE.len(),
            NUM_CHANNELS,
            "Incompatible color type"
        );

        let mut data = Vec::with_capacity((Self::WIDTH * Self::HEIGHT) as usize * NUM_CHANNELS);
        for x in 0..Self::WIDTH {
            for y in 0..Self::HEIGHT {
                data.extend(IntoIterator::into_iter(Self::generate_pixel(x, y)));
            }
        }
        Self::from(data)
    }
}

struct TestImageColor(Vec<u16>);

impl From<Vec<u16>> for TestImageColor {
    fn from(value: Vec<u16>) -> Self {
        Self(value)
    }
}

impl TestImage<3> for TestImageColor {
    const WIDTH: u32 = 1;
    const HEIGHT: u32 = 7;
    type Color = colortype::RGB16;

    fn reference_data(&self) -> &[u16] {
        &self.0
    }

    fn generate_pixel(x: u32, y: u32) -> [<Self::Color as ColorType>::Inner; 3] {
        let val = (x + y) % <Self::Color as ColorType>::Inner::MAX as u32;
        [val as <Self::Color as ColorType>::Inner; 3]
    }
}

struct TestImageGrayscale(Vec<u8>);

impl From<Vec<u8>> for TestImageGrayscale {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl TestImage<1> for TestImageGrayscale {
    const WIDTH: u32 = 21;
    const HEIGHT: u32 = 10;
    type Color = colortype::Gray8;

    fn reference_data(&self) -> &[u8] {
        &self.0
    }

    fn generate_pixel(x: u32, y: u32) -> [<Self::Color as ColorType>::Inner; 1] {
        let val = (x + y) % <Self::Color as ColorType>::Inner::MAX as u32;
        [val as <Self::Color as ColorType>::Inner]
    }
}

fn encode_decode_with_compression(compression: Compression) {
    let mut data = Cursor::new(Vec::new());

    let image_rgb = TestImageColor::generate();
    let image_grayscale = TestImageGrayscale::generate();

    // Encode tiff with compression
    {
        // Create a multipage image with 2 images
        let mut encoder = TiffEncoder::new(&mut data)
            .unwrap()
            .with_compression(compression);
        image_rgb.compress(&mut encoder);
        image_grayscale.compress(&mut encoder);
    }

    // Decode tiff
    data.set_position(0);
    {
        let mut decoder = Decoder::new(data).unwrap();

        // Check the RGB image
        assert_eq!(
            match decoder.read_image() {
                Ok(DecodingResult::U16(image_data)) => image_data,
                unexpected => panic!("Descoding RGB failed: {unexpected:?}"),
            },
            image_rgb.reference_data()
        );

        // Check the grayscale image
        decoder.next_image().unwrap();
        assert_eq!(
            match decoder.read_image() {
                Ok(DecodingResult::U8(image_data)) => image_data,
                unexpected => panic!("Decoding grayscale failed: {unexpected:?}"),
            },
            image_grayscale.reference_data()
        );
    }
}

#[test]
fn encode_decode_without_compression() {
    encode_decode_with_compression(Compression::Uncompressed);
}

#[test]
#[cfg(feature = "lzw")]
fn encode_decode_with_lzw() {
    encode_decode_with_compression(Compression::Lzw);
}

#[test]
#[cfg(feature = "deflate")]
fn encode_decode_with_deflate() {
    encode_decode_with_compression(Compression::Deflate(DeflateLevel::Fast));
    encode_decode_with_compression(Compression::Deflate(DeflateLevel::Balanced));
    encode_decode_with_compression(Compression::Deflate(DeflateLevel::Best));
}

#[test]
fn encode_decode_with_packbits() {
    encode_decode_with_compression(Compression::Packbits);
}
