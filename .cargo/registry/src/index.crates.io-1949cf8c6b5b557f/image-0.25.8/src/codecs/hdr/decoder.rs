use std::io::{self, Read};

use std::num::{ParseFloatError, ParseIntError};
use std::{error, fmt};

use crate::error::{
    DecodingError, ImageError, ImageFormatHint, ImageResult, UnsupportedError, UnsupportedErrorKind,
};
use crate::{ColorType, ImageDecoder, ImageFormat, Rgb};

/// Errors that can occur during decoding and parsing of a HDR image
#[derive(Debug, Clone, PartialEq, Eq)]
enum DecoderError {
    /// HDR's "#?RADIANCE" signature wrong or missing
    RadianceHdrSignatureInvalid,
    /// EOF before end of header
    TruncatedHeader,
    /// EOF instead of image dimensions
    TruncatedDimensions,

    /// A value couldn't be parsed
    UnparsableF32(LineType, ParseFloatError),
    /// A value couldn't be parsed
    UnparsableU32(LineType, ParseIntError),
    /// Not enough numbers in line
    LineTooShort(LineType),

    /// COLORCORR contains too many numbers in strict mode
    ExtraneousColorcorrNumbers,

    /// Dimensions line had too few elements
    DimensionsLineTooShort(usize, usize),
    /// Dimensions line had too many elements
    DimensionsLineTooLong(usize),

    /// The length of a scanline (1) wasn't a match for the specified length (2)
    WrongScanlineLength(usize, usize),
    /// First pixel of a scanline is a run length marker
    FirstPixelRlMarker,
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecoderError::RadianceHdrSignatureInvalid => {
                f.write_str("Radiance HDR signature not found")
            }
            DecoderError::TruncatedHeader => f.write_str("EOF in header"),
            DecoderError::TruncatedDimensions => f.write_str("EOF in dimensions line"),
            DecoderError::UnparsableF32(line, pe) => {
                f.write_fmt(format_args!("Cannot parse {line} value as f32: {pe}"))
            }
            DecoderError::UnparsableU32(line, pe) => {
                f.write_fmt(format_args!("Cannot parse {line} value as u32: {pe}"))
            }
            DecoderError::LineTooShort(line) => {
                f.write_fmt(format_args!("Not enough numbers in {line}"))
            }
            DecoderError::ExtraneousColorcorrNumbers => f.write_str("Extra numbers in COLORCORR"),
            DecoderError::DimensionsLineTooShort(elements, expected) => f.write_fmt(format_args!(
                "Dimensions line too short: have {elements} elements, expected {expected}"
            )),
            DecoderError::DimensionsLineTooLong(expected) => f.write_fmt(format_args!(
                "Dimensions line too long, expected {expected} elements"
            )),
            DecoderError::WrongScanlineLength(len, expected) => f.write_fmt(format_args!(
                "Wrong length of decoded scanline: got {len}, expected {expected}"
            )),
            DecoderError::FirstPixelRlMarker => {
                f.write_str("First pixel of a scanline shouldn't be run length marker")
            }
        }
    }
}

impl From<DecoderError> for ImageError {
    fn from(e: DecoderError) -> ImageError {
        ImageError::Decoding(DecodingError::new(ImageFormat::Hdr.into(), e))
    }
}

impl error::Error for DecoderError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            DecoderError::UnparsableF32(_, err) => Some(err),
            DecoderError::UnparsableU32(_, err) => Some(err),
            _ => None,
        }
    }
}

/// Lines which contain parsable data that can fail
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum LineType {
    Exposure,
    Pixaspect,
    Colorcorr,
    DimensionsHeight,
    DimensionsWidth,
}

impl fmt::Display for LineType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LineType::Exposure => "EXPOSURE",
            LineType::Pixaspect => "PIXASPECT",
            LineType::Colorcorr => "COLORCORR",
            LineType::DimensionsHeight => "height dimension",
            LineType::DimensionsWidth => "width dimension",
        })
    }
}

/// Radiance HDR file signature
pub const SIGNATURE: &[u8] = b"#?RADIANCE";
const SIGNATURE_LENGTH: usize = 10;

/// An Radiance HDR decoder
#[derive(Debug)]
pub struct HdrDecoder<R> {
    r: R,
    width: u32,
    height: u32,
    meta: HdrMetadata,
}

