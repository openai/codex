use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use std::iter;
use std::path::Path;

use crate::error::{ImageFormatHint, ImageResult, UnsupportedError, UnsupportedErrorKind};
use crate::hooks::{GenericReader, DECODING_HOOKS, GUESS_FORMAT_HOOKS};
use crate::io::limits::Limits;
use crate::{DynamicImage, ImageDecoder, ImageError, ImageFormat};

use super::free_functions;

#[derive(Clone)]
enum Format {
    BuiltIn(ImageFormat),
    Extension(OsString),
}

/// A multi-format image reader.
///
/// Wraps an input reader to facilitate automatic detection of an image's format, appropriate
/// decoding method, and dispatches into the set of supported [`ImageDecoder`] implementations.
///
/// ## Usage
///
/// Opening a file, deducing the format based on the file path automatically, and trying to decode
/// the image contained can be performed by constructing the reader and immediately consuming it.
///
/// ```no_run
/// # use image::ImageError;
/// # use image::ImageReader;
/// # fn main() -> Result<(), ImageError> {
/// let image = ImageReader::open("path/to/image.png")?
///     .decode()?;
/// # Ok(()) }
/// ```
///
/// It is also possible to make a guess based on the content. This is especially handy if the
/// source is some blob in memory and you have constructed the reader in another way. Here is an
/// example with a `pnm` black-and-white subformat that encodes its pixel matrix with ascii values.
///
/// ```
/// # use image::ImageError;
/// # use image::ImageReader;
/// # fn main() -> Result<(), ImageError> {
/// use std::io::Cursor;
/// use image::ImageFormat;
///
/// let raw_data = b"P1 2 2\n\
///     0 1\n\
///     1 0\n";
///
/// let mut reader = ImageReader::new(Cursor::new(raw_data))
///     .with_guessed_format()
///     .expect("Cursor io never fails");
/// assert_eq!(reader.format(), Some(ImageFormat::Pnm));
///
/// # #[cfg(feature = "pnm")]
/// let image = reader.decode()?;
/// # Ok(()) }
/// ```
///
/// As a final fallback or if only a specific format must be used, the reader always allows manual
/// specification of the supposed image format with [`set_format`].
///
/// [`set_format`]: #method.set_format
/// [`ImageDecoder`]: ../trait.ImageDecoder.html
pub struct ImageReader<R: Read + Seek> {
    /// The reader. Should be buffered.
    inner: R,
    /// The format, if one has been set or deduced.
    format: Option<Format>,
    /// Decoding limits
    limits: Limits,
}

impl<'a, R: 'a + BufRead + Seek> ImageReader<R> {
    /// Create a new image reader without a preset format.
    ///
    /// Assumes the reader is already buffered. For optimal performance,
    /// consider wrapping the reader with a `BufReader::new()`.
    ///
    /// It is possible to guess the format based on the content of the read object with
    /// [`with_guessed_format`], or to set the format directly with [`set_format`].
    ///
    /// [`with_guessed_format`]: #method.with_guessed_format
    /// [`set_format`]: method.set_format
    pub fn new(buffered_reader: R) -> Self {
        ImageReader {
            inner: buffered_reader,
            format: None,
            limits: Limits::default(),
        }
    }

    /// Construct a reader with specified format.
    ///
    /// Assumes the reader is already buffered. For optimal performance,
    /// consider wrapping the reader with a `BufReader::new()`.
    pub fn with_format(buffered_reader: R, format: ImageFormat) -> Self {
        ImageReader {
            inner: buffered_reader,
            format: Some(Format::BuiltIn(format)),
            limits: Limits::default(),
        }
    }

    /// Get the currently determined format.
    pub fn format(&self) -> Option<ImageFormat> {
        match self.format {
            Some(Format::BuiltIn(ref format)) => Some(*format),
            Some(Format::Extension(ref ext)) => ImageFormat::from_extension(ext),
            None => None,
        }
    }

    /// Supply the format as which to interpret the read image.
    pub fn set_format(&mut self, format: ImageFormat) {
        self.format = Some(Format::BuiltIn(format));
    }

    /// Remove the current information on the image format.
    ///
    /// Note that many operations require format information to be present and will return e.g. an
    /// `ImageError::Unsupported` when the image format has not been set.
    pub fn clear_format(&mut self) {
        self.format = None;
    }

