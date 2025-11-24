use std::error;
use std::fmt::{self, Display};
use std::io::{self, Read};
use std::mem::size_of;
use std::num::ParseIntError;
use std::str;

use super::{ArbitraryHeader, ArbitraryTuplType, BitmapHeader, GraymapHeader, PixmapHeader};
use super::{HeaderRecord, PnmHeader, PnmSubtype, SampleEncoding};
use crate::color::{ColorType, ExtendedColorType};
use crate::error::{
    DecodingError, ImageError, ImageResult, UnsupportedError, UnsupportedErrorKind,
};
use crate::io::ReadExt;
use crate::{utils, ImageDecoder, ImageFormat};

use byteorder_lite::{BigEndian, ByteOrder, NativeEndian};

/// All errors that can occur when attempting to parse a PNM
#[derive(Debug, Clone)]
enum DecoderError {
    /// PNM's "P[123456]" signature wrong or missing
    PnmMagicInvalid([u8; 2]),
    /// Couldn't parse the specified string as an integer from the specified source
    UnparsableValue(ErrorDataSource, String, ParseIntError),

    /// More than the exactly one allowed plane specified by the format
    NonAsciiByteInHeader(u8),
    /// The PAM header contained a non-ASCII byte
    NonAsciiLineInPamHeader,
    /// Couldn't parse an integer: expected but did not get an ASCII digit
    InvalidDigit(ErrorDataSource),

    /// The byte after the P7 magic was not 0x0A NEWLINE
    NotNewlineAfterP7Magic(u8),
    /// The PNM header had too few lines
    UnexpectedPnmHeaderEnd,

    /// The specified line was specified twice
    HeaderLineDuplicated(PnmHeaderLine),
    /// The line with the specified ID was not understood
    HeaderLineUnknown(String),
    /// At least one of the required lines were missing from the header (are `None` here)
    ///
    /// Same names as [`PnmHeaderLine`](enum.PnmHeaderLine.html)
    #[allow(missing_docs)]
    HeaderLineMissing {
        height: Option<u32>,
        width: Option<u32>,
        depth: Option<u32>,
        maxval: Option<u32>,
    },

    /// Not enough data was provided to the Decoder to decode the image
    InputTooShort,
    /// Sample raster contained unexpected byte
    UnexpectedByteInRaster(u8),
    /// Specified sample was out of bounds (e.g. >1 in B&W)
    SampleOutOfBounds(u8),
    /// The image's maxval is zero
    MaxvalZero,
    /// The image's maxval exceeds 0xFFFF
    MaxvalTooBig(u32),

    /// The specified tuple type supports restricted depths and maxvals, those restrictions were not met
    InvalidDepthOrMaxval {
        tuple_type: ArbitraryTuplType,
        depth: u32,
        maxval: u32,
    },
    /// The specified tuple type supports restricted depths, those restrictions were not met
    InvalidDepth {
        tuple_type: ArbitraryTuplType,
        depth: u32,
    },
    /// The tuple type was not recognised by the parser
    TupleTypeUnrecognised,

    /// Overflowed the specified value when parsing
    Overflow,
}

impl Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecoderError::PnmMagicInvalid(magic) => f.write_fmt(format_args!(
                "Expected magic constant for PNM: P1..P7, got [{:#04X?}, {:#04X?}]",
                magic[0], magic[1]
            )),
            DecoderError::UnparsableValue(src, data, err) => {
                f.write_fmt(format_args!("Error parsing {data:?} as {src}: {err}"))
            }

            DecoderError::NonAsciiByteInHeader(c) => {
                f.write_fmt(format_args!("Non-ASCII character {c:#04X?} in header"))
            }
            DecoderError::NonAsciiLineInPamHeader => f.write_str("Non-ASCII line in PAM header"),
            DecoderError::InvalidDigit(src) => {
                f.write_fmt(format_args!("Non-ASCII-digit character when parsing number in {src}"))
            }

            DecoderError::NotNewlineAfterP7Magic(c) => f.write_fmt(format_args!(
                "Expected newline after P7 magic, got {c:#04X?}"
            )),
            DecoderError::UnexpectedPnmHeaderEnd => f.write_str("Unexpected end of PNM header"),

            DecoderError::HeaderLineDuplicated(line) => {
                f.write_fmt(format_args!("Duplicate {line} line"))
            }
            DecoderError::HeaderLineUnknown(identifier) => f.write_fmt(format_args!(
                "Unknown header line with identifier {identifier:?}"
            )),
            DecoderError::HeaderLineMissing {
                height,
                width,
                depth,
                maxval,
            } => f.write_fmt(format_args!(
                "Missing header line: have height={height:?}, width={width:?}, depth={depth:?}, maxval={maxval:?}"
            )),

            DecoderError::InputTooShort => {
                f.write_str("Not enough data was provided to the Decoder to decode the image")
            }
            DecoderError::UnexpectedByteInRaster(c) => f.write_fmt(format_args!(
                "Unexpected character {c:#04X?} within sample raster"
            )),
            DecoderError::SampleOutOfBounds(val) => {
                f.write_fmt(format_args!("Sample value {val} outside of bounds"))
            }
            DecoderError::MaxvalZero => f.write_str("Image MAXVAL is zero"),
            DecoderError::MaxvalTooBig(maxval) => {
                f.write_fmt(format_args!("Image MAXVAL exceeds {}: {}", 0xFFFF, maxval))
            }

            DecoderError::InvalidDepthOrMaxval {
                tuple_type,
                depth,
                maxval,
            } => f.write_fmt(format_args!(
                "Invalid depth ({}) or maxval ({}) for tuple type {}",
                depth,
                maxval,
                tuple_type.name()
            )),
            DecoderError::InvalidDepth { tuple_type, depth } => f.write_fmt(format_args!(
                "Invalid depth ({}) for tuple type {}",
                depth,
                tuple_type.name()
            )),
            DecoderError::TupleTypeUnrecognised => f.write_str("Tuple type not recognized"),
            DecoderError::Overflow => f.write_str("Overflow when parsing value"),
        }
    }
}