/// Refer to [wikipedia](https://en.wikipedia.org/wiki/RGBE_image_format)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rgbe8Pixel {
    /// Color components
    pub(crate) c: [u8; 3],
    /// Exponent
    pub(crate) e: u8,
}

/// Creates `Rgbe8Pixel` from components
pub(crate) fn rgbe8(r: u8, g: u8, b: u8, e: u8) -> Rgbe8Pixel {
    Rgbe8Pixel { c: [r, g, b], e }
}

impl Rgbe8Pixel {
    /// Converts `Rgbe8Pixel` into `Rgb<f32>` linearly
    #[inline]
    pub(crate) fn to_hdr(self) -> Rgb<f32> {
        if self.e == 0 {
            Rgb([0.0, 0.0, 0.0])
        } else {
            //            let exp = f32::ldexp(1., self.e as isize - (128 + 8)); // unstable
            let exp = f32::exp2(<f32 as From<_>>::from(self.e) - (128.0 + 8.0));
            Rgb([
                exp * <f32 as From<_>>::from(self.c[0]),
                exp * <f32 as From<_>>::from(self.c[1]),
                exp * <f32 as From<_>>::from(self.c[2]),
            ])
        }
    }
}

impl<R: Read> HdrDecoder<R> {
    /// Reads Radiance HDR image header from stream ```r```
    /// if the header is valid, creates `HdrDecoder`
    /// strict mode is enabled
    pub fn new(reader: R) -> ImageResult<Self> {
        HdrDecoder::with_strictness(reader, true)
    }

    /// Allows reading old Radiance HDR images
    pub fn new_nonstrict(reader: R) -> ImageResult<Self> {
        Self::with_strictness(reader, false)
    }

    /// Reads Radiance HDR image header from stream `reader`,
    /// if the header is valid, creates `HdrDecoder`.
    ///
    /// strict enables strict mode
    ///
    /// Warning! Reading wrong file in non-strict mode
    ///   could consume file size worth of memory in the process.
    pub fn with_strictness(mut reader: R, strict: bool) -> ImageResult<HdrDecoder<R>> {
        let mut attributes = HdrMetadata::new();

        {
            // scope to make borrowck happy
            let r = &mut reader;
            if strict {
                let mut signature = [0; SIGNATURE_LENGTH];
                r.read_exact(&mut signature)?;
                if signature != SIGNATURE {
                    return Err(DecoderError::RadianceHdrSignatureInvalid.into());
                } // no else
                  // skip signature line ending
                read_line_u8(r)?;
            } else {
                // Old Radiance HDR files (*.pic) don't use signature
                // Let them be parsed in non-strict mode
            }
            // read header data until empty line
            loop {
                match read_line_u8(r)? {
                    None => {
                        // EOF before end of header
                        return Err(DecoderError::TruncatedHeader.into());
                    }
                    Some(line) => {
                        if line.is_empty() {
                            // end of header
                            break;
                        } else if line[0] == b'#' {
                            // line[0] will not panic, line.len() == 0 is false here
                            // skip comments
                            continue;
                        } // no else
                          // process attribute line
                        let line = String::from_utf8_lossy(&line[..]);
                        attributes.update_header_info(&line, strict)?;
                    } // <= Some(line)
                } // match read_line_u8()
            } // loop
        } // scope to end borrow of reader
          // parse dimensions
        let (width, height) = match read_line_u8(&mut reader)? {
            None => {
                // EOF instead of image dimensions
                return Err(DecoderError::TruncatedDimensions.into());
            }
            Some(dimensions) => {
                let dimensions = String::from_utf8_lossy(&dimensions[..]);
                parse_dimensions_line(&dimensions, strict)?
            }
        };

        // color type is always rgb8
        if crate::utils::check_dimension_overflow(width, height, ColorType::Rgb8.bytes_per_pixel())
        {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Hdr.into(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "Image dimensions ({width}x{height}) are too large"
                    )),
                ),
            ));
        }

        Ok(HdrDecoder {
            r: reader,

            width,
            height,
            meta: HdrMetadata {
                width,
                height,
                ..attributes
            },
        })
    } // end with_strictness

    /// Returns file metadata. Refer to `HdrMetadata` for details.
    pub fn metadata(&self) -> HdrMetadata {
        self.meta.clone()
    }

    /// Consumes decoder and returns a vector of transformed pixels
    fn read_image_transform<T: Send, F: Send + Sync + Fn(Rgbe8Pixel) -> T>(
        mut self,
        f: F,
        output_slice: &mut [T],
    ) -> ImageResult<()> {
        assert_eq!(
            output_slice.len(),
            self.width as usize * self.height as usize
        );

        // Don't read anything if image is empty
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let chunks_iter = output_slice.chunks_mut(self.width as usize);

        let mut buf = vec![Default::default(); self.width as usize];
        for chunk in chunks_iter {
            // read_scanline overwrites the entire buffer or returns an Err,
            // so not resetting the buffer here is ok.
            read_scanline(&mut self.r, &mut buf[..])?;
            for (dst, &pix) in chunk.iter_mut().zip(buf.iter()) {
                *dst = f(pix);
            }
        }
        Ok(())
    }
}

