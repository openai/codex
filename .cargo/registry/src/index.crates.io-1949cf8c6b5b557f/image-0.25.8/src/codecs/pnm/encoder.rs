//! Encoding of PNM Images
use crate::utils::vec_try_with_capacity;
use std::fmt;
use std::io;
use std::io::Write;

use super::AutoBreak;
use super::{ArbitraryHeader, ArbitraryTuplType, BitmapHeader, GraymapHeader, PixmapHeader};
use super::{HeaderRecord, PnmHeader, PnmSubtype, SampleEncoding};

use crate::color::ExtendedColorType;
use crate::error::{
    ImageError, ImageResult, ParameterError, ParameterErrorKind, UnsupportedError,
    UnsupportedErrorKind,
};
use crate::{ImageEncoder, ImageFormat};

use byteorder_lite::{BigEndian, WriteBytesExt};

enum HeaderStrategy {
    Dynamic,
    Subtype(PnmSubtype),
    Chosen(PnmHeader),
}

#[derive(Clone, Copy)]
pub enum FlatSamples<'a> {
    U8(&'a [u8]),
    U16(&'a [u16]),
}

/// Encodes images to any of the `pnm` image formats.
pub struct PnmEncoder<W: Write> {
    writer: W,
    header: HeaderStrategy,
}

/// Encapsulate the checking system in the type system. Non of the fields are actually accessed
/// but requiring them forces us to validly construct the struct anyways.
struct CheckedImageBuffer<'a> {
    _image: FlatSamples<'a>,
    _width: u32,
    _height: u32,
    _color: ExtendedColorType,
}

// Check the header against the buffer. Each struct produces the next after a check.
struct UncheckedHeader<'a> {
    header: &'a PnmHeader,
}

struct CheckedDimensions<'a> {
    unchecked: UncheckedHeader<'a>,
    width: u32,
    height: u32,
}

struct CheckedHeaderColor<'a> {
    dimensions: CheckedDimensions<'a>,
    color: ExtendedColorType,
}

struct CheckedHeader<'a> {
    color: CheckedHeaderColor<'a>,
    encoding: TupleEncoding<'a>,
    _image: CheckedImageBuffer<'a>,
}

enum TupleEncoding<'a> {
    PbmBits {
        samples: FlatSamples<'a>,
        width: u32,
    },
    Ascii {
        samples: FlatSamples<'a>,
    },
    Bytes {
        samples: FlatSamples<'a>,
    },
}

impl<W: Write> PnmEncoder<W> {
    /// Create new `PnmEncoder` from the `writer`.
    ///
    /// The encoded images will have some `pnm` format. If more control over the image type is
    /// required, use either one of `with_subtype` or `with_header`. For more information on the
    /// behaviour, see `with_dynamic_header`.
    pub fn new(writer: W) -> Self {
        PnmEncoder {
            writer,
            header: HeaderStrategy::Dynamic,
        }
    }

    /// Encode a specific pnm subtype image.
    ///
    /// The magic number and encoding type will be chosen as provided while the rest of the header
    /// data will be generated dynamically. Trying to encode incompatible images (e.g. encoding an
    /// RGB image as Graymap) will result in an error.
    ///
    /// This will overwrite the effect of earlier calls to `with_header` and `with_dynamic_header`.
    pub fn with_subtype(self, subtype: PnmSubtype) -> Self {
        PnmEncoder {
            writer: self.writer,
            header: HeaderStrategy::Subtype(subtype),
        }
    }

    /// Enforce the use of a chosen header.
    ///
    /// While this option gives the most control over the actual written data, the encoding process
    /// will error in case the header data and image parameters do not agree. It is the users
    /// obligation to ensure that the width and height are set accordingly, for example.
    ///
    /// Choose this option if you want a lossless decoding/encoding round trip.
    ///
    /// This will overwrite the effect of earlier calls to `with_subtype` and `with_dynamic_header`.
    pub fn with_header(self, header: PnmHeader) -> Self {
        PnmEncoder {
            writer: self.writer,
            header: HeaderStrategy::Chosen(header),
        }
    }

