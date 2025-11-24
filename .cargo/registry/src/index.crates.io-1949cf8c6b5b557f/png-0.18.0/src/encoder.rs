use borrow::Cow;
use io::{Read, Write};
use ops::{Deref, DerefMut};
use std::{borrow, error, fmt, io, mem, ops, result};

use crc32fast::Hasher as Crc32;
use flate2::write::ZlibEncoder;

use crate::chunk::{self, ChunkType};
use crate::common::{
    AnimationControl, BitDepth, BlendOp, BytesPerPixel, ColorType, Compression, DisposeOp,
    FrameControl, Info, ParameterError, ParameterErrorKind, PixelDimensions, ScaledFloat, Unit,
};
use crate::filter::{filter, Filter};
use crate::text_metadata::{
    encode_iso_8859_1, EncodableTextChunk, ITXtChunk, TEXtChunk, TextEncodingError, ZTXtChunk,
};
use crate::traits::WriteBytesExt;
use crate::DeflateCompression;

pub type Result<T> = result::Result<T, EncodingError>;

#[derive(Debug)]
pub enum EncodingError {
    IoError(io::Error),
    Format(FormatError),
    Parameter(ParameterError),
    LimitsExceeded,
}

#[derive(Debug)]
pub struct FormatError {
    inner: FormatErrorKind,
}

#[derive(Debug)]
enum FormatErrorKind {
    ZeroWidth,
    ZeroHeight,
    InvalidColorCombination(BitDepth, ColorType),
    NoPalette,
    // TODO: wait, what?
    WrittenTooMuch(usize),
    NotAnimated,
    OutOfBounds,
    EndReached,
    ZeroFrames,
    MissingFrames,
    MissingData(usize),
    Unrecoverable,
    BadTextEncoding(TextEncodingError),
}

impl error::Error for EncodingError {
    fn cause(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            EncodingError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for EncodingError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        use self::EncodingError::*;
        match self {
            IoError(err) => write!(fmt, "{}", err),
            Format(desc) => write!(fmt, "{}", desc),
            Parameter(desc) => write!(fmt, "{}", desc),
            LimitsExceeded => write!(fmt, "Limits are exceeded."),
        }
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        use FormatErrorKind::*;
        match self.inner {
            ZeroWidth => write!(fmt, "Zero width not allowed"),
            ZeroHeight => write!(fmt, "Zero height not allowed"),
            ZeroFrames => write!(fmt, "Zero frames not allowed"),
            InvalidColorCombination(depth, color) => write!(
                fmt,
                "Invalid combination of bit-depth '{:?}' and color-type '{:?}'",
                depth, color
            ),
            NoPalette => write!(fmt, "can't write indexed image without palette"),
            WrittenTooMuch(index) => write!(fmt, "wrong data size, got {} bytes too many", index),
            NotAnimated => write!(fmt, "not an animation"),
            OutOfBounds => write!(
                fmt,
                "the dimension and position go over the frame boundaries"
            ),
            EndReached => write!(fmt, "all the frames have been already written"),
            MissingFrames => write!(fmt, "there are still frames to be written"),
            MissingData(n) => write!(fmt, "there are still {} bytes to be written", n),
            Unrecoverable => write!(
                fmt,
                "a previous error put the writer into an unrecoverable state"
            ),
            BadTextEncoding(tee) => match tee {
                TextEncodingError::Unrepresentable => write!(
                    fmt,
                    "The text metadata cannot be encoded into valid ISO 8859-1"
                ),
                TextEncodingError::InvalidKeywordSize => write!(fmt, "Invalid keyword size"),
                TextEncodingError::CompressionError => {
                    write!(fmt, "Unable to compress text metadata")
                }
            },
        }
    }
}

impl From<io::Error> for EncodingError {
    fn from(err: io::Error) -> EncodingError {
        EncodingError::IoError(err)
    }
}

impl From<EncodingError> for io::Error {
    fn from(err: EncodingError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, err.to_string())
    }
}

// Private impl.
impl From<FormatErrorKind> for FormatError {
    fn from(kind: FormatErrorKind) -> Self {
        FormatError { inner: kind }
    }
}

impl From<TextEncodingError> for EncodingError {
    fn from(tee: TextEncodingError) -> Self {
        EncodingError::Format(FormatError {
            inner: FormatErrorKind::BadTextEncoding(tee),
        })
    }
}

/// PNG Encoder.
///
/// This configures the PNG format options such as animation chunks, palette use, color types,
/// auxiliary chunks etc.
///
/// FIXME: Configuring APNG might be easier (less individual errors) if we had an _adapter_ which
/// borrows this mutably but guarantees that `info.frame_control` is not `None`.
pub struct Encoder<'a, W: Write> {
    w: W,
    info: Info<'a>,
    options: Options,
}

/// Encoding options, internal type, forwarded to the Writer.
#[derive(Default)]
struct Options {
    filter: Filter,
    sep_def_img: bool,
    validate_sequence: bool,
    compression: DeflateCompression,
}