impl<R: Read> ImageDecoder for HdrDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.meta.width, self.meta.height)
    }

    fn color_type(&self) -> ColorType {
        ColorType::Rgb32F
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));

        let mut img = vec![Rgb([0.0, 0.0, 0.0]); self.width as usize * self.height as usize];
        self.read_image_transform(|pix| pix.to_hdr(), &mut img[..])?;

        for (i, Rgb(data)) in img.into_iter().enumerate() {
            buf[(i * 12)..][..12].copy_from_slice(bytemuck::cast_slice(&data));
        }

        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

// Precondition: buf.len() > 0
fn read_scanline<R: Read>(r: &mut R, buf: &mut [Rgbe8Pixel]) -> ImageResult<()> {
    assert!(!buf.is_empty());
    let width = buf.len();
    // first 4 bytes in scanline allow to determine compression method
    let fb = read_rgbe(r)?;
    if fb.c[0] == 2 && fb.c[1] == 2 && fb.c[2] < 128 {
        // denormalized pixel value (2,2,<128,_) indicates new per component RLE method
        // decode_component guarantees that offset is within 0 .. width
        // therefore we can skip bounds checking here, but we will not
        decode_component(r, width, |offset, value| buf[offset].c[0] = value)?;
        decode_component(r, width, |offset, value| buf[offset].c[1] = value)?;
        decode_component(r, width, |offset, value| buf[offset].c[2] = value)?;
        decode_component(r, width, |offset, value| buf[offset].e = value)?;
    } else {
        // old RLE method (it was considered old around 1991, should it be here?)
        decode_old_rle(r, fb, buf)?;
    }
    Ok(())
}

#[inline(always)]
fn read_byte<R: Read>(r: &mut R) -> io::Result<u8> {
    let mut buf = [0u8];
    r.read_exact(&mut buf[..])?;
    Ok(buf[0])
}

// Guarantees that first parameter of set_component will be within pos .. pos+width
#[inline]
fn decode_component<R: Read, S: FnMut(usize, u8)>(
    r: &mut R,
    width: usize,
    mut set_component: S,
) -> ImageResult<()> {
    let mut buf = [0; 128];
    let mut pos = 0;
    while pos < width {
        // increment position by a number of decompressed values
        pos += {
            let rl = read_byte(r)?;
            if rl <= 128 {
                // sanity check
                if pos + rl as usize > width {
                    return Err(DecoderError::WrongScanlineLength(pos + rl as usize, width).into());
                }
                // read values
                r.read_exact(&mut buf[0..rl as usize])?;
                for (offset, &value) in buf[0..rl as usize].iter().enumerate() {
                    set_component(pos + offset, value);
                }
                rl as usize
            } else {
                // run
                let rl = rl - 128;
                // sanity check
                if pos + rl as usize > width {
                    return Err(DecoderError::WrongScanlineLength(pos + rl as usize, width).into());
                }
                // fill with same value
                let value = read_byte(r)?;
                for offset in 0..rl as usize {
                    set_component(pos + offset, value);
                }
                rl as usize
            }
        };
    }
    if pos != width {
        return Err(DecoderError::WrongScanlineLength(pos, width).into());
    }
    Ok(())
}

