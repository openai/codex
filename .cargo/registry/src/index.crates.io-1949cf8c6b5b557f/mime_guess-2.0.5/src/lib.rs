//! Guessing of MIME types by file extension.
//!
//! Uses a static list of file-extension : MIME type mappings.
//!
//! ```
//! # extern crate mime;
//! // the file doesn't have to exist, it just looks at the path
//! let guess = mime_guess::from_path("some_file.gif");
//! assert_eq!(guess.first(), Some(mime::IMAGE_GIF));
//!
//! ```
//!
//! #### Note: MIME Types Returned Are Not Stable/Guaranteed
//! The media types returned for a given extension are not considered to be part of the crate's
//! stable API and are often updated in patch <br /> (`x.y.[z + 1]`) releases to be as correct as
//! possible.
//!
//! Additionally, only the extensions of paths/filenames are inspected in order to guess the MIME
//! type. The file that may or may not reside at that path may or may not be a valid file of the
//! returned MIME type.  Be wary of unsafe or un-validated assumptions about file structure or
//! length.
pub extern crate mime;
extern crate unicase;

pub use mime::Mime;

use std::ffi::OsStr;
use std::iter::FusedIterator;
use std::path::Path;
use std::{iter, slice};

#[cfg(feature = "phf")]
#[path = "impl_phf.rs"]
mod impl_;

#[cfg(not(feature = "phf"))]
#[path = "impl_bin_search.rs"]
mod impl_;

/// A "guess" of the MIME/Media Type(s) of an extension or path as one or more
/// [`Mime`](struct.Mime.html) instances.
///
/// ### Note: Ordering
/// A given file format may have one or more applicable Media Types; in this case
/// the first Media Type returned is whatever is declared in the latest IETF RFC for the
/// presumed file format or the one that explicitly supercedes all others.
/// Ordering of additional Media Types is arbitrary.
///
/// ### Note: Values Not Stable
/// The exact Media Types returned in any given guess are not considered to be stable and are often
/// updated in patch releases in order to reflect the most up-to-date information possible.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
// FIXME: change repr when `mime` gains macro/const fn constructor
pub struct MimeGuess(&'static [&'static str]);

impl MimeGuess {
    /// Guess the MIME type of a file (real or otherwise) with the given extension.
    ///
    /// The search is case-insensitive.
    ///
    /// If `ext` is empty or has no (currently) known MIME type mapping, then an empty guess is
    /// returned.
    pub fn from_ext(ext: &str) -> MimeGuess {
        if ext.is_empty() {
            return MimeGuess(&[]);
        }

        impl_::get_mime_types(ext).map_or(MimeGuess(&[]), |v| MimeGuess(v))
    }

    /// Guess the MIME type of `path` by its extension (as defined by
    /// [`Path::extension()`]). **No disk access is performed.**
    ///
    /// If `path` has no extension, the extension cannot be converted to `str`, or has
    /// no known MIME type mapping, then an empty guess is returned.
    ///
    /// The search is case-insensitive.
    ///
    /// ## Note
    /// **Guess** is the operative word here, as there are no guarantees that the contents of the
    /// file that `path` points to match the MIME type associated with the path's extension.
    ///
    /// Take care when processing files with assumptions based on the return value of this function.
    ///
    /// [`Path::extension()`]: https://doc.rust-lang.org/std/path/struct.Path.html#method.extension
    pub fn from_path<P: AsRef<Path>>(path: P) -> MimeGuess {
        path.as_ref()
            .extension()
            .and_then(OsStr::to_str)
            .map_or(MimeGuess(&[]), Self::from_ext)
    }

    /// `true` if the guess did not return any known mappings for the given path or extension.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the number of MIME types in the current guess.
    pub fn count(&self) -> usize {
        self.0.len()
    }

    /// Get the first guessed `Mime`, if applicable.
    ///
    /// See [Note: Ordering](#note-ordering) above.
    pub fn first(&self) -> Option<Mime> {
        self.first_raw().map(expect_mime)
    }