    /// Create the header dynamically for each image.
    ///
    /// This is the default option upon creation of the encoder. With this, most images should be
    /// encodable but the specific format chosen is out of the users control. The pnm subtype is
    /// chosen arbitrarily by the library.
    ///
    /// This will overwrite the effect of earlier calls to `with_subtype` and `with_header`.
    pub fn with_dynamic_header(self) -> Self {
        PnmEncoder {
            writer: self.writer,
            header: HeaderStrategy::Dynamic,
        }
    }

    /// Encode an image whose samples are represented as a sequence of `u8` or `u16` data.
    ///
    /// If `image` is a slice of `u8`, the samples will be interpreted based on the chosen `color` option.
    /// Color types of 16-bit precision means that the bytes are reinterpreted as 16-bit samples,
    /// otherwise they are treated as 8-bit samples.
    /// If `image` is a slice of `u16`, the samples will be interpreted as 16-bit samples directly.
    ///
    /// Some `pnm` subtypes are incompatible with some color options, a chosen header most
    /// certainly with any deviation from the original decoded image.
    pub fn encode<'s, S>(
        &mut self,
        image: S,
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()>
    where
        S: Into<FlatSamples<'s>>,
    {
        let image = image.into();

        // adapt samples so that they are aligned even in 16-bit samples,
        // required due to the narrowing of the image buffer to &[u8]
        // on dynamic image writing
        let image = match (image, color) {
            (
                FlatSamples::U8(samples),
                ExtendedColorType::L16
                | ExtendedColorType::La16
                | ExtendedColorType::Rgb16
                | ExtendedColorType::Rgba16,
            ) => {
                match bytemuck::try_cast_slice(samples) {
                    // proceed with aligned 16-bit samples
                    Ok(samples) => FlatSamples::U16(samples),
                    Err(_e) => {
                        // reallocation is required
                        let new_samples: Vec<u16> = samples
                            .chunks(2)
                            .map(|chunk| u16::from_ne_bytes([chunk[0], chunk[1]]))
                            .collect();

                        let image = FlatSamples::U16(&new_samples);

                        // make a separate encoding path,
                        // because the image buffer lifetime has changed
                        return self.encode_impl(image, width, height, color);
                    }
                }
            }
            // should not be necessary for any other case
            _ => image,
        };

        self.encode_impl(image, width, height, color)
    }

    /// Encode an image whose samples are already interpreted correctly.
    fn encode_impl(
        &mut self,
        samples: FlatSamples<'_>,
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        match self.header {
            HeaderStrategy::Dynamic => self.write_dynamic_header(samples, width, height, color),
            HeaderStrategy::Subtype(subtype) => {
                self.write_subtyped_header(subtype, samples, width, height, color)
            }
            HeaderStrategy::Chosen(ref header) => {
                Self::write_with_header(&mut self.writer, header, samples, width, height, color)
            }
        }
    }

