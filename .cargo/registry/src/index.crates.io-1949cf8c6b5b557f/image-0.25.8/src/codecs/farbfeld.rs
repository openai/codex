//! Decoding of farbfeld images
//!
//! farbfeld is a lossless image format which is easy to parse, pipe and compress.
//!
//! It has the following format:
//!
//! | Bytes  | Description                                             |
//! |--------|---------------------------------------------------------|
//! | 8      | "farbfeld" magic value                                  |
//! | 4      | 32-Bit BE unsigned integer (width)                      |
//! | 4      | 32-Bit BE unsigned integer (height)                     |
//! | [2222] | 4⋅16-Bit BE unsigned integers [RGBA] / pixel, row-major |
//!
//! The RGB-data should be sRGB for best interoperability and not alpha-premultiplied.
//!
//! # Related Links
//! * <https://tools.suckless.org/farbfeld/> - the farbfeld specification

use std::io::{self, Read, Seek, SeekFrom, Write};

use crate::color::ExtendedColorType;
use crate::error::{
    DecodingError, ImageError, ImageResult, UnsupportedError, UnsupportedErrorKind,
};
use crate::io::free_functions::load_rect;
use crate::{ColorType, ImageDecoder, ImageDecoderRect, ImageEncoder, ImageFormat};

/// farbfeld Reader
pub struct FarbfeldReader<R: Read> {
    width: u32,
    height: u32,
    inner: R,
    /// Relative to the start of the pixel data
    current_offset: u64,
    cached_byte: Option<u8>,
}

impl<R: Read> FarbfeldReader<R> {
    fn new(mut buffered_read: R) -> ImageResult<FarbfeldReader<R>> {
        fn read_dimm<R: Read>(from: &mut R) -> ImageResult<u32> {
            let mut buf = [0u8; 4];
            from.read_exact(&mut buf).map_err(|err| {
                ImageError::Decoding(DecodingError::new(ImageFormat::Farbfeld.into(), err))
            })?;
            Ok(u32::from_be_bytes(buf))
        }

        let mut magic = [0u8; 8];
        buffered_read.read_exact(&mut magic).map_err(|err| {
            ImageError::Decoding(DecodingError::new(ImageFormat::Farbfeld.into(), err))
        })?;
        if &magic != b"farbfeld" {
            return Err(ImageError::Decoding(DecodingError::new(
                ImageFormat::Farbfeld.into(),
                format!("Invalid magic: {magic:02x?}"),
            )));
        }

        let reader = FarbfeldReader {
            width: read_dimm(&mut buffered_read)?,
            height: read_dimm(&mut buffered_read)?,
            inner: buffered_read,
            current_offset: 0,
            cached_byte: None,
        };

        if crate::utils::check_dimension_overflow(
            reader.width,
            reader.height,
            // ExtendedColorType is always rgba16
            8,
        ) {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Farbfeld.into(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "Image dimensions ({}x{}) are too large",
                        reader.width, reader.height
                    )),
                ),
            ));
        }

        Ok(reader)
    }
}

impl<R: Read> Read for FarbfeldReader<R> {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let mut bytes_written = 0;
        if let Some(byte) = self.cached_byte.take() {
            buf[0] = byte;
            buf = &mut buf[1..];
            bytes_written = 1;
            self.current_offset += 1;
        }

        if buf.len() == 1 {
            buf[0] = cache_byte(&mut self.inner, &mut self.cached_byte)?;
            bytes_written += 1;
            self.current_offset += 1;
        } else {
            for channel_out in buf.chunks_exact_mut(2) {
                consume_channel(&mut self.inner, channel_out)?;
                bytes_written += 2;
                self.current_offset += 2;
            }
        }

        Ok(bytes_written)
    }
}

