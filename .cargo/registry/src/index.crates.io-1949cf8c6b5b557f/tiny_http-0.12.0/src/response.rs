use crate::common::{HTTPVersion, Header, StatusCode};
use httpdate::HttpDate;
use std::cmp::Ordering;
use std::sync::mpsc::Receiver;

use std::io::Result as IoResult;
use std::io::{self, Cursor, Read, Write};

use std::fs::File;

use std::str::FromStr;
use std::time::SystemTime;

/// Object representing an HTTP response whose purpose is to be given to a `Request`.
///
/// Some headers cannot be changed. Trying to define the value
/// of one of these will have no effect:
///
///  - `Connection`
///  - `Trailer`
///  - `Transfer-Encoding`
///  - `Upgrade`
///
/// Some headers have special behaviors:
///
///  - `Content-Encoding`: If you define this header, the library
///     will assume that the data from the `Read` object has the specified encoding
///     and will just pass-through.
///
///  - `Content-Length`: The length of the data should be set manually
///     using the `Reponse` object's API. Attempting to set the value of this
///     header will be equivalent to modifying the size of the data but the header
///     itself may not be present in the final result.
///
///  - `Content-Type`: You may only set this header to one value at a time. If you
///     try to set it more than once, the existing value will be overwritten. This
///     behavior differs from the default for most headers, which is to allow them to
///     be set multiple times in the same response.
///
pub struct Response<R> {
    reader: R,
    status_code: StatusCode,
    headers: Vec<Header>,
    data_length: Option<usize>,
    chunked_threshold: Option<usize>,
}

/// A `Response` without a template parameter.
pub type ResponseBox = Response<Box<dyn Read + Send>>;

/// Transfer encoding to use when sending the message.
/// Note that only *supported* encoding are listed here.
#[derive(Copy, Clone)]
enum TransferEncoding {
    Identity,
    Chunked,
}

impl FromStr for TransferEncoding {
    type Err = ();

    fn from_str(input: &str) -> Result<TransferEncoding, ()> {
        if input.eq_ignore_ascii_case("identity") {
            Ok(TransferEncoding::Identity)
        } else if input.eq_ignore_ascii_case("chunked") {
            Ok(TransferEncoding::Chunked)
        } else {
            Err(())
        }
    }
}

/// Builds a Date: header with the current date.
fn build_date_header() -> Header {
    let d = HttpDate::from(SystemTime::now());
    Header::from_bytes(&b"Date"[..], &d.to_string().into_bytes()[..]).unwrap()
}

fn write_message_header<W>(
    mut writer: W,
    http_version: &HTTPVersion,
    status_code: &StatusCode,
    headers: &[Header],
) -> IoResult<()>
where
    W: Write,
{
    // writing status line
    write!(
        &mut writer,
        "HTTP/{}.{} {} {}\r\n",
        http_version.0,
        http_version.1,
        status_code.0,
        status_code.default_reason_phrase()
    )?;

    // writing headers
    for header in headers.iter() {
        writer.write_all(header.field.as_str().as_ref())?;
        write!(&mut writer, ": ")?;
        writer.write_all(header.value.as_str().as_ref())?;
        write!(&mut writer, "\r\n")?;
    }

    // separator between header and data
    write!(&mut writer, "\r\n")?;

    Ok(())
}