    /// Choose any valid pnm format that the image can be expressed in and write its header.
    ///
    /// Returns how the body should be written if successful.
    fn write_dynamic_header(
        &mut self,
        image: FlatSamples,
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        let depth = u32::from(color.channel_count());
        let (maxval, tupltype) = match color {
            ExtendedColorType::L1 => (1, ArbitraryTuplType::BlackAndWhite),
            ExtendedColorType::L8 => (0xff, ArbitraryTuplType::Grayscale),
            ExtendedColorType::L16 => (0xffff, ArbitraryTuplType::Grayscale),
            ExtendedColorType::La1 => (1, ArbitraryTuplType::BlackAndWhiteAlpha),
            ExtendedColorType::La8 => (0xff, ArbitraryTuplType::GrayscaleAlpha),
            ExtendedColorType::La16 => (0xffff, ArbitraryTuplType::GrayscaleAlpha),
            ExtendedColorType::Rgb8 => (0xff, ArbitraryTuplType::RGB),
            ExtendedColorType::Rgb16 => (0xffff, ArbitraryTuplType::RGB),
            ExtendedColorType::Rgba8 => (0xff, ArbitraryTuplType::RGBAlpha),
            ExtendedColorType::Rgba16 => (0xffff, ArbitraryTuplType::RGBAlpha),
            _ => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Pnm.into(),
                        UnsupportedErrorKind::Color(color),
                    ),
                ))
            }
        };

        let header = PnmHeader {
            decoded: HeaderRecord::Arbitrary(ArbitraryHeader {
                width,
                height,
                depth,
                maxval,
                tupltype: Some(tupltype),
            }),
            encoded: None,
        };

        Self::write_with_header(&mut self.writer, &header, image, width, height, color)
    }

    /// Try to encode the image with the chosen format, give its corresponding pixel encoding type.
    fn write_subtyped_header(
        &mut self,
        subtype: PnmSubtype,
        image: FlatSamples,
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        let header = match (subtype, color) {
            (PnmSubtype::ArbitraryMap, color) => {
                return self.write_dynamic_header(image, width, height, color)
            }
            (PnmSubtype::Pixmap(encoding), ExtendedColorType::Rgb8) => PnmHeader {
                decoded: HeaderRecord::Pixmap(PixmapHeader {
                    encoding,
                    width,
                    height,
                    maxval: 255,
                }),
                encoded: None,
            },
            (PnmSubtype::Graymap(encoding), ExtendedColorType::L8) => PnmHeader {
                decoded: HeaderRecord::Graymap(GraymapHeader {
                    encoding,
                    width,
                    height,
                    maxwhite: 255,
                }),
                encoded: None,
            },
            (PnmSubtype::Bitmap(encoding), ExtendedColorType::L8 | ExtendedColorType::L1) => {
                PnmHeader {
                    decoded: HeaderRecord::Bitmap(BitmapHeader {
                        encoding,
                        height,
                        width,
                    }),
                    encoded: None,
                }
            }
            (_, _) => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Pnm.into(),
                        UnsupportedErrorKind::Color(color),
                    ),
                ))
            }
        };

        Self::write_with_header(&mut self.writer, &header, image, width, height, color)
    }

    /// Try to encode the image with the chosen header, checking if values are correct.
    ///
    /// Returns how the body should be written if successful.
    fn write_with_header(
        writer: &mut dyn Write,
        header: &PnmHeader,
        image: FlatSamples,
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        let unchecked = UncheckedHeader { header };

        unchecked
            .check_header_dimensions(width, height)?
            .check_header_color(color)?
            .check_sample_values(image)?
            .write_header(writer)?
            .write_image(writer)
    }
}

impl<W: Write> ImageEncoder for PnmEncoder<W> {
    #[track_caller]
    fn write_image(
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

        self.encode(buf, width, height, color_type)
    }
}

impl<'a> CheckedImageBuffer<'a> {
    fn check(
        image: FlatSamples<'a>,
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<CheckedImageBuffer<'a>> {
        let components = color.channel_count() as usize;
        let uwidth = width as usize;
        let uheight = height as usize;
        let expected_len = components
            .checked_mul(uwidth)
            .and_then(|v| v.checked_mul(uheight));
        if Some(image.len()) != expected_len {
            // Image buffer does not correspond to size and colour.
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )));
        }
        Ok(CheckedImageBuffer {
            _image: image,
            _width: width,
            _height: height,
            _color: color,
        })
    }
}

impl<'a> UncheckedHeader<'a> {
    fn check_header_dimensions(
        self,
        width: u32,
        height: u32,
    ) -> ImageResult<CheckedDimensions<'a>> {
        if self.header.width() != width || self.header.height() != height {
            // Chosen header does not match Image dimensions.
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )));
        }

        Ok(CheckedDimensions {
            unchecked: self,
            width,
            height,
        })
    }
}