/// Note: should `pnm` be extracted into a separate crate,
/// this will need to be hidden until that crate hits version `1.0`.
impl From<DecoderError> for ImageError {
    fn from(e: DecoderError) -> ImageError {
        ImageError::Decoding(DecodingError::new(ImageFormat::Pnm.into(), e))
    }
}

impl error::Error for DecoderError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            DecoderError::UnparsableValue(_, _, err) => Some(err),
            _ => None,
        }
    }
}

/// Single-value lines in a PNM header
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum PnmHeaderLine {
    /// "HEIGHT"
    Height,
    /// "WIDTH"
    Width,
    /// "DEPTH"
    Depth,
    /// "MAXVAL", a.k.a. `maxwhite`
    Maxval,
}

impl Display for PnmHeaderLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PnmHeaderLine::Height => "HEIGHT",
            PnmHeaderLine::Width => "WIDTH",
            PnmHeaderLine::Depth => "DEPTH",
            PnmHeaderLine::Maxval => "MAXVAL",
        })
    }
}

/// Single-value lines in a PNM header
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum ErrorDataSource {
    /// One of the header lines
    Line(PnmHeaderLine),
    /// Value in the preamble
    Preamble,
    /// Sample/pixel data
    Sample,
}

impl Display for ErrorDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorDataSource::Line(l) => l.fmt(f),
            ErrorDataSource::Preamble => f.write_str("number in preamble"),
            ErrorDataSource::Sample => f.write_str("sample"),
        }
    }
}

/// Dynamic representation, represents all decodable (sample, depth) combinations.
#[derive(Clone, Copy)]
enum TupleType {
    PbmBit,
    BWBit,
    BWAlphaBit,
    GrayU8,
    GrayAlphaU8,
    GrayU16,
    GrayAlphaU16,
    RGBU8,
    RGBAlphaU8,
    RGBU16,
    RGBAlphaU16,
}

trait Sample {
    type Representation;

    /// Representation size in bytes
    fn sample_size() -> u32 {
        size_of::<Self::Representation>() as u32
    }
    fn bytelen(width: u32, height: u32, samples: u32) -> ImageResult<usize> {
        Ok((width * height * samples * Self::sample_size()) as usize)
    }
    fn from_bytes(bytes: &[u8], row_size: usize, output_buf: &mut [u8]) -> ImageResult<()>;
    fn from_ascii(reader: &mut dyn Read, output_buf: &mut [u8]) -> ImageResult<()>;
}

struct U8;
struct U16;
struct PbmBit;
struct BWBit;

trait DecodableImageHeader {
    fn tuple_type(&self) -> ImageResult<TupleType>;
}

/// PNM decoder
pub struct PnmDecoder<R> {
    reader: R,
    header: PnmHeader,
    tuple: TupleType,
}

impl<R: Read> PnmDecoder<R> {
    /// Create a new decoder that decodes from the stream ```read```
    pub fn new(mut buffered_read: R) -> ImageResult<PnmDecoder<R>> {
        let magic = buffered_read.read_magic_constant()?;

        let subtype = match magic {
            [b'P', b'1'] => PnmSubtype::Bitmap(SampleEncoding::Ascii),
            [b'P', b'2'] => PnmSubtype::Graymap(SampleEncoding::Ascii),
            [b'P', b'3'] => PnmSubtype::Pixmap(SampleEncoding::Ascii),
            [b'P', b'4'] => PnmSubtype::Bitmap(SampleEncoding::Binary),
            [b'P', b'5'] => PnmSubtype::Graymap(SampleEncoding::Binary),
            [b'P', b'6'] => PnmSubtype::Pixmap(SampleEncoding::Binary),
            [b'P', b'7'] => PnmSubtype::ArbitraryMap,
            _ => return Err(DecoderError::PnmMagicInvalid(magic).into()),
        };

        let decoder = match subtype {
            PnmSubtype::Bitmap(enc) => PnmDecoder::read_bitmap_header(buffered_read, enc),
            PnmSubtype::Graymap(enc) => PnmDecoder::read_graymap_header(buffered_read, enc),
            PnmSubtype::Pixmap(enc) => PnmDecoder::read_pixmap_header(buffered_read, enc),
            PnmSubtype::ArbitraryMap => PnmDecoder::read_arbitrary_header(buffered_read),
        }?;

        if utils::check_dimension_overflow(
            decoder.dimensions().0,
            decoder.dimensions().1,
            decoder.color_type().bytes_per_pixel(),
        ) {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Pnm.into(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "Image dimensions ({}x{}) are too large",
                        decoder.dimensions().0,
                        decoder.dimensions().1
                    )),
                ),
            ));
        }

        Ok(decoder)
    }

    /// Get the header of the decoded image.
    pub fn header(&self) -> &PnmHeader {
        &self.header
    }

    /// Extract the reader and header after an image has been read.
    pub fn into_inner(self) -> (R, PnmHeader) {
        (self.reader, self.header)
    }

    fn read_bitmap_header(mut reader: R, encoding: SampleEncoding) -> ImageResult<PnmDecoder<R>> {
        let header = reader.read_bitmap_header(encoding)?;
        Ok(PnmDecoder {
            reader,
            tuple: TupleType::PbmBit,
            header: PnmHeader {
                decoded: HeaderRecord::Bitmap(header),
                encoded: None,
            },
        })
    }

    fn read_graymap_header(mut reader: R, encoding: SampleEncoding) -> ImageResult<PnmDecoder<R>> {
        let header = reader.read_graymap_header(encoding)?;
        let tuple_type = header.tuple_type()?;
        Ok(PnmDecoder {
            reader,
            tuple: tuple_type,
            header: PnmHeader {
                decoded: HeaderRecord::Graymap(header),
                encoded: None,
            },
        })
    }

    fn read_pixmap_header(mut reader: R, encoding: SampleEncoding) -> ImageResult<PnmDecoder<R>> {
        let header = reader.read_pixmap_header(encoding)?;
        let tuple_type = header.tuple_type()?;
        Ok(PnmDecoder {
            reader,
            tuple: tuple_type,
            header: PnmHeader {
                decoded: HeaderRecord::Pixmap(header),
                encoded: None,
            },
        })
    }

    fn read_arbitrary_header(mut reader: R) -> ImageResult<PnmDecoder<R>> {
        let header = reader.read_arbitrary_header()?;
        let tuple_type = header.tuple_type()?;
        Ok(PnmDecoder {
            reader,
            tuple: tuple_type,
            header: PnmHeader {
                decoded: HeaderRecord::Arbitrary(header),
                encoded: None,
            },
        })
    }
}