// Decodes scanline, places it into buf
// Precondition: buf.len() > 0
// fb - first 4 bytes of scanline
fn decode_old_rle<R: Read>(r: &mut R, fb: Rgbe8Pixel, buf: &mut [Rgbe8Pixel]) -> ImageResult<()> {
    assert!(!buf.is_empty());
    let width = buf.len();
    // convenience function.
    // returns run length if pixel is a run length marker
    #[inline]
    fn rl_marker(pix: Rgbe8Pixel) -> Option<usize> {
        if pix.c == [1, 1, 1] {
            Some(pix.e as usize)
        } else {
            None
        }
    }
    // first pixel in scanline should not be run length marker
    // it is error if it is
    if rl_marker(fb).is_some() {
        return Err(DecoderError::FirstPixelRlMarker.into());
    }
    buf[0] = fb; // set first pixel of scanline

    let mut x_off = 1; // current offset from beginning of a scanline
    let mut rl_mult = 1; // current run length multiplier
    let mut prev_pixel = fb;
    while x_off < width {
        let pix = read_rgbe(r)?;
        // it's harder to forget to increase x_off if I write this this way.
        x_off += {
            if let Some(rl) = rl_marker(pix) {
                // rl_mult takes care of consecutive RL markers
                let rl = rl * rl_mult;
                rl_mult *= 256;
                if x_off + rl <= width {
                    // do run
                    for b in &mut buf[x_off..x_off + rl] {
                        *b = prev_pixel;
                    }
                } else {
                    return Err(DecoderError::WrongScanlineLength(x_off + rl, width).into());
                };
                rl // value to increase x_off by
            } else {
                rl_mult = 1; // chain of consecutive RL markers is broken
                prev_pixel = pix;
                buf[x_off] = pix;
                1 // value to increase x_off by
            }
        };
    }
    if x_off != width {
        return Err(DecoderError::WrongScanlineLength(x_off, width).into());
    }
    Ok(())
}

fn read_rgbe<R: Read>(r: &mut R) -> io::Result<Rgbe8Pixel> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf[..])?;
    Ok(Rgbe8Pixel {
        c: [buf[0], buf[1], buf[2]],
        e: buf[3],
    })
}

/// Metadata for Radiance HDR image
#[derive(Debug, Clone)]
pub struct HdrMetadata {
    /// Width of decoded image. It could be either scanline length,
    /// or scanline count, depending on image orientation.
    pub width: u32,
    /// Height of decoded image. It depends on orientation too.
    pub height: u32,
    /// Orientation matrix. For standard orientation it is ((1,0),(0,1)) - left to right, top to bottom.
    /// First pair tells how resulting pixel coordinates change along a scanline.
    /// Second pair tells how they change from one scanline to the next.
    pub orientation: ((i8, i8), (i8, i8)),
    /// Divide color values by exposure to get to get physical radiance in
    /// watts/steradian/m<sup>2</sup>
    ///
    /// Image may not contain physical data, even if this field is set.
    pub exposure: Option<f32>,
    /// Divide color values by corresponding tuple member (r, g, b) to get to get physical radiance
    /// in watts/steradian/m<sup>2</sup>
    ///
    /// Image may not contain physical data, even if this field is set.
    pub color_correction: Option<(f32, f32, f32)>,
    /// Pixel height divided by pixel width
    pub pixel_aspect_ratio: Option<f32>,
    /// All lines contained in image header are put here. Ordering of lines is preserved.
    /// Lines in the form "key=value" are represented as ("key", "value").
    /// All other lines are ("", "line")
    pub custom_attributes: Vec<(String, String)>,
}

impl HdrMetadata {
    fn new() -> HdrMetadata {
        HdrMetadata {
            width: 0,
            height: 0,
            orientation: ((1, 0), (0, 1)),
            exposure: None,
            color_correction: None,
            pixel_aspect_ratio: None,
            custom_attributes: vec![],
        }
    }