impl<'a> CheckedDimensions<'a> {
    // Check color compatibility with the header. This will only error when we are certain that
    // the combination is bogus (e.g. combining Pixmap and Palette) but allows uncertain
    // combinations (basically a ArbitraryTuplType::Custom with any color of fitting depth).
    fn check_header_color(self, color: ExtendedColorType) -> ImageResult<CheckedHeaderColor<'a>> {
        let components = u32::from(color.channel_count());

        match *self.unchecked.header {
            PnmHeader {
                decoded: HeaderRecord::Bitmap(_),
                ..
            } => match color {
                ExtendedColorType::L1 | ExtendedColorType::L8 | ExtendedColorType::L16 => (),
                _ => {
                    return Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::Generic(
                            "PBM format only support luma color types".to_owned(),
                        ),
                    )))
                }
            },
            PnmHeader {
                decoded: HeaderRecord::Graymap(_),
                ..
            } => match color {
                ExtendedColorType::L1 | ExtendedColorType::L8 | ExtendedColorType::L16 => (),
                _ => {
                    return Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::Generic(
                            "PGM format only support luma color types".to_owned(),
                        ),
                    )))
                }
            },
            PnmHeader {
                decoded: HeaderRecord::Pixmap(_),
                ..
            } => match color {
                ExtendedColorType::Rgb8 => (),
                _ => {
                    return Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::Generic(
                            "PPM format only support ExtendedColorType::Rgb8".to_owned(),
                        ),
                    )))
                }
            },
            PnmHeader {
                decoded:
                    HeaderRecord::Arbitrary(ArbitraryHeader {
                        depth,
                        ref tupltype,
                        ..
                    }),
                ..
            } => match (tupltype, color) {
                (&Some(ArbitraryTuplType::BlackAndWhite), ExtendedColorType::L1) => (),
                (&Some(ArbitraryTuplType::BlackAndWhiteAlpha), ExtendedColorType::La8) => (),

                (&Some(ArbitraryTuplType::Grayscale), ExtendedColorType::L1) => (),
                (&Some(ArbitraryTuplType::Grayscale), ExtendedColorType::L8) => (),
                (&Some(ArbitraryTuplType::Grayscale), ExtendedColorType::L16) => (),
                (&Some(ArbitraryTuplType::GrayscaleAlpha), ExtendedColorType::La8) => (),

                (&Some(ArbitraryTuplType::RGB), ExtendedColorType::Rgb8) => (),
                (&Some(ArbitraryTuplType::RGB), ExtendedColorType::Rgb16) => (),
                (&Some(ArbitraryTuplType::RGBAlpha), ExtendedColorType::Rgba8) => (),
                (&Some(ArbitraryTuplType::RGBAlpha), ExtendedColorType::Rgba16) => (),

                (&None, _) if depth == components => (),
                (&Some(ArbitraryTuplType::Custom(_)), _) if depth == components => (),
                _ if depth != components => {
                    return Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::Generic(format!(
                            "Depth mismatch: header {depth} vs. color {components}"
                        )),
                    )))
                }
                _ => {
                    return Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::Generic(
                            "Invalid color type for selected PAM color type".to_owned(),
                        ),
                    )))
                }
            },
        }

        Ok(CheckedHeaderColor {
            dimensions: self,
            color,
        })
    }
}

