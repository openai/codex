use std::fs::File;
use std::io::{self, BufRead, BufWriter, Seek, Write};
use std::path::Path;
use std::{iter, mem::size_of};

use crate::io::encoder::ImageEncoderBoxed;
use crate::{codecs::*, ExtendedColorType, ImageReader};

use crate::error::{
    ImageError, ImageFormatHint, ImageResult, LimitError, LimitErrorKind, ParameterError,
    ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::{DynamicImage, ImageDecoder, ImageFormat};

/// Create a new image from a Reader.
///
/// Assumes the reader is already buffered. For optimal performance,
/// consider wrapping the reader with a `BufReader::new()`.
///
/// Try [`ImageReader`] for more advanced uses.
pub fn load<R: BufRead + Seek>(r: R, format: ImageFormat) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(r);
    reader.set_format(format);
    reader.decode()
}

/// Saves the supplied buffer to a file at the path specified.
///
/// The image format is derived from the file extension. The buffer is assumed to have the correct
/// format according to the specified color type. This will lead to corrupted files if the buffer
/// contains malformed data.
pub fn save_buffer(
    path: impl AsRef<Path>,
    buf: &[u8],
    width: u32,
    height: u32,
    color: impl Into<ExtendedColorType>,
) -> ImageResult<()> {
    let format = ImageFormat::from_path(path.as_ref())?;
    save_buffer_with_format(path, buf, width, height, color, format)
}

/// Saves the supplied buffer to a file given the path and desired format.
///
/// The buffer is assumed to have the correct format according to the specified color type. This
/// will lead to corrupted files if the buffer contains malformed data.
pub fn save_buffer_with_format(
    path: impl AsRef<Path>,
    buf: &[u8],
    width: u32,
    height: u32,
    color: impl Into<ExtendedColorType>,
    format: ImageFormat,
) -> ImageResult<()> {
    let buffered_file_write = &mut BufWriter::new(File::create(path)?); // always seekable
    let encoder = encoder_for_format(format, buffered_file_write)?;
    encoder.write_image(buf, width, height, color.into())
}

pub(crate) fn encoder_for_format<'a, W: Write + Seek>(
    format: ImageFormat,
    buffered_write: &'a mut W,
) -> ImageResult<Box<dyn ImageEncoderBoxed + 'a>> {
    Ok(match format {
        #[cfg(feature = "png")]
        ImageFormat::Png => Box::new(png::PngEncoder::new(buffered_write)),
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => Box::new(jpeg::JpegEncoder::new(buffered_write)),
        #[cfg(feature = "pnm")]
        ImageFormat::Pnm => Box::new(pnm::PnmEncoder::new(buffered_write)),
        #[cfg(feature = "gif")]
        ImageFormat::Gif => Box::new(gif::GifEncoder::new(buffered_write)),
        #[cfg(feature = "ico")]
        ImageFormat::Ico => Box::new(ico::IcoEncoder::new(buffered_write)),
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => Box::new(bmp::BmpEncoder::new(buffered_write)),
        #[cfg(feature = "ff")]
        ImageFormat::Farbfeld => Box::new(farbfeld::FarbfeldEncoder::new(buffered_write)),
        #[cfg(feature = "tga")]
        ImageFormat::Tga => Box::new(tga::TgaEncoder::new(buffered_write)),
        #[cfg(feature = "exr")]
        ImageFormat::OpenExr => Box::new(openexr::OpenExrEncoder::new(buffered_write)),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => Box::new(tiff::TiffEncoder::new(buffered_write)),
        #[cfg(feature = "avif")]
        ImageFormat::Avif => Box::new(avif::AvifEncoder::new(buffered_write)),
        #[cfg(feature = "qoi")]
        ImageFormat::Qoi => Box::new(qoi::QoiEncoder::new(buffered_write)),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => Box::new(webp::WebPEncoder::new_lossless(buffered_write)),
        #[cfg(feature = "hdr")]
        ImageFormat::Hdr => Box::new(hdr::HdrEncoder::new(buffered_write)),
        _ => {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormatHint::Unknown,
                    UnsupportedErrorKind::Format(ImageFormatHint::Name(format!("{format:?}"))),
                ),
            ));
        }
    })
}