    /// Disable all decoding limits.
    pub fn no_limits(&mut self) {
        self.limits = Limits::no_limits();
    }

    /// Set a custom set of decoding limits.
    pub fn limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// Unwrap the reader.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Makes a decoder.
    ///
    /// For all formats except PNG, the limits are ignored and can be set with
    /// `ImageDecoder::set_limits` after calling this function. PNG is handled specially because that
    /// decoder has a different API which does not allow setting limits after construction.
    fn make_decoder(
        format: Format,
        reader: R,
        limits_for_png: Limits,
    ) -> ImageResult<Box<dyn ImageDecoder + 'a>> {
        #[allow(unused)]
        use crate::codecs::*;

        let format = match format {
            Format::BuiltIn(format) => format,
            Format::Extension(ext) => {
                {
                    let hooks = DECODING_HOOKS.read().unwrap();
                    if let Some(hooks) = hooks.as_ref() {
                        if let Some(hook) = hooks.get(&ext) {
                            return hook(GenericReader(BufReader::new(Box::new(reader))));
                        }
                    }
                }

                ImageFormat::from_extension(&ext).ok_or(ImageError::Unsupported(
                    ImageFormatHint::PathExtension(ext.into()).into(),
                ))?
            }
        };

        #[allow(unreachable_patterns)]
        // Default is unreachable if all features are supported.
        Ok(match format {
            #[cfg(feature = "avif-native")]
            ImageFormat::Avif => Box::new(avif::AvifDecoder::new(reader)?),
            #[cfg(feature = "png")]
            ImageFormat::Png => Box::new(png::PngDecoder::with_limits(reader, limits_for_png)?),
            #[cfg(feature = "gif")]
            ImageFormat::Gif => Box::new(gif::GifDecoder::new(reader)?),
            #[cfg(feature = "jpeg")]
            ImageFormat::Jpeg => Box::new(jpeg::JpegDecoder::new(reader)?),
            #[cfg(feature = "webp")]
            ImageFormat::WebP => Box::new(webp::WebPDecoder::new(reader)?),
            #[cfg(feature = "tiff")]
            ImageFormat::Tiff => Box::new(tiff::TiffDecoder::new(reader)?),
            #[cfg(feature = "tga")]
            ImageFormat::Tga => Box::new(tga::TgaDecoder::new(reader)?),
            #[cfg(feature = "dds")]
            ImageFormat::Dds => Box::new(dds::DdsDecoder::new(reader)?),
            #[cfg(feature = "bmp")]
            ImageFormat::Bmp => Box::new(bmp::BmpDecoder::new(reader)?),
            #[cfg(feature = "ico")]
            ImageFormat::Ico => Box::new(ico::IcoDecoder::new(reader)?),
            #[cfg(feature = "hdr")]
            ImageFormat::Hdr => Box::new(hdr::HdrDecoder::new(reader)?),
            #[cfg(feature = "exr")]
            ImageFormat::OpenExr => Box::new(openexr::OpenExrDecoder::new(reader)?),
            #[cfg(feature = "pnm")]
            ImageFormat::Pnm => Box::new(pnm::PnmDecoder::new(reader)?),
            #[cfg(feature = "ff")]
            ImageFormat::Farbfeld => Box::new(farbfeld::FarbfeldDecoder::new(reader)?),
            #[cfg(feature = "qoi")]
            ImageFormat::Qoi => Box::new(qoi::QoiDecoder::new(reader)?),
            #[cfg(feature = "pcx")]
            ImageFormat::Pcx => Box::new(pcx::PCXDecoder::new(reader)?),
            format => {
                return Err(ImageError::Unsupported(
                    ImageFormatHint::Exact(format).into(),
                ));
            }
        })
    }

    /// Convert the reader into a decoder.
    pub fn into_decoder(mut self) -> ImageResult<impl ImageDecoder + 'a> {
        let mut decoder =
            Self::make_decoder(self.require_format()?, self.inner, self.limits.clone())?;
        decoder.set_limits(self.limits)?;
        Ok(decoder)
    }

    /// Make a format guess based on the content, replacing it on success.
    ///
    /// Returns `Ok` with the guess if no io error occurs. Additionally, replaces the current
    /// format if the guess was successful. If the guess was unable to determine a format then
    /// the current format of the reader is unchanged.
    ///
    /// Returns an error if the underlying reader fails. The format is unchanged. The error is a
    /// `std::io::Error` and not `ImageError` since the only error case is an error when the
    /// underlying reader seeks.
    ///
    /// When an error occurs, the reader may not have been properly reset and it is potentially
    /// hazardous to continue with more io.
    ///
    /// ## Usage
    ///
    /// This supplements the path based type deduction from [`ImageReader::open()`] with content based deduction.
    /// This is more common in Linux and UNIX operating systems and also helpful if the path can
    /// not be directly controlled.
    ///
    /// ```no_run
    /// # use image::ImageError;
    /// # use image::ImageReader;
    /// # fn main() -> Result<(), ImageError> {
    /// let image = ImageReader::open("image.unknown")?
    ///     .with_guessed_format()?
    ///     .decode()?;
    /// # Ok(()) }
    /// ```
    pub fn with_guessed_format(mut self) -> io::Result<Self> {
        let format = self.guess_format()?;
        // Replace format if found, keep current state if not.
        self.format = format.or(self.format);
        Ok(self)
    }

    fn guess_format(&mut self) -> io::Result<Option<Format>> {
        let mut start = [0; 16];

        // Save current offset, read start, restore offset.
        let cur = self.inner.stream_position()?;
        let len = io::copy(
            // Accept shorter files but read at most 16 bytes.
            &mut self.inner.by_ref().take(16),
            &mut Cursor::new(&mut start[..]),
        )?;
        self.inner.seek(SeekFrom::Start(cur))?;

        let hooks = GUESS_FORMAT_HOOKS.read().unwrap();
        for &(signature, mask, ref extension) in &*hooks {
            if mask.is_empty() {
                if start.starts_with(signature) {
                    return Ok(Some(Format::Extension(extension.clone())));
                }
            } else if start.len() >= signature.len()
                && start
                    .iter()
                    .zip(signature.iter())
                    .zip(mask.iter().chain(iter::repeat(&0xFF)))
                    .all(|((&byte, &sig), &mask)| byte & mask == sig)
            {
                return Ok(Some(Format::Extension(extension.clone())));
            }
        }

        if let Some(format) = free_functions::guess_format_impl(&start[..len as usize]) {
            return Ok(Some(Format::BuiltIn(format)));
        }

        Ok(None)
    }

    /// Read the image dimensions.
    ///
    /// Uses the current format to construct the correct reader for the format.
    ///
    /// If no format was determined, returns an `ImageError::Unsupported`.
    pub fn into_dimensions(self) -> ImageResult<(u32, u32)> {
        self.into_decoder().map(|d| d.dimensions())
    }

    /// Read the image (replaces `load`).
    ///
    /// Uses the current format to construct the correct reader for the format.
    ///
    /// If no format was determined, returns an `ImageError::Unsupported`.
    pub fn decode(mut self) -> ImageResult<DynamicImage> {
        let format = self.require_format()?;

        let mut limits = self.limits;
        let mut decoder = Self::make_decoder(format, self.inner, limits.clone())?;

        // Check that we do not allocate a bigger buffer than we are allowed to
        // FIXME: should this rather go in `DynamicImage::from_decoder` somehow?
        limits.reserve(decoder.total_bytes())?;
        decoder.set_limits(limits)?;

        DynamicImage::from_decoder(decoder)
    }

    fn require_format(&mut self) -> ImageResult<Format> {
        self.format.clone().ok_or_else(|| {
            ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                ImageFormatHint::Unknown,
                UnsupportedErrorKind::Format(ImageFormatHint::Unknown),
            ))
        })
    }
}

impl ImageReader<BufReader<File>> {
    /// Open a file to read, format will be guessed from path.
    ///
    /// This will not attempt any io operation on the opened file.
    ///
    /// If you want to inspect the content for a better guess on the format, which does not depend
    /// on file extensions, follow this call with a call to [`with_guessed_format`].
    ///
    /// [`with_guessed_format`]: #method.with_guessed_format
    pub fn open<P>(path: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        Self::open_impl(path.as_ref())
    }

    fn open_impl(path: &Path) -> io::Result<Self> {
        let format = path
            .extension()
            .filter(|ext| !ext.is_empty())
            .map(|ext| Format::Extension(ext.to_owned()));

        Ok(ImageReader {
            inner: BufReader::new(File::open(path)?),
            format,
            limits: Limits::default(),
        })
    }
}