impl<'a> CheckedHeaderColor<'a> {
    fn check_sample_values(self, image: FlatSamples<'a>) -> ImageResult<CheckedHeader<'a>> {
        let header_maxval = match self.dimensions.unchecked.header.decoded {
            HeaderRecord::Bitmap(_) => 1,
            HeaderRecord::Graymap(GraymapHeader { maxwhite, .. }) => maxwhite,
            HeaderRecord::Pixmap(PixmapHeader { maxval, .. }) => maxval,
            HeaderRecord::Arbitrary(ArbitraryHeader { maxval, .. }) => maxval,
        };

        // We trust the image color bit count to be correct at least.
        let max_sample = match self.color {
            ExtendedColorType::Unknown(n) if n <= 16 => (1 << n) - 1,
            ExtendedColorType::L1 => 1,
            ExtendedColorType::L8
            | ExtendedColorType::La8
            | ExtendedColorType::Rgb8
            | ExtendedColorType::Rgba8
            | ExtendedColorType::Bgr8
            | ExtendedColorType::Bgra8 => 0xff,
            ExtendedColorType::L16
            | ExtendedColorType::La16
            | ExtendedColorType::Rgb16
            | ExtendedColorType::Rgba16 => 0xffff,
            _ => {
                // Unsupported target color type.
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Pnm.into(),
                        UnsupportedErrorKind::Color(self.color),
                    ),
                ));
            }
        };

        // Avoid the performance heavy check if possible, e.g. if the header has been chosen by us.
        if header_maxval < max_sample && !image.all_smaller(header_maxval) {
            // Sample value greater than allowed for chosen header.
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Pnm.into(),
                    UnsupportedErrorKind::GenericFeature(
                        "Sample value greater than allowed for chosen header".to_owned(),
                    ),
                ),
            ));
        }

        let encoding = image.encoding_for(&self.dimensions.unchecked.header.decoded);

        let image = CheckedImageBuffer::check(
            image,
            self.dimensions.width,
            self.dimensions.height,
            self.color,
        )?;

        Ok(CheckedHeader {
            color: self,
            encoding,
            _image: image,
        })
    }
}

impl<'a> CheckedHeader<'a> {
    fn write_header(self, writer: &mut dyn Write) -> ImageResult<TupleEncoding<'a>> {
        self.header().write(writer)?;
        Ok(self.encoding)
    }

    fn header(&self) -> &PnmHeader {
        self.color.dimensions.unchecked.header
    }
}

struct SampleWriter<'a>(&'a mut dyn Write);

impl SampleWriter<'_> {
    fn write_samples_ascii<V>(self, samples: V) -> io::Result<()>
    where
        V: Iterator,
        V::Item: fmt::Display,
    {
        let mut auto_break_writer = AutoBreak::new(self.0, 70)?;
        for value in samples {
            write!(auto_break_writer, "{value} ")?;
        }
        auto_break_writer.flush()
    }

    fn write_pbm_bits<V>(self, samples: &[V], width: u32) -> io::Result<()>
    /* Default gives 0 for all primitives. TODO: replace this with `Zeroable` once it hits stable */
    where
        V: Default + Eq + Copy,
    {
        // The length of an encoded scanline
        let line_width = (width - 1) / 8 + 1;

        // We'll be writing single bytes, so buffer
        let mut line_buffer = vec_try_with_capacity(line_width as usize)?;

        for line in samples.chunks(width as usize) {
            for byte_bits in line.chunks(8) {
                let mut byte = 0u8;
                for i in 0..8 {
                    // Black pixels are encoded as 1s
                    if let Some(&v) = byte_bits.get(i) {
                        if v == V::default() {
                            byte |= 1u8 << (7 - i);
                        }
                    }
                }
                line_buffer.push(byte);
            }
            self.0.write_all(line_buffer.as_slice())?;
            line_buffer.clear();
        }

        self.0.flush()
    }
}

impl<'a> FlatSamples<'a> {
    fn len(&self) -> usize {
        match *self {
            FlatSamples::U8(arr) => arr.len(),
            FlatSamples::U16(arr) => arr.len(),
        }
    }

    fn all_smaller(&self, max_val: u32) -> bool {
        match *self {
            FlatSamples::U8(arr) => arr.iter().all(|&val| u32::from(val) <= max_val),
            FlatSamples::U16(arr) => arr.iter().all(|&val| u32::from(val) <= max_val),
        }
    }

