pub use tiff_value::*;

use std::{
    cmp,
    io::{self, Seek, Write},
    marker::PhantomData,
    mem,
    num::{NonZeroU64, TryFromIntError},
};

use crate::{
    decoder::ifd::Entry,
    error::{TiffResult, UsageError},
    tags::{CompressionMethod, IfdPointer, ResolutionUnit, SampleFormat, Tag, Type},
    Directory, TiffError, TiffFormatError,
};

pub mod colortype;
pub mod compression;
mod tiff_value;
mod writer;

use self::colortype::*;
use self::compression::Compression as Comp;
use self::compression::*;
use self::writer::*;

/// Type of prediction to prepare the image with.
///
/// Image data can be very unpredictable, and thus very hard to compress. Predictors are simple
/// passes ran over the image data to prepare it for compression. This is mostly used for LZW
/// compression, where using [Predictor::Horizontal] we see a 35% improvement in compression
/// ratio over the unpredicted compression !
///
/// [Predictor::FloatingPoint] is currently not supported.
pub type Predictor = crate::tags::Predictor;
#[cfg(feature = "deflate")]
pub type DeflateLevel = compression::DeflateLevel;

#[derive(Clone, Copy, PartialEq)]
pub enum Compression {
    Uncompressed,
    #[cfg(feature = "lzw")]
    Lzw,
    #[cfg(feature = "deflate")]
    Deflate(DeflateLevel),
    Packbits,
}

impl Default for Compression {
    fn default() -> Self {
        Self::Uncompressed
    }
}

impl Compression {
    fn tag(&self) -> CompressionMethod {
        match self {
            Compression::Uncompressed => CompressionMethod::None,
            #[cfg(feature = "lzw")]
            Compression::Lzw => CompressionMethod::LZW,
            #[cfg(feature = "deflate")]
            Compression::Deflate(_) => CompressionMethod::Deflate,
            Compression::Packbits => CompressionMethod::PackBits,
        }
    }

    fn get_algorithm(&self) -> Compressor {
        match self {
            Compression::Uncompressed => compression::Uncompressed {}.get_algorithm(),
            #[cfg(feature = "lzw")]
            Compression::Lzw => compression::Lzw {}.get_algorithm(),
            #[cfg(feature = "deflate")]
            Compression::Deflate(level) => compression::Deflate::with_level(*level).get_algorithm(),
            Compression::Packbits => compression::Packbits {}.get_algorithm(),
        }
    }
}

/// Encoder for Tiff and BigTiff files.
///
/// With this type you can get a `DirectoryEncoder` or a `ImageEncoder`
/// to encode Tiff/BigTiff ifd directories with images.
///
/// See `DirectoryEncoder` and `ImageEncoder`.
///
/// # Examples
/// ```
/// # extern crate tiff;
/// # fn main() {
/// # let mut file = std::io::Cursor::new(Vec::new());
/// # let image_data = vec![0; 100*100*3];
/// use tiff::encoder::*;
///
/// // create a standard Tiff file
/// let mut tiff = TiffEncoder::new(&mut file).unwrap();
/// tiff.write_image::<colortype::RGB8>(100, 100, &image_data).unwrap();
///
/// // create a BigTiff file
/// let mut bigtiff = TiffEncoder::new_big(&mut file).unwrap();
/// bigtiff.write_image::<colortype::RGB8>(100, 100, &image_data).unwrap();
///
/// # }
/// ```
pub struct TiffEncoder<W, K: TiffKind = TiffKindStandard> {
    writer: TiffWriter<W>,
    kind: PhantomData<K>,
    predictor: Predictor,
    compression: Compression,
    /// The offset of the last main image directory's `next` field.
    last_ifd_chain: NonZeroU64,
}