static MAGIC_BYTES: [(&[u8], &[u8], ImageFormat); 22] = [
    (b"\x89PNG\r\n\x1a\n", b"", ImageFormat::Png),
    (&[0xff, 0xd8, 0xff], b"", ImageFormat::Jpeg),
    (b"GIF89a", b"", ImageFormat::Gif),
    (b"GIF87a", b"", ImageFormat::Gif),
    (
        b"RIFF\0\0\0\0WEBP",
        b"\xFF\xFF\xFF\xFF\0\0\0\0",
        ImageFormat::WebP,
    ),
    (b"MM\x00*", b"", ImageFormat::Tiff),
    (b"II*\x00", b"", ImageFormat::Tiff),
    (b"DDS ", b"", ImageFormat::Dds),
    (b"BM", b"", ImageFormat::Bmp),
    (&[0, 0, 1, 0], b"", ImageFormat::Ico),
    (b"#?RADIANCE", b"", ImageFormat::Hdr),
    (b"\0\0\0\0ftypavif", b"\xFF\xFF\0\0", ImageFormat::Avif),
    (&[0x76, 0x2f, 0x31, 0x01], b"", ImageFormat::OpenExr), // = &exr::meta::magic_number::BYTES
    (b"qoif", b"", ImageFormat::Qoi),
    (b"P1", b"", ImageFormat::Pnm),
    (b"P2", b"", ImageFormat::Pnm),
    (b"P3", b"", ImageFormat::Pnm),
    (b"P4", b"", ImageFormat::Pnm),
    (b"P5", b"", ImageFormat::Pnm),
    (b"P6", b"", ImageFormat::Pnm),
    (b"P7", b"", ImageFormat::Pnm),
    (b"farbfeld", b"", ImageFormat::Farbfeld),
];

/// Guess image format from memory block
///
/// Makes an educated guess about the image format based on the Magic Bytes at the beginning.
/// TGA is not supported by this function.
/// This is not to be trusted on the validity of the whole memory block
pub fn guess_format(buffer: &[u8]) -> ImageResult<ImageFormat> {
    match guess_format_impl(buffer) {
        Some(format) => Ok(format),
        None => Err(ImageError::Unsupported(ImageFormatHint::Unknown.into())),
    }
}

pub(crate) fn guess_format_impl(buffer: &[u8]) -> Option<ImageFormat> {
    for &(signature, mask, format) in &MAGIC_BYTES {
        if mask.is_empty() {
            if buffer.starts_with(signature) {
                return Some(format);
            }
        } else if buffer.len() >= signature.len()
            && buffer
                .iter()
                .zip(signature.iter())
                .zip(mask.iter().chain(iter::repeat(&0xFF)))
                .all(|((&byte, &sig), &mask)| byte & mask == sig)
        {
            return Some(format);
        }
    }

    None
}

