//! multipart/form-data
//!
//! To send a `multipart/form-data` body, a [`Form`] is built up, adding
//! fields or customized [`Part`]s, and then calling the
//! [`multipart`][builder] method on the `RequestBuilder`.
//!
//! # Example
//!
//! ```
//! use reqwest::blocking::multipart;
//!
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let form = multipart::Form::new()
//!     // Adding just a simple text field...
//!     .text("username", "seanmonstar")
//!     // And a file...
//!     .file("photo", "/path/to/photo.png")?;
//!
//! // Customize all the details of a Part if needed...
//! let bio = multipart::Part::text("hallo peeps")
//!     .file_name("bio.txt")
//!     .mime_str("text/plain")?;
//!
//! // Add the custom part to our form...
//! let form = form.part("biography", bio);
//!
//! // And finally, send the form
//! let client = reqwest::blocking::Client::new();
//! let resp = client
//!     .post("http://localhost:8080/user")
//!     .multipart(form)
//!     .send()?;
//! # Ok(())
//! # }
//! # fn main() {}
//! ```
//!
//! [builder]: ../struct.RequestBuilder.html#method.multipart
use std::borrow::Cow;
use std::fmt;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;

use mime_guess::{self, Mime};

use super::Body;
use crate::async_impl::multipart::{FormParts, PartMetadata, PartProps};
use crate::header::HeaderMap;

/// A multipart/form-data request.
pub struct Form {
    inner: FormParts<Part>,
}

/// A field in a multipart form.
pub struct Part {
    meta: PartMetadata,
    value: Body,
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

impl Form {
    /// Creates a new Form without any content.
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
    /// let form = reqwest::blocking::multipart::Form::new()
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
    /// # fn run() -> std::io::Result<()> {
    /// let form = reqwest::blocking::multipart::Form::new()
    ///     .file("key", "/path/to/file")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Errors when the file cannot be opened.
    pub fn file<T, U>(self, name: T, path: U) -> io::Result<Form>
    where
        T: Into<Cow<'static, str>>,
        U: AsRef<Path>,
    {
        Ok(self.part(name, Part::file(path)?))
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

    pub(crate) fn reader(self) -> Reader {
        Reader::new(self)
    }

    /// Produce a reader over the multipart form data.
    pub fn into_reader(self) -> impl Read {
        self.reader()
    }

    // If predictable, computes the length the request will have
    // The length should be predictable if only String and file fields have been added,
    // but not if a generic reader has been added;
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
        Part::new(body)
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
        Part::new(body)
    }

    /// Adds a generic reader.
    ///
    /// Does not set filename or mime.
    pub fn reader<T: Read + Send + 'static>(value: T) -> Part {
        Part::new(Body::new(value))
    }

    /// Adds a generic reader with known length.
    ///
    /// Does not set filename or mime.
    pub fn reader_with_length<T: Read + Send + 'static>(value: T, length: u64) -> Part {
        Part::new(Body::sized(value, length))
    }

    /// Makes a file parameter.
    ///
    /// # Errors
    ///
    /// Errors when the file cannot be opened.
    pub fn file<T: AsRef<Path>>(path: T) -> io::Result<Part> {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .map(|filename| filename.to_string_lossy().into_owned());
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let mime = mime_guess::from_ext(ext).first_or_octet_stream();
        let file = File::open(path)?;
        let field = Part::new(Body::from(file)).mime(mime);

        Ok(if let Some(file_name) = file_name {
            field.file_name(file_name)
        } else {
            field
        })
    }

    fn new(value: Body) -> Part {
        Part {
            meta: PartMetadata::new(),
            value,
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
            value: self.value,
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
        self.value.len()
    }

    fn metadata(&self) -> &PartMetadata {
        &self.meta
    }
}

pub(crate) struct Reader {
    form: Form,
    active_reader: Option<Box<dyn Read + Send>>,
}

impl fmt::Debug for Reader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Reader").field("form", &self.form).finish()
    }
}

impl Reader {
    fn new(form: Form) -> Reader {
        let mut reader = Reader {
            form,
            active_reader: None,
        };
        reader.next_reader();
        reader
    }