impl<R: Read + Seek> Seek for FarbfeldReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        fn parse_offset(original_offset: u64, end_offset: u64, pos: SeekFrom) -> Option<i64> {
            match pos {
                SeekFrom::Start(off) => i64::try_from(off)
                    .ok()?
                    .checked_sub(i64::try_from(original_offset).ok()?),
                SeekFrom::End(off) => {
                    if off < i64::try_from(end_offset).unwrap_or(i64::MAX) {
                        None
                    } else {
                        Some(i64::try_from(end_offset.checked_sub(original_offset)?).ok()? + off)
                    }
                }
                SeekFrom::Current(off) => {
                    if off < i64::try_from(original_offset).unwrap_or(i64::MAX) {
                        None
                    } else {
                        Some(off)
                    }
                }
            }
        }

        let original_offset = self.current_offset;
        let end_offset = u64::from(self.width) * u64::from(self.height) * 2;
        let offset_from_current =
            parse_offset(original_offset, end_offset, pos).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid seek to a negative or overflowing position",
                )
            })?;

        // TODO: convert to seek_relative() once that gets stabilised
        self.inner.seek(SeekFrom::Current(offset_from_current))?;
        self.current_offset = if offset_from_current < 0 {
            original_offset.checked_sub(offset_from_current.wrapping_neg() as u64)
        } else {
            original_offset.checked_add(offset_from_current as u64)
        }
        .expect("This should've been checked above");

        if self.current_offset < end_offset && self.current_offset % 2 == 1 {
            let curr = self.inner.seek(SeekFrom::Current(-1))?;
            cache_byte(&mut self.inner, &mut self.cached_byte)?;
            self.inner.seek(SeekFrom::Start(curr))?;
        } else {
            self.cached_byte = None;
        }

        Ok(original_offset)
    }
}

fn consume_channel<R: Read>(from: &mut R, mut to: &mut [u8]) -> io::Result<()> {
    let mut ibuf = [0u8; 2];
    from.read_exact(&mut ibuf)?;
    to.write_all(&u16::from_be_bytes(ibuf).to_ne_bytes())?;

    Ok(())
}

fn cache_byte<R: Read>(from: &mut R, cached_byte: &mut Option<u8>) -> io::Result<u8> {
    let mut obuf = [0u8; 2];
    consume_channel(from, &mut obuf)?;
    *cached_byte = Some(obuf[1]);
    Ok(obuf[0])
}

/// farbfeld decoder
pub struct FarbfeldDecoder<R: Read> {
    reader: FarbfeldReader<R>,
}

impl<R: Read> FarbfeldDecoder<R> {
    /// Creates a new decoder that decodes from the stream ```r```
    pub fn new(buffered_read: R) -> ImageResult<FarbfeldDecoder<R>> {
        Ok(FarbfeldDecoder {
            reader: FarbfeldReader::new(buffered_read)?,
        })
    }
}

impl<R: Read> ImageDecoder for FarbfeldDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.reader.width, self.reader.height)
    }

    fn color_type(&self) -> ColorType {
        ColorType::Rgba16
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        self.reader.read_exact(buf)?;
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

impl<R: Read + Seek> ImageDecoderRect for FarbfeldDecoder<R> {
    fn read_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        buf: &mut [u8],
        row_pitch: usize,
    ) -> ImageResult<()> {
        // A "scanline" (defined as "shortest non-caching read" in the doc) is just one channel in this case

        let start = self.reader.stream_position()?;
        load_rect(
            x,
            y,
            width,
            height,
            buf,
            row_pitch,
            self,
            2,
            |s, scanline| s.reader.seek(SeekFrom::Start(scanline * 2)).map(|_| ()),
            |s, buf| s.reader.read_exact(buf),
        )?;
        self.reader.seek(SeekFrom::Start(start))?;
        Ok(())
    }
}

/// farbfeld encoder
pub struct FarbfeldEncoder<W: Write> {
    w: W,
}

impl<W: Write> FarbfeldEncoder<W> {
    /// Create a new encoder that writes its output to ```w```. The writer should be buffered.
    pub fn new(buffered_writer: W) -> FarbfeldEncoder<W> {
        FarbfeldEncoder { w: buffered_writer }
    }

    /// Encodes the image `data` (native endian) that has dimensions `width` and `height`.
    ///
    /// # Panics
    ///
    /// Panics if `width * height * 8 != data.len()`.
    #[track_caller]
    pub fn encode(self, data: &[u8], width: u32, height: u32) -> ImageResult<()> {
        let expected_buffer_len = (u64::from(width) * u64::from(height)).saturating_mul(8);
        assert_eq!(
            expected_buffer_len,
            data.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            data.len(),
        );
        self.encode_impl(data, width, height)?;
        Ok(())
    }

    fn encode_impl(mut self, data: &[u8], width: u32, height: u32) -> io::Result<()> {
        self.w.write_all(b"farbfeld")?;

        self.w.write_all(&width.to_be_bytes())?;
        self.w.write_all(&height.to_be_bytes())?;

        for channel in data.chunks_exact(2) {
            self.w
                .write_all(&u16::from_ne_bytes(channel.try_into().unwrap()).to_be_bytes())?;
        }

        Ok(())
    }
}

