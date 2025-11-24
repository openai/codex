//! multipart/form-data
use std::borrow::Cow;
use std::fmt;
use std::pin::Pin;

#[cfg(feature = "stream")]
use std::io;
#[cfg(feature = "stream")]
use std::path::Path;

use bytes::Bytes;
use mime_guess::Mime;
use percent_encoding::{self, AsciiSet, NON_ALPHANUMERIC};
#[cfg(feature = "stream")]
use tokio::fs::File;

use futures_core::Stream;
use futures_util::{future, stream, StreamExt};

use super::Body;
use crate::header::HeaderMap;

/// An async multipart/form-data request.
pub struct Form {
    inner: FormParts<Part>,
}

/// A field in a multipart form.
pub struct Part {
    meta: PartMetadata,
    value: Body,
    body_length: Option<u64>,
}

pub(crate) struct FormParts<P> {
    pub(crate) boundary: String,
    pub(crate) computed_headers: Vec<Vec<u8>>,
    pub(crate) fields: Vec<(Cow<'static, str>, P)>,
    pub(crate) percent_encoding: PercentEncoding,
}

pub(crate) struct PartMetadata {
    mime: Option<Mime>,
    file_name: Option<Cow<'static, str>>,
    pub(crate) headers: HeaderMap,
}

pub(crate) trait PartProps {
    fn value_len(&self) -> Option<u64>;
    fn metadata(&self) -> &PartMetadata;
}

// ===== impl Form =====

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

impl Form {
    /// Creates a new async Form without any content.
    pub fn new() -> Form {
        Form {
            inner: FormParts::new(),
        }
    }

    /// Get the boundary that this form will use.
    #[inline]
    pub fn boundary(&self) -> &str {
        self.inner.boundary()
    }

    /// Add a data field with supplied name and value.
    ///
    /// # Examples
    ///
    /// ```
    /// let form = reqwest::multipart::Form::new()
    ///     .text("username", "seanmonstar")
    ///     .text("password", "secret");
    /// ```
    pub fn text<T, U>(self, name: T, value: U) -> Form
    where
        T: Into<Cow<'static, str>>,
        U: Into<Cow<'static, str>>,
    {
        self.part(name, Part::text(value))
    }

    /// Adds a file field.
    ///
    /// The path will be used to try to guess the filename and mime.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn run() -> std::io::Result<()> {
    /// let form = reqwest::multipart::Form::new()
    ///     .file("key", "/path/to/file").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Errors when the file cannot be opened.
    #[cfg(feature = "stream")]
    #[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
    pub async fn file<T, U>(self, name: T, path: U) -> io::Result<Form>
    where
        T: Into<Cow<'static, str>>,
        U: AsRef<Path>,
    {
        Ok(self.part(name, Part::file(path).await?))
    }

    /// Adds a customized Part.
    pub fn part<T>(self, name: T, part: Part) -> Form
    where
        T: Into<Cow<'static, str>>,
    {
        self.with_inner(move |inner| inner.part(name, part))
    }

    /// Configure this `Form` to percent-encode using the `path-segment` rules.
    pub fn percent_encode_path_segment(self) -> Form {
        self.with_inner(|inner| inner.percent_encode_path_segment())
    }

    /// Configure this `Form` to percent-encode using the `attr-char` rules.
    pub fn percent_encode_attr_chars(self) -> Form {
        self.with_inner(|inner| inner.percent_encode_attr_chars())
    }

    /// Configure this `Form` to skip percent-encoding
    pub fn percent_encode_noop(self) -> Form {
        self.with_inner(|inner| inner.percent_encode_noop())
    }

    /// Consume this instance and transform into an instance of Body for use in a request.
    pub(crate) fn stream(self) -> Body {
        if self.inner.fields.is_empty() {
            return Body::empty();
        }

        Body::stream(self.into_stream())
    }

