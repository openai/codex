use super::header::Header;
use crate::{codecs::tga::header::ImageType, error::EncodingError, utils::vec_try_with_capacity};
use crate::{DynamicImage, ExtendedColorType, ImageEncoder, ImageError, ImageFormat, ImageResult};
use std::{error, fmt, io::Write};

/// Errors that can occur during encoding and saving of a TGA image.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum EncoderError {
    /// Invalid TGA width.
    WidthInvalid(u32),

    /// Invalid TGA height.
    HeightInvalid(u32),
}

impl fmt::Display for EncoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncoderError::WidthInvalid(s) => f.write_fmt(format_args!("Invalid TGA width: {s}")),
            EncoderError::HeightInvalid(s) => f.write_fmt(format_args!("Invalid TGA height: {s}")),
        }
    }
}

impl From<EncoderError> for ImageError {
    fn from(e: EncoderError) -> ImageError {
        ImageError::Encoding(EncodingError::new(ImageFormat::Tga.into(), e))
    }
}

impl error::Error for EncoderError {}

/// TGA encoder.
pub struct TgaEncoder<W: Write> {
    writer: W,

    /// Run-length encoding
    use_rle: bool,
}

const MAX_RUN_LENGTH: u8 = 128;

#[derive(Debug, Eq, PartialEq)]
enum PacketType {
    Raw,
    Rle,
}

impl<W: Write> TgaEncoder<W> {
    /// Create a new encoder that writes its output to ```w```.
    pub fn new(w: W) -> TgaEncoder<W> {
        TgaEncoder {
            writer: w,
            use_rle: true,
        }
    }

    /// Disables run-length encoding
    pub fn disable_rle(mut self) -> TgaEncoder<W> {
        self.use_rle = false;
        self
    }

    /// Writes a raw packet to the writer
    fn write_raw_packet(&mut self, pixels: &[u8], counter: u8) -> ImageResult<()> {
        // Set high bit = 0 and store counter - 1 (because 0 would be useless)
        // The counter fills 7 bits max, so the high bit is set to 0 implicitly
        let header = counter - 1;
        self.writer.write_all(&[header])?;
        self.writer.write_all(pixels)?;
        Ok(())
    }

    /// Writes a run-length encoded packet to the writer
    fn write_rle_encoded_packet(&mut self, pixel: &[u8], counter: u8) -> ImageResult<()> {
        // Set high bit = 1 and store counter - 1 (because 0 would be useless)
        let header = 0x80 | (counter - 1);
        self.writer.write_all(&[header])?;
        self.writer.write_all(pixel)?;
        Ok(())
    }

    /// Writes the run-length encoded buffer to the writer
    fn run_length_encode(
        &mut self,
        image: &[u8],
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        use PacketType::*;

        let bytes_per_pixel = color_type.bits_per_pixel() / 8;
        let capacity_in_bytes = usize::from(MAX_RUN_LENGTH) * usize::from(bytes_per_pixel);

        // Buffer to temporarily store pixels
        // so we can choose whether to use RLE or not when we need to
        let mut buf = vec_try_with_capacity(capacity_in_bytes)?;

        let mut counter = 0;
        let mut prev_pixel = None;
        let mut packet_type = Rle;

        for pixel in image.chunks(usize::from(bytes_per_pixel)) {
            // Make sure we are not at the first pixel
            if let Some(prev) = prev_pixel {
                if pixel == prev {
                    if packet_type == Raw && counter > 0 {
                        self.write_raw_packet(&buf, counter)?;
                        counter = 0;
                        buf.clear();
                    }

                    packet_type = Rle;
                } else if packet_type == Rle && counter > 0 {
                    self.write_rle_encoded_packet(prev, counter)?;
                    counter = 0;
                    packet_type = Raw;
                    buf.clear();
                }
            }

            counter += 1;
            buf.extend_from_slice(pixel);

            debug_assert!(buf.len() <= capacity_in_bytes);

            if counter == MAX_RUN_LENGTH {
                match packet_type {
                    Rle => self.write_rle_encoded_packet(prev_pixel.unwrap(), counter),
                    Raw => self.write_raw_packet(&buf, counter),
                }?;

                counter = 0;
                packet_type = Rle;
                buf.clear();
            }

            prev_pixel = Some(pixel);
        }

        if counter > 0 {
            match packet_type {
                Rle => self.write_rle_encoded_packet(prev_pixel.unwrap(), counter),
                Raw => self.write_raw_packet(&buf, counter),
            }?;
        }

        Ok(())
    }