trait HeaderReader: Read {
    /// Reads the two magic constant bytes
    fn read_magic_constant(&mut self) -> ImageResult<[u8; 2]> {
        let mut magic: [u8; 2] = [0, 0];
        self.read_exact(&mut magic)?;
        Ok(magic)
    }

    /// Reads a string as well as a single whitespace after it, ignoring comments
    fn read_next_string(&mut self) -> ImageResult<String> {
        let mut bytes = Vec::new();

        // pair input bytes with a bool mask to remove comments
        #[allow(clippy::unbuffered_bytes)]
        let mark_comments = self.bytes().scan(true, |partof, read| {
            let byte = match read {
                Err(err) => return Some((*partof, Err(err))),
                Ok(byte) => byte,
            };
            let cur_enabled = *partof && byte != b'#';
            let next_enabled = cur_enabled || (byte == b'\r' || byte == b'\n');
            *partof = next_enabled;
            Some((cur_enabled, Ok(byte)))
        });

        for (_, byte) in mark_comments.filter(|e| e.0) {
            match byte {
                Ok(b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r' | b' ') => {
                    if !bytes.is_empty() {
                        break; // We're done as we already have some content
                    }
                }
                Ok(byte) if !byte.is_ascii() => {
                    return Err(DecoderError::NonAsciiByteInHeader(byte).into())
                }
                Ok(byte) => {
                    bytes.push(byte);
                }
                Err(_) => break,
            }
        }

        if bytes.is_empty() {
            return Err(ImageError::IoError(io::ErrorKind::UnexpectedEof.into()));
        }

        if !bytes.as_slice().is_ascii() {
            // We have only filled the buffer with characters for which `byte.is_ascii()` holds.
            unreachable!("Non-ASCII character should have returned sooner")
        }

        let string = String::from_utf8(bytes)
            // We checked the precondition ourselves a few lines before, `bytes.as_slice().is_ascii()`.
            .unwrap_or_else(|_| unreachable!("Only ASCII characters should be decoded"));

        Ok(string)
    }

    fn read_next_line(&mut self) -> ImageResult<String> {
        let mut buffer = Vec::new();
        loop {
            let mut byte = [0];
            if self.read(&mut byte)? == 0 || byte[0] == b'\n' {
                break;
            }
            buffer.push(byte[0]);
        }

        String::from_utf8(buffer)
            .map_err(|e| ImageError::Decoding(DecodingError::new(ImageFormat::Pnm.into(), e)))
    }

    fn read_next_u32(&mut self) -> ImageResult<u32> {
        let s = self.read_next_string()?;
        s.parse::<u32>()
            .map_err(|err| DecoderError::UnparsableValue(ErrorDataSource::Preamble, s, err).into())
    }

    fn read_bitmap_header(&mut self, encoding: SampleEncoding) -> ImageResult<BitmapHeader> {
        let width = self.read_next_u32()?;
        let height = self.read_next_u32()?;
        Ok(BitmapHeader {
            encoding,
            height,
            width,
        })
    }

    fn read_graymap_header(&mut self, encoding: SampleEncoding) -> ImageResult<GraymapHeader> {
        self.read_pixmap_header(encoding).map(
            |PixmapHeader {
                 encoding,
                 width,
                 height,
                 maxval,
             }| GraymapHeader {
                encoding,
                width,
                height,
                maxwhite: maxval,
            },
        )
    }

    fn read_pixmap_header(&mut self, encoding: SampleEncoding) -> ImageResult<PixmapHeader> {
        let width = self.read_next_u32()?;
        let height = self.read_next_u32()?;
        let maxval = self.read_next_u32()?;
        Ok(PixmapHeader {
            encoding,
            height,
            width,
            maxval,
        })
    }