/// Constructor functions to create standard Tiff files.
impl<W: Write + Seek> TiffEncoder<W> {
    /// Creates a new encoder for standard Tiff files.
    ///
    /// To create BigTiff files, use [`new_big`][TiffEncoder::new_big] or
    /// [`new_generic`][TiffEncoder::new_generic].
    pub fn new(writer: W) -> TiffResult<TiffEncoder<W, TiffKindStandard>> {
        TiffEncoder::new_generic(writer)
    }
}

/// Constructor functions to create BigTiff files.
impl<W: Write + Seek> TiffEncoder<W, TiffKindBig> {
    /// Creates a new encoder for BigTiff files.
    ///
    /// To create standard Tiff files, use [`new`][TiffEncoder::new] or
    /// [`new_generic`][TiffEncoder::new_generic].
    pub fn new_big(writer: W) -> TiffResult<Self> {
        TiffEncoder::new_generic(writer)
    }
}

/// Generic functions that are available for both Tiff and BigTiff encoders.
impl<W: Write + Seek, K: TiffKind> TiffEncoder<W, K> {
    /// Creates a new Tiff or BigTiff encoder, inferred from the return type.
    pub fn new_generic(writer: W) -> TiffResult<Self> {
        let mut writer = TiffWriter::new(writer);
        K::write_header(&mut writer)?;

        let last_ifd_chain = NonZeroU64::new(writer.previous_ifd_pointer::<K>())
            .expect("Header is at a non-zero offset");

        Ok(TiffEncoder {
            writer,
            kind: PhantomData,
            predictor: Predictor::None,
            compression: Compression::Uncompressed,
            last_ifd_chain,
        })
    }

    /// Set the predictor to use
    ///
    /// A predictor is used to simplify the file before writing it. This is very
    /// useful when writing a file compressed using LZW as it can improve efficiency
    pub fn with_predictor(mut self, predictor: Predictor) -> Self {
        self.predictor = predictor;

        self
    }

    /// Set the compression method to use
    pub fn with_compression(mut self, compression: Compression) -> Self {
        self.compression = compression;

        self
    }

    /// Create a [`DirectoryEncoder`] to encode an ifd directory.
    #[deprecated = "`image_directory` replaced the old behavior and clarifies the intent"]
    #[doc(hidden)]
    pub fn new_directory(&mut self) -> TiffResult<DirectoryEncoder<'_, W, K>> {
        Self::chain_directory(&mut self.writer, &mut self.last_ifd_chain)
    }

    /// Create a [`DirectoryEncoder`] to encode an ifd directory.
    ///
    /// The caller is responsible for ensuring that the directory is a valid image in the main TIFF
    /// IFD sequence. To encode additional directories that are not linked into the sequence, use
    /// [`Self::extra_directory`][TiffEncoder::extra_directory].
    pub fn image_directory(&mut self) -> TiffResult<DirectoryEncoder<'_, W, K>> {
        Self::chain_directory(&mut self.writer, &mut self.last_ifd_chain)
    }

    /// Create a [`DirectoryEncoder`] to encode an ifd directory.
    ///
    /// The directory is not linked into the sequence of directories. For instance, encode Exif
    /// directories or SubIfd directories with this method.
    pub fn extra_directory(&mut self) -> TiffResult<DirectoryEncoder<'_, W, K>> {
        Self::unchained_directory(&mut self.writer)
    }

    /// Create an [`ImageEncoder`] to encode an image one slice at a time.
    pub fn new_image<C: ColorType>(
        &mut self,
        width: u32,
        height: u32,
    ) -> TiffResult<ImageEncoder<'_, W, C, K>> {
        let encoder = Self::chain_directory(&mut self.writer, &mut self.last_ifd_chain)?;
        ImageEncoder::new(encoder, width, height, self.compression, self.predictor)
    }

    /// Convenience function to write an entire image from memory.
    pub fn write_image<C: ColorType>(
        &mut self,
        width: u32,
        height: u32,
        data: &[C::Inner],
    ) -> TiffResult<()>
    where
        [C::Inner]: TiffValue,
    {
        let encoder = Self::chain_directory(&mut self.writer, &mut self.last_ifd_chain)?;
        let image: ImageEncoder<W, C, K> =
            ImageEncoder::new(encoder, width, height, self.compression, self.predictor)?;
        image.write_data(data)
    }

    fn chain_directory<'lt>(
        writer: &'lt mut TiffWriter<W>,
        last_ifd_chain: &'lt mut NonZeroU64,
    ) -> TiffResult<DirectoryEncoder<'lt, W, K>> {
        let last_ifd = *last_ifd_chain;
        DirectoryEncoder::new(writer, Some(last_ifd), Some(last_ifd_chain))
    }

    fn unchained_directory(writer: &mut TiffWriter<W>) -> TiffResult<DirectoryEncoder<'_, W, K>> {
        DirectoryEncoder::new(writer, None, None)
    }
}