    fn encoding_for(&self, header: &HeaderRecord) -> TupleEncoding<'a> {
        match *header {
            HeaderRecord::Bitmap(BitmapHeader {
                encoding: SampleEncoding::Binary,
                width,
                ..
            }) => TupleEncoding::PbmBits {
                samples: *self,
                width,
            },

            HeaderRecord::Bitmap(BitmapHeader {
                encoding: SampleEncoding::Ascii,
                ..
            }) => TupleEncoding::Ascii { samples: *self },

            HeaderRecord::Arbitrary(_) => TupleEncoding::Bytes { samples: *self },

            HeaderRecord::Graymap(GraymapHeader {
                encoding: SampleEncoding::Ascii,
                ..
            })
            | HeaderRecord::Pixmap(PixmapHeader {
                encoding: SampleEncoding::Ascii,
                ..
            }) => TupleEncoding::Ascii { samples: *self },

            HeaderRecord::Graymap(GraymapHeader {
                encoding: SampleEncoding::Binary,
                ..
            })
            | HeaderRecord::Pixmap(PixmapHeader {
                encoding: SampleEncoding::Binary,
                ..
            }) => TupleEncoding::Bytes { samples: *self },
        }
    }
}

impl<'a> From<&'a [u8]> for FlatSamples<'a> {
    fn from(samples: &'a [u8]) -> Self {
        FlatSamples::U8(samples)
    }
}

impl<'a> From<&'a [u16]> for FlatSamples<'a> {
    fn from(samples: &'a [u16]) -> Self {
        FlatSamples::U16(samples)
    }
}

impl TupleEncoding<'_> {
    fn write_image(&self, writer: &mut dyn Write) -> ImageResult<()> {
        match *self {
            TupleEncoding::PbmBits {
                samples: FlatSamples::U8(samples),
                width,
            } => SampleWriter(writer)
                .write_pbm_bits(samples, width)
                .map_err(ImageError::IoError),
            TupleEncoding::PbmBits {
                samples: FlatSamples::U16(samples),
                width,
            } => SampleWriter(writer)
                .write_pbm_bits(samples, width)
                .map_err(ImageError::IoError),

            TupleEncoding::Bytes {
                samples: FlatSamples::U8(samples),
            } => writer.write_all(samples).map_err(ImageError::IoError),
            TupleEncoding::Bytes {
                samples: FlatSamples::U16(samples),
            } => samples.iter().try_for_each(|&sample| {
                writer
                    .write_u16::<BigEndian>(sample)
                    .map_err(ImageError::IoError)
            }),

            TupleEncoding::Ascii {
                samples: FlatSamples::U8(samples),
            } => SampleWriter(writer)
                .write_samples_ascii(samples.iter())
                .map_err(ImageError::IoError),
            TupleEncoding::Ascii {
                samples: FlatSamples::U16(samples),
            } => SampleWriter(writer)
                .write_samples_ascii(samples.iter())
                .map_err(ImageError::IoError),
        }
    }
}

#[test]
fn pbm_allows_black() {
    let imgbuf = crate::DynamicImage::new_luma8(50, 50);

    let mut buffer = vec![];
    let encoder =
        PnmEncoder::new(&mut buffer).with_subtype(PnmSubtype::Bitmap(SampleEncoding::Ascii));

    imgbuf
        .write_with_encoder(encoder)
        .expect("all-zeroes is a black image");
}

#[test]
fn pbm_allows_white() {
    let imgbuf =
        crate::DynamicImage::ImageLuma8(crate::ImageBuffer::from_pixel(50, 50, crate::Luma([1])));

    let mut buffer = vec![];
    let encoder =
        PnmEncoder::new(&mut buffer).with_subtype(PnmSubtype::Bitmap(SampleEncoding::Ascii));

    imgbuf
        .write_with_encoder(encoder)
        .expect("all-zeroes is a white image");
}

#[test]
fn pbm_verifies_pixels() {
    let imgbuf =
        crate::DynamicImage::ImageLuma8(crate::ImageBuffer::from_pixel(50, 50, crate::Luma([255])));

    let mut buffer = vec![];
    let encoder =
        PnmEncoder::new(&mut buffer).with_subtype(PnmSubtype::Bitmap(SampleEncoding::Ascii));

    imgbuf
        .write_with_encoder(encoder)
        .expect_err("failed to catch violating samples");
}