/// Decodes a specific region of the image, represented by the rectangle
/// starting from ```x``` and ```y``` and having ```length``` and ```width```
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn load_rect<D, F1, F2, E>(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    buf: &mut [u8],
    row_pitch: usize,
    decoder: &mut D,
    scanline_bytes: usize,
    mut seek_scanline: F1,
    mut read_scanline: F2,
) -> ImageResult<()>
where
    D: ImageDecoder,
    F1: FnMut(&mut D, u64) -> io::Result<()>,
    F2: FnMut(&mut D, &mut [u8]) -> Result<(), E>,
    ImageError: From<E>,
{
    let scanline_bytes = u64::try_from(scanline_bytes).unwrap();
    let row_pitch = u64::try_from(row_pitch).unwrap();

    let (x, y, width, height) = (
        u64::from(x),
        u64::from(y),
        u64::from(width),
        u64::from(height),
    );
    let dimensions = decoder.dimensions();
    let bytes_per_pixel = u64::from(decoder.color_type().bytes_per_pixel());
    let row_bytes = bytes_per_pixel * u64::from(dimensions.0);
    let total_bytes = width * height * bytes_per_pixel;

    assert!(
        buf.len() >= usize::try_from(total_bytes).unwrap_or(usize::MAX),
        "output buffer too short\n expected `{}`, provided `{}`",
        total_bytes,
        buf.len()
    );

    let mut current_scanline = 0;
    let mut tmp = Vec::new();
    let mut tmp_scanline = None;

    {
        // Read a range of the image starting from byte number `start` and continuing until byte
        // number `end`. Updates `current_scanline` and `bytes_read` appropriately.
        let mut read_image_range =
            |mut start: u64, end: u64, mut output: &mut [u8]| -> ImageResult<()> {
                // If the first scanline we need is already stored in the temporary buffer, then handle
                // it first.
                let target_scanline = start / scanline_bytes;
                if tmp_scanline == Some(target_scanline) {
                    let position = target_scanline * scanline_bytes;
                    let offset = start.saturating_sub(position);
                    let len = (end - start)
                        .min(scanline_bytes - offset)
                        .min(end - position);

                    output
                        .write_all(&tmp[offset as usize..][..len as usize])
                        .unwrap();
                    start += len;

                    if start == end {
                        return Ok(());
                    }
                }

                let target_scanline = start / scanline_bytes;
                if target_scanline != current_scanline {
                    seek_scanline(decoder, target_scanline)?;
                    current_scanline = target_scanline;
                }

                let mut position = current_scanline * scanline_bytes;
                while position < end {
                    if position >= start && end - position >= scanline_bytes {
                        read_scanline(decoder, &mut output[..(scanline_bytes as usize)])?;
                        output = &mut output[scanline_bytes as usize..];
                    } else {
                        tmp.resize(scanline_bytes as usize, 0u8);
                        read_scanline(decoder, &mut tmp)?;
                        tmp_scanline = Some(current_scanline);

                        let offset = start.saturating_sub(position);
                        let len = (end - start)
                            .min(scanline_bytes - offset)
                            .min(end - position);

                        output
                            .write_all(&tmp[offset as usize..][..len as usize])
                            .unwrap();
                    }

                    current_scanline += 1;
                    position += scanline_bytes;
                }
                Ok(())
            };

        if x + width > u64::from(dimensions.0)
            || y + height > u64::from(dimensions.1)
            || width == 0
            || height == 0
        {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )));
        }
        if scanline_bytes > usize::MAX as u64 {
            return Err(ImageError::Limits(LimitError::from_kind(
                LimitErrorKind::InsufficientMemory,
            )));
        }

        if x == 0 && width == u64::from(dimensions.0) && row_pitch == row_bytes {
            let start = x * bytes_per_pixel + y * row_bytes;
            let end = (x + width) * bytes_per_pixel + (y + height - 1) * row_bytes;
            read_image_range(start, end, buf)?;
        } else {
            for (output_slice, row) in buf.chunks_mut(row_pitch as usize).zip(y..(y + height)) {
                let start = x * bytes_per_pixel + row * row_bytes;
                let end = (x + width) * bytes_per_pixel + row * row_bytes;
                read_image_range(start, end, output_slice)?;
            }
        }
    }

    // Seek back to the start
    Ok(seek_scanline(decoder, 0)?)
}