impl<'a, W: Write> Encoder<'a, W> {
    pub fn new(w: W, width: u32, height: u32) -> Encoder<'static, W> {
        Encoder {
            w,
            info: Info::with_size(width, height),
            options: Options::default(),
        }
    }

    pub fn with_info(w: W, info: Info<'a>) -> Result<Encoder<'a, W>> {
        if info.animation_control.is_some() != info.frame_control.is_some() {
            return Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()));
        }

        if let Some(actl) = info.animation_control {
            if actl.num_frames == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroFrames.into()));
            }
        }

        Ok(Encoder {
            w,
            info,
            options: Options::default(),
        })
    }

    /// Specify that the image is animated.
    ///
    /// `num_frames` controls how many frames the animation has, while
    /// `num_plays` controls how many times the animation should be
    /// repeated until it stops, if it's zero then it will repeat
    /// infinitely.
    ///
    /// When this method is returns successfully then the images written will be encoded as fdAT
    /// chunks, except for the first image that is still encoded as `IDAT`. You can control if the
    /// first frame should be treated as an animation frame with [`Encoder::set_sep_def_img()`].
    ///
    /// This method returns an error if `num_frames` is 0.
    pub fn set_animated(&mut self, num_frames: u32, num_plays: u32) -> Result<()> {
        if num_frames == 0 {
            return Err(EncodingError::Format(FormatErrorKind::ZeroFrames.into()));
        }

        let actl = AnimationControl {
            num_frames,
            num_plays,
        };

        let fctl = FrameControl {
            sequence_number: 0,
            width: self.info.width,
            height: self.info.height,
            ..Default::default()
        };

        self.info.animation_control = Some(actl);
        self.info.frame_control = Some(fctl);
        Ok(())
    }

    /// Mark the first animated frame as a 'separate default image'.
    ///
    /// In APNG each animated frame is preceded by a special control chunk, `fcTL`. It's up to the
    /// encoder to decide if the first image, the standard `IDAT` data, should be part of the
    /// animation by emitting this chunk or by not doing so. A default image that is _not_ part of
    /// the animation is often interpreted as a thumbnail.
    ///
    /// This method will return an error when animation control was not configured
    /// (which is done by calling [`Encoder::set_animated`]).
    pub fn set_sep_def_img(&mut self, sep_def_img: bool) -> Result<()> {
        if self.info.animation_control.is_some() {
            self.options.sep_def_img = sep_def_img;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Sets the raw byte contents of the PLTE chunk. This method accepts
    /// both borrowed and owned byte data.
    pub fn set_palette<T: Into<Cow<'a, [u8]>>>(&mut self, palette: T) {
        self.info.palette = Some(palette.into());
    }

    /// Sets the raw byte contents of the tRNS chunk. This method accepts
    /// both borrowed and owned byte data.
    pub fn set_trns<T: Into<Cow<'a, [u8]>>>(&mut self, trns: T) {
        self.info.trns = Some(trns.into());
    }

    /// Set the display gamma of the source system on which the image was generated or last edited.
    pub fn set_source_gamma(&mut self, source_gamma: ScaledFloat) {
        self.info.source_gamma = Some(source_gamma);
    }

    /// Set the chromaticities for the source system's display channels (red, green, blue) and the whitepoint
    /// of the source system on which the image was generated or last edited.
    pub fn set_source_chromaticities(
        &mut self,
        source_chromaticities: super::SourceChromaticities,
    ) {
        self.info.source_chromaticities = Some(source_chromaticities);
    }

    /// Mark the image data as conforming to the SRGB color space with the specified rendering intent.
    ///
    /// Any ICC profiles will be ignored.
    ///
    /// Source gamma and chromaticities will be written only if they're set to fallback
    /// values specified in [11.3.2.5](https://www.w3.org/TR/png-3/#sRGB-gAMA-cHRM).
    pub fn set_source_srgb(&mut self, rendering_intent: super::SrgbRenderingIntent) {
        self.info.set_source_srgb(rendering_intent);
    }

    /// Start encoding by writing the header data.
    ///
    /// The remaining data can be supplied by methods on the returned [`Writer`].
    pub fn write_header(self) -> Result<Writer<W>> {
        Writer::new(self.w, PartialInfo::new(&self.info), self.options).init(&self.info)
    }

    /// Set the color of the encoded image.
    ///
    /// These correspond to the color types in the png IHDR data that will be written. The length
    /// of the image data that is later supplied must match the color type, otherwise an error will
    /// be emitted.
    pub fn set_color(&mut self, color: ColorType) {
        self.info.color_type = color;
    }

    /// Set the indicated depth of the image data.
    pub fn set_depth(&mut self, depth: BitDepth) {
        self.info.bit_depth = depth;
    }

    /// Set compression parameters, see [Compression] for the available options.
    pub fn set_compression(&mut self, compression: Compression) {
        self.set_deflate_compression(DeflateCompression::from_simple(compression));
        self.set_filter(Filter::from_simple(compression));
    }

    /// Provides in-depth customization of DEFLATE compression options.
    ///
    /// For a simpler selection of compression options see [Self::set_compression].
    pub fn set_deflate_compression(&mut self, compression: DeflateCompression) {
        self.options.compression = compression;
    }

    /// Set the used filter type.
    ///
    /// The default filter is [`Filter::Adaptive`] which automatically selects the best filter
    /// for each row of the image.
    ///
    /// You should only change this if you are after very fast compression,
    /// and either don't care about compression ratio or know exactly what works best for your images.
    pub fn set_filter(&mut self, filter: Filter) {
        self.options.filter = filter;
    }

    /// Set the fraction of time every frame is going to be displayed, in seconds.
    ///
    /// *Note that this parameter can be set for each individual frame after
    /// [`Encoder::write_header`] is called. (see [`Writer::set_frame_delay`])*
    ///
    /// If the denominator is 0, it is to be treated as if it were 100
    /// (that is, the numerator then specifies 1/100ths of a second).
    /// If the value of the numerator is 0 the decoder should render the next frame
    /// as quickly as possible, though viewers may impose a reasonable lower bound.
    ///
    /// The default value is 0 for both the numerator and denominator.
    ///
    /// This method will return an error if the image is not animated.
    /// (see [`set_animated`])
    ///
    /// [`write_header`]: Self::write_header
    /// [`set_animated`]: Self::set_animated
    pub fn set_frame_delay(&mut self, numerator: u16, denominator: u16) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.delay_den = denominator;
            fctl.delay_num = numerator;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the blend operation for every frame.
    ///
    /// The blend operation specifies whether the frame is to be alpha blended
    /// into the current output buffer content, or whether it should completely
    /// replace its region in the output buffer.
    ///
    /// *Note that this parameter can be set for each individual frame after
    /// [`write_header`] is called. (see [`Writer::set_blend_op`])*
    ///
    /// See the [`BlendOp`] documentation for the possible values and their effects.
    ///
    /// *Note that for the first frame the two blend modes are functionally
    /// equivalent due to the clearing of the output buffer at the beginning
    /// of each play.*
    ///
    /// The default value is [`BlendOp::Source`].
    ///
    /// This method will return an error if the image is not animated.
    /// (see [`set_animated`])
    ///
    /// [`write_header`]: Self::write_header
    /// [`set_animated`]: Self::set_animated
    pub fn set_blend_op(&mut self, op: BlendOp) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.blend_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the dispose operation for every frame.
    ///
    /// The dispose operation specifies how the output buffer should be changed
    /// at the end of the delay (before rendering the next frame)
    ///
    /// *Note that this parameter can be set for each individual frame after
    /// [`write_header`] is called (see [`Writer::set_dispose_op`])*
    ///
    /// See the [`DisposeOp`] documentation for the possible values and their effects.
    ///
    /// *Note that if the first frame uses [`DisposeOp::Previous`]
    /// it will be treated as [`DisposeOp::Background`].*
    ///
    /// The default value is [`DisposeOp::None`].
    ///
    /// This method will return an error if the image is not animated.
    /// (see [`set_animated`])
    ///
    /// [`set_animated`]: Self::set_animated
    /// [`write_header`]: Self::write_header
    pub fn set_dispose_op(&mut self, op: DisposeOp) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.dispose_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }
    pub fn set_pixel_dims(&mut self, pixel_dims: Option<PixelDimensions>) {
        self.info.pixel_dims = pixel_dims
    }
    /// Convenience function to add tEXt chunks to [`Info`] struct
    pub fn add_text_chunk(&mut self, keyword: String, text: String) -> Result<()> {
        let text_chunk = TEXtChunk::new(keyword, text);
        self.info.uncompressed_latin1_text.push(text_chunk);
        Ok(())
    }

    /// Convenience function to add zTXt chunks to [`Info`] struct
    pub fn add_ztxt_chunk(&mut self, keyword: String, text: String) -> Result<()> {
        let text_chunk = ZTXtChunk::new(keyword, text);
        self.info.compressed_latin1_text.push(text_chunk);
        Ok(())
    }

    /// Convenience function to add iTXt chunks to [`Info`] struct
    ///
    /// This function only sets the `keyword` and `text` field of the iTXt chunk.
    /// To set the other fields, create a [`ITXtChunk`] directly, and then encode it to the output stream.
    pub fn add_itxt_chunk(&mut self, keyword: String, text: String) -> Result<()> {
        let text_chunk = ITXtChunk::new(keyword, text);
        self.info.utf8_text.push(text_chunk);
        Ok(())
    }

    /// Validate the written image sequence.
    ///
    /// When validation is turned on (it's turned off by default) then attempts to write more than
    /// one `IDAT` image or images beyond the number of frames indicated in the animation control
    /// chunk will fail and return an error result instead. Attempts to [finish][finish] the image
    /// with missing frames will also return an error.
    ///
    /// [finish]: StreamWriter::finish
    ///
    /// (It's possible to circumvent these checks by writing raw chunks instead.)
    pub fn validate_sequence(&mut self, validate: bool) {
        self.options.validate_sequence = validate;
    }
}

/// PNG writer
///
/// Progresses through the image by writing images, frames, or raw individual chunks. This is
/// constructed through [`Encoder::write_header()`].
///
/// FIXME: Writing of animated chunks might be clearer if we had an _adapter_ that you would call
/// to guarantee the next image to be prefaced with a fcTL-chunk, and all other chunks would be
/// guaranteed to be `IDAT`/not affected by APNG's frame control.
pub struct Writer<W: Write> {
    /// The underlying writer.
    w: W,
    /// The local version of the `Info` struct.
    info: PartialInfo,
    /// Global encoding options.
    options: Options,
    /// The total number of image frames, counting all consecutive IDAT and fdAT chunks.
    images_written: u64,
    /// The total number of animation frames, that is equivalent to counting fcTL chunks.
    animation_written: u32,
    /// A flag to note when the IEND chunk was already added.
    /// This is only set on code paths that drop `Self` to control the destructor.
    iend_written: bool,
}

/// Contains the subset of attributes of [Info] needed for [Writer] to function
struct PartialInfo {
    width: u32,
    height: u32,
    bit_depth: BitDepth,
    color_type: ColorType,
    frame_control: Option<FrameControl>,
    animation_control: Option<AnimationControl>,
    has_palette: bool,
}

impl PartialInfo {
    fn new(info: &Info) -> Self {
        PartialInfo {
            width: info.width,
            height: info.height,
            bit_depth: info.bit_depth,
            color_type: info.color_type,
            frame_control: info.frame_control,
            animation_control: info.animation_control,
            has_palette: info.palette.is_some(),
        }
    }

    fn bpp_in_prediction(&self) -> BytesPerPixel {
        BytesPerPixel::from_usize(self.bytes_per_pixel())
    }

    fn bytes_per_pixel(&self) -> usize {
        self.color_type.bytes_per_pixel(self.bit_depth)
    }

    fn raw_row_length(&self) -> usize {
        self.raw_row_length_from_width(self.width)
    }

    fn raw_row_length_from_width(&self, width: u32) -> usize {
        self.color_type
            .raw_row_length_from_width(self.bit_depth, width)
    }
}

const DEFAULT_BUFFER_LENGTH: usize = 4 * 1024;

pub(crate) fn write_chunk<W: Write>(mut w: W, name: chunk::ChunkType, data: &[u8]) -> Result<()> {
    w.write_be(data.len() as u32)?;
    w.write_all(&name.0)?;
    w.write_all(data)?;
    let mut crc = Crc32::new();
    crc.update(&name.0);
    crc.update(data);
    w.write_be(crc.finalize())?;
    Ok(())
}

impl<W: Write> Writer<W> {
    fn new(w: W, info: PartialInfo, options: Options) -> Writer<W> {
        Writer {
            w,
            info,
            options,
            images_written: 0,
            animation_written: 0,
            iend_written: false,
        }
    }

    fn init(mut self, info: &Info<'_>) -> Result<Self> {
        if self.info.width == 0 {
            return Err(EncodingError::Format(FormatErrorKind::ZeroWidth.into()));
        }

        if self.info.height == 0 {
            return Err(EncodingError::Format(FormatErrorKind::ZeroHeight.into()));
        }

        if self
            .info
            .color_type
            .is_combination_invalid(self.info.bit_depth)
        {
            return Err(EncodingError::Format(
                FormatErrorKind::InvalidColorCombination(self.info.bit_depth, self.info.color_type)
                    .into(),
            ));
        }

        self.encode_header(info)?;

        Ok(self)
    }

