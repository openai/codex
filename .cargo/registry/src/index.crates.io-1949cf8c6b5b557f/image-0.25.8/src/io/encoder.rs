use crate::error::{ImageFormatHint, ImageResult, UnsupportedError, UnsupportedErrorKind};
use crate::{ColorType, DynamicImage, ExtendedColorType};

/// Nominally public but DO NOT expose this type.
///
/// To be somewhat sure here's a compile fail test:
///
/// ```compile_fail
/// use image::MethodSealedToImage;
/// ```
///
/// ```compile_fail
/// use image::io::MethodSealedToImage;
/// ```
///
/// The same implementation strategy for a partially public trait is used in the standard library,
/// for the different effect of forbidding `Error::type_id` overrides thus making them reliable for
/// their calls through the `dyn` version of the trait.
///
/// Read more: <https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/>
#[derive(Clone, Copy)]
pub struct MethodSealedToImage;

/// The trait all encoders implement
pub trait ImageEncoder {
    /// Writes all the bytes in an image to the encoder.
    ///
    /// This function takes a slice of bytes of the pixel data of the image
    /// and encodes them. Just like for [`ImageDecoder::read_image`], no particular
    /// alignment is required and data is expected to be in native endian.
    /// The implementation will reorder the endianness as necessary for the target encoding format.
    ///
    /// # Panics
    ///
    /// Panics if `width * height * color_type.bytes_per_pixel() != buf.len()`.
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()>;

    /// Set the ICC profile to use for the image.
    ///
    /// This function is a no-op for formats that don't support ICC profiles.
    /// For formats that do support ICC profiles, the profile will be embedded
    /// in the image when it is saved.
    ///
    /// # Errors
    ///
    /// This function returns an error if the format does not support ICC profiles.
    fn set_icc_profile(&mut self, icc_profile: Vec<u8>) -> Result<(), UnsupportedError> {
        let _ = icc_profile;
        Err(UnsupportedError::from_format_and_kind(
            ImageFormatHint::Unknown,
            UnsupportedErrorKind::GenericFeature(
                "ICC profiles are not supported for this format".into(),
            ),
        ))
    }

    /// Set the EXIF metadata to use for the image.
    ///
    /// This function is a no-op for formats that don't support EXIF metadata.
    /// For formats that do support EXIF metadata, the metadata will be embedded
    /// in the image when it is saved.
    ///
    /// # Errors
    ///
    /// This function returns an error if the format does not support EXIF metadata or if the
    /// encoder doesn't implement saving EXIF metadata yet.
    fn set_exif_metadata(&mut self, exif: Vec<u8>) -> Result<(), UnsupportedError> {
        let _ = exif;
        Err(UnsupportedError::from_format_and_kind(
            ImageFormatHint::Unknown,
            UnsupportedErrorKind::GenericFeature(
                "EXIF metadata is not supported for this format".into(),
            ),
        ))
    }

    /// Convert the image to a compatible format for the encoder. This is used by the encoding
    /// methods on `DynamicImage`.
    ///
    /// Note that this is method is sealed to the crate and effectively pub(crate) due to the
    /// argument type not being nameable.
    #[doc(hidden)]
    fn make_compatible_img(
        &self,
        _: MethodSealedToImage,
        _input: &DynamicImage,
    ) -> Option<DynamicImage> {
        None
    }
}

pub(crate) trait ImageEncoderBoxed: ImageEncoder {
    fn write_image(
        self: Box<Self>,
        buf: &'_ [u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()>;
}
impl<T: ImageEncoder> ImageEncoderBoxed for T {
    fn write_image(
        self: Box<Self>,
        buf: &'_ [u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        (*self).write_image(buf, width, height, color)
    }
}

/// Implement `dynimage_conversion_sequence` for the common case of supporting only 8-bit colors
/// (with and without alpha).
#[allow(unused)]
pub(crate) fn dynimage_conversion_8bit(img: &DynamicImage) -> Option<DynamicImage> {
    use ColorType::*;

    match img.color() {
        Rgb8 | Rgba8 | L8 | La8 => None,
        L16 => Some(img.to_luma8().into()),
        La16 => Some(img.to_luma_alpha8().into()),
        Rgb16 | Rgb32F => Some(img.to_rgb8().into()),
        Rgba16 | Rgba32F => Some(img.to_rgba8().into()),
    }
}
