use std::io;
use std::str;
use std::string;

use quick_error::quick_error;

use crate::decoder::ChunkType;
use crate::tags::{
    CompressionMethod, PhotometricInterpretation, PlanarConfiguration, SampleFormat, Tag,
};
use crate::ColorType;

quick_error! {
    /// Tiff error kinds.
    #[derive(Debug)]
    pub enum TiffError {
        /// The Image is not formatted properly.
        FormatError(err: TiffFormatError) {
            display("format error: {err}")
            from()
            source(err)
        }
        /// The Decoder does not support features required by the image.
        UnsupportedError(err: TiffUnsupportedError) {
            display("unsupported error: {err}")
            from()
            source(err)
        }
        /// An I/O Error occurred while decoding the image.
        IoError(err: io::Error) {
            display("{err}")
            from()
            source(err)
        }
        /// The Limits of the Decoder is exceeded.
        LimitsExceeded {
            display("decoder limits exceeded")
        }
        /// An integer conversion to or from a platform size failed.
        IntSizeError {
            display("platform or format size limits exceeded")
        }
        /// The image does not support the requested operation
        UsageError(err: UsageError) {
            display("usage error: {err}")
            from()
            source(err)
        }
    }
}

quick_error! {
    /// The image is not formatted properly.
    ///
    /// This indicates that the encoder producing the image might behave incorrectly or that the
    /// input file has been corrupted.
    #[derive(Debug, Clone, PartialEq)]
    #[non_exhaustive]
    pub enum TiffFormatError {
        TiffSignatureNotFound {
            display("TIFF signature not found")
        }
        TiffSignatureInvalid {
            display("TIFF signature invalid")
        }
        ImageFileDirectoryNotFound {
            display("image file directory not found")
        }
        InconsistentSizesEncountered {
            display("inconsistent sizes encountered")
        }
        InvalidDimensions(width: u32, height: u32) {
            display("invalid dimensions: {width}x{height}")
        }
        InvalidTag {
            display("image contains invalid tag")
        }
        InvalidTagValueType(tag: Tag) {
            display("tag `{tag:?}` did not have the expected value type")
        }
        RequiredTagNotFound(tag: Tag) {
            display("required tag `{tag:?}` not found")
        }
        UnknownPredictor(pred: u16) {
            display("unknown predictor “{pred}” encountered")
        }
        UnknownPlanarConfiguration(cfg: u16) {
            display("unknown planar configuration “{cfg}”")
        }
        InvalidTypeForTag {
            display("tag has invalid type")
        }
        StripTileTagConflict {
            display("file should contain either (StripByteCounts and StripOffsets) or (TileByteCounts and TileOffsets), other combination was found")
        }
        CycleInOffsets {
            display("file contained a cycle in the list of IFDs")
        }
        SamplesPerPixelIsZero {
            display("samples per pixel is zero")
        }
        CompressedDataCorrupt(message: String) {
            display("compressed data is corrupt: {message}")
        }
    }
}

quick_error! {
    /// The Decoder does not support features required by the image.
    ///
    /// This only captures known failures for which the standard either does not require support or an
    /// implementation has been planned but not yet completed. Some variants may become unused over
    /// time and will then get deprecated before being removed.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    #[non_exhaustive]
    pub enum TiffUnsupportedError {
        FloatingPointPredictor(color_type: ColorType) {
            display("floating point predictor for {color_type:?} is unsupported")
        }
        HorizontalPredictor(color_type: ColorType) {
            display("horizontal predictor for {color_type:?} is unsupported")
        }
        InconsistentBitsPerSample(bits_per_sample: Vec<u8>) {
            display("inconsistent bits per sample: {bits_per_sample:?}")
        }
        InterpretationWithBits(interpretation: PhotometricInterpretation, bits_per_sample: Vec<u8>) {
            display("Photometric interpretation {interpretation:?} with bits per sample {bits_per_sample:?} is unsupported")
        }
        UnknownInterpretation {
            display("unknown photometric interpretation")
        }
        UnknownCompressionMethod {
            display("unknown compression method")
        }
        UnsupportedCompressionMethod(method: CompressionMethod) {
            display("compression method {method:?} is unsupported")
        }
        UnsupportedSampleDepth(depth: u8) {
            display("{depth} samples per pixel is unsupported")
        }
        UnsupportedSampleFormat(sample_format: Vec<SampleFormat>) {
            display("sample format {sample_format:?} is unsupported")
        }
        UnsupportedColorType(color_type: ColorType) {
            display("color type {color_type:?} is unsupported")
        }
        UnsupportedBitsPerChannel(bits_per_channel: u8) {
            display("{bits_per_channel} bits per channel not supported")
        }
        UnsupportedPlanarConfig(planar: Option<PlanarConfiguration>) {
            display("unsupported planar configuration “{planar:?}”")
        }
        UnsupportedDataType {
            display("unsupported data type.")
        }
        UnsupportedInterpretation(interpretation: PhotometricInterpretation) {
            display("unsupported photometric interpretation \"{interpretation:?}\"")
        }
        MisalignedTileBoundaries {
            display("tile rows are not aligned to byte boundaries")
        }
    }
}

quick_error! {
    /// User attempted to use the Decoder in a way that is incompatible with a specific image.
    ///
    /// For example: attempting to read a tile from a stripped image.
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum UsageError {
        InvalidChunkType(expected: ChunkType, actual: ChunkType) {
            display("requested operation is only valid for images with chunk encoding of type {expected:?} but got {actual:?}")
        }
        InvalidChunkIndex(index: u32) {
            display("invalid chunk index ({index}) requested")
        }
        PredictorCompressionMismatch {
            display("requested predictor is not compatible with the requested compression")
        }
        PredictorIncompatible {
            display("the requested predictor is not compatible with the image's format")
        }
        PredictorUnavailable {
            display("the requested predictor is not available")
        }
        InsufficientOutputBufferSize { needed: usize, provided: usize } {
            display("the borrowed output buffer is not large enough for the decoded data, needed {needed} but have {provided}")
        }
    }
}

impl From<str::Utf8Error> for TiffError {
    fn from(_err: str::Utf8Error) -> TiffError {
        TiffError::FormatError(TiffFormatError::InvalidTag)
    }
}

impl From<string::FromUtf8Error> for TiffError {
    fn from(_err: string::FromUtf8Error) -> TiffError {
        TiffError::FormatError(TiffFormatError::InvalidTag)
    }
}

impl From<std::num::TryFromIntError> for TiffError {
    fn from(_err: std::num::TryFromIntError) -> TiffError {
        TiffError::IntSizeError
    }
}

#[cfg(feature = "lzw")]
impl From<weezl::LzwError> for TiffError {
    fn from(err: weezl::LzwError) -> TiffError {
        TiffError::FormatError(TiffFormatError::CompressedDataCorrupt(err.to_string()))
    }
}

#[cfg(feature = "jpeg")]
impl From<zune_jpeg::errors::DecodeErrors> for TiffError {
    fn from(err: zune_jpeg::errors::DecodeErrors) -> Self {
        TiffError::FormatError(TiffFormatError::CompressedDataCorrupt(err.to_string()))
    }
}

/// Result of an image decoding/encoding process
pub type TiffResult<T> = Result<T, TiffError>;