/// Low level interface to encode ifd directories.
///
/// You should call `finish` on this when you are finished with it.
/// Encoding can silently fail while this is dropping.
pub struct DirectoryEncoder<'a, W: 'a + Write + Seek, K: TiffKind> {
    writer: &'a mut TiffWriter<W>,
    /// The position of the previous directory's `next` field, if any.
    chained_ifd_pos: Option<NonZeroU64>,
    /// An output to write the `next` field offset on completion.
    write_chain: Option<&'a mut NonZeroU64>,
    kind: PhantomData<K>,
    // We use BTreeMap to make sure tags are written in correct order
    directory: Directory,
    dropped: bool,
}

/// The offset of an encoded directory in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DirectoryOffset<K: TiffKind> {
    /// The start of the directory as a Tiff value.
    ///
    /// This is a bit of a wart in the strongly typed design of the encoder. The value _type_ must
    /// itself know how to be represented in the Tiff file but that may differ based on endianess
    /// as well as the offset size (`u32` or BigTIFF's `u64`). Thankfully we're allowed to
    /// represent offsets with `LONG` or `IFD` in the usual case.
    pub offset: K::OffsetType,
    /// The start of the directory as a pure offset.
    pub pointer: IfdPointer,
    /// The offset of its sequence link field, in our private representation.
    ifd_chain: NonZeroU64,
    /// The kind of Tiff file the offset is for.
    kind: PhantomData<K>,
}