fn choose_transfer_encoding(
    status_code: StatusCode,
    request_headers: &[Header],
    http_version: &HTTPVersion,
    entity_length: &Option<usize>,
    has_additional_headers: bool,
    chunked_threshold: usize,
) -> TransferEncoding {
    use crate::util;

    // HTTP 1.0 doesn't support other encoding
    if *http_version <= (1, 0) {
        return TransferEncoding::Identity;
    }

    // Per section 3.3.1 of RFC7230:
    // A server MUST NOT send a Transfer-Encoding header field in any response with a status code
    // of 1xx (Informational) or 204 (No Content).
    if status_code.0 < 200 || status_code.0 == 204 {
        return TransferEncoding::Identity;
    }

    // parsing the request's TE header
    let user_request = request_headers
        .iter()
        // finding TE
        .find(|h| h.field.equiv("TE"))
        // getting its value
        .map(|h| h.value.clone())
        // getting the corresponding TransferEncoding
        .and_then(|value| {
            // getting list of requested elements
            let mut parse = util::parse_header_value(value.as_str()); // TODO: remove conversion

            // sorting elements by most priority
            parse.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

            // trying to parse each requested encoding
            for value in parse.iter() {
                // q=0 are ignored
                if value.1 <= 0.0 {
                    continue;
                }

                if let Ok(te) = TransferEncoding::from_str(value.0) {
                    return Some(te);
                }
            }

            // encoding not found
            None
        });

    if let Some(user_request) = user_request {
        return user_request;
    }

    // if we have additional headers, using chunked
    if has_additional_headers {
        return TransferEncoding::Chunked;
    }

    // if we don't have a Content-Length, or if the Content-Length is too big, using chunks writer
    if entity_length
        .as_ref()
        .map_or(true, |val| *val >= chunked_threshold)
    {
        return TransferEncoding::Chunked;
    }

    // Identity by default
    TransferEncoding::Identity
}