    /// Get the first guessed Media Type as a string, if applicable.
    ///
    /// See [Note: Ordering](#note-ordering) above.
    pub fn first_raw(&self) -> Option<&'static str> {
        self.0.get(0).cloned()
    }

    /// Get the first guessed `Mime`, or if the guess is empty, return
    /// [`application/octet-stream`] instead.
    ///
    /// See [Note: Ordering](#note-ordering) above.
    ///
    /// ### Note: HTTP Applications
    /// For HTTP request and response bodies if a value for the `Content-Type` header
    /// cannot be determined it might be preferable to not send one at all instead of defaulting to
    /// `application/octet-stream` as the recipient will expect to infer the format directly from
    /// the content instead. ([RFC 7231, Section 3.1.1.5][rfc7231])
    ///
    /// On the contrary, for `multipart/form-data` bodies, the `Content-Type` of a form-data part is
    /// assumed to be `text/plain` unless specified so a default of `application/octet-stream`
    /// for non-text parts is safer. ([RFC 7578, Section 4.4][rfc7578])
    ///
    /// [`application/octet-stream`]: https://docs.rs/mime/0.3/mime/constant.APPLICATION_OCTET_STREAM.html
    /// [rfc7231]: https://tools.ietf.org/html/rfc7231#section-3.1.1.5
    /// [rfc7578]: https://tools.ietf.org/html/rfc7578#section-4.4
    pub fn first_or_octet_stream(&self) -> Mime {
        self.first_or(mime::APPLICATION_OCTET_STREAM)
    }

    /// Get the first guessed `Mime`, or if the guess is empty, return
    /// [`text/plain`](::mime::TEXT_PLAIN) instead.
    ///
    /// See [Note: Ordering](#note-ordering) above.
    pub fn first_or_text_plain(&self) -> Mime {
        self.first_or(mime::TEXT_PLAIN)
    }

    /// Get the first guessed `Mime`, or if the guess is empty, return the given `Mime` instead.
    ///
    /// See [Note: Ordering](#note-ordering) above.
    pub fn first_or(&self, default: Mime) -> Mime {
        self.first().unwrap_or(default)
    }

    /// Get the first guessed `Mime`, or if the guess is empty, execute the closure and return its
    /// result.
    ///
    /// See [Note: Ordering](#note-ordering) above.
    pub fn first_or_else<F>(&self, default_fn: F) -> Mime
    where
        F: FnOnce() -> Mime,
    {
        self.first().unwrap_or_else(default_fn)
    }

    /// Get an iterator over the `Mime` values contained in this guess.
    ///
    /// See [Note: Ordering](#note-ordering) above.
    pub fn iter(&self) -> Iter {
        Iter(self.iter_raw().map(expect_mime))
    }

    /// Get an iterator over the raw media-type strings in this guess.
    ///
    /// See [Note: Ordering](#note-ordering) above.
    pub fn iter_raw(&self) -> IterRaw {
        IterRaw(self.0.iter().cloned())
    }
}