    fn next_reader(&mut self) {
        self.active_reader = if !self.form.inner.fields.is_empty() {
            // We need to move out of the vector here because we are consuming the field's reader
            let (name, field) = self.form.inner.fields.remove(0);
            let boundary = Cursor::new(format!("--{}\r\n", self.form.boundary()));
            let header = Cursor::new({
                // Try to use cached headers created by compute_length
                let mut h = if !self.form.inner.computed_headers.is_empty() {
                    self.form.inner.computed_headers.remove(0)
                } else {
                    self.form
                        .inner
                        .percent_encoding
                        .encode_headers(&name, field.metadata())
                };
                h.extend_from_slice(b"\r\n\r\n");
                h
            });
            let reader = boundary
                .chain(header)
                .chain(field.value.into_reader())
                .chain(Cursor::new("\r\n"));
            // According to https://tools.ietf.org/html/rfc2046#section-5.1.1
            // the very last field has a special boundary
            if !self.form.inner.fields.is_empty() {
                Some(Box::new(reader))
            } else {
                Some(Box::new(reader.chain(Cursor::new(format!(
                    "--{}--\r\n",
                    self.form.boundary()
                )))))
            }
        } else {
            None
        }
    }
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut total_bytes_read = 0usize;
        let mut last_read_bytes;
        loop {
            match self.active_reader {
                Some(ref mut reader) => {
                    last_read_bytes = reader.read(&mut buf[total_bytes_read..])?;
                    total_bytes_read += last_read_bytes;
                    if total_bytes_read == buf.len() {
                        return Ok(total_bytes_read);
                    }
                }
                None => return Ok(total_bytes_read),
            };
            if last_read_bytes == 0 && !buf.is_empty() {
                self.next_reader();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_empty() {
        let mut output = Vec::new();
        let mut form = Form::new();
        let length = form.compute_length();
        form.reader().read_to_end(&mut output).unwrap();
        assert_eq!(output, b"");
        assert_eq!(length.unwrap(), 0);
    }

    #[test]
    fn read_to_end() {
        let mut output = Vec::new();
        let mut form = Form::new()
            .part("reader1", Part::reader(std::io::empty()))
            .part("key1", Part::text("value1"))
            .part("key2", Part::text("value2").mime(mime::IMAGE_BMP))
            .part("reader2", Part::reader(std::io::empty()))
            .part("key3", Part::text("value3").file_name("filename"));
        form.inner.boundary = "boundary".to_string();
        let length = form.compute_length();
        let expected = "--boundary\r\n\
             Content-Disposition: form-data; name=\"reader1\"\r\n\r\n\
             \r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"key1\"\r\n\r\n\
             value1\r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"key2\"\r\n\
             Content-Type: image/bmp\r\n\r\n\
             value2\r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"reader2\"\r\n\r\n\
             \r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"key3\"; filename=\"filename\"\r\n\r\n\
             value3\r\n--boundary--\r\n";
        form.reader().read_to_end(&mut output).unwrap();
        // These prints are for debug purposes in case the test fails
        println!(
            "START REAL\n{}\nEND REAL",
            std::str::from_utf8(&output).unwrap()
        );
        println!("START EXPECTED\n{expected}\nEND EXPECTED");
        assert_eq!(std::str::from_utf8(&output).unwrap(), expected);
        assert!(length.is_none());
    }

    #[test]
    fn read_to_end_with_length() {
        let mut output = Vec::new();
        let mut form = Form::new()
            .text("key1", "value1")
            .part("key2", Part::text("value2").mime(mime::IMAGE_BMP))
            .part("key3", Part::text("value3").file_name("filename"));
        form.inner.boundary = "boundary".to_string();
        let length = form.compute_length();
        let expected = "--boundary\r\n\
             Content-Disposition: form-data; name=\"key1\"\r\n\r\n\
             value1\r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"key2\"\r\n\
             Content-Type: image/bmp\r\n\r\n\
             value2\r\n\
             --boundary\r\n\
             Content-Disposition: form-data; name=\"key3\"; filename=\"filename\"\r\n\r\n\
             value3\r\n--boundary--\r\n";
        form.reader().read_to_end(&mut output).unwrap();
        // These prints are for debug purposes in case the test fails
        println!(
            "START REAL\n{}\nEND REAL",
            std::str::from_utf8(&output).unwrap()
        );
        println!("START EXPECTED\n{expected}\nEND EXPECTED");
        assert_eq!(std::str::from_utf8(&output).unwrap(), expected);
        assert_eq!(length.unwrap(), expected.len() as u64);
    }

    #[test]
    fn read_to_end_with_header() {
        let mut output = Vec::new();
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
        form.reader().read_to_end(&mut output).unwrap();
        // These prints are for debug purposes in case the test fails
        println!(
            "START REAL\n{}\nEND REAL",
            std::str::from_utf8(&output).unwrap()
        );
        println!("START EXPECTED\n{expected}\nEND EXPECTED");
        assert_eq!(std::str::from_utf8(&output).unwrap(), expected);
    }
}
