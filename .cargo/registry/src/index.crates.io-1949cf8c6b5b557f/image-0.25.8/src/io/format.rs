use std::ffi::OsStr;
use std::path::Path;

use crate::error::{ImageError, ImageFormatHint, ImageResult};

/// An enumeration of supported image formats.
/// Not all formats support both encoding and decoding.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ImageFormat {
    /// An Image in PNG Format
    Png,

    /// An Image in JPEG Format
    Jpeg,

    /// An Image in GIF Format
    Gif,

    /// An Image in WEBP Format
    WebP,

    /// An Image in general PNM Format
    Pnm,

    /// An Image in TIFF Format
    Tiff,

    /// An Image in TGA Format
    Tga,

    /// An Image in DDS Format
    Dds,

    /// An Image in BMP Format
    Bmp,

    /// An Image in ICO Format
    Ico,

    /// An Image in Radiance HDR Format
    Hdr,

    /// An Image in OpenEXR Format
    OpenExr,

    /// An Image in farbfeld Format
    Farbfeld,

    /// An Image in AVIF Format
    Avif,

    /// An Image in QOI Format
    Qoi,

    /// An Image in PCX Format
    #[cfg_attr(not(feature = "serde"), deprecated)]
    #[doc(hidden)]
    Pcx,
}

impl ImageFormat {
    /// Return the image format specified by a path's file extension.
    ///
    /// # Example
    ///
    /// ```
    /// use image::ImageFormat;
    ///
    /// let format = ImageFormat::from_extension("jpg");
    /// assert_eq!(format, Some(ImageFormat::Jpeg));
    /// ```
    #[inline]
    pub fn from_extension<S>(ext: S) -> Option<Self>
    where
        S: AsRef<OsStr>,
    {
        // thin wrapper function to strip generics
        fn inner(ext: &OsStr) -> Option<ImageFormat> {
            let ext = ext.to_str()?.to_ascii_lowercase();
            // NOTE: when updating this, also update extensions_str()
            Some(match ext.as_str() {
                "avif" => ImageFormat::Avif,
                "jpg" | "jpeg" | "jfif" => ImageFormat::Jpeg,
                "png" | "apng" => ImageFormat::Png,
                "gif" => ImageFormat::Gif,
                "webp" => ImageFormat::WebP,
                "tif" | "tiff" => ImageFormat::Tiff,
                "tga" => ImageFormat::Tga,
                "dds" => ImageFormat::Dds,
                "bmp" => ImageFormat::Bmp,
                "ico" => ImageFormat::Ico,
                "hdr" => ImageFormat::Hdr,
                "exr" => ImageFormat::OpenExr,
                "pbm" | "pam" | "ppm" | "pgm" | "pnm" => ImageFormat::Pnm,
                "ff" => ImageFormat::Farbfeld,
                "qoi" => ImageFormat::Qoi,
                _ => return None,
            })
        }

        inner(ext.as_ref())
    }

    /// Return the image format specified by the path's file extension.
    ///
    /// # Example
    ///
    /// ```
    /// use image::ImageFormat;
    ///
    /// let format = ImageFormat::from_path("images/ferris.png")?;
    /// assert_eq!(format, ImageFormat::Png);
    ///
    /// # Ok::<(), image::error::ImageError>(())
    /// ```
    #[inline]
    pub fn from_path<P>(path: P) -> ImageResult<Self>
    where
        P: AsRef<Path>,
    {
        // thin wrapper function to strip generics
        fn inner(path: &Path) -> ImageResult<ImageFormat> {
            let exact_ext = path.extension();
            exact_ext
                .and_then(ImageFormat::from_extension)
                .ok_or_else(|| {
                    let format_hint = match exact_ext {
                        None => ImageFormatHint::Unknown,
                        Some(os) => ImageFormatHint::PathExtension(os.into()),
                    };
                    ImageError::Unsupported(format_hint.into())
                })
        }

        inner(path.as_ref())
    }