    // Updates header info, in strict mode returns error for malformed lines (no '=' separator)
    // unknown attributes are skipped
    fn update_header_info(&mut self, line: &str, strict: bool) -> ImageResult<()> {
        // split line at first '='
        // old Radiance HDR files (*.pic) feature tabs in key, so                vvv trim
        let maybe_key_value = split_at_first(line, "=").map(|(key, value)| (key.trim(), value));
        // save all header lines in custom_attributes
        match maybe_key_value {
            Some((key, val)) => self
                .custom_attributes
                .push((key.to_owned(), val.to_owned())),
            None => self
                .custom_attributes
                .push((String::new(), line.to_owned())),
        }
        // parse known attributes
        match maybe_key_value {
            Some(("FORMAT", val)) => {
                if val.trim() != "32-bit_rle_rgbe" {
                    // XYZE isn't supported yet
                    return Err(ImageError::Unsupported(
                        UnsupportedError::from_format_and_kind(
                            ImageFormat::Hdr.into(),
                            UnsupportedErrorKind::Format(ImageFormatHint::Name(limit_string_len(
                                val, 20,
                            ))),
                        ),
                    ));
                }
            }
            Some(("EXPOSURE", val)) => {
                match val.trim().parse::<f32>() {
                    Ok(v) => {
                        self.exposure = Some(self.exposure.unwrap_or(1.0) * v); // all encountered exposure values should be multiplied
                    }
                    Err(parse_error) => {
                        if strict {
                            return Err(DecoderError::UnparsableF32(
                                LineType::Exposure,
                                parse_error,
                            )
                            .into());
                        } // no else, skip this line in non-strict mode
                    }
                }
            }
            Some(("PIXASPECT", val)) => {
                match val.trim().parse::<f32>() {
                    Ok(v) => {
                        self.pixel_aspect_ratio = Some(self.pixel_aspect_ratio.unwrap_or(1.0) * v);
                        // all encountered exposure values should be multiplied
                    }
                    Err(parse_error) => {
                        if strict {
                            return Err(DecoderError::UnparsableF32(
                                LineType::Pixaspect,
                                parse_error,
                            )
                            .into());
                        } // no else, skip this line in non-strict mode
                    }
                }
            }
            Some(("COLORCORR", val)) => {
                let mut rgbcorr = [1.0, 1.0, 1.0];
                match parse_space_separated_f32(val, &mut rgbcorr, LineType::Colorcorr) {
                    Ok(extra_numbers) => {
                        if strict && extra_numbers {
                            return Err(DecoderError::ExtraneousColorcorrNumbers.into());
                        } // no else, just ignore extra numbers
                        let (rc, gc, bc) = self.color_correction.unwrap_or((1.0, 1.0, 1.0));
                        self.color_correction =
                            Some((rc * rgbcorr[0], gc * rgbcorr[1], bc * rgbcorr[2]));
                    }
                    Err(err) => {
                        if strict {
                            return Err(err);
                        } // no else, skip malformed line in non-strict mode
                    }
                }
            }
            None => {
                // old Radiance HDR files (*.pic) contain commands in a header
                // just skip them
            }
            _ => {
                // skip unknown attribute
            }
        } // match attributes
        Ok(())
    }
}

fn parse_space_separated_f32(line: &str, vals: &mut [f32], line_tp: LineType) -> ImageResult<bool> {
    let mut nums = line.split_whitespace();
    for val in vals.iter_mut() {
        if let Some(num) = nums.next() {
            match num.parse::<f32>() {
                Ok(v) => *val = v,
                Err(err) => return Err(DecoderError::UnparsableF32(line_tp, err).into()),
            }
        } else {
            // not enough numbers in line
            return Err(DecoderError::LineTooShort(line_tp).into());
        }
    }
    Ok(nums.next().is_some())
}

// Parses dimension line "-Y height +X width"
// returns (width, height) or error
fn parse_dimensions_line(line: &str, strict: bool) -> ImageResult<(u32, u32)> {
    const DIMENSIONS_COUNT: usize = 4;

    let mut dim_parts = line.split_whitespace();
    let c1_tag = dim_parts
        .next()
        .ok_or(DecoderError::DimensionsLineTooShort(0, DIMENSIONS_COUNT))?;
    let c1_str = dim_parts
        .next()
        .ok_or(DecoderError::DimensionsLineTooShort(1, DIMENSIONS_COUNT))?;
    let c2_tag = dim_parts
        .next()
        .ok_or(DecoderError::DimensionsLineTooShort(2, DIMENSIONS_COUNT))?;
    let c2_str = dim_parts
        .next()
        .ok_or(DecoderError::DimensionsLineTooShort(3, DIMENSIONS_COUNT))?;
    if strict && dim_parts.next().is_some() {
        // extra data in dimensions line
        return Err(DecoderError::DimensionsLineTooLong(DIMENSIONS_COUNT).into());
    } // no else
      // dimensions line is in the form "-Y 10 +X 20"
      // There are 8 possible orientations: +Y +X, +X -Y and so on
    match (c1_tag, c2_tag) {
        ("-Y", "+X") => {
            // Common orientation (left-right, top-down)
            // c1_str is height, c2_str is width
            let height = c1_str
                .parse::<u32>()
                .map_err(|pe| DecoderError::UnparsableU32(LineType::DimensionsHeight, pe))?;
            let width = c2_str
                .parse::<u32>()
                .map_err(|pe| DecoderError::UnparsableU32(LineType::DimensionsWidth, pe))?;
            Ok((width, height))
        }
        _ => Err(ImageError::Unsupported(
            UnsupportedError::from_format_and_kind(
                ImageFormat::Hdr.into(),
                UnsupportedErrorKind::GenericFeature(format!(
                    "Orientation {} {}",
                    limit_string_len(c1_tag, 4),
                    limit_string_len(c2_tag, 4)
                )),
            ),
        )),
    } // final expression. Returns value
}