    /// Encodes the image ```buf``` that has dimensions ```width```
    /// and ```height``` and ```ColorType``` ```color_type```.
    ///
    /// The dimensions of the image must be between 0 and 65535 (inclusive) or
    /// an error will be returned.
    ///
    /// # Panics
    ///
    /// Panics if `width * height * color_type.bytes_per_pixel() != data.len()`.
    #[track_caller]
    pub fn encode(
        mut self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        let expected_buffer_len = color_type.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            buf.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            buf.len(),
        );

        // Validate dimensions.
        let width = u16::try_from(width)
            .map_err(|_| ImageError::from(EncoderError::WidthInvalid(width)))?;

        let height = u16::try_from(height)
            .map_err(|_| ImageError::from(EncoderError::HeightInvalid(height)))?;

        // Write out TGA header.
        let header = Header::from_pixel_info(color_type, width, height, self.use_rle)?;
        header.write_to(&mut self.writer)?;

        let image_type = ImageType::new(header.image_type);

        match image_type {
            //TODO: support RunColorMap, and change match to image_type.is_encoded()
            ImageType::RunTrueColor | ImageType::RunGrayScale => {
                // Write run-length encoded image data

                match color_type {
                    ExtendedColorType::Rgb8 | ExtendedColorType::Rgba8 => {
                        let mut image = Vec::from(buf);

                        for pixel in image.chunks_mut(usize::from(color_type.bits_per_pixel() / 8))
                        {
                            pixel.swap(0, 2);
                        }

                        self.run_length_encode(&image, color_type)?;
                    }
                    _ => {
                        self.run_length_encode(buf, color_type)?;
                    }
                }
            }
            _ => {
                // Write uncompressed image data

                match color_type {
                    ExtendedColorType::Rgb8 | ExtendedColorType::Rgba8 => {
                        let mut image = Vec::from(buf);

                        for pixel in image.chunks_mut(usize::from(color_type.bits_per_pixel() / 8))
                        {
                            pixel.swap(0, 2);
                        }

                        self.writer.write_all(&image)?;
                    }
                    _ => {
                        self.writer.write_all(buf)?;
                    }
                }
            }
        }

        Ok(())
    }
}

impl<W: Write> ImageEncoder for TgaEncoder<W> {
    #[track_caller]
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        self.encode(buf, width, height, color_type)
    }

    fn make_compatible_img(
        &self,
        _: crate::io::encoder::MethodSealedToImage,
        img: &DynamicImage,
    ) -> Option<DynamicImage> {
        crate::io::encoder::dynimage_conversion_8bit(img)
    }
}

#[cfg(test)]
mod tests {
    use super::{EncoderError, TgaEncoder};
    use crate::{codecs::tga::TgaDecoder, ExtendedColorType, ImageDecoder, ImageError};
    use std::{error::Error, io::Cursor};

    #[test]
    fn test_image_width_too_large() {
        // TGA cannot encode images larger than 65,535×65,535
        // create a 65,536×1 8-bit black image buffer
        let size = usize::from(u16::MAX) + 1;
        let dimension = size as u32;
        let img = vec![0u8; size];

        // Try to encode an image that is too large
        let mut encoded = Vec::new();
        let encoder = TgaEncoder::new(&mut encoded);
        let result = encoder.encode(&img, dimension, 1, ExtendedColorType::L8);

        match result {
            Err(ImageError::Encoding(err)) => {
                let err = err
                    .source()
                    .unwrap()
                    .downcast_ref::<EncoderError>()
                    .unwrap();
                assert_eq!(*err, EncoderError::WidthInvalid(dimension));
            }
            other => panic!(
                "Encoding an image that is too wide should return a InvalidWidth \
                it returned {other:?} instead"
            ),
        }
    }

    #[test]
    fn test_image_height_too_large() {
        // TGA cannot encode images larger than 65,535×65,535
        // create a 65,536×1 8-bit black image buffer
        let size = usize::from(u16::MAX) + 1;
        let dimension = size as u32;
        let img = vec![0u8; size];

        // Try to encode an image that is too large
        let mut encoded = Vec::new();
        let encoder = TgaEncoder::new(&mut encoded);
        let result = encoder.encode(&img, 1, dimension, ExtendedColorType::L8);

        match result {
            Err(ImageError::Encoding(err)) => {
                let err = err
                    .source()
                    .unwrap()
                    .downcast_ref::<EncoderError>()
                    .unwrap();
                assert_eq!(*err, EncoderError::HeightInvalid(dimension));
            }
            other => panic!(
                "Encoding an image that is too tall should return a InvalidHeight \
                it returned {other:?} instead"
            ),
        }
    }

