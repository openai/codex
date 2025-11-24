//! Decoding and encoding of QOI images

use crate::error::{DecodingError, EncodingError, UnsupportedError, UnsupportedErrorKind};
use crate::{
    ColorType, ExtendedColorType, ImageDecoder, ImageEncoder, ImageError, ImageFormat, ImageResult,
};
use std::io::{Read, Write};

/// QOI decoder
pub struct QoiDecoder<R> {
    decoder: qoi::Decoder<R>,
}

impl<R> QoiDecoder<R>
where
    R: Read,
{
    /// Creates a new decoder that decodes from the stream ```reader```
    pub fn new(reader: R) -> ImageResult<Self> {
        let decoder = qoi::Decoder::from_stream(reader).map_err(decoding_error)?;
        Ok(Self { decoder })
    }
}

impl<R: Read> ImageDecoder for QoiDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.decoder.header().width, self.decoder.header().height)
    }

    fn color_type(&self) -> ColorType {
        match self.decoder.header().channels {
            qoi::Channels::Rgb => ColorType::Rgb8,
            qoi::Channels::Rgba => ColorType::Rgba8,
        }
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        self.decoder.decode_to_buf(buf).map_err(decoding_error)?;
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

fn decoding_error(error: qoi::Error) -> ImageError {
    ImageError::Decoding(DecodingError::new(ImageFormat::Qoi.into(), error))
}

fn encoding_error(error: qoi::Error) -> ImageError {
    ImageError::Encoding(EncodingError::new(ImageFormat::Qoi.into(), error))
}

/// QOI encoder
pub struct QoiEncoder<W> {
    writer: W,
}

impl<W: Write> QoiEncoder<W> {
    /// Creates a new encoder that writes its output to ```writer```
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> ImageEncoder for QoiEncoder<W> {
    #[track_caller]
    fn write_image(
        mut self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        if !matches!(
            color_type,
            ExtendedColorType::Rgba8 | ExtendedColorType::Rgb8
        ) {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Qoi.into(),
                    UnsupportedErrorKind::Color(color_type),
                ),
            ));
        }

        let expected_buffer_len = color_type.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            buf.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            buf.len(),
        );

        // Encode data in QOI
        let data = qoi::encode_to_vec(buf, width, height).map_err(encoding_error)?;

        // Write data to buffer
        self.writer.write_all(&data[..])?;
        self.writer.flush()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn decode_test_image() {
        let decoder = QoiDecoder::new(File::open("tests/images/qoi/basic-test.qoi").unwrap())
            .expect("Unable to read QOI file");

        assert_eq!((5, 5), decoder.dimensions());
        assert_eq!(ColorType::Rgba8, decoder.color_type());
    }
}