impl<R> Response<R>
where
    R: Read,
{
    /// Creates a new Response object.
    ///
    /// The `additional_headers` argument is a receiver that
    ///  may provide headers even after the response has been sent.
    ///
    /// All the other arguments are straight-forward.
    pub fn new(
        status_code: StatusCode,
        headers: Vec<Header>,
        data: R,
        data_length: Option<usize>,
        additional_headers: Option<Receiver<Header>>,
    ) -> Response<R> {
        let mut response = Response {
            reader: data,
            status_code,
            headers: Vec::with_capacity(16),
            data_length,
            chunked_threshold: None,
        };

        for h in headers {
            response.add_header(h)
        }

        // dummy implementation
        if let Some(additional_headers) = additional_headers {
            for h in additional_headers.iter() {
                response.add_header(h)
            }
        }

        response
    }

    /// Set a threshold for `Content-Length` where we chose chunked
    /// transfer. Notice that chunked transfer might happen regardless of
    /// this threshold, for instance when the request headers indicate
    /// it is wanted or when there is no `Content-Length`.
    pub fn with_chunked_threshold(mut self, length: usize) -> Response<R> {
        self.chunked_threshold = Some(length);
        self
    }

    /// Convert the response into the underlying `Read` type.
    ///
    /// This is mainly useful for testing as it must consume the `Response`.
    pub fn into_reader(self) -> R {
        self.reader
    }

    /// The current `Content-Length` threshold for switching over to
    /// chunked transfer. The default is 32768 bytes. Notice that
    /// chunked transfer is mutually exclusive with sending a
    /// `Content-Length` header as per the HTTP spec.
    pub fn chunked_threshold(&self) -> usize {
        self.chunked_threshold.unwrap_or(32768)
    }

    /// Adds a header to the list.
    /// Does all the checks.
    pub fn add_header<H>(&mut self, header: H)
    where
        H: Into<Header>,
    {
        let header = header.into();

        // ignoring forbidden headers
        if header.field.equiv("Connection")
            || header.field.equiv("Trailer")
            || header.field.equiv("Transfer-Encoding")
            || header.field.equiv("Upgrade")
        {
            return;
        }

        // if the header is Content-Length, setting the data length
        if header.field.equiv("Content-Length") {
            if let Ok(val) = usize::from_str(header.value.as_str()) {
                self.data_length = Some(val)
            }

            return;
        // if the header is Content-Type and it's already set, overwrite it
        } else if header.field.equiv("Content-Type") {
            if let Some(content_type_header) = self
                .headers
                .iter_mut()
                .find(|h| h.field.equiv("Content-Type"))
            {
                content_type_header.value = header.value;
                return;
            }
        }

        self.headers.push(header);
    }

    /// Returns the same request, but with an additional header.
    ///
    /// Some headers cannot be modified and some other have a
    ///  special behavior. See the documentation above.
    #[inline]
    pub fn with_header<H>(mut self, header: H) -> Response<R>
    where
        H: Into<Header>,
    {
        self.add_header(header.into());
        self
    }

    /// Returns the same request, but with a different status code.
    #[inline]
    pub fn with_status_code<S>(mut self, code: S) -> Response<R>
    where
        S: Into<StatusCode>,
    {
        self.status_code = code.into();
        self
    }

    /// Returns the same request, but with different data.
    pub fn with_data<S>(self, reader: S, data_length: Option<usize>) -> Response<S>
    where
        S: Read,
    {
        Response {
            reader,
            headers: self.headers,
            status_code: self.status_code,
            data_length,
            chunked_threshold: self.chunked_threshold,
        }
    }

    /// Prints the HTTP response to a writer.
    ///
    /// This function is the one used to send the response to the client's socket.
    /// Therefore you shouldn't expect anything pretty-printed or even readable.
    ///
    /// The HTTP version and headers passed as arguments are used to
    ///  decide which features (most notably, encoding) to use.
    ///
    /// Note: does not flush the writer.
    pub fn raw_print<W: Write>(
        mut self,
        mut writer: W,
        http_version: HTTPVersion,
        request_headers: &[Header],
        do_not_send_body: bool,
        upgrade: Option<&str>,
    ) -> IoResult<()> {
        let mut transfer_encoding = Some(choose_transfer_encoding(
            self.status_code,
            request_headers,
            &http_version,
            &self.data_length,
            false, /* TODO */
            self.chunked_threshold(),
        ));

        // add `Date` if not in the headers
        if !self.headers.iter().any(|h| h.field.equiv("Date")) {
            self.headers.insert(0, build_date_header());
        }

        // add `Server` if not in the headers
        if !self.headers.iter().any(|h| h.field.equiv("Server")) {
            self.headers.insert(
                0,
                Header::from_bytes(&b"Server"[..], &b"tiny-http (Rust)"[..]).unwrap(),
            );
        }

        // handling upgrade
        if let Some(upgrade) = upgrade {
            self.headers.insert(
                0,
                Header::from_bytes(&b"Upgrade"[..], upgrade.as_bytes()).unwrap(),
            );
            self.headers.insert(
                0,
                Header::from_bytes(&b"Connection"[..], &b"upgrade"[..]).unwrap(),
            );
            transfer_encoding = None;
        }

        // if the transfer encoding is identity, the content length must be known ; therefore if
        // we don't know it, we buffer the entire response first here
        // while this is an expensive operation, it is only ever needed for clients using HTTP 1.0
        let (mut reader, data_length): (Box<dyn Read>, _) =
            match (self.data_length, transfer_encoding) {
                (Some(l), _) => (Box::new(self.reader), Some(l)),
                (None, Some(TransferEncoding::Identity)) => {
                    let mut buf = Vec::new();
                    self.reader.read_to_end(&mut buf)?;
                    let l = buf.len();
                    (Box::new(Cursor::new(buf)), Some(l))
                }
                _ => (Box::new(self.reader), None),
            };

        // checking whether to ignore the body of the response
        let do_not_send_body = do_not_send_body
            || match self.status_code.0 {
                // status code 1xx, 204 and 304 MUST not include a body
                100..=199 | 204 | 304 => true,
                _ => false,
            };

        // preparing headers for transfer
        match transfer_encoding {
            Some(TransferEncoding::Chunked) => self
                .headers
                .push(Header::from_bytes(&b"Transfer-Encoding"[..], &b"chunked"[..]).unwrap()),

            Some(TransferEncoding::Identity) => {
                assert!(data_length.is_some());
                let data_length = data_length.unwrap();

                self.headers.push(
                    Header::from_bytes(
                        &b"Content-Length"[..],
                        format!("{}", data_length).as_bytes(),
                    )
                    .unwrap(),
                )
            }

            _ => (),
        };

        // sending headers
        write_message_header(
            writer.by_ref(),
            &http_version,
            &self.status_code,
            &self.headers,
        )?;

        // sending the body
        if !do_not_send_body {
            match transfer_encoding {
                Some(TransferEncoding::Chunked) => {
                    use chunked_transfer::Encoder;

                    let mut writer = Encoder::new(writer);
                    io::copy(&mut reader, &mut writer)?;
                }

                Some(TransferEncoding::Identity) => {
                    assert!(data_length.is_some());
                    let data_length = data_length.unwrap();

                    if data_length >= 1 {
                        io::copy(&mut reader, &mut writer)?;
                    }
                }

                _ => (),
            }
        }

        Ok(())
    }

    /// Retrieves the current value of the `Response` status code
    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }

    /// Retrieves the current value of the `Response` data length
    pub fn data_length(&self) -> Option<usize> {
        self.data_length
    }

    /// Retrieves the current list of `Response` headers
    pub fn headers(&self) -> &[Header] {
        &self.headers
    }
}