    /// Encode PNG signature, IHDR, and then chunks that were added to the `Info`
    fn encode_header(&mut self, info: &Info<'_>) -> Result<()> {
        self.w.write_all(&[137, 80, 78, 71, 13, 10, 26, 10])?; // PNG signature

        // Encode the IHDR chunk
        let mut data = [0; 13];
        data[..4].copy_from_slice(&info.width.to_be_bytes());
        data[4..8].copy_from_slice(&info.height.to_be_bytes());
        data[8] = info.bit_depth as u8;
        data[9] = info.color_type as u8;
        data[12] = info.interlaced as u8;
        self.write_chunk(chunk::IHDR, &data)?;

        // Encode the pHYs chunk
        if let Some(pd) = info.pixel_dims {
            let mut phys_data = [0; 9];
            phys_data[0..4].copy_from_slice(&pd.xppu.to_be_bytes());
            phys_data[4..8].copy_from_slice(&pd.yppu.to_be_bytes());
            match pd.unit {
                Unit::Meter => phys_data[8] = 1,
                Unit::Unspecified => phys_data[8] = 0,
            }
            self.write_chunk(chunk::pHYs, &phys_data)?;
        }

        // If specified, the sRGB information overrides the source gamma and chromaticities.
        if let Some(srgb) = &info.srgb {
            srgb.encode(&mut self.w)?;

            // gAMA and cHRM are optional, for backwards compatibility
            let srgb_gamma = crate::srgb::substitute_gamma();
            if Some(srgb_gamma) == info.source_gamma {
                srgb_gamma.encode_gama(&mut self.w)?
            }
            let srgb_chromaticities = crate::srgb::substitute_chromaticities();
            if Some(srgb_chromaticities) == info.source_chromaticities {
                srgb_chromaticities.encode(&mut self.w)?;
            }
        } else {
            if let Some(gma) = info.source_gamma {
                gma.encode_gama(&mut self.w)?
            }
            if let Some(chrms) = info.source_chromaticities {
                chrms.encode(&mut self.w)?;
            }
            if let Some(iccp) = &info.icc_profile {
                self.write_iccp_chunk("_", iccp)?
            }
        }

        if let Some(exif) = &info.exif_metadata {
            self.write_chunk(chunk::eXIf, exif)?;
        }

        if let Some(actl) = info.animation_control {
            actl.encode(&mut self.w)?;
        }

        // The position of the PLTE chunk is important, it must come before the tRNS chunk and after
        // many of the other metadata chunks.
        if let Some(p) = &info.palette {
            self.write_chunk(chunk::PLTE, p)?;
        };

        if let Some(t) = &info.trns {
            self.write_chunk(chunk::tRNS, t)?;
        }

        for text_chunk in &info.uncompressed_latin1_text {
            self.write_text_chunk(text_chunk)?;
        }

        for text_chunk in &info.compressed_latin1_text {
            self.write_text_chunk(text_chunk)?;
        }

        for text_chunk in &info.utf8_text {
            self.write_text_chunk(text_chunk)?;
        }

        Ok(())
    }

    /// Write a raw chunk of PNG data.
    ///
    /// This function calculates the required CRC sum so this should not be included in the input
    /// `data`, otherwise the data is not filtered in any way. This function returns an error if
    /// the length of `data` can't be parsed as a `u32` though the length of the chunk data should
    /// not exceed `i32::MAX` or 2,147,483,647.
    pub fn write_chunk(&mut self, name: ChunkType, data: &[u8]) -> Result<()> {
        use std::convert::TryFrom;

        if u32::try_from(data.len()).map_or(true, |length| length > i32::MAX as u32) {
            let kind = FormatErrorKind::WrittenTooMuch(data.len() - i32::MAX as usize);
            return Err(EncodingError::Format(kind.into()));
        }

        write_chunk(&mut self.w, name, data)
    }

    pub fn write_text_chunk<T: EncodableTextChunk>(&mut self, text_chunk: &T) -> Result<()> {
        text_chunk.encode(&mut self.w)
    }

    fn write_iccp_chunk(&mut self, profile_name: &str, icc_profile: &[u8]) -> Result<()> {
        let profile_name = encode_iso_8859_1(profile_name)?;
        if profile_name.is_empty() || profile_name.len() > 79 {
            return Err(TextEncodingError::InvalidKeywordSize.into());
        }

        let estimated_compressed_size = icc_profile.len() * 3 / 4;
        let chunk_size = profile_name
            .len()
            .checked_add(2) // string NUL + compression type. Checked add optimizes out later Vec reallocations.
            .and_then(|s| s.checked_add(estimated_compressed_size))
            .ok_or(EncodingError::LimitsExceeded)?;

        let mut data = Vec::new();
        data.try_reserve_exact(chunk_size)
            .map_err(|_| EncodingError::LimitsExceeded)?;

        data.extend(profile_name.into_iter().chain([0, 0]));

        let mut encoder = ZlibEncoder::new(data, flate2::Compression::default());
        encoder.write_all(icc_profile)?;

        self.write_chunk(chunk::iCCP, &encoder.finish()?)
    }

    /// Check if we should allow writing another image.
    fn validate_new_image(&self) -> Result<()> {
        if !self.options.validate_sequence {
            return Ok(());
        }

        match self.info.animation_control {
            None => {
                if self.images_written == 0 {
                    Ok(())
                } else {
                    Err(EncodingError::Format(FormatErrorKind::EndReached.into()))
                }
            }
            Some(_) => {
                if self.info.frame_control.is_some() {
                    Ok(())
                } else {
                    Err(EncodingError::Format(FormatErrorKind::EndReached.into()))
                }
            }
        }
    }

    fn validate_sequence_done(&self) -> Result<()> {
        if !self.options.validate_sequence {
            return Ok(());
        }

        if (self.info.animation_control.is_some() && self.info.frame_control.is_some())
            || self.images_written == 0
        {
            Err(EncodingError::Format(FormatErrorKind::MissingFrames.into()))
        } else {
            Ok(())
        }
    }

    const MAX_IDAT_CHUNK_LEN: u32 = u32::MAX >> 1;
    #[allow(non_upper_case_globals)]
    const MAX_fdAT_CHUNK_LEN: u32 = (u32::MAX >> 1) - 4;

    /// Writes the next image data.
    pub fn write_image_data(&mut self, data: &[u8]) -> Result<()> {
        if self.info.color_type == ColorType::Indexed && !self.info.has_palette {
            return Err(EncodingError::Format(FormatErrorKind::NoPalette.into()));
        }

        self.validate_new_image()?;

        let width: usize;
        let height: usize;
        if let Some(ref mut fctl) = self.info.frame_control {
            width = fctl.width as usize;
            height = fctl.height as usize;
        } else {
            width = self.info.width as usize;
            height = self.info.height as usize;
        }

        let in_len = self.info.raw_row_length_from_width(width as u32) - 1;
        let data_size = in_len * height;
        if data_size != data.len() {
            return Err(EncodingError::Parameter(
                ParameterErrorKind::ImageBufferSize {
                    expected: data_size,
                    actual: data.len(),
                }
                .into(),
            ));
        }

        let prev = vec![0; in_len];
        let mut prev = prev.as_slice();

        let bpp = self.info.bpp_in_prediction();
        let filter_method = self.options.filter;

        let zlib_encoded = match self.options.compression {
            DeflateCompression::NoCompression => {
                let mut compressor =
                    fdeflate::StoredOnlyCompressor::new(std::io::Cursor::new(Vec::new()))?;
                for line in data.chunks(in_len) {
                    compressor.write_data(&[0])?;
                    compressor.write_data(line)?;
                }
                compressor.finish()?.into_inner()
            }
            DeflateCompression::FdeflateUltraFast => {
                let mut compressor = fdeflate::Compressor::new(std::io::Cursor::new(Vec::new()))?;

                let mut current = vec![0; in_len + 1];
                for line in data.chunks(in_len) {
                    let filter_type = filter(filter_method, bpp, prev, line, &mut current[1..]);

                    current[0] = filter_type as u8;
                    compressor.write_data(&current)?;
                    prev = line;
                }

                let compressed = compressor.finish()?.into_inner();
                if compressed.len()
                    > fdeflate::StoredOnlyCompressor::<()>::compressed_size((in_len + 1) * height)
                {
                    // Write uncompressed data since the result from fast compression would take
                    // more space than that.
                    //
                    // This is essentially a fallback to NoCompression.
                    let mut compressor =
                        fdeflate::StoredOnlyCompressor::new(std::io::Cursor::new(Vec::new()))?;
                    for line in data.chunks(in_len) {
                        compressor.write_data(&[0])?;
                        compressor.write_data(line)?;
                    }
                    compressor.finish()?.into_inner()
                } else {
                    compressed
                }
            }
            DeflateCompression::Level(level) => {
                let mut current = vec![0; in_len];

                let mut zlib =
                    ZlibEncoder::new(Vec::new(), flate2::Compression::new(u32::from(level)));
                for line in data.chunks(in_len) {
                    let filter_type = filter(filter_method, bpp, prev, line, &mut current);

                    zlib.write_all(&[filter_type as u8])?;
                    zlib.write_all(&current)?;
                    prev = line;
                }
                zlib.finish()?
            }
        };

        match self.info.frame_control {
            None => {
                self.write_zlib_encoded_idat(&zlib_encoded)?;
            }
            Some(_) if self.should_skip_frame_control_on_default_image() => {
                self.write_zlib_encoded_idat(&zlib_encoded)?;
            }
            Some(ref mut fctl) => {
                fctl.encode(&mut self.w)?;
                fctl.sequence_number = fctl.sequence_number.wrapping_add(1);
                self.animation_written += 1;

                // If the default image is the first frame of an animation, it's still an IDAT.
                if self.images_written == 0 {
                    self.write_zlib_encoded_idat(&zlib_encoded)?;
                } else {
                    let buff_size = zlib_encoded.len().min(Self::MAX_fdAT_CHUNK_LEN as usize);
                    let mut alldata = vec![0u8; 4 + buff_size];
                    for chunk in zlib_encoded.chunks(Self::MAX_fdAT_CHUNK_LEN as usize) {
                        alldata[..4].copy_from_slice(&fctl.sequence_number.to_be_bytes());
                        alldata[4..][..chunk.len()].copy_from_slice(chunk);
                        write_chunk(&mut self.w, chunk::fdAT, &alldata[..4 + chunk.len()])?;
                        fctl.sequence_number = fctl.sequence_number.wrapping_add(1);
                    }
                }
            }
        }

        self.increment_images_written();

        Ok(())
    }

    fn increment_images_written(&mut self) {
        self.images_written = self.images_written.saturating_add(1);

        if let Some(actl) = self.info.animation_control {
            if actl.num_frames <= self.animation_written {
                // If we've written all animation frames, all following will be normal image chunks.
                self.info.frame_control = None;
            }
        }
    }