impl<W: Write> ImageEncoder for FarbfeldEncoder<W> {
    #[track_caller]
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        if color_type != ExtendedColorType::Rgba16 {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Farbfeld.into(),
                    UnsupportedErrorKind::Color(color_type),
                ),
            ));
        }

        self.encode(buf, width, height)
    }
}

#[cfg(test)]
mod tests {
    use crate::codecs::farbfeld::FarbfeldDecoder;
    use crate::ImageDecoderRect;
    use byteorder_lite::{ByteOrder, NativeEndian};
    use std::io::{Cursor, Seek, SeekFrom};

    static RECTANGLE_IN: &[u8] =     b"farbfeld\
                                       \x00\x00\x00\x02\x00\x00\x00\x03\
                                       \xFF\x01\xFE\x02\xFD\x03\xFC\x04\xFB\x05\xFA\x06\xF9\x07\xF8\x08\
                                       \xF7\x09\xF6\x0A\xF5\x0B\xF4\x0C\xF3\x0D\xF2\x0E\xF1\x0F\xF0\x10\
                                       \xEF\x11\xEE\x12\xED\x13\xEC\x14\xEB\x15\xEA\x16\xE9\x17\xE8\x18";

    #[test]
    fn read_rect_1x2() {
        static RECTANGLE_OUT: &[u16] = &[
            0xF30D, 0xF20E, 0xF10F, 0xF010, 0xEB15, 0xEA16, 0xE917, 0xE818,
        ];

        read_rect(1, 1, 1, 2, RECTANGLE_OUT);
    }

    #[test]
    fn read_rect_2x2() {
        static RECTANGLE_OUT: &[u16] = &[
            0xFF01, 0xFE02, 0xFD03, 0xFC04, 0xFB05, 0xFA06, 0xF907, 0xF808, 0xF709, 0xF60A, 0xF50B,
            0xF40C, 0xF30D, 0xF20E, 0xF10F, 0xF010,
        ];

        read_rect(0, 0, 2, 2, RECTANGLE_OUT);
    }

    #[test]
    fn read_rect_2x1() {
        static RECTANGLE_OUT: &[u16] = &[
            0xEF11, 0xEE12, 0xED13, 0xEC14, 0xEB15, 0xEA16, 0xE917, 0xE818,
        ];

        read_rect(0, 2, 2, 1, RECTANGLE_OUT);
    }

    #[test]
    fn read_rect_2x3() {
        static RECTANGLE_OUT: &[u16] = &[
            0xFF01, 0xFE02, 0xFD03, 0xFC04, 0xFB05, 0xFA06, 0xF907, 0xF808, 0xF709, 0xF60A, 0xF50B,
            0xF40C, 0xF30D, 0xF20E, 0xF10F, 0xF010, 0xEF11, 0xEE12, 0xED13, 0xEC14, 0xEB15, 0xEA16,
            0xE917, 0xE818,
        ];

        read_rect(0, 0, 2, 3, RECTANGLE_OUT);
    }

    #[test]
    fn read_rect_in_stream() {
        static RECTANGLE_OUT: &[u16] = &[0xEF11, 0xEE12, 0xED13, 0xEC14];

        let mut input = vec![];
        input.extend_from_slice(b"This is a 31-byte-long prologue");
        input.extend_from_slice(RECTANGLE_IN);
        let mut input_cur = Cursor::new(input);
        input_cur.seek(SeekFrom::Start(31)).unwrap();

        let mut out_buf = [0u8; 64];
        FarbfeldDecoder::new(input_cur)
            .unwrap()
            .read_rect(0, 2, 1, 1, &mut out_buf, 8)
            .unwrap();
        let exp = degenerate_pixels(RECTANGLE_OUT);
        assert_eq!(&out_buf[..exp.len()], &exp[..]);
    }

    #[test]
    fn dimension_overflow() {
        let header = b"farbfeld\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF";

        assert!(FarbfeldDecoder::new(Cursor::new(header)).is_err());
    }

    fn read_rect(x: u32, y: u32, width: u32, height: u32, exp_wide: &[u16]) {
        let mut out_buf = [0u8; 64];
        FarbfeldDecoder::new(Cursor::new(RECTANGLE_IN))
            .unwrap()
            .read_rect(x, y, width, height, &mut out_buf, width as usize * 8)
            .unwrap();
        let exp = degenerate_pixels(exp_wide);
        assert_eq!(&out_buf[..exp.len()], &exp[..]);
    }

    fn degenerate_pixels(exp_wide: &[u16]) -> Vec<u8> {
        let mut exp = vec![0u8; exp_wide.len() * 2];
        NativeEndian::write_u16_into(exp_wide, &mut exp);
        exp
    }
}