    fn read_arbitrary_header(&mut self) -> ImageResult<ArbitraryHeader> {
        fn parse_single_value_line(
            line_val: &mut Option<u32>,
            rest: &str,
            line: PnmHeaderLine,
        ) -> ImageResult<()> {
            if line_val.is_some() {
                Err(DecoderError::HeaderLineDuplicated(line).into())
            } else {
                let v = rest.trim().parse().map_err(|err| {
                    DecoderError::UnparsableValue(ErrorDataSource::Line(line), rest.to_owned(), err)
                })?;
                *line_val = Some(v);
                Ok(())
            }
        }

        #[allow(clippy::unbuffered_bytes)]
        match self.bytes().next() {
            None => return Err(ImageError::IoError(io::ErrorKind::UnexpectedEof.into())),
            Some(Err(io)) => return Err(ImageError::IoError(io)),
            Some(Ok(b'\n')) => (),
            Some(Ok(c)) => return Err(DecoderError::NotNewlineAfterP7Magic(c).into()),
        }

        let mut line;
        let mut height: Option<u32> = None;
        let mut width: Option<u32> = None;
        let mut depth: Option<u32> = None;
        let mut maxval: Option<u32> = None;
        let mut tupltype: Option<String> = None;
        loop {
            line = self.read_next_line()?;
            if line.is_empty() {
                return Err(DecoderError::UnexpectedPnmHeaderEnd.into());
            }
            if line.as_bytes()[0] == b'#' {
                continue;
            }
            if !line.is_ascii() {
                return Err(DecoderError::NonAsciiLineInPamHeader.into());
            }
            #[allow(deprecated)]
            let (identifier, rest) = line
                .trim_left()
                .split_at(line.find(char::is_whitespace).unwrap_or(line.len()));
            match identifier {
                "ENDHDR" => break,
                "HEIGHT" => parse_single_value_line(&mut height, rest, PnmHeaderLine::Height)?,
                "WIDTH" => parse_single_value_line(&mut width, rest, PnmHeaderLine::Width)?,
                "DEPTH" => parse_single_value_line(&mut depth, rest, PnmHeaderLine::Depth)?,
                "MAXVAL" => parse_single_value_line(&mut maxval, rest, PnmHeaderLine::Maxval)?,
                "TUPLTYPE" => {
                    let identifier = rest.trim();
                    if tupltype.is_some() {
                        let appended = tupltype.take().map(|mut v| {
                            v.push(' ');
                            v.push_str(identifier);
                            v
                        });
                        tupltype = appended;
                    } else {
                        tupltype = Some(identifier.to_string());
                    }
                }
                _ => return Err(DecoderError::HeaderLineUnknown(identifier.to_string()).into()),
            }
        }

        let (Some(h), Some(w), Some(d), Some(m)) = (height, width, depth, maxval) else {
            return Err(DecoderError::HeaderLineMissing {
                height,
                width,
                depth,
                maxval,
            }
            .into());
        };

        let tupltype = match tupltype {
            None => None,
            Some(ref t) if t == "BLACKANDWHITE" => Some(ArbitraryTuplType::BlackAndWhite),
            Some(ref t) if t == "BLACKANDWHITE_ALPHA" => {
                Some(ArbitraryTuplType::BlackAndWhiteAlpha)
            }
            Some(ref t) if t == "GRAYSCALE" => Some(ArbitraryTuplType::Grayscale),
            Some(ref t) if t == "GRAYSCALE_ALPHA" => Some(ArbitraryTuplType::GrayscaleAlpha),
            Some(ref t) if t == "RGB" => Some(ArbitraryTuplType::RGB),
            Some(ref t) if t == "RGB_ALPHA" => Some(ArbitraryTuplType::RGBAlpha),
            Some(other) => Some(ArbitraryTuplType::Custom(other)),
        };

        Ok(ArbitraryHeader {
            height: h,
            width: w,
            depth: d,
            maxval: m,
            tupltype,
        })
    }
}

impl<R> HeaderReader for R where R: Read {}