    /// Produce a stream of the bytes in this `Form`, consuming it.
    pub fn into_stream(mut self) -> impl Stream<Item = Result<Bytes, crate::Error>> + Send + Sync {
        if self.inner.fields.is_empty() {
            let empty_stream: Pin<
                Box<dyn Stream<Item = Result<Bytes, crate::Error>> + Send + Sync>,
            > = Box::pin(futures_util::stream::empty());
            return empty_stream;
        }

        // create initial part to init reduce chain
        let (name, part) = self.inner.fields.remove(0);
        let start = Box::pin(self.part_stream(name, part))
            as Pin<Box<dyn Stream<Item = crate::Result<Bytes>> + Send + Sync>>;

        let fields = self.inner.take_fields();
        // for each field, chain an additional stream
        let stream = fields.into_iter().fold(start, |memo, (name, part)| {
            let part_stream = self.part_stream(name, part);
            Box::pin(memo.chain(part_stream))
                as Pin<Box<dyn Stream<Item = crate::Result<Bytes>> + Send + Sync>>
        });
        // append special ending boundary
        let last = stream::once(future::ready(Ok(
            format!("--{}--\r\n", self.boundary()).into()
        )));
        Box::pin(stream.chain(last))
    }

    /// Generate a hyper::Body stream for a single Part instance of a Form request.
    pub(crate) fn part_stream<T>(
        &mut self,
        name: T,
        part: Part,
    ) -> impl Stream<Item = Result<Bytes, crate::Error>>
    where
        T: Into<Cow<'static, str>>,
    {
        // start with boundary
        let boundary = stream::once(future::ready(Ok(
            format!("--{}\r\n", self.boundary()).into()
        )));
        // append headers
        let header = stream::once(future::ready(Ok({
            let mut h = self
                .inner
                .percent_encoding
                .encode_headers(&name.into(), &part.meta);
            h.extend_from_slice(b"\r\n\r\n");
            h.into()
        })));
        // then append form data followed by terminating CRLF
        boundary
            .chain(header)
            .chain(part.value.into_stream())
            .chain(stream::once(future::ready(Ok("\r\n".into()))))
    }

    pub(crate) fn compute_length(&mut self) -> Option<u64> {
        self.inner.compute_length()
    }

    fn with_inner<F>(self, func: F) -> Self
    where
        F: FnOnce(FormParts<Part>) -> FormParts<Part>,
    {
        Form {
            inner: func(self.inner),
        }
    }
}

impl fmt::Debug for Form {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt_fields("Form", f)
    }
}

// ===== impl Part =====

impl Part {
    /// Makes a text parameter.
    pub fn text<T>(value: T) -> Part
    where
        T: Into<Cow<'static, str>>,
    {
        let body = match value.into() {
            Cow::Borrowed(slice) => Body::from(slice),
            Cow::Owned(string) => Body::from(string),
        };
        Part::new(body, None)
    }

    /// Makes a new parameter from arbitrary bytes.
    pub fn bytes<T>(value: T) -> Part
    where
        T: Into<Cow<'static, [u8]>>,
    {
        let body = match value.into() {
            Cow::Borrowed(slice) => Body::from(slice),
            Cow::Owned(vec) => Body::from(vec),
        };
        Part::new(body, None)
    }

    /// Makes a new parameter from an arbitrary stream.
    pub fn stream<T: Into<Body>>(value: T) -> Part {
        Part::new(value.into(), None)
    }

    /// Makes a new parameter from an arbitrary stream with a known length. This is particularly
    /// useful when adding something like file contents as a stream, where you can know the content
    /// length beforehand.
    pub fn stream_with_length<T: Into<Body>>(value: T, length: u64) -> Part {
        Part::new(value.into(), Some(length))
    }

    /// Makes a file parameter.
    ///
    /// # Errors
    ///
    /// Errors when the file cannot be opened.
    #[cfg(feature = "stream")]
    #[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
    pub async fn file<T: AsRef<Path>>(path: T) -> io::Result<Part> {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .map(|filename| filename.to_string_lossy().into_owned());
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let mime = mime_guess::from_ext(ext).first_or_octet_stream();
        let file = File::open(path).await?;
        let len = file.metadata().await.map(|m| m.len()).ok();
        let field = match len {
            Some(len) => Part::stream_with_length(file, len),
            None => Part::stream(file),
        }
        .mime(mime);

        Ok(if let Some(file_name) = file_name {
            field.file_name(file_name)
        } else {
            field
        })
    }

    fn new(value: Body, body_length: Option<u64>) -> Part {
        Part {
            meta: PartMetadata::new(),
            value,
            body_length,
        }
    }

    /// Tries to set the mime of this part.
    pub fn mime_str(self, mime: &str) -> crate::Result<Part> {
        Ok(self.mime(mime.parse().map_err(crate::error::builder)?))
    }