impl IntoIterator for MimeGuess {
    type Item = Mime;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a MimeGuess {
    type Item = Mime;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An iterator over the `Mime` types of a `MimeGuess`.
///
/// See [Note: Ordering on `MimeGuess`](struct.MimeGuess.html#note-ordering).
#[derive(Clone, Debug)]
pub struct Iter(iter::Map<IterRaw, fn(&'static str) -> Mime>);

impl Iterator for Iter {
    type Item = Mime;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for Iter {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl FusedIterator for Iter {}

impl ExactSizeIterator for Iter {
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// An iterator over the raw media type strings of a `MimeGuess`.
///
/// See [Note: Ordering on `MimeGuess`](struct.MimeGuess.html#note-ordering).
#[derive(Clone, Debug)]
pub struct IterRaw(iter::Cloned<slice::Iter<'static, &'static str>>);

impl Iterator for IterRaw {
    type Item = &'static str;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for IterRaw {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl FusedIterator for IterRaw {}

impl ExactSizeIterator for IterRaw {
    fn len(&self) -> usize {
        self.0.len()
    }
}

fn expect_mime(s: &str) -> Mime {
    // `.parse()` should be checked at compile time to never fail
    s.parse()
        .unwrap_or_else(|e| panic!("failed to parse media-type {:?}: {}", s, e))
}

/// Wrapper of [`MimeGuess::from_ext()`](struct.MimeGuess.html#method.from_ext).
pub fn from_ext(ext: &str) -> MimeGuess {
    MimeGuess::from_ext(ext)
}

/// Wrapper of [`MimeGuess::from_path()`](struct.MimeGuess.html#method.from_path).
pub fn from_path<P: AsRef<Path>>(path: P) -> MimeGuess {
    MimeGuess::from_path(path)
}

/// Guess the MIME type of `path` by its extension (as defined by `Path::extension()`).
///
/// If `path` has no extension, or its extension has no known MIME type mapping,
/// then the MIME type is assumed to be `application/octet-stream`.
///
/// ## Note
/// **Guess** is the operative word here, as there are no guarantees that the contents of the file
/// that `path` points to match the MIME type associated with the path's extension.
///
/// Take care when processing files with assumptions based on the return value of this function.
///
/// In HTTP applications, it might be [preferable][rfc7231] to not send a `Content-Type`
/// header at all instead of defaulting to `application/octet-stream`.
///
/// [rfc7231]: https://tools.ietf.org/html/rfc7231#section-3.1.1.5
#[deprecated(
    since = "2.0.0",
    note = "Use `from_path(path).first_or_octet_stream()` instead"
)]
pub fn guess_mime_type<P: AsRef<Path>>(path: P) -> Mime {
    from_path(path).first_or_octet_stream()
}

/// Guess the MIME type of `path` by its extension (as defined by `Path::extension()`).
///
/// If `path` has no extension, or its extension has no known MIME type mapping,
/// then `None` is returned.
///
#[deprecated(since = "2.0.0", note = "Use `from_path(path).first()` instead")]
pub fn guess_mime_type_opt<P: AsRef<Path>>(path: P) -> Option<Mime> {
    from_path(path).first()
}

/// Guess the MIME type string of `path` by its extension (as defined by `Path::extension()`).
///
/// If `path` has no extension, or its extension has no known MIME type mapping,
/// then `None` is returned.
///
/// ## Note
/// **Guess** is the operative word here, as there are no guarantees that the contents of the file
/// that `path` points to match the MIME type associated with the path's extension.
///
/// Take care when processing files with assumptions based on the return value of this function.
#[deprecated(since = "2.0.0", note = "Use `from_path(path).first_raw()` instead")]
pub fn mime_str_for_path_ext<P: AsRef<Path>>(path: P) -> Option<&'static str> {
    from_path(path).first_raw()
}

/// Get the MIME type associated with a file extension.
///
/// If there is no association for the extension, or `ext` is empty,
/// `application/octet-stream` is returned.
///
/// ## Note
/// In HTTP applications, it might be [preferable][rfc7231] to not send a `Content-Type`
/// header at all instead of defaulting to `application/octet-stream`.
///
/// [rfc7231]: https://tools.ietf.org/html/rfc7231#section-3.1.1.5
#[deprecated(
    since = "2.0.0",
    note = "use `from_ext(search_ext).first_or_octet_stream()` instead"
)]
pub fn get_mime_type(search_ext: &str) -> Mime {
    from_ext(search_ext).first_or_octet_stream()
}

/// Get the MIME type associated with a file extension.
///
/// If there is no association for the extension, or `ext` is empty,
/// `None` is returned.
#[deprecated(since = "2.0.0", note = "use `from_ext(search_ext).first()` instead")]
pub fn get_mime_type_opt(search_ext: &str) -> Option<Mime> {
    from_ext(search_ext).first()
}

/// Get the MIME type string associated with a file extension. Case-insensitive.
///
/// If `search_ext` is not already lowercase,
/// it will be converted to lowercase to facilitate the search.
///
/// Returns `None` if `search_ext` is empty or an associated extension was not found.
#[deprecated(
    since = "2.0.0",
    note = "use `from_ext(search_ext).first_raw()` instead"
)]
pub fn get_mime_type_str(search_ext: &str) -> Option<&'static str> {
    from_ext(search_ext).first_raw()
}

/// Get a list of known extensions for a given `Mime`.
///
/// Ignores parameters (only searches with `<main type>/<subtype>`). Case-insensitive (for extension types).
///
/// Returns `None` if the MIME type is unknown.
///
/// ### Wildcards
/// If the top-level of the MIME type is a wildcard (`*`), returns all extensions.
///
/// If the sub-level of the MIME type is a wildcard, returns all extensions for the top-level.
#[cfg(feature = "rev-mappings")]
pub fn get_mime_extensions(mime: &Mime) -> Option<&'static [&'static str]> {
    get_extensions(mime.type_().as_ref(), mime.subtype().as_ref())
}