    fn write_iend(&mut self) -> Result<()> {
        self.iend_written = true;
        self.write_chunk(chunk::IEND, &[])
    }

    fn should_skip_frame_control_on_default_image(&self) -> bool {
        self.options.sep_def_img && self.images_written == 0
    }

    fn write_zlib_encoded_idat(&mut self, zlib_encoded: &[u8]) -> Result<()> {
        for chunk in zlib_encoded.chunks(Self::MAX_IDAT_CHUNK_LEN as usize) {
            self.write_chunk(chunk::IDAT, chunk)?;
        }
        Ok(())
    }

    /// Set the used filter type.
    ///
    /// The default filter is [`Filter::Adaptive`] which automatically selects the best filter
    /// for each row of the image.
    ///
    /// You should only change this if you are after very fast compression,
    /// and either don't care about compression ratio or know exactly what works best for your images.
    pub fn set_filter(&mut self, filter: Filter) {
        self.options.filter = filter;
    }

    /// Set the fraction of time the following frames are going to be displayed,
    /// in seconds
    ///
    /// If the denominator is 0, it is to be treated as if it were 100
    /// (that is, the numerator then specifies 1/100ths of a second).
    /// If the value of the numerator is 0 the decoder should render the next frame
    /// as quickly as possible, though viewers may impose a reasonable lower bound.
    ///
    /// This method will return an error if the image is not animated.
    pub fn set_frame_delay(&mut self, numerator: u16, denominator: u16) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.delay_den = denominator;
            fctl.delay_num = numerator;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the dimension of the following frames.
    ///
    /// This function will return an error when:
    /// - The image is not an animated;
    ///
    /// - The selected dimension, considering also the current frame position,
    ///   goes outside the image boundaries;
    ///
    /// - One or both the width and height are 0;
    ///
    // ??? TODO ???
    // - The next frame is the default image
    pub fn set_frame_dimension(&mut self, width: u32, height: u32) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            if Some(width) > self.info.width.checked_sub(fctl.x_offset)
                || Some(height) > self.info.height.checked_sub(fctl.y_offset)
            {
                return Err(EncodingError::Format(FormatErrorKind::OutOfBounds.into()));
            } else if width == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroWidth.into()));
            } else if height == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroHeight.into()));
            }
            fctl.width = width;
            fctl.height = height;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the position of the following frames.
    ///
    /// An error will be returned if:
    /// - The image is not animated;
    ///
    /// - The selected position, considering also the current frame dimension,
    ///   goes outside the image boundaries;
    ///
    // ??? TODO ???
    // - The next frame is the default image
    pub fn set_frame_position(&mut self, x: u32, y: u32) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            if Some(x) > self.info.width.checked_sub(fctl.width)
                || Some(y) > self.info.height.checked_sub(fctl.height)
            {
                return Err(EncodingError::Format(FormatErrorKind::OutOfBounds.into()));
            }
            fctl.x_offset = x;
            fctl.y_offset = y;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the frame dimension to occupy all the image, starting from
    /// the current position.
    ///
    /// To reset the frame to the full image size [`reset_frame_position`]
    /// should be called first.
    ///
    /// This method will return an error if the image is not animated.
    ///
    /// [`reset_frame_position`]: Writer::reset_frame_position
    pub fn reset_frame_dimension(&mut self) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.width = self.info.width - fctl.x_offset;
            fctl.height = self.info.height - fctl.y_offset;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the frame position to (0, 0).
    ///
    /// Equivalent to calling [`set_frame_position(0, 0)`].
    ///
    /// This method will return an error if the image is not animated.
    ///
    /// [`set_frame_position(0, 0)`]: Writer::set_frame_position
    pub fn reset_frame_position(&mut self) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.x_offset = 0;
            fctl.y_offset = 0;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the blend operation for the following frames.
    ///
    /// The blend operation specifies whether the frame is to be alpha blended
    /// into the current output buffer content, or whether it should completely
    /// replace its region in the output buffer.
    ///
    /// See the [`BlendOp`] documentation for the possible values and their effects.
    ///
    /// *Note that for the first frame the two blend modes are functionally
    /// equivalent due to the clearing of the output buffer at the beginning
    /// of each play.*
    ///
    /// This method will return an error if the image is not animated.
    pub fn set_blend_op(&mut self, op: BlendOp) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.blend_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the dispose operation for the following frames.
    ///
    /// The dispose operation specifies how the output buffer should be changed
    /// at the end of the delay (before rendering the next frame)
    ///
    /// See the [`DisposeOp`] documentation for the possible values and their effects.
    ///
    /// *Note that if the first frame uses [`DisposeOp::Previous`]
    /// it will be treated as [`DisposeOp::Background`].*
    ///
    /// This method will return an error if the image is not animated.
    pub fn set_dispose_op(&mut self, op: DisposeOp) -> Result<()> {
        if let Some(ref mut fctl) = self.info.frame_control {
            fctl.dispose_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Create a stream writer.
    ///
    /// This allows you to create images that do not fit in memory. The default
    /// chunk size is 4K, use `stream_writer_with_size` to set another chunk
    /// size.
    ///
    /// This borrows the writer which allows for manually appending additional
    /// chunks after the image data has been written.
    pub fn stream_writer(&mut self) -> Result<StreamWriter<'_, W>> {
        self.stream_writer_with_size(DEFAULT_BUFFER_LENGTH)
    }

    /// Create a stream writer with custom buffer size.
    ///
    /// See [`stream_writer`].
    ///
    /// [`stream_writer`]: Self::stream_writer
    pub fn stream_writer_with_size(&mut self, size: usize) -> Result<StreamWriter<'_, W>> {
        StreamWriter::new(ChunkOutput::Borrowed(self), size)
    }

    /// Turn this into a stream writer for image data.
    ///
    /// This allows you to create images that do not fit in memory. The default
    /// chunk size is 4K, use [`stream_writer_with_size`] to set another chunk
    /// size.
    ///
    /// [`stream_writer_with_size`]: Self::stream_writer_with_size
    pub fn into_stream_writer(self) -> Result<StreamWriter<'static, W>> {
        self.into_stream_writer_with_size(DEFAULT_BUFFER_LENGTH)
    }

    /// Turn this into a stream writer with custom buffer size.
    ///
    /// See [`into_stream_writer`].
    ///
    /// [`into_stream_writer`]: Self::into_stream_writer
    pub fn into_stream_writer_with_size(self, size: usize) -> Result<StreamWriter<'static, W>> {
        StreamWriter::new(ChunkOutput::Owned(self), size)
    }

    /// Consume the stream writer with validation.
    ///
    /// Unlike a simple drop this ensures that the final chunk was written correctly. When other
    /// validation options (chunk sequencing) had been turned on in the configuration then it will
    /// also do a check on their correctness _before_ writing the final chunk.
    pub fn finish(mut self) -> Result<()> {
        self.validate_sequence_done()?;
        self.write_iend()?;
        self.w.flush()?;

        // Explicitly drop `self` just for clarity.
        drop(self);
        Ok(())
    }
}

impl<W: Write> Drop for Writer<W> {
    fn drop(&mut self) {
        if !self.iend_written {
            let _ = self.write_iend();
        }
    }
}

enum ChunkOutput<'a, W: Write> {
    Borrowed(&'a mut Writer<W>),
    Owned(Writer<W>),
}

// opted for deref for practical reasons
impl<'a, W: Write> Deref for ChunkOutput<'a, W> {
    type Target = Writer<W>;

    fn deref(&self) -> &Self::Target {
        match self {
            ChunkOutput::Borrowed(writer) => writer,
            ChunkOutput::Owned(writer) => writer,
        }
    }
}

impl<'a, W: Write> DerefMut for ChunkOutput<'a, W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            ChunkOutput::Borrowed(writer) => writer,
            ChunkOutput::Owned(writer) => writer,
        }
    }
}

/// This writer is used between the actual writer and the
/// ZlibEncoder and has the job of packaging the compressed
/// data into a PNG chunk, based on the image metadata
///
/// Currently the way it works is that the specified buffer
/// will hold one chunk at the time and buffer the incoming
/// data until `flush` is called or the maximum chunk size
/// is reached.
///
/// The maximum chunk is the smallest between the selected buffer size
/// and `u32::MAX >> 1` (`0x7fffffff` or `2147483647` dec)
///
/// When a chunk has to be flushed the length (that is now known)
/// and the CRC will be written at the correct locations in the chunk.
struct ChunkWriter<'a, W: Write> {
    writer: ChunkOutput<'a, W>,
    buffer: Vec<u8>,
    /// keeps track of where the last byte was written
    index: usize,
    curr_chunk: ChunkType,
}