    // Re-export when mime 0.4 is available, with split MediaType/MediaRange.
    fn mime(self, mime: Mime) -> Part {
        self.with_inner(move |inner| inner.mime(mime))
    }

    /// Sets the filename, builder style.
    pub fn file_name<T>(self, filename: T) -> Part
    where
        T: Into<Cow<'static, str>>,
    {
        self.with_inner(move |inner| inner.file_name(filename))
    }

    /// Sets custom headers for the part.
    pub fn headers(self, headers: HeaderMap) -> Part {
        self.with_inner(move |inner| inner.headers(headers))
    }

    fn with_inner<F>(self, func: F) -> Self
    where
        F: FnOnce(PartMetadata) -> PartMetadata,
    {
        Part {
            meta: func(self.meta),
            ..self
        }
    }
}

impl fmt::Debug for Part {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut dbg = f.debug_struct("Part");
        dbg.field("value", &self.value);
        self.meta.fmt_fields(&mut dbg);
        dbg.finish()
    }
}

impl PartProps for Part {
    fn value_len(&self) -> Option<u64> {
        if self.body_length.is_some() {
            self.body_length
        } else {
            self.value.content_length()
        }
    }

    fn metadata(&self) -> &PartMetadata {
        &self.meta
    }
}

// ===== impl FormParts =====

impl<P: PartProps> FormParts<P> {
    pub(crate) fn new() -> Self {
        FormParts {
            boundary: gen_boundary(),
            computed_headers: Vec::new(),
            fields: Vec::new(),
            percent_encoding: PercentEncoding::PathSegment,
        }
    }

    pub(crate) fn boundary(&self) -> &str {
        &self.boundary
    }

    /// Adds a customized Part.
    pub(crate) fn part<T>(mut self, name: T, part: P) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.fields.push((name.into(), part));
        self
    }

    /// Configure this `Form` to percent-encode using the `path-segment` rules.
    pub(crate) fn percent_encode_path_segment(mut self) -> Self {
        self.percent_encoding = PercentEncoding::PathSegment;
        self
    }

    /// Configure this `Form` to percent-encode using the `attr-char` rules.
    pub(crate) fn percent_encode_attr_chars(mut self) -> Self {
        self.percent_encoding = PercentEncoding::AttrChar;
        self
    }

    /// Configure this `Form` to skip percent-encoding
    pub(crate) fn percent_encode_noop(mut self) -> Self {
        self.percent_encoding = PercentEncoding::NoOp;
        self
    }

    // If predictable, computes the length the request will have
    // The length should be predictable if only String and file fields have been added,
    // but not if a generic reader has been added;
    pub(crate) fn compute_length(&mut self) -> Option<u64> {
        let mut length = 0u64;
        for &(ref name, ref field) in self.fields.iter() {
            match field.value_len() {
                Some(value_length) => {
                    // We are constructing the header just to get its length. To not have to
                    // construct it again when the request is sent we cache these headers.
                    let header = self.percent_encoding.encode_headers(name, field.metadata());
                    let header_length = header.len();
                    self.computed_headers.push(header);
                    // The additions mimic the format string out of which the field is constructed
                    // in Reader. Not the cleanest solution because if that format string is
                    // ever changed then this formula needs to be changed too which is not an
                    // obvious dependency in the code.
                    length += 2
                        + self.boundary().len() as u64
                        + 2
                        + header_length as u64
                        + 4
                        + value_length
                        + 2
                }
                _ => return None,
            }
        }
        // If there is at least one field there is a special boundary for the very last field.
        if !self.fields.is_empty() {
            length += 2 + self.boundary().len() as u64 + 4
        }
        Some(length)
    }

    /// Take the fields vector of this instance, replacing with an empty vector.
    fn take_fields(&mut self) -> Vec<(Cow<'static, str>, P)> {
        std::mem::replace(&mut self.fields, Vec::new())
    }
}

impl<P: fmt::Debug> FormParts<P> {
    pub(crate) fn fmt_fields(&self, ty_name: &'static str, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(ty_name)
            .field("boundary", &self.boundary)
            .field("parts", &self.fields)
            .finish()
    }
}

// ===== impl PartMetadata =====

impl PartMetadata {
    pub(crate) fn new() -> Self {
        PartMetadata {
            mime: None,
            file_name: None,
            headers: HeaderMap::default(),
        }
    }