    /// Return the image format specified by a MIME type.
    ///
    /// # Example
    ///
    /// ```
    /// use image::ImageFormat;
    ///
    /// let format = ImageFormat::from_mime_type("image/png").unwrap();
    /// assert_eq!(format, ImageFormat::Png);
    /// ```
    pub fn from_mime_type<M>(mime_type: M) -> Option<Self>
    where
        M: AsRef<str>,
    {
        match mime_type.as_ref() {
            "image/avif" => Some(ImageFormat::Avif),
            "image/jpeg" => Some(ImageFormat::Jpeg),
            "image/png" => Some(ImageFormat::Png),
            "image/gif" => Some(ImageFormat::Gif),
            "image/webp" => Some(ImageFormat::WebP),
            "image/tiff" => Some(ImageFormat::Tiff),
            "image/x-targa" | "image/x-tga" => Some(ImageFormat::Tga),
            "image/vnd-ms.dds" => Some(ImageFormat::Dds),
            "image/bmp" => Some(ImageFormat::Bmp),
            "image/x-icon" | "image/vnd.microsoft.icon" => Some(ImageFormat::Ico),
            "image/vnd.radiance" => Some(ImageFormat::Hdr),
            "image/x-exr" => Some(ImageFormat::OpenExr),
            "image/x-portable-bitmap"
            | "image/x-portable-graymap"
            | "image/x-portable-pixmap"
            | "image/x-portable-anymap" => Some(ImageFormat::Pnm),
            // Qoi's MIME type is being worked on.
            // See: https://github.com/phoboslab/qoi/issues/167
            "image/x-qoi" => Some(ImageFormat::Qoi),
            _ => None,
        }
    }