impl<'a, W: Write> ChunkWriter<'a, W> {
    fn new(writer: ChunkOutput<'a, W>, buf_len: usize) -> ChunkWriter<'a, W> {
        // currently buf_len will determine the size of each chunk
        // the len is capped to the maximum size every chunk can hold
        // (this wont ever overflow an u32)
        //
        // TODO (maybe): find a way to hold two chunks at a time if `usize`
        //               is 64 bits.
        const CAP: usize = u32::MAX as usize >> 1;
        let curr_chunk = if writer.images_written == 0 {
            chunk::IDAT
        } else {
            chunk::fdAT
        };
        ChunkWriter {
            writer,
            buffer: vec![0; CAP.min(buf_len)],
            index: 0,
            curr_chunk,
        }
    }

    /// Returns the size of each scanline for the next frame
    /// paired with the size of the whole frame
    ///
    /// This is used by the `StreamWriter` to know when the scanline ends
    /// so it can filter compress it and also to know when to start
    /// the next one
    fn next_frame_info(&self) -> (usize, usize) {
        let wrt = self.writer.deref();

        let width: usize;
        let height: usize;
        if let Some(fctl) = wrt.info.frame_control {
            width = fctl.width as usize;
            height = fctl.height as usize;
        } else {
            width = wrt.info.width as usize;
            height = wrt.info.height as usize;
        }

        let in_len = wrt.info.raw_row_length_from_width(width as u32) - 1;
        let data_size = in_len * height;

        (in_len, data_size)
    }

    /// NOTE: this bypasses the internal buffer so the flush method should be called before this
    ///       in the case there is some data left in the buffer when this is called, it will panic
    fn write_header(&mut self) -> Result<()> {
        assert_eq!(self.index, 0, "Called when not flushed");
        let wrt = self.writer.deref_mut();

        self.curr_chunk = if wrt.images_written == 0 {
            chunk::IDAT
        } else {
            chunk::fdAT
        };

        match wrt.info.frame_control {
            Some(_) if wrt.should_skip_frame_control_on_default_image() => {}
            Some(ref mut fctl) => {
                fctl.encode(&mut wrt.w)?;
                fctl.sequence_number += 1;
            }
            _ => {}
        }

        Ok(())
    }

    /// Set the [`FrameControl`] for the following frame
    ///
    /// It will ignore the `sequence_number` of the parameter
    /// as it is updated internally.
    fn set_fctl(&mut self, f: FrameControl) {
        if let Some(ref mut fctl) = self.writer.info.frame_control {
            // Ignore the sequence number
            *fctl = FrameControl {
                sequence_number: fctl.sequence_number,
                ..f
            };
        } else {
            panic!("This function must be called on an animated PNG")
        }
    }

    /// Flushes the current chunk
    fn flush_inner(&mut self) -> io::Result<()> {
        if self.index > 0 {
            // flush the chunk and reset everything
            write_chunk(
                &mut self.writer.w,
                self.curr_chunk,
                &self.buffer[..self.index],
            )?;

            self.index = 0;
        }
        Ok(())
    }
}

impl<'a, W: Write> Write for ChunkWriter<'a, W> {
    fn write(&mut self, mut data: &[u8]) -> io::Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        // index == 0 means a chunk has been flushed out
        if self.index == 0 {
            let wrt = self.writer.deref_mut();

            // Prepare the next animated frame, if any.
            let no_fctl = wrt.should_skip_frame_control_on_default_image();
            if wrt.info.frame_control.is_some() && !no_fctl {
                let fctl = wrt.info.frame_control.as_mut().unwrap();
                self.buffer[0..4].copy_from_slice(&fctl.sequence_number.to_be_bytes());
                fctl.sequence_number += 1;
                self.index = 4;
            }
        }

        // Cap the buffer length to the maximum number of bytes that can't still
        // be added to the current chunk
        let written = data.len().min(self.buffer.len() - self.index);
        data = &data[..written];

        self.buffer[self.index..][..written].copy_from_slice(data);
        self.index += written;

        // if the maximum data for this chunk as been reached it needs to be flushed
        if self.index == self.buffer.len() {
            self.flush_inner()?;
        }

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_inner()
    }
}

impl<W: Write> Drop for ChunkWriter<'_, W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

// TODO: find a better name
//
/// This enum is used to be allow the `StreamWriter` to keep
/// its inner `ChunkWriter` without wrapping it inside a
/// `ZlibEncoder`. This is used in the case that between the
/// change of state that happens when the last write of a frame
/// is performed an error occurs, which obviously has to be returned.
/// This creates the problem of where to store the writer before
/// exiting the function, and this is where `Wrapper` comes in.
///
/// Unfortunately the `ZlibWriter` can't be used because on the
/// write following the error, `finish` would be called and that
/// would write some data even if 0 bytes where compressed.
///
/// If the `finish` function fails then there is nothing much to
/// do as the `ChunkWriter` would get lost so the `Unrecoverable`
/// variant is used to signal that.
enum Wrapper<'a, W: Write> {
    Chunk(ChunkWriter<'a, W>),
    Flate2(ZlibEncoder<ChunkWriter<'a, W>>),
    FDeflate(fdeflate::Compressor<ChunkWriter<'a, W>>),
    Unrecoverable,
    /// This is used in-between, should never be matched
    None,
}

impl<'a, W: Write> Wrapper<'a, W> {
    fn from_level(writer: ChunkWriter<'a, W>, compression: DeflateCompression) -> io::Result<Self> {
        Ok(match compression {
            DeflateCompression::NoCompression => {
                Wrapper::Flate2(ZlibEncoder::new(writer, flate2::Compression::none()))
            }
            DeflateCompression::FdeflateUltraFast => {
                Wrapper::FDeflate(fdeflate::Compressor::new(writer)?)
            }
            DeflateCompression::Level(level) => Wrapper::Flate2(ZlibEncoder::new(
                writer,
                flate2::Compression::new(u32::from(level)),
            )),
        })
    }

    /// Like `Option::take` this returns the `Wrapper` contained
    /// in `self` and replaces it with `Wrapper::None`
    fn take(&mut self) -> Wrapper<'a, W> {
        let mut swap = Wrapper::None;
        mem::swap(self, &mut swap);
        swap
    }
}

/// Streaming PNG writer
///
/// This may silently fail in the destructor, so it is a good idea to call
/// [`finish`] or [`flush`] before dropping.
///
/// [`finish`]: Self::finish
/// [`flush`]: Write::flush
pub struct StreamWriter<'a, W: Write> {
    /// The option here is needed in order to access the inner `ChunkWriter` in-between
    /// each frame, which is needed for writing the fcTL chunks between each frame
    writer: Wrapper<'a, W>,
    prev_buf: Vec<u8>,
    curr_buf: Vec<u8>,
    filtered_buf: Vec<u8>,
    /// Amount of data already written
    index: usize,
    /// length of the current scanline
    line_len: usize,
    /// size of the frame (width * height * sample_size)
    to_write: usize,

    width: u32,
    height: u32,

    bpp: BytesPerPixel,
    filter: Filter,
    fctl: Option<FrameControl>,
    compression: DeflateCompression,
}

impl<'a, W: Write> StreamWriter<'a, W> {
    fn new(writer: ChunkOutput<'a, W>, buf_len: usize) -> Result<StreamWriter<'a, W>> {
        let PartialInfo {
            width,
            height,
            frame_control: fctl,
            ..
        } = writer.info;

        let bpp = writer.info.bpp_in_prediction();
        let in_len = writer.info.raw_row_length() - 1;
        let filter = writer.options.filter;
        let compression = writer.options.compression;
        let prev_buf = vec![0; in_len];
        let curr_buf = vec![0; in_len];
        let filtered_buf = vec![0; in_len];

        let mut chunk_writer = ChunkWriter::new(writer, buf_len);
        let (line_len, to_write) = chunk_writer.next_frame_info();
        chunk_writer.write_header()?;

        Ok(StreamWriter {
            writer: Wrapper::from_level(chunk_writer, compression)?,
            index: 0,
            prev_buf,
            curr_buf,
            filtered_buf,
            bpp,
            filter,
            width,
            height,
            line_len,
            to_write,
            fctl,
            compression,
        })
    }

    /// Set the used filter type.
    ///
    /// The default filter is [`Filter::Adaptive`] which automatically selects the best filter
    /// for each row of the image.
    ///
    /// You should only change this if you are after very fast compression,
    /// and either don't care about compression ratio or know exactly what works best for your images.
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
    }