impl<R: Read> ImageDecoder for PnmDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.header.width(), self.header.height())
    }

    fn color_type(&self) -> ColorType {
        match self.tuple {
            TupleType::PbmBit => ColorType::L8,
            TupleType::BWBit => ColorType::L8,
            TupleType::BWAlphaBit => ColorType::La8,
            TupleType::GrayU8 => ColorType::L8,
            TupleType::GrayAlphaU8 => ColorType::La8,
            TupleType::GrayU16 => ColorType::L16,
            TupleType::GrayAlphaU16 => ColorType::La16,
            TupleType::RGBU8 => ColorType::Rgb8,
            TupleType::RGBAlphaU8 => ColorType::Rgba8,
            TupleType::RGBU16 => ColorType::Rgb16,
            TupleType::RGBAlphaU16 => ColorType::Rgba16,
        }
    }

    fn original_color_type(&self) -> ExtendedColorType {
        match self.tuple {
            TupleType::PbmBit => ExtendedColorType::L1,
            TupleType::BWBit => ExtendedColorType::L1,
            TupleType::BWAlphaBit => ExtendedColorType::La1,
            TupleType::GrayU8 => ExtendedColorType::L8,
            TupleType::GrayAlphaU8 => ExtendedColorType::La8,
            TupleType::GrayU16 => ExtendedColorType::L16,
            TupleType::GrayAlphaU16 => ExtendedColorType::La16,
            TupleType::RGBU8 => ExtendedColorType::Rgb8,
            TupleType::RGBAlphaU8 => ExtendedColorType::Rgba8,
            TupleType::RGBU16 => ExtendedColorType::Rgb16,
            TupleType::RGBAlphaU16 => ExtendedColorType::Rgba16,
        }
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        match self.tuple {
            TupleType::PbmBit => self.read_samples::<PbmBit>(1, buf),
            TupleType::BWBit => self.read_samples::<BWBit>(1, buf),
            TupleType::BWAlphaBit => self.read_samples::<BWBit>(2, buf),
            TupleType::RGBU8 => self.read_samples::<U8>(3, buf),
            TupleType::RGBAlphaU8 => self.read_samples::<U8>(4, buf),
            TupleType::RGBU16 => self.read_samples::<U16>(3, buf),
            TupleType::RGBAlphaU16 => self.read_samples::<U16>(4, buf),
            TupleType::GrayU8 => self.read_samples::<U8>(1, buf),
            TupleType::GrayAlphaU8 => self.read_samples::<U8>(2, buf),
            TupleType::GrayU16 => self.read_samples::<U16>(1, buf),
            TupleType::GrayAlphaU16 => self.read_samples::<U16>(2, buf),
        }
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

impl<R: Read> PnmDecoder<R> {
    fn read_samples<S: Sample>(&mut self, components: u32, buf: &mut [u8]) -> ImageResult<()> {
        match self.subtype().sample_encoding() {
            SampleEncoding::Binary => {
                let width = self.header.width();
                let height = self.header.height();
                let bytecount = S::bytelen(width, height, components)?;

                let mut bytes = vec![];
                self.reader.read_exact_vec(&mut bytes, bytecount)?;

                let width: usize = width.try_into().map_err(|_| DecoderError::Overflow)?;
                let components: usize =
                    components.try_into().map_err(|_| DecoderError::Overflow)?;
                let row_size = width
                    .checked_mul(components)
                    .ok_or(DecoderError::Overflow)?;

                S::from_bytes(&bytes, row_size, buf)?;
            }
            SampleEncoding::Ascii => {
                self.read_ascii::<S>(buf)?;
            }
        }

        // Scale samples if 8bit or 16bit is not saturated
        let current_sample_max = self.header.maximal_sample();
        let target_sample_max = 256_u32.pow(S::sample_size()) - 1;

        if current_sample_max != target_sample_max {
            let factor = target_sample_max as f32 / current_sample_max as f32;

            if S::sample_size() == 1 {
                for v in buf.iter_mut() {
                    *v = (f32::from(*v) * factor).round() as u8;
                }
            } else if S::sample_size() == 2 {
                for chunk in buf.chunks_exact_mut(2) {
                    let v = NativeEndian::read_u16(chunk);
                    NativeEndian::write_u16(chunk, (f32::from(v) * factor).round() as u16);
                }
            }
        }

        Ok(())
    }

    fn read_ascii<Basic: Sample>(&mut self, output_buf: &mut [u8]) -> ImageResult<()> {
        Basic::from_ascii(&mut self.reader, output_buf)
    }

    /// Get the pnm subtype, depending on the magic constant contained in the header
    pub fn subtype(&self) -> PnmSubtype {
        self.header.subtype()
    }
}

fn read_separated_ascii<T: TryFrom<u16>>(reader: &mut dyn Read) -> ImageResult<T> {
    let is_separator = |v: &u8| matches!(*v, b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r' | b' ');

    let mut v: u16 = 0;
    let mut had_any = false;
    #[allow(clippy::unbuffered_bytes)]
    for rc in reader
        .bytes()
        .skip_while(|v| v.as_ref().ok().is_some_and(is_separator))
        .take_while(|v| v.as_ref().ok().is_some_and(|c| !is_separator(c)))
    {
        let c = rc?;
        let digit = match c {
            b'0'..=b'9' => u16::from(c - b'0'),
            _ => return Err(DecoderError::InvalidDigit(ErrorDataSource::Sample).into()),
        };
        v = v.checked_mul(10).ok_or(DecoderError::Overflow)?;
        v = v.checked_add(digit).ok_or(DecoderError::Overflow)?;
        had_any = true;
    }

    if !had_any {
        return Err(DecoderError::InputTooShort.into());
    }

    Ok(T::try_from(v).or(Err(DecoderError::Overflow))?)
}

impl Sample for U8 {
    type Representation = u8;

    fn from_bytes(bytes: &[u8], _row_size: usize, output_buf: &mut [u8]) -> ImageResult<()> {
        output_buf.copy_from_slice(bytes);
        Ok(())
    }

    fn from_ascii(reader: &mut dyn Read, output_buf: &mut [u8]) -> ImageResult<()> {
        for b in output_buf {
            *b = read_separated_ascii(reader)?;
        }
        Ok(())
    }
}

impl Sample for U16 {
    type Representation = u16;

    fn from_bytes(bytes: &[u8], _row_size: usize, output_buf: &mut [u8]) -> ImageResult<()> {
        output_buf.copy_from_slice(bytes);
        for chunk in output_buf.chunks_exact_mut(2) {
            let v = BigEndian::read_u16(chunk);
            NativeEndian::write_u16(chunk, v);
        }
        Ok(())
    }

    fn from_ascii(reader: &mut dyn Read, output_buf: &mut [u8]) -> ImageResult<()> {
        for chunk in output_buf.chunks_exact_mut(2) {
            let v = read_separated_ascii::<u16>(reader)?;
            NativeEndian::write_u16(chunk, v);
        }
        Ok(())
    }
}

// The image is encoded in rows of bits, high order bits first. Any bits beyond the row bits should
// be ignored. Also, contrary to rgb, black pixels are encoded as a 1 while white is 0. This will
// need to be reversed for the grayscale output.
impl Sample for PbmBit {
    type Representation = u8;

    fn bytelen(width: u32, height: u32, samples: u32) -> ImageResult<usize> {
        let count = width * samples;
        let linelen = (count / 8) + u32::from((count % 8) != 0);
        Ok((linelen * height) as usize)
    }

    fn from_bytes(bytes: &[u8], row_size: usize, output_buf: &mut [u8]) -> ImageResult<()> {
        let mut expanded = utils::expand_bits(1, row_size.try_into().unwrap(), bytes);
        for b in &mut expanded {
            *b = !*b;
        }
        output_buf.copy_from_slice(&expanded);
        Ok(())
    }

    fn from_ascii(reader: &mut dyn Read, output_buf: &mut [u8]) -> ImageResult<()> {
        #[allow(clippy::unbuffered_bytes)]
        let mut bytes = reader.bytes();
        for b in output_buf {
            loop {
                let byte = bytes
                    .next()
                    .ok_or_else::<ImageError, _>(|| DecoderError::InputTooShort.into())??;
                match byte {
                    b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r' | b' ' => continue,
                    b'0' => *b = 255,
                    b'1' => *b = 0,
                    c => return Err(DecoderError::UnexpectedByteInRaster(c).into()),
                }
                break;
            }
        }

        Ok(())
    }
}

// Encoded just like a normal U8 but we check the values.
impl Sample for BWBit {
    type Representation = u8;

    fn from_bytes(bytes: &[u8], row_size: usize, output_buf: &mut [u8]) -> ImageResult<()> {
        U8::from_bytes(bytes, row_size, output_buf)?;
        if let Some(val) = output_buf.iter().find(|&val| *val > 1) {
            return Err(DecoderError::SampleOutOfBounds(*val).into());
        }
        Ok(())
    }

    fn from_ascii(_reader: &mut dyn Read, _output_buf: &mut [u8]) -> ImageResult<()> {
        unreachable!("BW bits from anymaps are never encoded as ASCII")
    }
}

impl DecodableImageHeader for BitmapHeader {
    fn tuple_type(&self) -> ImageResult<TupleType> {
        Ok(TupleType::PbmBit)
    }
}

impl DecodableImageHeader for GraymapHeader {
    fn tuple_type(&self) -> ImageResult<TupleType> {
        match self.maxwhite {
            0 => Err(DecoderError::MaxvalZero.into()),
            v if v <= 0xFF => Ok(TupleType::GrayU8),
            v if v <= 0xFFFF => Ok(TupleType::GrayU16),
            _ => Err(DecoderError::MaxvalTooBig(self.maxwhite).into()),
        }
    }
}

impl DecodableImageHeader for PixmapHeader {
    fn tuple_type(&self) -> ImageResult<TupleType> {
        match self.maxval {
            0 => Err(DecoderError::MaxvalZero.into()),
            v if v <= 0xFF => Ok(TupleType::RGBU8),
            v if v <= 0xFFFF => Ok(TupleType::RGBU16),
            _ => Err(DecoderError::MaxvalTooBig(self.maxval).into()),
        }
    }
}

impl DecodableImageHeader for ArbitraryHeader {
    fn tuple_type(&self) -> ImageResult<TupleType> {
        match self.tupltype {
            _ if self.maxval == 0 => Err(DecoderError::MaxvalZero.into()),
            None if self.depth == 1 => Ok(TupleType::GrayU8),
            None if self.depth == 2 => Ok(TupleType::GrayAlphaU8),
            None if self.depth == 3 => Ok(TupleType::RGBU8),
            None if self.depth == 4 => Ok(TupleType::RGBAlphaU8),

            Some(ArbitraryTuplType::BlackAndWhite) if self.maxval == 1 && self.depth == 1 => {
                Ok(TupleType::BWBit)
            }
            Some(ArbitraryTuplType::BlackAndWhite) => Err(DecoderError::InvalidDepthOrMaxval {
                tuple_type: ArbitraryTuplType::BlackAndWhite,
                maxval: self.maxval,
                depth: self.depth,
            }
            .into()),

            Some(ArbitraryTuplType::Grayscale) if self.depth == 1 && self.maxval <= 0xFF => {
                Ok(TupleType::GrayU8)
            }
            Some(ArbitraryTuplType::Grayscale) if self.depth <= 1 && self.maxval <= 0xFFFF => {
                Ok(TupleType::GrayU16)
            }
            Some(ArbitraryTuplType::Grayscale) => Err(DecoderError::InvalidDepthOrMaxval {
                tuple_type: ArbitraryTuplType::Grayscale,
                maxval: self.maxval,
                depth: self.depth,
            }
            .into()),

            Some(ArbitraryTuplType::RGB) if self.depth == 3 && self.maxval <= 0xFF => {
                Ok(TupleType::RGBU8)
            }
            Some(ArbitraryTuplType::RGB) if self.depth == 3 && self.maxval <= 0xFFFF => {
                Ok(TupleType::RGBU16)
            }
            Some(ArbitraryTuplType::RGB) => Err(DecoderError::InvalidDepth {
                tuple_type: ArbitraryTuplType::RGB,
                depth: self.depth,
            }
            .into()),

            Some(ArbitraryTuplType::BlackAndWhiteAlpha) if self.depth == 2 && self.maxval == 1 => {
                Ok(TupleType::BWAlphaBit)
            }
            Some(ArbitraryTuplType::BlackAndWhiteAlpha) => {
                Err(DecoderError::InvalidDepthOrMaxval {
                    tuple_type: ArbitraryTuplType::BlackAndWhiteAlpha,
                    maxval: self.maxval,
                    depth: self.depth,
                }
                .into())
            }

            Some(ArbitraryTuplType::GrayscaleAlpha) if self.depth == 2 && self.maxval <= 0xFF => {
                Ok(TupleType::GrayAlphaU8)
            }
            Some(ArbitraryTuplType::GrayscaleAlpha) if self.depth == 2 && self.maxval <= 0xFFFF => {
                Ok(TupleType::GrayAlphaU16)
            }
            Some(ArbitraryTuplType::GrayscaleAlpha) => Err(DecoderError::InvalidDepth {
                tuple_type: ArbitraryTuplType::GrayscaleAlpha,
                depth: self.depth,
            }
            .into()),

            Some(ArbitraryTuplType::RGBAlpha) if self.depth == 4 && self.maxval <= 0xFF => {
                Ok(TupleType::RGBAlphaU8)
            }
            Some(ArbitraryTuplType::RGBAlpha) if self.depth == 4 && self.maxval <= 0xFFFF => {
                Ok(TupleType::RGBAlphaU16)
            }
            Some(ArbitraryTuplType::RGBAlpha) => Err(DecoderError::InvalidDepth {
                tuple_type: ArbitraryTuplType::RGBAlpha,
                depth: self.depth,
            }
            .into()),

            Some(ArbitraryTuplType::Custom(ref custom)) => Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Pnm.into(),
                    UnsupportedErrorKind::GenericFeature(format!("Tuple type {custom:?}")),
                ),
            )),
            None => Err(DecoderError::TupleTypeUnrecognised.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Tests reading of a valid blackandwhite pam
    #[test]
    fn pam_blackandwhite() {
        let pamdata = b"P7
WIDTH 4
HEIGHT 4
DEPTH 1
MAXVAL 1
TUPLTYPE BLACKANDWHITE
# Comment line
ENDHDR
\x01\x00\x00\x01\x01\x00\x00\x01\x01\x00\x00\x01\x01\x00\x00\x01";
        let decoder = PnmDecoder::new(&pamdata[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::L8);
        assert_eq!(decoder.original_color_type(), ExtendedColorType::L1);
        assert_eq!(decoder.dimensions(), (4, 4));
        assert_eq!(decoder.subtype(), PnmSubtype::ArbitraryMap);

        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(
            image,
            vec![
                0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00,
                0x00, 0xFF
            ]
        );
        match PnmDecoder::new(&pamdata[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Arbitrary(ArbitraryHeader {
                            width: 4,
                            height: 4,
                            maxval: 1,
                            depth: 1,
                            tupltype: Some(ArbitraryTuplType::BlackAndWhite),
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    /// Tests reading of a valid blackandwhite_alpha pam
    #[test]
    fn pam_blackandwhite_alpha() {
        let pamdata = b"P7
WIDTH 2
HEIGHT 2
DEPTH 2
MAXVAL 1
TUPLTYPE BLACKANDWHITE_ALPHA
# Comment line
ENDHDR
\x01\x00\x00\x01\x01\x00\x00\x01";
        let decoder = PnmDecoder::new(&pamdata[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::La8);
        assert_eq!(decoder.original_color_type(), ExtendedColorType::La1);
        assert_eq!(decoder.dimensions(), (2, 2));
        assert_eq!(decoder.subtype(), PnmSubtype::ArbitraryMap);

        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(image, vec![0xFF, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF,]);
        match PnmDecoder::new(&pamdata[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Arbitrary(ArbitraryHeader {
                            width: 2,
                            height: 2,
                            maxval: 1,
                            depth: 2,
                            tupltype: Some(ArbitraryTuplType::BlackAndWhiteAlpha),
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    /// Tests reading of a valid grayscale pam
    #[test]
    fn pam_grayscale() {
        let pamdata = b"P7
WIDTH 4
HEIGHT 4
DEPTH 1
MAXVAL 255
TUPLTYPE GRAYSCALE
# Comment line
ENDHDR
\xde\xad\xbe\xef\xde\xad\xbe\xef\xde\xad\xbe\xef\xde\xad\xbe\xef";
        let decoder = PnmDecoder::new(&pamdata[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::L8);
        assert_eq!(decoder.dimensions(), (4, 4));
        assert_eq!(decoder.subtype(), PnmSubtype::ArbitraryMap);

        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(
            image,
            vec![
                0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
                0xbe, 0xef
            ]
        );
        match PnmDecoder::new(&pamdata[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Arbitrary(ArbitraryHeader {
                            width: 4,
                            height: 4,
                            depth: 1,
                            maxval: 255,
                            tupltype: Some(ArbitraryTuplType::Grayscale),
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    /// Tests reading of a valid grayscale_alpha pam
    #[test]
    fn pam_grayscale_alpha() {
        let pamdata = b"P7
HEIGHT 1
WIDTH 2
MAXVAL 65535
DEPTH 2
TUPLTYPE GRAYSCALE_ALPHA
# Comment line
ENDHDR
\xdc\xba\x32\x10\xdc\xba\x32\x10";
        let decoder = PnmDecoder::new(&pamdata[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::La16);
        assert_eq!(decoder.original_color_type(), ExtendedColorType::La16);
        assert_eq!(decoder.dimensions(), (2, 1));
        assert_eq!(decoder.subtype(), PnmSubtype::ArbitraryMap);

        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(
            image,
            [
                u16::to_ne_bytes(0xdcba),
                u16::to_ne_bytes(0x3210),
                u16::to_ne_bytes(0xdcba),
                u16::to_ne_bytes(0x3210)
            ]
            .concat()
        );
        match PnmDecoder::new(&pamdata[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Arbitrary(ArbitraryHeader {
                            width: 2,
                            height: 1,
                            maxval: 65535,
                            depth: 2,
                            tupltype: Some(ArbitraryTuplType::GrayscaleAlpha),
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    /// Tests reading of a valid rgb pam
    #[test]
    fn pam_rgb() {
        let pamdata = b"P7
# Comment line
MAXVAL 255
TUPLTYPE RGB
DEPTH 3
WIDTH 2
HEIGHT 2
ENDHDR
\xde\xad\xbe\xef\xde\xad\xbe\xef\xde\xad\xbe\xef";
        let decoder = PnmDecoder::new(&pamdata[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::Rgb8);
        assert_eq!(decoder.dimensions(), (2, 2));
        assert_eq!(decoder.subtype(), PnmSubtype::ArbitraryMap);

        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(
            image,
            vec![0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef]
        );
        match PnmDecoder::new(&pamdata[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Arbitrary(ArbitraryHeader {
                            maxval: 255,
                            tupltype: Some(ArbitraryTuplType::RGB),
                            depth: 3,
                            width: 2,
                            height: 2,
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    /// Tests reading of a valid rgb_alpha pam
    #[test]
    fn pam_rgb_alpha() {
        let pamdata = b"P7
WIDTH 1
HEIGHT 3
DEPTH 4
MAXVAL 15
TUPLTYPE RGB_ALPHA
# Comment line
ENDHDR
\x00\x01\x02\x03\x0a\x0b\x0c\x0d\x05\x06\x07\x08";
        let decoder = PnmDecoder::new(&pamdata[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::Rgba8);
        assert_eq!(decoder.original_color_type(), ExtendedColorType::Rgba8);
        assert_eq!(decoder.dimensions(), (1, 3));
        assert_eq!(decoder.subtype(), PnmSubtype::ArbitraryMap);

        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(image, b"\x00\x11\x22\x33\xaa\xbb\xcc\xdd\x55\x66\x77\x88",);
        match PnmDecoder::new(&pamdata[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Arbitrary(ArbitraryHeader {
                            width: 1,
                            height: 3,
                            maxval: 15,
                            depth: 4,
                            tupltype: Some(ArbitraryTuplType::RGBAlpha),
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    #[test]
    fn pbm_binary() {
        // The data contains two rows of the image (each line is padded to the full byte). For
        // comments on its format, see documentation of `impl SampleType for PbmBit`.
        let pbmbinary = [&b"P4 6 2\n"[..], &[0b0110_1100_u8, 0b1011_0111]].concat();
        let decoder = PnmDecoder::new(&pbmbinary[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::L8);
        assert_eq!(decoder.original_color_type(), ExtendedColorType::L1);
        assert_eq!(decoder.dimensions(), (6, 2));
        assert_eq!(
            decoder.subtype(),
            PnmSubtype::Bitmap(SampleEncoding::Binary)
        );
        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(image, vec![255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 255, 0]);
        match PnmDecoder::new(&pbmbinary[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Bitmap(BitmapHeader {
                            encoding: SampleEncoding::Binary,
                            width: 6,
                            height: 2,
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    /// A previous infinite loop.
    #[test]
    fn pbm_binary_ascii_termination() {
        use std::io::{BufReader, Cursor, Error, ErrorKind, Read, Result};
        struct FailRead(Cursor<&'static [u8]>);

        impl Read for FailRead {
            fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
                match self.0.read(buf) {
                    Ok(n) if n > 0 => Ok(n),
                    _ => Err(Error::new(
                        ErrorKind::BrokenPipe,
                        "Simulated broken pipe error",
                    )),
                }
            }
        }

        let pbmbinary = BufReader::new(FailRead(Cursor::new(b"P1 1 1\n")));

        let decoder = PnmDecoder::new(pbmbinary).unwrap();
        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder
            .read_image(&mut image)
            .expect_err("Image is malformed");
    }

    #[test]
    fn pbm_ascii() {
        // The data contains two rows of the image (each line is padded to the full byte). For
        // comments on its format, see documentation of `impl SampleType for PbmBit`.  Tests all
        // whitespace characters that should be allowed (the 6 characters according to POSIX).
        let pbmbinary = b"P1 6 2\n 0 1 1 0 1 1\n1 0 1 1 0\t\n\x0b\x0c\r1";
        let decoder = PnmDecoder::new(&pbmbinary[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::L8);
        assert_eq!(decoder.original_color_type(), ExtendedColorType::L1);
        assert_eq!(decoder.dimensions(), (6, 2));
        assert_eq!(decoder.subtype(), PnmSubtype::Bitmap(SampleEncoding::Ascii));

        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(image, vec![255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 255, 0]);
        match PnmDecoder::new(&pbmbinary[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Bitmap(BitmapHeader {
                            encoding: SampleEncoding::Ascii,
                            width: 6,
                            height: 2,
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    #[test]
    fn pbm_ascii_nospace() {
        // The data contains two rows of the image (each line is padded to the full byte). Notably,
        // it is completely within specification for the ascii data not to contain separating
        // whitespace for the pbm format or any mix.
        let pbmbinary = b"P1 6 2\n011011101101";
        let decoder = PnmDecoder::new(&pbmbinary[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::L8);
        assert_eq!(decoder.original_color_type(), ExtendedColorType::L1);
        assert_eq!(decoder.dimensions(), (6, 2));
        assert_eq!(decoder.subtype(), PnmSubtype::Bitmap(SampleEncoding::Ascii));

        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(image, vec![255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 255, 0]);
        match PnmDecoder::new(&pbmbinary[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Bitmap(BitmapHeader {
                            encoding: SampleEncoding::Ascii,
                            width: 6,
                            height: 2,
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    #[test]
    fn pgm_binary() {
        // The data contains two rows of the image (each line is padded to the full byte). For
        // comments on its format, see documentation of `impl SampleType for PbmBit`.
        let elements = (0..16).collect::<Vec<_>>();
        let pbmbinary = [&b"P5 4 4 255\n"[..], &elements].concat();
        let decoder = PnmDecoder::new(&pbmbinary[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::L8);
        assert_eq!(decoder.dimensions(), (4, 4));
        assert_eq!(
            decoder.subtype(),
            PnmSubtype::Graymap(SampleEncoding::Binary)
        );
        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(image, elements);
        match PnmDecoder::new(&pbmbinary[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Graymap(GraymapHeader {
                            encoding: SampleEncoding::Binary,
                            width: 4,
                            height: 4,
                            maxwhite: 255,
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    #[test]
    fn pgm_ascii() {
        // The data contains two rows of the image (each line is padded to the full byte). For
        // comments on its format, see documentation of `impl SampleType for PbmBit`.
        let pbmbinary = b"P2 4 4 255\n 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15";
        let decoder = PnmDecoder::new(&pbmbinary[..]).unwrap();
        assert_eq!(decoder.color_type(), ColorType::L8);
        assert_eq!(decoder.dimensions(), (4, 4));
        assert_eq!(
            decoder.subtype(),
            PnmSubtype::Graymap(SampleEncoding::Ascii)
        );
        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(image, (0..16).collect::<Vec<_>>());
        match PnmDecoder::new(&pbmbinary[..]).unwrap().into_inner() {
            (
                _,
                PnmHeader {
                    decoded:
                        HeaderRecord::Graymap(GraymapHeader {
                            encoding: SampleEncoding::Ascii,
                            width: 4,
                            height: 4,
                            maxwhite: 255,
                        }),
                    encoded: _,
                },
            ) => (),
            _ => panic!("Decoded header is incorrect"),
        }
    }

    #[test]
    fn ppm_ascii() {
        let ascii = b"P3 1 1 2000\n0 1000 2000";
        let decoder = PnmDecoder::new(&ascii[..]).unwrap();
        let mut image = vec![0; decoder.total_bytes() as usize];
        decoder.read_image(&mut image).unwrap();
        assert_eq!(
            image,
            [
                0_u16.to_ne_bytes(),
                (u16::MAX / 2 + 1).to_ne_bytes(),
                u16::MAX.to_ne_bytes()
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn dimension_overflow() {
        let pamdata = b"P7
# Comment line
MAXVAL 255
TUPLTYPE RGB
DEPTH 3
WIDTH 4294967295
HEIGHT 4294967295
ENDHDR
\xde\xad\xbe\xef\xde\xad\xbe\xef\xde\xad\xbe\xef";

        assert!(PnmDecoder::new(&pamdata[..]).is_err());
    }

    #[test]
    fn issue_1508() {
        let _ = crate::load_from_memory(b"P391919 16999 1 1 9 919 16999 1 9999 999* 99999 N");
    }

    #[test]
    fn issue_1616_overflow() {
        let data = [
            80, 54, 10, 52, 50, 57, 52, 56, 50, 57, 52, 56, 35, 56, 10, 52, 10, 48, 10, 12, 12, 56,
        ];
        // Validate: we have a header. Note: we might already calculate that this will fail but
        // then we could not return information about the header to the caller.
        let decoder = PnmDecoder::new(&data[..]).unwrap();
        let mut image = vec![0; decoder.total_bytes() as usize];
        let _ = decoder.read_image(&mut image);
    }

    #[test]
    fn data_too_short() {
        let data = b"P3 16 16 1\n";
        let decoder = PnmDecoder::new(&data[..]).unwrap();
        let mut image = vec![0; decoder.total_bytes() as usize];

        let _ = decoder.read_image(&mut image).unwrap_err();
    }
}