    /// Return the MIME type for this image format or "application/octet-stream" if no MIME type
    /// exists for the format.
    ///
    /// Some notes on a few of the MIME types:
    ///
    /// - The portable anymap format has a separate MIME type for the pixmap, graymap and bitmap
    ///   formats, but this method returns the general "image/x-portable-anymap" MIME type.
    /// - The Targa format has two common MIME types, "image/x-targa"  and "image/x-tga"; this
    ///   method returns "image/x-targa" for that format.
    /// - The QOI MIME type is still a work in progress. This method returns "image/x-qoi" for
    ///   that format.
    ///
    /// # Example
    ///
    /// ```
    /// use image::ImageFormat;
    ///
    /// let mime_type = ImageFormat::Png.to_mime_type();
    /// assert_eq!(mime_type, "image/png");
    /// ```
    #[must_use]
    pub fn to_mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Avif => "image/avif",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Tiff => "image/tiff",
            // the targa MIME type has two options, but this one seems to be used more
            ImageFormat::Tga => "image/x-targa",
            ImageFormat::Dds => "image/vnd-ms.dds",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Ico => "image/x-icon",
            ImageFormat::Hdr => "image/vnd.radiance",
            ImageFormat::OpenExr => "image/x-exr",
            // return the most general MIME type
            ImageFormat::Pnm => "image/x-portable-anymap",
            // Qoi's MIME type is being worked on.
            // See: https://github.com/phoboslab/qoi/issues/167
            ImageFormat::Qoi => "image/x-qoi",
            // farbfeld's MIME type taken from https://www.wikidata.org/wiki/Q28206109
            ImageFormat::Farbfeld => "application/octet-stream",
            #[allow(deprecated)]
            ImageFormat::Pcx => "image/vnd.zbrush.pcx",
        }
    }

    /// Return if the `ImageFormat` can be decoded by the lib.
    #[inline]
    #[must_use]
    pub fn can_read(&self) -> bool {
        // Needs to be updated once a new variant's decoder is added to free_functions.rs::load
        match self {
            ImageFormat::Png => true,
            ImageFormat::Gif => true,
            ImageFormat::Jpeg => true,
            ImageFormat::WebP => true,
            ImageFormat::Tiff => true,
            ImageFormat::Tga => true,
            ImageFormat::Dds => false,
            ImageFormat::Bmp => true,
            ImageFormat::Ico => true,
            ImageFormat::Hdr => true,
            ImageFormat::OpenExr => true,
            ImageFormat::Pnm => true,
            ImageFormat::Farbfeld => true,
            ImageFormat::Avif => true,
            ImageFormat::Qoi => true,
            #[allow(deprecated)]
            ImageFormat::Pcx => false,
        }
    }

    /// Return if the `ImageFormat` can be encoded by the lib.
    #[inline]
    #[must_use]
    pub fn can_write(&self) -> bool {
        // Needs to be updated once a new variant's encoder is added to free_functions.rs::save_buffer_with_format_impl
        match self {
            ImageFormat::Gif => true,
            ImageFormat::Ico => true,
            ImageFormat::Jpeg => true,
            ImageFormat::Png => true,
            ImageFormat::Bmp => true,
            ImageFormat::Tiff => true,
            ImageFormat::Tga => true,
            ImageFormat::Pnm => true,
            ImageFormat::Farbfeld => true,
            ImageFormat::Avif => true,
            ImageFormat::WebP => true,
            ImageFormat::Hdr => true,
            ImageFormat::OpenExr => true,
            ImageFormat::Dds => false,
            ImageFormat::Qoi => true,
            #[allow(deprecated)]
            ImageFormat::Pcx => false,
        }
    }

    /// Return a list of applicable extensions for this format.
    ///
    /// All currently recognized image formats specify at least on extension but for future
    /// compatibility you should not rely on this fact. The list may be empty if the format has no
    /// recognized file representation, for example in case it is used as a purely transient memory
    /// format.
    ///
    /// The method name `extensions` remains reserved for introducing another method in the future
    /// that yields a slice of `OsStr` which is blocked by several features of const evaluation.
    #[must_use]
    pub fn extensions_str(self) -> &'static [&'static str] {
        // NOTE: when updating this, also update from_extension()
        match self {
            ImageFormat::Png => &["png"],
            ImageFormat::Jpeg => &["jpg", "jpeg"],
            ImageFormat::Gif => &["gif"],
            ImageFormat::WebP => &["webp"],
            ImageFormat::Pnm => &["pbm", "pam", "ppm", "pgm", "pnm"],
            ImageFormat::Tiff => &["tiff", "tif"],
            ImageFormat::Tga => &["tga"],
            ImageFormat::Dds => &["dds"],
            ImageFormat::Bmp => &["bmp"],
            ImageFormat::Ico => &["ico"],
            ImageFormat::Hdr => &["hdr"],
            ImageFormat::OpenExr => &["exr"],
            ImageFormat::Farbfeld => &["ff"],
            // According to: https://aomediacodec.github.io/av1-avif/#mime-registration
            ImageFormat::Avif => &["avif"],
            ImageFormat::Qoi => &["qoi"],
            #[allow(deprecated)]
            ImageFormat::Pcx => &["pcx"],
        }
    }

    /// Return the `ImageFormat`s which are enabled for reading.
    #[inline]
    #[must_use]
    pub fn reading_enabled(&self) -> bool {
        match self {
            ImageFormat::Png => cfg!(feature = "png"),
            ImageFormat::Gif => cfg!(feature = "gif"),
            ImageFormat::Jpeg => cfg!(feature = "jpeg"),
            ImageFormat::WebP => cfg!(feature = "webp"),
            ImageFormat::Tiff => cfg!(feature = "tiff"),
            ImageFormat::Tga => cfg!(feature = "tga"),
            ImageFormat::Bmp => cfg!(feature = "bmp"),
            ImageFormat::Ico => cfg!(feature = "ico"),
            ImageFormat::Hdr => cfg!(feature = "hdr"),
            ImageFormat::OpenExr => cfg!(feature = "exr"),
            ImageFormat::Pnm => cfg!(feature = "pnm"),
            ImageFormat::Farbfeld => cfg!(feature = "ff"),
            ImageFormat::Avif => cfg!(feature = "avif"),
            ImageFormat::Qoi => cfg!(feature = "qoi"),
            #[allow(deprecated)]
            ImageFormat::Pcx => false,
            ImageFormat::Dds => false,
        }
    }

    /// Return the `ImageFormat`s which are enabled for writing.
    #[inline]
    #[must_use]
    pub fn writing_enabled(&self) -> bool {
        match self {
            ImageFormat::Gif => cfg!(feature = "gif"),
            ImageFormat::Ico => cfg!(feature = "ico"),
            ImageFormat::Jpeg => cfg!(feature = "jpeg"),
            ImageFormat::Png => cfg!(feature = "png"),
            ImageFormat::Bmp => cfg!(feature = "bmp"),
            ImageFormat::Tiff => cfg!(feature = "tiff"),
            ImageFormat::Tga => cfg!(feature = "tga"),
            ImageFormat::Pnm => cfg!(feature = "pnm"),
            ImageFormat::Farbfeld => cfg!(feature = "ff"),
            ImageFormat::Avif => cfg!(feature = "avif"),
            ImageFormat::WebP => cfg!(feature = "webp"),
            ImageFormat::OpenExr => cfg!(feature = "exr"),
            ImageFormat::Qoi => cfg!(feature = "qoi"),
            ImageFormat::Hdr => cfg!(feature = "hdr"),
            #[allow(deprecated)]
            ImageFormat::Pcx => false,
            ImageFormat::Dds => false,
        }
    }

    /// Return all `ImageFormat`s
    pub fn all() -> impl Iterator<Item = ImageFormat> {
        [
            ImageFormat::Gif,
            ImageFormat::Ico,
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
            ImageFormat::Tga,
            ImageFormat::Pnm,
            ImageFormat::Farbfeld,
            ImageFormat::Avif,
            ImageFormat::WebP,
            ImageFormat::OpenExr,
            ImageFormat::Qoi,
            ImageFormat::Dds,
            ImageFormat::Hdr,
            #[allow(deprecated)]
            ImageFormat::Pcx,
        ]
        .iter()
        .copied()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::{ImageFormat, ImageResult};

    #[test]
    fn test_image_format_from_path() {
        fn from_path(s: &str) -> ImageResult<ImageFormat> {
            ImageFormat::from_path(Path::new(s))
        }
        assert_eq!(from_path("./a.jpg").unwrap(), ImageFormat::Jpeg);
        assert_eq!(from_path("./a.jpeg").unwrap(), ImageFormat::Jpeg);
        assert_eq!(from_path("./a.JPEG").unwrap(), ImageFormat::Jpeg);
        assert_eq!(from_path("./a.pNg").unwrap(), ImageFormat::Png);
        assert_eq!(from_path("./a.gif").unwrap(), ImageFormat::Gif);
        assert_eq!(from_path("./a.webp").unwrap(), ImageFormat::WebP);
        assert_eq!(from_path("./a.tiFF").unwrap(), ImageFormat::Tiff);
        assert_eq!(from_path("./a.tif").unwrap(), ImageFormat::Tiff);
        assert_eq!(from_path("./a.tga").unwrap(), ImageFormat::Tga);
        assert_eq!(from_path("./a.dds").unwrap(), ImageFormat::Dds);
        assert_eq!(from_path("./a.bmp").unwrap(), ImageFormat::Bmp);
        assert_eq!(from_path("./a.Ico").unwrap(), ImageFormat::Ico);
        assert_eq!(from_path("./a.hdr").unwrap(), ImageFormat::Hdr);
        assert_eq!(from_path("./a.exr").unwrap(), ImageFormat::OpenExr);
        assert_eq!(from_path("./a.pbm").unwrap(), ImageFormat::Pnm);
        assert_eq!(from_path("./a.pAM").unwrap(), ImageFormat::Pnm);
        assert_eq!(from_path("./a.Ppm").unwrap(), ImageFormat::Pnm);
        assert_eq!(from_path("./a.pgm").unwrap(), ImageFormat::Pnm);
        assert_eq!(from_path("./a.AViF").unwrap(), ImageFormat::Avif);
        assert!(from_path("./a.txt").is_err());
        assert!(from_path("./a").is_err());
    }

    #[test]
    fn image_formats_are_recognized() {
        use ImageFormat::*;
        const ALL_FORMATS: &[ImageFormat] = &[
            Avif, Png, Jpeg, Gif, WebP, Pnm, Tiff, Tga, Dds, Bmp, Ico, Hdr, Farbfeld, OpenExr,
        ];
        for &format in ALL_FORMATS {
            let mut file = Path::new("file.nothing").to_owned();
            for ext in format.extensions_str() {
                assert!(file.set_extension(ext));
                match ImageFormat::from_path(&file) {
                    Err(_) => panic!("Path {} not recognized as {:?}", file.display(), format),
                    Ok(result) => assert_eq!(format, result),
                }
            }
        }
    }

    #[test]
    fn all() {
        let all_formats: HashSet<ImageFormat> = ImageFormat::all().collect();
        assert!(all_formats.contains(&ImageFormat::Avif));
        assert!(all_formats.contains(&ImageFormat::Gif));
        assert!(all_formats.contains(&ImageFormat::Bmp));
        assert!(all_formats.contains(&ImageFormat::Farbfeld));
        assert!(all_formats.contains(&ImageFormat::Jpeg));
    }

    #[test]
    fn reading_enabled() {
        assert_eq!(cfg!(feature = "jpeg"), ImageFormat::Jpeg.reading_enabled());
        assert_eq!(
            cfg!(feature = "ff"),
            ImageFormat::Farbfeld.reading_enabled()
        );
        assert!(!ImageFormat::Dds.reading_enabled());
    }

    #[test]
    fn writing_enabled() {
        assert_eq!(cfg!(feature = "jpeg"), ImageFormat::Jpeg.writing_enabled());
        assert_eq!(
            cfg!(feature = "ff"),
            ImageFormat::Farbfeld.writing_enabled()
        );
        assert!(!ImageFormat::Dds.writing_enabled());
    }
}