impl<R> Response<R>
where
    R: Read + Send + 'static,
{
    /// Turns this response into a `Response<Box<Read + Send>>`.
    pub fn boxed(self) -> ResponseBox {
        Response {
            reader: Box::new(self.reader) as Box<dyn Read + Send>,
            status_code: self.status_code,
            headers: self.headers,
            data_length: self.data_length,
            chunked_threshold: self.chunked_threshold,
        }
    }
}

impl Response<File> {
    /// Builds a new `Response` from a `File`.
    ///
    /// The `Content-Type` will **not** be automatically detected,
    ///  you must set it yourself.
    pub fn from_file(file: File) -> Response<File> {
        let file_size = file.metadata().ok().map(|v| v.len() as usize);

        Response::new(
            StatusCode(200),
            Vec::with_capacity(0),
            file,
            file_size,
            None,
        )
    }
}

impl Response<Cursor<Vec<u8>>> {
    pub fn from_data<D>(data: D) -> Response<Cursor<Vec<u8>>>
    where
        D: Into<Vec<u8>>,
    {
        let data = data.into();
        let data_len = data.len();

        Response::new(
            StatusCode(200),
            Vec::with_capacity(0),
            Cursor::new(data),
            Some(data_len),
            None,
        )
    }

    pub fn from_string<S>(data: S) -> Response<Cursor<Vec<u8>>>
    where
        S: Into<String>,
    {
        let data = data.into();
        let data_len = data.len();

        Response::new(
            StatusCode(200),
            vec![
                Header::from_bytes(&b"Content-Type"[..], &b"text/plain; charset=UTF-8"[..])
                    .unwrap(),
            ],
            Cursor::new(data.into_bytes()),
            Some(data_len),
            None,
        )
    }
}

impl Response<io::Empty> {
    /// Builds an empty `Response` with the given status code.
    pub fn empty<S>(status_code: S) -> Response<io::Empty>
    where
        S: Into<StatusCode>,
    {
        Response::new(
            status_code.into(),
            Vec::with_capacity(0),
            io::empty(),
            Some(0),
            None,
        )
    }

    /// DEPRECATED. Use `empty` instead.
    pub fn new_empty(status_code: StatusCode) -> Response<io::Empty> {
        Response::empty(status_code)
    }
}

impl Clone for Response<io::Empty> {
    fn clone(&self) -> Response<io::Empty> {
        Response {
            reader: io::empty(),
            status_code: self.status_code,
            headers: self.headers.clone(),
            data_length: self.data_length,
            chunked_threshold: self.chunked_threshold,
        }
    }
}