/// Reads all of the bytes of a decoder into a Vec<T>. No particular alignment
/// of the output buffer is guaranteed.
///
/// Panics if there isn't enough memory to decode the image.
pub(crate) fn decoder_to_vec<T>(decoder: impl ImageDecoder) -> ImageResult<Vec<T>>
where
    T: crate::traits::Primitive + bytemuck::Pod,
{
    let total_bytes = usize::try_from(decoder.total_bytes());
    if total_bytes.is_err() || total_bytes.unwrap() > isize::MAX as usize {
        return Err(ImageError::Limits(LimitError::from_kind(
            LimitErrorKind::InsufficientMemory,
        )));
    }

    let mut buf = vec![num_traits::Zero::zero(); total_bytes.unwrap() / size_of::<T>()];
    decoder.read_image(bytemuck::cast_slice_mut(buf.as_mut_slice()))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use crate::ColorType;
    use std::io;

    use super::{load_rect, ImageDecoder, ImageResult};

    #[test]
    fn test_load_rect() {
        struct MockDecoder {
            scanline_number: u64,
            scanline_bytes: u64,
        }
        impl ImageDecoder for MockDecoder {
            fn dimensions(&self) -> (u32, u32) {
                (5, 5)
            }
            fn color_type(&self) -> ColorType {
                ColorType::L8
            }
            fn read_image(self, _buf: &mut [u8]) -> ImageResult<()> {
                unimplemented!()
            }
            fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
                (*self).read_image(buf)
            }
        }

        const DATA: [u8; 25] = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24,
        ];

        fn seek_scanline(m: &mut MockDecoder, n: u64) -> io::Result<()> {
            m.scanline_number = n;
            Ok(())
        }
        fn read_scanline(m: &mut MockDecoder, buf: &mut [u8]) -> io::Result<()> {
            let bytes_read = m.scanline_number * m.scanline_bytes;
            if bytes_read >= 25 {
                return Ok(());
            }

            let len = m.scanline_bytes.min(25 - bytes_read);
            buf[..(len as usize)].copy_from_slice(&DATA[(bytes_read as usize)..][..(len as usize)]);
            m.scanline_number += 1;
            Ok(())
        }

        for scanline_bytes in 1..30 {
            let mut output = [0u8; 26];

            load_rect(
                0,
                0,
                5,
                5,
                &mut output,
                5,
                &mut MockDecoder {
                    scanline_number: 0,
                    scanline_bytes,
                },
                scanline_bytes as usize,
                seek_scanline,
                read_scanline,
            )
            .unwrap();
            assert_eq!(output[0..25], DATA);
            assert_eq!(output[25], 0);

            output = [0u8; 26];
            load_rect(
                3,
                2,
                1,
                1,
                &mut output,
                1,
                &mut MockDecoder {
                    scanline_number: 0,
                    scanline_bytes,
                },
                scanline_bytes as usize,
                seek_scanline,
                read_scanline,
            )
            .unwrap();
            assert_eq!(output[0..2], [13, 0]);

            output = [0u8; 26];
            load_rect(
                3,
                2,
                2,
                2,
                &mut output,
                2,
                &mut MockDecoder {
                    scanline_number: 0,
                    scanline_bytes,
                },
                scanline_bytes as usize,
                seek_scanline,
                read_scanline,
            )
            .unwrap();
            assert_eq!(output[0..5], [13, 14, 18, 19, 0]);

            output = [0u8; 26];
            load_rect(
                1,
                1,
                2,
                4,
                &mut output,
                2,
                &mut MockDecoder {
                    scanline_number: 0,
                    scanline_bytes,
                },
                scanline_bytes as usize,
                seek_scanline,
                read_scanline,
            )
            .unwrap();
            assert_eq!(output[0..9], [6, 7, 11, 12, 16, 17, 21, 22, 0]);
        }
    }

    #[test]
    fn test_load_rect_single_scanline() {
        const DATA: [u8; 25] = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24,
        ];

        struct MockDecoder;
        impl ImageDecoder for MockDecoder {
            fn dimensions(&self) -> (u32, u32) {
                (5, 5)
            }
            fn color_type(&self) -> ColorType {
                ColorType::L8
            }
            fn read_image(self, _buf: &mut [u8]) -> ImageResult<()> {
                unimplemented!()
            }
            fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
                (*self).read_image(buf)
            }
        }

        // Ensure that seek scanline is called only once.
        let mut seeks = 0;
        let seek_scanline = |_d: &mut MockDecoder, n: u64| -> io::Result<()> {
            seeks += 1;
            assert_eq!(n, 0);
            assert_eq!(seeks, 1);
            Ok(())
        };

        fn read_scanline(_m: &mut MockDecoder, buf: &mut [u8]) -> io::Result<()> {
            buf.copy_from_slice(&DATA);
            Ok(())
        }

        let mut output = [0; 26];
        load_rect(
            1,
            1,
            2,
            4,
            &mut output,
            2,
            &mut MockDecoder,
            DATA.len(),
            seek_scanline,
            read_scanline,
        )
        .unwrap();
        assert_eq!(output[0..9], [6, 7, 11, 12, 16, 17, 21, 22, 0]);
    }
}