    /// Set the fraction of time the following frames are going to be displayed,
    /// in seconds
    ///
    /// If the denominator is 0, it is to be treated as if it were 100
    /// (that is, the numerator then specifies 1/100ths of a second).
    /// If the value of the numerator is 0 the decoder should render the next frame
    /// as quickly as possible, though viewers may impose a reasonable lower bound.
    ///
    /// This method will return an error if the image is not animated.
    pub fn set_frame_delay(&mut self, numerator: u16, denominator: u16) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.delay_den = denominator;
            fctl.delay_num = numerator;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the dimension of the following frames.
    ///
    /// This function will return an error when:
    /// - The image is not an animated;
    ///
    /// - The selected dimension, considering also the current frame position,
    ///   goes outside the image boundaries;
    ///
    /// - One or both the width and height are 0;
    ///
    pub fn set_frame_dimension(&mut self, width: u32, height: u32) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            if Some(width) > self.width.checked_sub(fctl.x_offset)
                || Some(height) > self.height.checked_sub(fctl.y_offset)
            {
                return Err(EncodingError::Format(FormatErrorKind::OutOfBounds.into()));
            } else if width == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroWidth.into()));
            } else if height == 0 {
                return Err(EncodingError::Format(FormatErrorKind::ZeroHeight.into()));
            }
            fctl.width = width;
            fctl.height = height;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the position of the following frames.
    ///
    /// An error will be returned if:
    /// - The image is not animated;
    ///
    /// - The selected position, considering also the current frame dimension,
    ///   goes outside the image boundaries;
    ///
    pub fn set_frame_position(&mut self, x: u32, y: u32) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            if Some(x) > self.width.checked_sub(fctl.width)
                || Some(y) > self.height.checked_sub(fctl.height)
            {
                return Err(EncodingError::Format(FormatErrorKind::OutOfBounds.into()));
            }
            fctl.x_offset = x;
            fctl.y_offset = y;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the frame dimension to occupy all the image, starting from
    /// the current position.
    ///
    /// To reset the frame to the full image size [`reset_frame_position`]
    /// should be called first.
    ///
    /// This method will return an error if the image is not animated.
    ///
    /// [`reset_frame_position`]: Writer::reset_frame_position
    pub fn reset_frame_dimension(&mut self) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.width = self.width - fctl.x_offset;
            fctl.height = self.height - fctl.y_offset;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the frame position to (0, 0).
    ///
    /// Equivalent to calling [`set_frame_position(0, 0)`].
    ///
    /// This method will return an error if the image is not animated.
    ///
    /// [`set_frame_position(0, 0)`]: Writer::set_frame_position
    pub fn reset_frame_position(&mut self) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.x_offset = 0;
            fctl.y_offset = 0;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the blend operation for the following frames.
    ///
    /// The blend operation specifies whether the frame is to be alpha blended
    /// into the current output buffer content, or whether it should completely
    /// replace its region in the output buffer.
    ///
    /// See the [`BlendOp`] documentation for the possible values and their effects.
    ///
    /// *Note that for the first frame the two blend modes are functionally
    /// equivalent due to the clearing of the output buffer at the beginning
    /// of each play.*
    ///
    /// This method will return an error if the image is not animated.
    pub fn set_blend_op(&mut self, op: BlendOp) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.blend_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Set the dispose operation for the following frames.
    ///
    /// The dispose operation specifies how the output buffer should be changed
    /// at the end of the delay (before rendering the next frame)
    ///
    /// See the [`DisposeOp`] documentation for the possible values and their effects.
    ///
    /// *Note that if the first frame uses [`DisposeOp::Previous`]
    /// it will be treated as [`DisposeOp::Background`].*
    ///
    /// This method will return an error if the image is not animated.
    pub fn set_dispose_op(&mut self, op: DisposeOp) -> Result<()> {
        if let Some(ref mut fctl) = self.fctl {
            fctl.dispose_op = op;
            Ok(())
        } else {
            Err(EncodingError::Format(FormatErrorKind::NotAnimated.into()))
        }
    }

    /// Consume the stream writer with validation.
    ///
    /// Unlike a simple drop this ensures that the all data was written correctly. When other
    /// validation options (chunk sequencing) had been turned on in the configuration of inner
    /// [`Writer`], then it will also do a check on their correctness. Differently from
    /// [`Writer::finish`], this just `flush`es, returns error if some data is abandoned.
    pub fn finish(mut self) -> Result<()> {
        self.finish_mut()
    }

    /// Internal helper that can be called both from `fn finish(mut self)`
    /// and from `fn drop(&mut self)`.
    fn finish_mut(&mut self) -> Result<()> {
        if self.to_write > 0 {
            let err = FormatErrorKind::MissingData(self.to_write).into();
            return Err(EncodingError::Format(err));
        }

        self.flush()?;
        match self.writer.take() {
            Wrapper::Chunk(wrt) => {
                wrt.writer.validate_sequence_done()?;
            }
            Wrapper::FDeflate(wrt) => {
                wrt.finish()?;
            }
            Wrapper::Flate2(wrt) => {
                wrt.finish()?;
            }
            Wrapper::None => unreachable!(),
            Wrapper::Unrecoverable => {
                let err = FormatErrorKind::Unrecoverable.into();
                return Err(EncodingError::Format(err));
            }
        }

        Ok(())
    }

    /// Flushes the buffered chunk, checks if it was the last frame,
    /// writes the next frame header and gets the next frame scanline size
    /// and image size.
    /// NOTE: This method must only be called when the writer is the variant Chunk(_)
    fn new_frame(&mut self) -> Result<()> {
        let wrt = match &mut self.writer {
            Wrapper::Chunk(wrt) => wrt,
            Wrapper::Unrecoverable => {
                let err = FormatErrorKind::Unrecoverable.into();
                return Err(EncodingError::Format(err));
            }
            Wrapper::Flate2(_) | Wrapper::FDeflate(_) => {
                unreachable!("never called on a half-finished frame")
            }
            Wrapper::None => unreachable!(),
        };
        wrt.flush()?;
        wrt.writer.validate_new_image()?;

        if let Some(fctl) = self.fctl {
            wrt.set_fctl(fctl);
        }
        let (scansize, size) = wrt.next_frame_info();
        self.line_len = scansize;
        self.to_write = size;

        wrt.write_header()?;
        wrt.writer.increment_images_written();

        // now it can be taken because the next statements cannot cause any errors
        match self.writer.take() {
            Wrapper::Chunk(wrt) => match Wrapper::from_level(wrt, self.compression) {
                Ok(writer) => self.writer = writer,
                Err(err) => {
                    self.writer = Wrapper::Unrecoverable;
                    return Err(err.into());
                }
            },
            _ => unreachable!(),
        };

        Ok(())
    }
}

impl<'a, W: Write> Write for StreamWriter<'a, W> {
    fn write(&mut self, mut data: &[u8]) -> io::Result<usize> {
        if let Wrapper::Unrecoverable = self.writer {
            let err = FormatErrorKind::Unrecoverable.into();
            return Err(EncodingError::Format(err).into());
        }

        if data.is_empty() {
            return Ok(0);
        }

        if self.to_write == 0 {
            match self.writer.take() {
                Wrapper::Flate2(wrt) => match wrt.finish() {
                    Ok(chunk) => self.writer = Wrapper::Chunk(chunk),
                    Err(err) => {
                        self.writer = Wrapper::Unrecoverable;
                        return Err(err);
                    }
                },
                Wrapper::FDeflate(wrt) => match wrt.finish() {
                    Ok(chunk) => self.writer = Wrapper::Chunk(chunk),
                    Err(err) => {
                        self.writer = Wrapper::Unrecoverable;
                        return Err(err);
                    }
                },
                chunk @ Wrapper::Chunk(_) => self.writer = chunk,
                Wrapper::Unrecoverable => unreachable!(),
                Wrapper::None => unreachable!(),
            };

            // Transition Wrapper::Chunk to Wrapper::Zlib.
            self.new_frame()?;
        }

        let written = data.read(&mut self.curr_buf[..self.line_len][self.index..])?;
        self.index += written;
        self.to_write -= written;

        if self.index == self.line_len {
            let filter_type = filter(
                self.filter,
                self.bpp,
                &self.prev_buf,
                &self.curr_buf,
                &mut self.filtered_buf,
            );
            // This can't fail as the other variant is used only to allow the zlib encoder to finish
            match &mut self.writer {
                Wrapper::Flate2(wrt) => {
                    wrt.write_all(&[filter_type as u8])?;
                    wrt.write_all(&self.filtered_buf)?;
                }
                Wrapper::FDeflate(wrt) => {
                    wrt.write_data(&[filter_type as u8])?;
                    wrt.write_data(&self.filtered_buf)?;
                }
                _ => unreachable!(),
            };

            mem::swap(&mut self.prev_buf, &mut self.curr_buf);
            self.index = 0;
        }

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.writer {
            Wrapper::Flate2(wrt) => wrt.flush()?,
            Wrapper::Chunk(wrt) => wrt.flush()?,
            Wrapper::FDeflate(_) => (), // TODO: Add `flush()` to `fdeflate::Compressor`?
            // This handles both the case where we entered an unrecoverable state after zlib
            // decoding failure and after a panic while we had taken the chunk/zlib reader.
            Wrapper::Unrecoverable | Wrapper::None => {
                let err = FormatErrorKind::Unrecoverable.into();
                return Err(EncodingError::Format(err).into());
            }
        }

        if self.index > 0 {
            let err = FormatErrorKind::WrittenTooMuch(self.index).into();
            return Err(EncodingError::Format(err).into());
        }

        Ok(())
    }
}