impl<'a, W: 'a + Write + Seek, K: TiffKind> DirectoryEncoder<'a, W, K> {
    /// Construct a directory writer appending to data, assuming the writer is currently positioned
    /// immediately after a previously written IFD to which to append.
    fn new(
        writer: &'a mut TiffWriter<W>,
        chained_ifd_pos: Option<NonZeroU64>,
        chain_into: Option<&'a mut NonZeroU64>,
    ) -> TiffResult<Self> {
        writer.pad_word_boundary()?; // TODO: Do we need to adjust this for BigTiff?
        Ok(Self {
            writer,
            chained_ifd_pos,
            write_chain: chain_into,
            kind: PhantomData,
            directory: Directory::empty(),
            dropped: false,
        })
    }

    /// Write a single ifd tag.
    pub fn write_tag<T: TiffValue>(&mut self, tag: Tag, value: T) -> TiffResult<()> {
        // Encodes the value if necessary. In the current bytes all integers are taken as native
        // endian and thus transparent but this keeps the interface generic.
        let mut bytes = Vec::with_capacity(value.bytes());
        {
            let mut writer = TiffWriter::new(&mut bytes);
            value.write(&mut writer)?;
        }

        let entry = Self::write_value(
            self.writer,
            &DirectoryEntry {
                data_type: <T>::FIELD_TYPE,
                count: value.count().try_into()?,
                data: bytes,
            },
        )?;

        self.directory.extend([(tag, entry)]);

        Ok(())
    }

    /// Write some data to the tiff file, the offset of the data is returned.
    ///
    /// This could be used to write tiff strips.
    pub fn write_data<T: TiffValue>(&mut self, value: T) -> TiffResult<u64> {
        let offset = self.writer.offset();
        value.write(self.writer)?;
        Ok(offset)
    }

    /// Define the parent directory.
    ///
    /// Each directory has an offset based link to its successor, forming a linked list in the
    /// file. The encoder writes its own offset into the parent's field when [`Self::finish`] is
    /// called or it is dropped. This redefines the parent. An offset representation of the parent
    /// must be acquired by finishing it with [`Self::finish_with_offsets`].
    pub fn set_parent(&mut self, offset: &DirectoryOffset<K>) {
        self.chained_ifd_pos = Some(offset.ifd_chain);
    }

    /// Write out the ifd directory itself.
    pub fn finish(mut self) -> TiffResult<()> {
        self.finish_internal()?;
        Ok(())
    }

    pub fn finish_with_offsets(mut self) -> TiffResult<DirectoryOffset<K>> {
        self.finish_internal()
    }

    fn write_directory(&mut self) -> TiffResult<u64> {
        // Start by turning all buffered unwritten values into entries.
        let offset = self.writer.offset();
        K::write_entry_count(self.writer, self.directory.len())?;

        let offset_bytes = mem::size_of::<K::OffsetType>();
        for (tag, entry) in self.directory.iter() {
            self.writer.write_u16(tag.to_u16())?;
            self.writer.write_u16(entry.field_type().to_u16())?;
            let count = K::convert_offset(entry.count())?;
            count.write(self.writer)?;
            self.writer.write_bytes(&entry.offset()[..offset_bytes])?;
        }

        Ok(offset)
    }

    fn write_value(
        writer: &mut TiffWriter<W>,
        value: &DirectoryEntry<K::OffsetType>,
    ) -> TiffResult<Entry> {
        let &DirectoryEntry {
            data: ref bytes,
            ref count,
            data_type,
        } = value;

        let in_entry_bytes = mem::size_of::<K::OffsetType>();
        let mut offset_bytes = [0; 8];

        if bytes.len() > in_entry_bytes {
            let offset = writer.offset();
            writer.write_bytes(bytes)?;

            let offset = K::convert_offset(offset)?;
            offset_bytes[..offset.bytes()].copy_from_slice(&offset.data());
        } else {
            // Note: we have indicated our native byte order in the header, hence this
            // corresponds to our byte order no matter the value type.
            offset_bytes[..bytes.len()].copy_from_slice(bytes);
        }

        // Undoing some hidden type. Offset is either u32 or u64. Due to the trait API being public
        // and some oversight, we can not clone the `count: K::OffsetType` and thus not convert it.
        // Instead, write it to a buffer...
        let mut count_bytes = [0; 8];
        // Nominally Cow but we only expect `Cow::Borrowed`.
        count_bytes[..count.bytes()].copy_from_slice(&count.data());

        Ok(if in_entry_bytes == 4 {
            let count = u32::from_ne_bytes(count_bytes[..4].try_into().unwrap());
            Entry::new(data_type, count, offset_bytes[..4].try_into().unwrap())
        } else {
            debug_assert_eq!(in_entry_bytes, 8);
            let count = u64::from_ne_bytes(count_bytes);
            Entry::new_u64(data_type, count, offset_bytes)
        })
    }

    /// Provides the number of bytes written by the underlying TiffWriter during the last call.
    fn last_written(&self) -> u64 {
        self.writer.last_written()
    }

    fn finish_internal(&mut self) -> TiffResult<DirectoryOffset<K>> {
        let ifd_pointer = self.write_directory()?;
        let offset = K::convert_offset(ifd_pointer)?;

        if let Some(prior) = self.chained_ifd_pos {
            let curr_pos = self.writer.offset();

            self.writer.goto_offset(prior.get())?;
            // Note how we are not writing the `offset` type itself here as doing so would need to
            // go through the `TiffValue` trait—the type is not constrained by much. But for the
            // trait we need a `TiffWriter` which comes with a bunch of additional state such as
            // compressor etc. that we have no need for.
            K::write_offset(self.writer, ifd_pointer)?;

            self.writer.goto_offset(curr_pos)?;
        }

        K::write_offset(self.writer, 0)?;

        let ifd_chain = NonZeroU64::new(self.writer.previous_ifd_pointer::<K>())
            .expect("IFD chain field is at a non-zero offset");

        if let Some(prior) = self.write_chain.take() {
            *prior = ifd_chain;
        }

        self.dropped = true;

        Ok(DirectoryOffset {
            pointer: IfdPointer(ifd_pointer),
            offset,
            ifd_chain,
            kind: PhantomData,
        })
    }
}