    pub(crate) fn mime(mut self, mime: Mime) -> Self {
        self.mime = Some(mime);
        self
    }

    pub(crate) fn file_name<T>(mut self, filename: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.file_name = Some(filename.into());
        self
    }

    pub(crate) fn headers<T>(mut self, headers: T) -> Self
    where
        T: Into<HeaderMap>,
    {
        self.headers = headers.into();
        self
    }
}

impl PartMetadata {
    pub(crate) fn fmt_fields<'f, 'fa, 'fb>(
        &self,
        debug_struct: &'f mut fmt::DebugStruct<'fa, 'fb>,
    ) -> &'f mut fmt::DebugStruct<'fa, 'fb> {
        debug_struct
            .field("mime", &self.mime)
            .field("file_name", &self.file_name)
            .field("headers", &self.headers)
    }
}

// https://url.spec.whatwg.org/#fragment-percent-encode-set
const FRAGMENT_ENCODE_SET: &AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`');

// https://url.spec.whatwg.org/#path-percent-encode-set
const PATH_ENCODE_SET: &AsciiSet = &FRAGMENT_ENCODE_SET.add(b'#').add(b'?').add(b'{').add(b'}');

const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &PATH_ENCODE_SET.add(b'/').add(b'%');

// https://tools.ietf.org/html/rfc8187#section-3.2.1
const ATTR_CHAR_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'!')
    .remove(b'#')
    .remove(b'$')
    .remove(b'&')
    .remove(b'+')
    .remove(b'-')
    .remove(b'.')
    .remove(b'^')
    .remove(b'_')
    .remove(b'`')
    .remove(b'|')
    .remove(b'~');

pub(crate) enum PercentEncoding {
    PathSegment,
    AttrChar,
    NoOp,
}

impl PercentEncoding {
    pub(crate) fn encode_headers(&self, name: &str, field: &PartMetadata) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"Content-Disposition: form-data; ");

        match self.percent_encode(name) {
            Cow::Borrowed(value) => {
                // nothing has been percent encoded
                buf.extend_from_slice(b"name=\"");
                buf.extend_from_slice(value.as_bytes());
                buf.extend_from_slice(b"\"");
            }
            Cow::Owned(value) => {
                // something has been percent encoded
                buf.extend_from_slice(b"name*=utf-8''");
                buf.extend_from_slice(value.as_bytes());
            }
        }

        // According to RFC7578 Section 4.2, `filename*=` syntax is invalid.
        // See https://github.com/seanmonstar/reqwest/issues/419.
        if let Some(filename) = &field.file_name {
            buf.extend_from_slice(b"; filename=\"");
            let legal_filename = filename
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\r', "\\\r")
                .replace('\n', "\\\n");
            buf.extend_from_slice(legal_filename.as_bytes());
            buf.extend_from_slice(b"\"");
        }

        if let Some(mime) = &field.mime {
            buf.extend_from_slice(b"\r\nContent-Type: ");
            buf.extend_from_slice(mime.as_ref().as_bytes());
        }

        for (k, v) in field.headers.iter() {
            buf.extend_from_slice(b"\r\n");
            buf.extend_from_slice(k.as_str().as_bytes());
            buf.extend_from_slice(b": ");
            buf.extend_from_slice(v.as_bytes());
        }
        buf
    }

    fn percent_encode<'a>(&self, value: &'a str) -> Cow<'a, str> {
        use percent_encoding::utf8_percent_encode as percent_encode;

        match self {
            Self::PathSegment => percent_encode(value, PATH_SEGMENT_ENCODE_SET).into(),
            Self::AttrChar => percent_encode(value, ATTR_CHAR_ENCODE_SET).into(),
            Self::NoOp => value.into(),
        }
    }
}