// Returns string with no more than len+3 characters
fn limit_string_len(s: &str, len: usize) -> String {
    let s_char_len = s.chars().count();
    if s_char_len > len {
        s.chars().take(len).chain("...".chars()).collect()
    } else {
        s.into()
    }
}

// Splits string into (before separator, after separator) tuple
// or None if separator isn't found
fn split_at_first<'a>(s: &'a str, separator: &str) -> Option<(&'a str, &'a str)> {
    match s.find(separator) {
        None | Some(0) => None,
        Some(p) if p >= s.len() - separator.len() => None,
        Some(p) => Some((&s[..p], &s[(p + separator.len())..])),
    }
}

// Reads input until b"\n" or EOF
// Returns vector of read bytes NOT including end of line characters
//   or return None to indicate end of file
fn read_line_u8<R: Read>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    // keeping repeated redundant allocations to avoid added complexity of having a `&mut tmp` argument
    #[allow(clippy::disallowed_methods)]
    let mut ret = Vec::with_capacity(16);
    loop {
        let mut byte = [0];
        if r.read(&mut byte)? == 0 || byte[0] == b'\n' {
            if ret.is_empty() && byte[0] != b'\n' {
                return Ok(None);
            }
            return Ok(Some(ret));
        }
        ret.push(byte[0]);
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, io::Cursor};

    use super::*;

    #[test]
    fn split_at_first_test() {
        assert_eq!(split_at_first(&Cow::Owned(String::new()), "="), None);
        assert_eq!(split_at_first(&Cow::Owned("=".into()), "="), None);
        assert_eq!(split_at_first(&Cow::Owned("= ".into()), "="), None);
        assert_eq!(
            split_at_first(&Cow::Owned(" = ".into()), "="),
            Some((" ", " "))
        );
        assert_eq!(
            split_at_first(&Cow::Owned("EXPOSURE= ".into()), "="),
            Some(("EXPOSURE", " "))
        );
        assert_eq!(
            split_at_first(&Cow::Owned("EXPOSURE= =".into()), "="),
            Some(("EXPOSURE", " ="))
        );
        assert_eq!(
            split_at_first(&Cow::Owned("EXPOSURE== =".into()), "=="),
            Some(("EXPOSURE", " ="))
        );
        assert_eq!(split_at_first(&Cow::Owned("EXPOSURE".into()), ""), None);
    }

    #[test]
    fn read_line_u8_test() {
        let buf: Vec<_> = (&b"One\nTwo\nThree\nFour\n\n\n"[..]).into();
        let input = &mut Cursor::new(buf);
        assert_eq!(&read_line_u8(input).unwrap().unwrap()[..], &b"One"[..]);
        assert_eq!(&read_line_u8(input).unwrap().unwrap()[..], &b"Two"[..]);
        assert_eq!(&read_line_u8(input).unwrap().unwrap()[..], &b"Three"[..]);
        assert_eq!(&read_line_u8(input).unwrap().unwrap()[..], &b"Four"[..]);
        assert_eq!(&read_line_u8(input).unwrap().unwrap()[..], &b""[..]);
        assert_eq!(&read_line_u8(input).unwrap().unwrap()[..], &b""[..]);
        assert_eq!(read_line_u8(input).unwrap(), None);
    }

    #[test]
    fn dimension_overflow() {
        let data = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n -Y 4294967295 +X 4294967295";

        assert!(HdrDecoder::new(Cursor::new(data)).is_err());
        assert!(HdrDecoder::new_nonstrict(Cursor::new(data)).is_err());
    }
}