impl<'a, W: Write + Seek, K: TiffKind> Drop for DirectoryEncoder<'a, W, K> {
    fn drop(&mut self) {
        if !self.dropped {
            let _ = self.finish_internal();
        }
    }
}

/// Type to encode images strip by strip.
///
/// You should call `finish` on this when you are finished with it.
/// Encoding can silently fail while this is dropping.
///
/// # Examples
/// ```
/// # extern crate tiff;
/// # fn main() {
/// # let mut file = std::io::Cursor::new(Vec::new());
/// # let image_data = vec![0; 100*100*3];
/// use tiff::encoder::*;
/// use tiff::tags::Tag;
///
/// let mut tiff = TiffEncoder::new(&mut file).unwrap();
/// let mut image = tiff.new_image::<colortype::RGB8>(100, 100).unwrap();
///
/// // You can encode tags here
/// image.encoder().write_tag(Tag::Artist, "Image-tiff").unwrap();
///
/// // Strip size can be configured before writing data
/// image.rows_per_strip(2).unwrap();
///
/// let mut idx = 0;
/// while image.next_strip_sample_count() > 0 {
///     let sample_count = image.next_strip_sample_count() as usize;
///     image.write_strip(&image_data[idx..idx+sample_count]).unwrap();
///     idx += sample_count;
/// }
/// image.finish().unwrap();
/// # }
/// ```
/// You can also call write_data function wich will encode by strip and finish
pub struct ImageEncoder<'a, W: 'a + Write + Seek, C: ColorType, K: TiffKind> {
    encoder: DirectoryEncoder<'a, W, K>,
    strip_idx: u64,
    strip_count: u64,
    row_samples: u64,
    width: u32,
    height: u32,
    rows_per_strip: u64,
    strip_offsets: Vec<K::OffsetType>,
    strip_byte_count: Vec<K::OffsetType>,
    dropped: bool,
    compression: Compression,
    predictor: Predictor,
    _phantom: ::std::marker::PhantomData<C>,
}