    #[test]
    fn test_compression_diff() {
        let image = [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2];

        let uncompressed_bytes = {
            let mut encoded_data = Vec::new();
            let encoder = TgaEncoder::new(&mut encoded_data).disable_rle();
            encoder
                .encode(&image, 5, 1, ExtendedColorType::Rgb8)
                .expect("could not encode image");

            encoded_data
        };

        let compressed_bytes = {
            let mut encoded_data = Vec::new();
            let encoder = TgaEncoder::new(&mut encoded_data);
            encoder
                .encode(&image, 5, 1, ExtendedColorType::Rgb8)
                .expect("could not encode image");

            encoded_data
        };

        assert!(uncompressed_bytes.len() > compressed_bytes.len());
    }

    mod compressed {
        use super::*;

        fn round_trip_image(
            image: &[u8],
            width: u32,
            height: u32,
            c: ExtendedColorType,
        ) -> Vec<u8> {
            let mut encoded_data = Vec::new();
            {
                let encoder = TgaEncoder::new(&mut encoded_data);
                encoder
                    .encode(image, width, height, c)
                    .expect("could not encode image");
            }
            let decoder = TgaDecoder::new(Cursor::new(&encoded_data)).expect("failed to decode");

            let mut buf = vec![0; decoder.total_bytes() as usize];
            decoder.read_image(&mut buf).expect("failed to decode");
            buf
        }

        #[test]
        fn mixed_packets() {
            let image = [
                255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            ];
            let decoded = round_trip_image(&image, 5, 1, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_gray() {
            let image = [0, 1, 2];
            let decoded = round_trip_image(&image, 3, 1, ExtendedColorType::L8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_graya() {
            let image = [0, 1, 2, 3, 4, 5];
            let decoded = round_trip_image(&image, 1, 3, ExtendedColorType::La8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_single_pixel_rgb() {
            let image = [0, 1, 2];
            let decoded = round_trip_image(&image, 1, 1, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_three_pixel_rgb() {
            let image = [0, 1, 2, 0, 1, 2, 0, 1, 2];
            let decoded = round_trip_image(&image, 3, 1, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_3px_rgb() {
            let image = [0; 3 * 3 * 3]; // 3x3 pixels, 3 bytes per pixel
            let decoded = round_trip_image(&image, 3, 3, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_different() {
            let image = [0, 1, 2, 0, 1, 3, 0, 1, 4];
            let decoded = round_trip_image(&image, 3, 1, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_different_2() {
            let image = [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 4];
            let decoded = round_trip_image(&image, 4, 1, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_different_3() {
            let image = [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 4, 0, 1, 2];
            let decoded = round_trip_image(&image, 5, 1, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_bw() {
            // This example demonstrates the run-length counter being saturated
            // It should never overflow and can be 128 max
            let image = crate::open("tests/images/tga/encoding/black_white.tga").unwrap();
            let (width, height) = (image.width(), image.height());
            let image = image.as_rgb8().unwrap().to_vec();

            let decoded = round_trip_image(&image, width, height, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }
    }

    mod uncompressed {
        use super::*;

        fn round_trip_image(
            image: &[u8],
            width: u32,
            height: u32,
            c: ExtendedColorType,
        ) -> Vec<u8> {
            let mut encoded_data = Vec::new();
            {
                let encoder = TgaEncoder::new(&mut encoded_data).disable_rle();
                encoder
                    .encode(image, width, height, c)
                    .expect("could not encode image");
            }

            let decoder = TgaDecoder::new(Cursor::new(&encoded_data)).expect("failed to decode");

            let mut buf = vec![0; decoder.total_bytes() as usize];
            decoder.read_image(&mut buf).expect("failed to decode");
            buf
        }

        #[test]
        fn round_trip_single_pixel_rgb() {
            let image = [0, 1, 2];
            let decoded = round_trip_image(&image, 1, 1, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_single_pixel_rgba() {
            let image = [0, 1, 2, 3];
            let decoded = round_trip_image(&image, 1, 1, ExtendedColorType::Rgba8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_gray() {
            let image = [0, 1, 2];
            let decoded = round_trip_image(&image, 3, 1, ExtendedColorType::L8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_graya() {
            let image = [0, 1, 2, 3, 4, 5];
            let decoded = round_trip_image(&image, 1, 3, ExtendedColorType::La8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }

        #[test]
        fn round_trip_3px_rgb() {
            let image = [0; 3 * 3 * 3]; // 3x3 pixels, 3 bytes per pixel
            let decoded = round_trip_image(&image, 3, 3, ExtendedColorType::Rgb8);
            assert_eq!(decoded.len(), image.len());
            assert_eq!(decoded.as_slice(), image);
        }
    }
}