fn gen_boundary() -> String {
    use crate::util::fast_random as random;

    let a = random();
    let b = random();
    let c = random();
    let d = random();

    format!("{a:016x}-{b:016x}-{c:016x}-{d:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use futures_util::TryStreamExt;
    use std::future;
    use tokio::{self, runtime};

    #[test]
    fn form_empty() {
        let form = Form::new();

        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("new rt");
        let body = form.stream().into_stream();
        let s = body.map_ok(|try_c| try_c.to_vec()).try_concat();

        let out = rt.block_on(s);
        assert!(out.unwrap().is_empty());
    }

    #[test]
    fn stream_to_end() {
        let mut form = Form::new()
            .part(
                "reader1",
                Part::stream(Body::stream(stream::once(future::ready::<
                    Result<String, crate::Error>,
                >(Ok(
                    "part1".to_owned()
                ))))),
            )
            .part("key1", Part::text("value1"))
            .part("key2", Part::text("value2").mime(mime::IMAGE_BMP))
            .part(
                "reader2",
                Part::stream(Body::stream(stream::once(future::ready::<
                    Result<String, crate::Error>,
                >(Ok(
                    "part2".to_owned()
                ))))),
            )
            .part("key3", Part::text("value3").file_name("filename"));
        form.inner.boundary = "boundary".to_string();
        let expected = "--boundary\r\n\
             Content-Disposition: form-data; name=\"reader1\"\r\n\r\n\
             part1\r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"key1\"\r\n\r\n\
             value1\r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"key2\"\r\n\
             Content-Type: image/bmp\r\n\r\n\
             value2\r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"reader2\"\r\n\r\n\
             part2\r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"key3\"; filename=\"filename\"\r\n\r\n\
             value3\r\n--boundary--\r\n";
        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("new rt");
        let body = form.stream().into_stream();
        let s = body.map(|try_c| try_c.map(|r| r.to_vec())).try_concat();

        let out = rt.block_on(s).unwrap();
        // These prints are for debug purposes in case the test fails
        println!(
            "START REAL\n{}\nEND REAL",
            std::str::from_utf8(&out).unwrap()
        );
        println!("START EXPECTED\n{expected}\nEND EXPECTED");
        assert_eq!(std::str::from_utf8(&out).unwrap(), expected);
    }

    #[test]
    fn stream_to_end_with_header() {
        let mut part = Part::text("value2").mime(mime::IMAGE_BMP);
        let mut headers = HeaderMap::new();
        headers.insert("Hdr3", "/a/b/c".parse().unwrap());
        part = part.headers(headers);
        let mut form = Form::new().part("key2", part);
        form.inner.boundary = "boundary".to_string();
        let expected = "--boundary\r\n\
                        Content-Disposition: form-data; name=\"key2\"\r\n\
                        Content-Type: image/bmp\r\n\
                        hdr3: /a/b/c\r\n\
                        \r\n\
                        value2\r\n\
                        --boundary--\r\n";
        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("new rt");
        let body = form.stream().into_stream();
        let s = body.map(|try_c| try_c.map(|r| r.to_vec())).try_concat();

        let out = rt.block_on(s).unwrap();
        // These prints are for debug purposes in case the test fails
        println!(
            "START REAL\n{}\nEND REAL",
            std::str::from_utf8(&out).unwrap()
        );
        println!("START EXPECTED\n{expected}\nEND EXPECTED");
        assert_eq!(std::str::from_utf8(&out).unwrap(), expected);
    }

    #[test]
    fn correct_content_length() {
        // Setup an arbitrary data stream
        let stream_data = b"just some stream data";
        let stream_len = stream_data.len();
        let stream_data = stream_data
            .chunks(3)
            .map(|c| Ok::<_, std::io::Error>(Bytes::from(c)));
        let the_stream = futures_util::stream::iter(stream_data);

        let bytes_data = b"some bytes data".to_vec();
        let bytes_len = bytes_data.len();

        let stream_part = Part::stream_with_length(Body::stream(the_stream), stream_len as u64);
        let body_part = Part::bytes(bytes_data);

        // A simple check to make sure we get the configured body length
        assert_eq!(stream_part.value_len().unwrap(), stream_len as u64);

        // Make sure it delegates to the underlying body if length is not specified
        assert_eq!(body_part.value_len().unwrap(), bytes_len as u64);
    }

    #[test]
    fn header_percent_encoding() {
        let name = "start%'\"\r\nßend";
        let field = Part::text("");

        assert_eq!(
            PercentEncoding::PathSegment.encode_headers(name, &field.meta),
            &b"Content-Disposition: form-data; name*=utf-8''start%25'%22%0D%0A%C3%9Fend"[..]
        );

        assert_eq!(
            PercentEncoding::AttrChar.encode_headers(name, &field.meta),
            &b"Content-Disposition: form-data; name*=utf-8''start%25%27%22%0D%0A%C3%9Fend"[..]
        );
    }
}