impl<'a, W: 'a + Write + Seek, T: ColorType, K: TiffKind> ImageEncoder<'a, W, T, K> {
    fn sanity_check(compression: Compression, predictor: Predictor) -> TiffResult<()> {
        match (predictor, compression, T::SAMPLE_FORMAT[0]) {
            (Predictor::Horizontal, _, SampleFormat::IEEEFP | SampleFormat::Void) => {
                Err(TiffError::UsageError(UsageError::PredictorIncompatible))
            }
            (Predictor::FloatingPoint, _, _) => {
                Err(TiffError::UsageError(UsageError::PredictorUnavailable))
            }
            _ => Ok(()),
        }
    }

    fn new(
        mut encoder: DirectoryEncoder<'a, W, K>,
        width: u32,
        height: u32,
        compression: Compression,
        predictor: Predictor,
    ) -> TiffResult<Self> {
        if width == 0 || height == 0 {
            return Err(TiffError::FormatError(TiffFormatError::InvalidDimensions(
                width, height,
            )));
        }

        Self::sanity_check(compression, predictor)?;

        let row_samples = u64::from(width) * u64::try_from(<T>::BITS_PER_SAMPLE.len())?;
        let row_bytes = row_samples * u64::from(<T::Inner>::BYTE_LEN);

        // Limit the strip size to prevent potential memory and security issues.
        // Also keep the multiple strip handling 'oiled'
        let rows_per_strip = {
            match compression.tag() {
                CompressionMethod::PackBits => 1, // Each row must be packed separately. Do not compress across row boundaries
                _ => 1_000_000_u64.div_ceil(row_bytes),
            }
        };

        let strip_count = u64::from(height).div_ceil(rows_per_strip);

        encoder.write_tag(Tag::ImageWidth, width)?;
        encoder.write_tag(Tag::ImageLength, height)?;
        encoder.write_tag(Tag::Compression, compression.tag().to_u16())?;
        encoder.write_tag(Tag::Predictor, predictor.to_u16())?;

        encoder.write_tag(Tag::BitsPerSample, <T>::BITS_PER_SAMPLE)?;
        let sample_format: Vec<_> = <T>::SAMPLE_FORMAT.iter().map(|s| s.to_u16()).collect();
        encoder.write_tag(Tag::SampleFormat, &sample_format[..])?;
        encoder.write_tag(Tag::PhotometricInterpretation, <T>::TIFF_VALUE.to_u16())?;

        encoder.write_tag(Tag::RowsPerStrip, u32::try_from(rows_per_strip)?)?;

        encoder.write_tag(
            Tag::SamplesPerPixel,
            u16::try_from(<T>::BITS_PER_SAMPLE.len())?,
        )?;
        encoder.write_tag(Tag::XResolution, Rational { n: 1, d: 1 })?;
        encoder.write_tag(Tag::YResolution, Rational { n: 1, d: 1 })?;
        encoder.write_tag(Tag::ResolutionUnit, ResolutionUnit::None.to_u16())?;

        Ok(ImageEncoder {
            encoder,
            strip_count,
            strip_idx: 0,
            row_samples,
            rows_per_strip,
            width,
            height,
            strip_offsets: Vec::new(),
            strip_byte_count: Vec::new(),
            dropped: false,
            compression,
            predictor,
            _phantom: ::std::marker::PhantomData,
        })
    }

    /// Number of samples the next strip should have.
    pub fn next_strip_sample_count(&self) -> u64 {
        if self.strip_idx >= self.strip_count {
            return 0;
        }

        let raw_start_row = self.strip_idx * self.rows_per_strip;
        let start_row = cmp::min(u64::from(self.height), raw_start_row);
        let end_row = cmp::min(u64::from(self.height), raw_start_row + self.rows_per_strip);

        (end_row - start_row) * self.row_samples
    }

    /// Write a single strip.
    pub fn write_strip(&mut self, value: &[T::Inner]) -> TiffResult<()>
    where
        [T::Inner]: TiffValue,
    {
        let samples = self.next_strip_sample_count();
        if u64::try_from(value.len())? != samples {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Slice is wrong size for strip",
            )
            .into());
        }

        // Write the (possible compressed) data to the encoder.
        let offset = match self.predictor {
            Predictor::None => self.encoder.write_data(value)?,
            Predictor::Horizontal => {
                let mut row_result = Vec::with_capacity(value.len());
                for row in value.chunks_exact(self.row_samples as usize) {
                    T::horizontal_predict(row, &mut row_result);
                }
                self.encoder.write_data(row_result.as_slice())?
            }
            _ => unimplemented!(),
        };

        let byte_count = self.encoder.last_written() as usize;

        self.strip_offsets.push(K::convert_offset(offset)?);
        self.strip_byte_count.push(byte_count.try_into()?);

        self.strip_idx += 1;
        Ok(())
    }

    /// Write strips from data
    pub fn write_data(mut self, data: &[T::Inner]) -> TiffResult<()>
    where
        [T::Inner]: TiffValue,
    {
        let num_pix = usize::try_from(self.width)?
            .checked_mul(usize::try_from(self.height)?)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Image width * height exceeds usize",
                )
            })?;
        if data.len() < num_pix {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Input data slice is undersized for provided dimensions",
            )
            .into());
        }

        self.encoder
            .writer
            .set_compression(self.compression.get_algorithm());

        let mut idx = 0;
        while self.next_strip_sample_count() > 0 {
            let sample_count = usize::try_from(self.next_strip_sample_count())?;
            self.write_strip(&data[idx..idx + sample_count])?;
            idx += sample_count;
        }

        self.encoder.writer.reset_compression();
        self.finish()?;
        Ok(())
    }

    /// Set image resolution
    pub fn resolution(&mut self, unit: ResolutionUnit, value: Rational) {
        self.encoder
            .write_tag(Tag::ResolutionUnit, unit.to_u16())
            .unwrap();
        self.encoder
            .write_tag(Tag::XResolution, value.clone())
            .unwrap();
        self.encoder.write_tag(Tag::YResolution, value).unwrap();
    }

    /// Set image resolution unit
    pub fn resolution_unit(&mut self, unit: ResolutionUnit) {
        self.encoder
            .write_tag(Tag::ResolutionUnit, unit.to_u16())
            .unwrap();
    }

    /// Set image x-resolution
    pub fn x_resolution(&mut self, value: Rational) {
        self.encoder.write_tag(Tag::XResolution, value).unwrap();
    }

    /// Set image y-resolution
    pub fn y_resolution(&mut self, value: Rational) {
        self.encoder.write_tag(Tag::YResolution, value).unwrap();
    }

    /// Set image number of lines per strip
    ///
    /// This function needs to be called before any calls to `write_data` or
    /// `write_strip` and will return an error otherwise.
    pub fn rows_per_strip(&mut self, value: u32) -> TiffResult<()> {
        if self.strip_idx != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot change strip size after data was written",
            )
            .into());
        }
        // Write tag as 32 bits
        self.encoder.write_tag(Tag::RowsPerStrip, value)?;

        let value: u64 = value as u64;
        self.strip_count = (self.height as u64).div_ceil(value);
        self.rows_per_strip = value;

        Ok(())
    }

    fn finish_internal(&mut self) -> TiffResult<DirectoryOffset<K>> {
        self.encoder
            .write_tag(Tag::StripOffsets, K::convert_slice(&self.strip_offsets))?;
        self.encoder.write_tag(
            Tag::StripByteCounts,
            K::convert_slice(&self.strip_byte_count),
        )?;
        self.dropped = true;

        self.encoder.finish_internal()
    }

    /// Get a reference of the underlying `DirectoryEncoder`
    pub fn encoder(&mut self) -> &mut DirectoryEncoder<'a, W, K> {
        &mut self.encoder
    }

    /// Write out image and ifd directory.
    pub fn finish(mut self) -> TiffResult<()> {
        self.finish_internal()?;
        Ok(())
    }
}