/// Get a list of known extensions for a MIME type string.
///
/// Ignores parameters (only searches `<main type>/<subtype>`). Case-insensitive.
///
/// Returns `None` if the MIME type is unknown.
///
/// ### Wildcards
/// If the top-level of the MIME type is a wildcard (`*`), returns all extensions.
///
/// If the sub-level of the MIME type is a wildcard, returns all extensions for the top-level.
///
/// ### Panics
/// If `mime_str` is not a valid MIME type specifier (naive).
#[cfg(feature = "rev-mappings")]
pub fn get_mime_extensions_str(mut mime_str: &str) -> Option<&'static [&'static str]> {
    mime_str = mime_str.trim();

    if let Some(sep_idx) = mime_str.find(';') {
        mime_str = &mime_str[..sep_idx];
    }

    let (top, sub) = {
        let split_idx = mime_str.find('/')?;
        (&mime_str[..split_idx], &mime_str[split_idx + 1..])
    };

    get_extensions(top, sub)
}

/// Get the extensions for a given top-level and sub-level of a MIME type
/// (`{toplevel}/{sublevel}`).
///
/// Returns `None` if `toplevel` or `sublevel` are unknown.
///
/// ### Wildcards
/// If the top-level of the MIME type is a wildcard (`*`), returns all extensions.
///
/// If the sub-level of the MIME type is a wildcard, returns all extensions for the top-level.
#[cfg(feature = "rev-mappings")]
pub fn get_extensions(toplevel: &str, sublevel: &str) -> Option<&'static [&'static str]> {
    impl_::get_extensions(toplevel, sublevel)
}

/// Get the MIME type for `application/octet-stream` (generic binary stream)
#[deprecated(since = "2.0.0", note = "use `mime::APPLICATION_OCTET_STREAM` instead")]
pub fn octet_stream() -> Mime {
    "application/octet-stream".parse().unwrap()
}

#[cfg(test)]
mod tests {
    include!("mime_types.rs");

    use super::{expect_mime, from_ext, from_path, get_mime_extensions_str};
    #[allow(deprecated, unused_imports)]
    use std::ascii::AsciiExt;

    use std::fmt::Debug;
    use std::path::Path;

    #[test]
    fn check_type_bounds() {
        fn assert_type_bounds<T: Clone + Debug + Send + Sync + 'static>() {}

        assert_type_bounds::<super::MimeGuess>();
        assert_type_bounds::<super::Iter>();
        assert_type_bounds::<super::IterRaw>();
    }

    #[test]
    fn test_mime_type_guessing() {
        assert_eq!(
            from_ext("gif").first_or_octet_stream().to_string(),
            "image/gif".to_string()
        );
        assert_eq!(
            from_ext("TXT").first_or_octet_stream().to_string(),
            "text/plain".to_string()
        );
        assert_eq!(
            from_ext("blahblah").first_or_octet_stream().to_string(),
            "application/octet-stream".to_string()
        );

        assert_eq!(
            from_path(Path::new("/path/to/file.gif"))
                .first_or_octet_stream()
                .to_string(),
            "image/gif".to_string()
        );
        assert_eq!(
            from_path("/path/to/file.gif")
                .first_or_octet_stream()
                .to_string(),
            "image/gif".to_string()
        );
    }

    #[test]
    fn test_mime_type_guessing_opt() {
        assert_eq!(
            from_ext("gif").first().unwrap().to_string(),
            "image/gif".to_string()
        );
        assert_eq!(
            from_ext("TXT").first().unwrap().to_string(),
            "text/plain".to_string()
        );
        assert_eq!(from_ext("blahblah").first(), None);

        assert_eq!(
            from_path("/path/to/file.gif").first().unwrap().to_string(),
            "image/gif".to_string()
        );
        assert_eq!(from_path("/path/to/file").first(), None);
    }

    #[test]
    fn test_are_mime_types_parseable() {
        for (_, mimes) in MIME_TYPES {
            mimes.iter().for_each(|s| {
                expect_mime(s);
            });
        }
    }

    // RFC: Is this test necessary anymore? --@cybergeek94, 2/1/2016
    #[test]
    fn test_are_extensions_ascii() {
        for (ext, _) in MIME_TYPES {
            assert!(ext.is_ascii(), "Extension not ASCII: {:?}", ext);
        }
    }

    #[test]
    fn test_are_extensions_sorted() {
        // simultaneously checks the requirement that duplicate extension entries are adjacent
        for (&(ext, _), &(n_ext, _)) in MIME_TYPES.iter().zip(MIME_TYPES.iter().skip(1)) {
            assert!(
                ext <= n_ext,
                "Extensions in src/mime_types should be sorted lexicographically
                in ascending order. Failed assert: {:?} <= {:?}",
                ext,
                n_ext
            );
        }
    }

    #[test]
    fn test_get_mime_extensions_str_no_panic_if_bad_mime() {
        assert_eq!(get_mime_extensions_str(""), None);
    }
}