impl<W: Write> Drop for StreamWriter<'_, W> {
    fn drop(&mut self) {
        let _ = self.finish_mut();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Decoder;

    use io::BufReader;
    use rand::{rng, Rng};
    use std::cmp;
    use std::fs::File;
    use std::io::Cursor;

    #[test]
    fn roundtrip1() {
        roundtrip_inner();
    }

    #[test]
    fn roundtrip2() {
        roundtrip_inner();
    }

    fn roundtrip_inner() {
        // More loops = more random testing, but also more test wait time
        for _ in 0..5 {
            for path in glob::glob("tests/pngsuite/*.png")
                .unwrap()
                .map(|r| r.unwrap())
            {
                if path.file_name().unwrap().to_str().unwrap().starts_with('x') {
                    // x* files are expected to fail to decode
                    continue;
                }
                eprintln!("{}", path.display());
                // Decode image
                let decoder = Decoder::new(BufReader::new(File::open(path).unwrap()));
                let mut reader = decoder.read_info().unwrap();
                let mut buf = vec![0; reader.output_buffer_size().unwrap()];
                let info = reader.next_frame(&mut buf).unwrap();
                use DeflateCompression::*;
                for compression in [NoCompression, FdeflateUltraFast, Level(4)] {
                    // Encode decoded image
                    let mut out = Vec::new();
                    {
                        let mut wrapper = RandomChunkWriter {
                            rng: rng(),
                            w: &mut out,
                        };

                        let mut encoder = Encoder::new(&mut wrapper, info.width, info.height);
                        encoder.set_color(info.color_type);
                        encoder.set_depth(info.bit_depth);
                        encoder.set_deflate_compression(compression);
                        if let Some(palette) = &reader.info().palette {
                            encoder.set_palette(palette.clone());
                        }
                        let mut encoder = encoder.write_header().unwrap();
                        encoder.write_image_data(&buf).unwrap();
                    }
                    // Decode encoded decoded image
                    let decoder = Decoder::new(Cursor::new(&*out));
                    let mut reader = decoder.read_info().unwrap();
                    let mut buf2 = vec![0; reader.output_buffer_size().unwrap()];
                    reader.next_frame(&mut buf2).unwrap();
                    // check if the encoded image is ok:
                    assert_eq!(buf, buf2);
                }
            }
        }
    }

    #[test]
    fn roundtrip_stream1() {
        roundtrip_stream_inner();
    }

    #[test]
    fn roundtrip_stream2() {
        roundtrip_stream_inner();
    }

    fn roundtrip_stream_inner() {
        // More loops = more random testing, but also more test wait time
        for _ in 0..5 {
            for path in glob::glob("tests/pngsuite/*.png")
                .unwrap()
                .map(|r| r.unwrap())
            {
                if path.file_name().unwrap().to_str().unwrap().starts_with('x') {
                    // x* files are expected to fail to decode
                    continue;
                }
                // Decode image
                let decoder = Decoder::new(BufReader::new(File::open(path).unwrap()));
                let mut reader = decoder.read_info().unwrap();
                let mut buf = vec![0; reader.output_buffer_size().unwrap()];
                let info = reader.next_frame(&mut buf).unwrap();
                use DeflateCompression::*;
                for compression in [NoCompression, FdeflateUltraFast, Level(4)] {
                    // Encode decoded image
                    let mut out = Vec::new();
                    {
                        let mut wrapper = RandomChunkWriter {
                            rng: rng(),
                            w: &mut out,
                        };

                        let mut encoder = Encoder::new(&mut wrapper, info.width, info.height);
                        encoder.set_color(info.color_type);
                        encoder.set_depth(info.bit_depth);
                        encoder.set_deflate_compression(compression);
                        if let Some(palette) = &reader.info().palette {
                            encoder.set_palette(palette.clone());
                        }
                        let mut encoder = encoder.write_header().unwrap();
                        let mut stream_writer = encoder.stream_writer().unwrap();

                        let mut outer_wrapper = RandomChunkWriter {
                            rng: rng(),
                            w: &mut stream_writer,
                        };

                        outer_wrapper.write_all(&buf).unwrap();
                    }
                    // Decode encoded decoded image
                    let decoder = Decoder::new(Cursor::new(&*out));
                    let mut reader = decoder.read_info().unwrap();
                    let mut buf2 = vec![0; reader.output_buffer_size().unwrap()];
                    reader.next_frame(&mut buf2).unwrap();
                    // check if the encoded image is ok:
                    assert_eq!(buf, buf2);
                }
            }
        }
    }

    #[test]
    fn image_palette() -> Result<()> {
        for &bit_depth in &[1u8, 2, 4, 8] {
            // Do a reference decoding, choose a fitting palette image from pngsuite
            let path = format!("tests/pngsuite/basn3p0{}.png", bit_depth);
            let decoder = Decoder::new(BufReader::new(File::open(&path).unwrap()));
            let mut reader = decoder.read_info().unwrap();

            let mut decoded_pixels = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.info();
            assert_eq!(
                info.width as usize * info.height as usize * usize::from(bit_depth),
                decoded_pixels.len() * 8
            );
            let info = reader.next_frame(&mut decoded_pixels).unwrap();
            let indexed_data = decoded_pixels;

            let palette = reader.info().palette.as_ref().unwrap();
            let mut out = Vec::new();
            {
                let mut encoder = Encoder::new(&mut out, info.width, info.height);
                encoder.set_depth(BitDepth::from_u8(bit_depth).unwrap());
                encoder.set_color(ColorType::Indexed);
                encoder.set_palette(palette.as_ref());

                let mut writer = encoder.write_header().unwrap();
                writer.write_image_data(&indexed_data).unwrap();
            }

            // Decode re-encoded image
            let decoder = Decoder::new(Cursor::new(&*out));
            let mut reader = decoder.read_info().unwrap();
            let mut redecoded = vec![0; reader.output_buffer_size().unwrap()];
            reader.next_frame(&mut redecoded).unwrap();
            // check if the encoded image is ok:
            assert_eq!(indexed_data, redecoded);
        }
        Ok(())
    }

    #[test]
    fn expect_error_on_wrong_image_len() -> Result<()> {
        let width = 10;
        let height = 10;

        let output = vec![0u8; 1024];
        let writer = Cursor::new(output);
        let mut encoder = Encoder::new(writer, width as u32, height as u32);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Rgb);
        let mut png_writer = encoder.write_header()?;

        let correct_image_size = width * height * 3;
        let image = vec![0u8; correct_image_size + 1];
        let result = png_writer.write_image_data(image.as_ref());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn expect_error_on_empty_image() -> Result<()> {
        let output = vec![0u8; 1024];
        let mut writer = Cursor::new(output);

        let encoder = Encoder::new(&mut writer, 0, 0);
        assert!(encoder.write_header().is_err());

        let encoder = Encoder::new(&mut writer, 100, 0);
        assert!(encoder.write_header().is_err());

        let encoder = Encoder::new(&mut writer, 0, 100);
        assert!(encoder.write_header().is_err());

        Ok(())
    }

    #[test]
    fn expect_error_on_invalid_bit_depth_color_type_combination() -> Result<()> {
        let output = vec![0u8; 1024];
        let mut writer = Cursor::new(output);

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::One);
        encoder.set_color(ColorType::Rgb);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::One);
        encoder.set_color(ColorType::GrayscaleAlpha);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::One);
        encoder.set_color(ColorType::Rgba);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Two);
        encoder.set_color(ColorType::Rgb);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Two);
        encoder.set_color(ColorType::GrayscaleAlpha);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Two);
        encoder.set_color(ColorType::Rgba);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Four);
        encoder.set_color(ColorType::Rgb);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Four);
        encoder.set_color(ColorType::GrayscaleAlpha);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Four);
        encoder.set_color(ColorType::Rgba);
        assert!(encoder.write_header().is_err());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Sixteen);
        encoder.set_color(ColorType::Indexed);
        assert!(encoder.write_header().is_err());

        Ok(())
    }

    #[test]
    fn can_write_header_with_valid_bit_depth_color_type_combination() -> Result<()> {
        let output = vec![0u8; 1024];
        let mut writer = Cursor::new(output);

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::One);
        encoder.set_color(ColorType::Grayscale);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::One);
        encoder.set_color(ColorType::Indexed);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Two);
        encoder.set_color(ColorType::Grayscale);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Two);
        encoder.set_color(ColorType::Indexed);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Four);
        encoder.set_color(ColorType::Grayscale);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Four);
        encoder.set_color(ColorType::Indexed);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Rgb);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Indexed);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::GrayscaleAlpha);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Rgba);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Sixteen);
        encoder.set_color(ColorType::Grayscale);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Sixteen);
        encoder.set_color(ColorType::Rgb);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Sixteen);
        encoder.set_color(ColorType::GrayscaleAlpha);
        assert!(encoder.write_header().is_ok());

        let mut encoder = Encoder::new(&mut writer, 1, 1);
        encoder.set_depth(BitDepth::Sixteen);
        encoder.set_color(ColorType::Rgba);
        assert!(encoder.write_header().is_ok());

        Ok(())
    }

    #[test]
    fn all_filters_roundtrip() -> io::Result<()> {
        let pixel: Vec<_> = (0..48).collect();

        let roundtrip = |filter: Filter| -> io::Result<()> {
            let mut buffer = vec![];
            let mut encoder = Encoder::new(&mut buffer, 4, 4);
            encoder.set_depth(BitDepth::Eight);
            encoder.set_color(ColorType::Rgb);
            encoder.set_filter(filter);
            encoder.write_header()?.write_image_data(&pixel)?;

            let decoder = crate::Decoder::new(Cursor::new(buffer));
            let mut reader = decoder.read_info()?;
            let info = reader.info();
            assert_eq!(info.width, 4);
            assert_eq!(info.height, 4);
            let mut dest = vec![0; pixel.len()];
            reader.next_frame(&mut dest)?;
            assert_eq!(dest, pixel, "Deviation with filter type {:?}", filter);

            Ok(())
        };

        roundtrip(Filter::NoFilter)?;
        roundtrip(Filter::Sub)?;
        roundtrip(Filter::Up)?;
        roundtrip(Filter::Avg)?;
        roundtrip(Filter::Paeth)?;

        Ok(())
    }

    #[test]
    fn some_gamma_roundtrip() -> io::Result<()> {
        let pixel: Vec<_> = (0..48).collect();

        let roundtrip = |gamma: Option<ScaledFloat>| -> io::Result<()> {
            let mut buffer = vec![];
            let mut encoder = Encoder::new(&mut buffer, 4, 4);
            encoder.set_depth(BitDepth::Eight);
            encoder.set_color(ColorType::Rgb);
            encoder.set_filter(Filter::Avg);
            if let Some(gamma) = gamma {
                encoder.set_source_gamma(gamma);
            }
            encoder.write_header()?.write_image_data(&pixel)?;

            let decoder = crate::Decoder::new(Cursor::new(buffer));
            let mut reader = decoder.read_info()?;
            assert_eq!(
                reader.info().gamma(),
                gamma,
                "Deviation with gamma {:?}",
                gamma
            );
            let mut dest = vec![0; pixel.len()];
            let info = reader.next_frame(&mut dest)?;
            assert_eq!(info.width, 4);
            assert_eq!(info.height, 4);

            Ok(())
        };

        roundtrip(None)?;
        roundtrip(Some(ScaledFloat::new(0.35)))?;
        roundtrip(Some(ScaledFloat::new(0.45)))?;
        roundtrip(Some(ScaledFloat::new(0.55)))?;
        roundtrip(Some(ScaledFloat::new(0.7)))?;
        roundtrip(Some(ScaledFloat::new(1.0)))?;
        roundtrip(Some(ScaledFloat::new(2.5)))?;

        Ok(())
    }

    #[test]
    fn write_image_chunks_beyond_first() -> Result<()> {
        let width = 10;
        let height = 10;

        let output = vec![0u8; 1024];
        let writer = Cursor::new(output);

        // Not an animation but we should still be able to write multiple images
        // See issue: <https://github.com/image-rs/image-png/issues/301>
        // This is technically all valid png so there is no issue with correctness.
        let mut encoder = Encoder::new(writer, width, height);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        let mut png_writer = encoder.write_header()?;

        for _ in 0..3 {
            let correct_image_size = (width * height) as usize;
            let image = vec![0u8; correct_image_size];
            png_writer.write_image_data(image.as_ref())?;
        }

        Ok(())
    }

    #[test]
    fn image_validate_sequence_without_animation() -> Result<()> {
        let width = 10;
        let height = 10;

        let output = vec![0u8; 1024];
        let writer = Cursor::new(output);

        let mut encoder = Encoder::new(writer, width, height);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        encoder.validate_sequence(true);
        let mut png_writer = encoder.write_header()?;

        let correct_image_size = (width * height) as usize;
        let image = vec![0u8; correct_image_size];
        png_writer.write_image_data(image.as_ref())?;

        assert!(png_writer.write_image_data(image.as_ref()).is_err());
        Ok(())
    }

    #[test]
    fn image_validate_animation() -> Result<()> {
        let width = 10;
        let height = 10;

        let output = vec![0u8; 1024];
        let writer = Cursor::new(output);
        let correct_image_size = (width * height) as usize;
        let image = vec![0u8; correct_image_size];

        let mut encoder = Encoder::new(writer, width, height);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        encoder.set_animated(1, 0)?;
        encoder.validate_sequence(true);
        let mut png_writer = encoder.write_header()?;

        png_writer.write_image_data(image.as_ref())?;

        Ok(())
    }

    #[test]
    fn image_validate_animation2() -> Result<()> {
        let width = 10;
        let height = 10;

        let output = vec![0u8; 1024];
        let writer = Cursor::new(output);
        let correct_image_size = (width * height) as usize;
        let image = vec![0u8; correct_image_size];

        let mut encoder = Encoder::new(writer, width, height);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        encoder.set_animated(2, 0)?;
        encoder.validate_sequence(true);
        let mut png_writer = encoder.write_header()?;

        png_writer.write_image_data(image.as_ref())?;
        png_writer.write_image_data(image.as_ref())?;
        png_writer.finish()?;

        Ok(())
    }

    #[test]
    fn image_validate_animation_sep_def_image() -> Result<()> {
        let width = 10;
        let height = 10;

        let output = vec![0u8; 1024];
        let writer = Cursor::new(output);
        let correct_image_size = (width * height) as usize;
        let image = vec![0u8; correct_image_size];

        let mut encoder = Encoder::new(writer, width, height);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        encoder.set_animated(1, 0)?;
        encoder.set_sep_def_img(true)?;
        encoder.validate_sequence(true);
        let mut png_writer = encoder.write_header()?;

        png_writer.write_image_data(image.as_ref())?;
        png_writer.write_image_data(image.as_ref())?;
        png_writer.finish()?;

        Ok(())
    }

    #[test]
    fn image_validate_missing_image() -> Result<()> {
        let width = 10;
        let height = 10;

        let output = vec![0u8; 1024];
        let writer = Cursor::new(output);

        let mut encoder = Encoder::new(writer, width, height);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        encoder.validate_sequence(true);
        let png_writer = encoder.write_header()?;

        assert!(png_writer.finish().is_err());
        Ok(())
    }

    #[test]
    fn image_validate_missing_animated_frame() -> Result<()> {
        let width = 10;
        let height = 10;

        let output = vec![0u8; 1024];
        let writer = Cursor::new(output);
        let correct_image_size = (width * height) as usize;
        let image = vec![0u8; correct_image_size];

        let mut encoder = Encoder::new(writer, width, height);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        encoder.set_animated(2, 0)?;
        encoder.validate_sequence(true);
        let mut png_writer = encoder.write_header()?;

        png_writer.write_image_data(image.as_ref())?;
        assert!(png_writer.finish().is_err());

        Ok(())
    }

    #[test]
    fn issue_307_stream_validation() -> Result<()> {
        let output = vec![0u8; 1024];
        let mut cursor = Cursor::new(output);

        let encoder = Encoder::new(&mut cursor, 1, 1); // Create a 1-pixel image
        let mut writer = encoder.write_header()?;
        let mut stream = writer.stream_writer()?;

        let written = stream.write(&[1, 2, 3, 4])?;
        assert_eq!(written, 1);
        stream.finish()?;
        drop(writer);

        {
            cursor.set_position(0);
            let mut decoder = Decoder::new(cursor).read_info().expect("A valid image");
            let mut buffer = [0u8; 1];
            decoder.next_frame(&mut buffer[..]).expect("Valid read");
            assert_eq!(buffer, [1]);
        }

        Ok(())
    }

    #[test]
    fn stream_filtering() -> Result<()> {
        let output = vec![0u8; 1024];
        let mut cursor = Cursor::new(output);

        let mut encoder = Encoder::new(&mut cursor, 8, 8);
        encoder.set_color(ColorType::Rgba);
        encoder.set_filter(Filter::Paeth);
        let mut writer = encoder.write_header()?;
        let mut stream = writer.stream_writer()?;

        for _ in 0..8 {
            let written = stream.write(&[1; 32])?;
            assert_eq!(written, 32);
        }
        stream.finish()?;
        drop(writer);

        {
            cursor.set_position(0);
            let mut decoder = Decoder::new(cursor).read_info().expect("A valid image");
            let mut buffer = [0u8; 256];
            decoder.next_frame(&mut buffer[..]).expect("Valid read");
            assert_eq!(buffer, [1; 256]);
        }

        Ok(())
    }

    fn test_stream_flushing(compression: Compression) -> Result<()> {
        let output = vec![0u8; 1024];
        let mut cursor = Cursor::new(output);

        let mut encoder = Encoder::new(&mut cursor, 8, 8);
        encoder.set_color(ColorType::Rgba);
        encoder.set_compression(compression);
        let mut writer = encoder.write_header()?;
        let mut stream = writer.stream_writer()?;

        for _ in 0..8 {
            let written = stream.write(&[1; 32])?;
            assert_eq!(written, 32);
            stream.flush()?;
        }
        stream.finish()?;
        drop(writer);

        {
            cursor.set_position(0);
            let mut decoder = Decoder::new(cursor).read_info().expect("A valid image");
            let mut buffer = [0u8; 256];
            decoder.next_frame(&mut buffer[..]).expect("Valid read");
            assert_eq!(buffer, [1; 256]);
        }

        Ok(())
    }

    #[test]
    fn stream_flushing_with_high_compression() -> Result<()> {
        test_stream_flushing(Compression::High)
    }

    #[test]
    fn stream_flushing_with_balanced_compression() -> Result<()> {
        test_stream_flushing(Compression::Balanced)
    }

    #[test]
    fn stream_flushing_with_fast_compression() -> Result<()> {
        test_stream_flushing(Compression::Fast)
    }

    #[test]
    fn stream_flushing_with_fastest_compression() -> Result<()> {
        test_stream_flushing(Compression::Fastest)
    }

    #[test]
    fn stream_flushing_with_no_compression() -> Result<()> {
        test_stream_flushing(Compression::NoCompression)
    }

    #[test]
    #[cfg(all(unix, not(target_pointer_width = "32")))]
    fn exper_error_on_huge_chunk() -> Result<()> {
        // Okay, so we want a proper 4 GB chunk but not actually spend the memory for reserving it.
        // Let's rely on overcommit? Otherwise we got the rather dumb option of mmap-ing /dev/zero.
        let empty = vec![0; 1usize << 31];
        let writer = Cursor::new(vec![0u8; 1024]);

        let mut encoder = Encoder::new(writer, 10, 10);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        let mut png_writer = encoder.write_header()?;

        assert!(png_writer.write_chunk(chunk::fdAT, &empty).is_err());
        Ok(())
    }

    #[test]
    #[cfg(all(unix, not(target_pointer_width = "32")))]
    fn exper_error_on_non_u32_chunk() -> Result<()> {
        // Okay, so we want a proper 4 GB chunk but not actually spend the memory for reserving it.
        // Let's rely on overcommit? Otherwise we got the rather dumb option of mmap-ing /dev/zero.
        let empty = vec![0; 1usize << 32];
        let writer = Cursor::new(vec![0u8; 1024]);

        let mut encoder = Encoder::new(writer, 10, 10);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_color(ColorType::Grayscale);
        let mut png_writer = encoder.write_header()?;

        assert!(png_writer.write_chunk(chunk::fdAT, &empty).is_err());
        Ok(())
    }

    #[test]
    fn finish_drops_inner_writer() -> Result<()> {
        struct NoWriter<'flag>(&'flag mut bool);

        impl Write for NoWriter<'_> {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        impl Drop for NoWriter<'_> {
            fn drop(&mut self) {
                *self.0 = true;
            }
        }

        let mut flag = false;

        {
            let mut encoder = Encoder::new(NoWriter(&mut flag), 10, 10);
            encoder.set_depth(BitDepth::Eight);
            encoder.set_color(ColorType::Grayscale);

            let mut writer = encoder.write_header()?;
            writer.write_image_data(&[0; 100])?;
            writer.finish()?;
        }

        assert!(flag, "PNG finished but writer was not dropped");
        Ok(())
    }

    /// A Writer that only writes a few bytes at a time
    struct RandomChunkWriter<R: Rng, W: Write> {
        rng: R,
        w: W,
    }

    impl<R: Rng, W: Write> Write for RandomChunkWriter<R, W> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            // choose a random length to write
            let len = cmp::min(self.rng.random_range(1..50), buf.len());

            self.w.write(&buf[0..len])
        }

        fn flush(&mut self) -> io::Result<()> {
            self.w.flush()
        }
    }
}