impl<'a, W: Write + Seek, C: ColorType, K: TiffKind> Drop for ImageEncoder<'a, W, C, K> {
    fn drop(&mut self) {
        if !self.dropped {
            let _ = self.finish_internal();
        }
    }
}

struct DirectoryEntry<S> {
    data_type: Type,
    count: S,
    data: Vec<u8>,
}

/// Trait to abstract over Tiff/BigTiff differences.
///
/// Implemented for [`TiffKindStandard`] and [`TiffKindBig`].
pub trait TiffKind {
    /// The type of offset fields, `u32` for normal Tiff, `u64` for BigTiff.
    type OffsetType: TryFrom<usize, Error = TryFromIntError> + Into<u64> + TiffValue;

    /// Needed for the `convert_slice` method.
    type OffsetArrayType: ?Sized + TiffValue;

    /// Write the (Big)Tiff header.
    fn write_header<W: Write>(writer: &mut TiffWriter<W>) -> TiffResult<()>;

    /// Convert a file offset to `Self::OffsetType`.
    ///
    /// This returns an error for normal Tiff if the offset is larger than `u32::MAX`.
    fn convert_offset(offset: u64) -> TiffResult<Self::OffsetType>;

    /// Write an offset value to the given writer.
    ///
    /// Like `convert_offset`, this errors if `offset > u32::MAX` for normal Tiff.
    fn write_offset<W: Write>(writer: &mut TiffWriter<W>, offset: u64) -> TiffResult<()>;

    /// Write the IFD entry count field with the given `count` value.
    ///
    /// The entry count field is an `u16` for normal Tiff and `u64` for BigTiff. Errors
    /// if the given `usize` is larger than the representable values.
    fn write_entry_count<W: Write>(writer: &mut TiffWriter<W>, count: usize) -> TiffResult<()>;

    /// Internal helper method for satisfying Rust's type checker.
    ///
    /// The `TiffValue` trait is implemented for both primitive values (e.g. `u8`, `u32`) and
    /// slices of primitive values (e.g. `[u8]`, `[u32]`). However, this is not represented in
    /// the type system, so there is no guarantee that that for all `T: TiffValue` there is also
    /// an implementation of `TiffValue` for `[T]`. This method works around that problem by
    /// providing a conversion from `[T]` to some value that implements `TiffValue`, thereby
    /// making all slices of `OffsetType` usable with `write_tag` and similar methods.
    ///
    /// Implementations of this trait should always set `OffsetArrayType` to `[OffsetType]`.
    fn convert_slice(slice: &[Self::OffsetType]) -> &Self::OffsetArrayType;
}

/// Create a standard Tiff file.
pub struct TiffKindStandard;

impl TiffKind for TiffKindStandard {
    type OffsetType = u32;
    type OffsetArrayType = [u32];

    fn write_header<W: Write>(writer: &mut TiffWriter<W>) -> TiffResult<()> {
        write_tiff_header(writer)?;
        // blank the IFD offset location
        writer.write_u32(0)?;

        Ok(())
    }

    fn convert_offset(offset: u64) -> TiffResult<Self::OffsetType> {
        Ok(Self::OffsetType::try_from(offset)?)
    }

    fn write_offset<W: Write>(writer: &mut TiffWriter<W>, offset: u64) -> TiffResult<()> {
        writer.write_u32(u32::try_from(offset)?)?;
        Ok(())
    }

    fn write_entry_count<W: Write>(writer: &mut TiffWriter<W>, count: usize) -> TiffResult<()> {
        writer.write_u16(u16::try_from(count)?)?;

        Ok(())
    }

    fn convert_slice(slice: &[Self::OffsetType]) -> &Self::OffsetArrayType {
        slice
    }
}

/// Create a BigTiff file.
pub struct TiffKindBig;

impl TiffKind for TiffKindBig {
    type OffsetType = u64;
    type OffsetArrayType = [u64];

    fn write_header<W: Write>(writer: &mut TiffWriter<W>) -> TiffResult<()> {
        write_bigtiff_header(writer)?;
        // blank the IFD offset location
        writer.write_u64(0)?;

        Ok(())
    }

    fn convert_offset(offset: u64) -> TiffResult<Self::OffsetType> {
        Ok(offset)
    }

    fn write_offset<W: Write>(writer: &mut TiffWriter<W>, offset: u64) -> TiffResult<()> {
        writer.write_u64(offset)?;
        Ok(())
    }

    fn write_entry_count<W: Write>(writer: &mut TiffWriter<W>, count: usize) -> TiffResult<()> {
        writer.write_u64(u64::try_from(count)?)?;
        Ok(())
    }

    fn convert_slice(slice: &[Self::OffsetType]) -> &Self::OffsetArrayType {
        slice
    }
}
