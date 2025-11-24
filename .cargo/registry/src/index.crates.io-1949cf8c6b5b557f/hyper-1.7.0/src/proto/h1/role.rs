use std::mem::MaybeUninit;

#[cfg(feature = "client")]
use std::fmt::{self, Write as _};

use bytes::Bytes;
use bytes::BytesMut;
#[cfg(feature = "client")]
use http::header::Entry;
#[cfg(feature = "server")]
use http::header::ValueIter;
use http::header::{self, HeaderMap, HeaderName, HeaderValue};
use http::{Method, StatusCode, Version};
use smallvec::{smallvec, smallvec_inline, SmallVec};

use crate::body::DecodedLength;
#[cfg(feature = "server")]
use crate::common::date;
use crate::error::Parse;
use crate::ext::HeaderCaseMap;
#[cfg(feature = "ffi")]
use crate::ext::OriginalHeaderOrder;
use crate::headers;
use crate::proto::h1::{
    Encode, Encoder, Http1Transaction, ParseContext, ParseResult, ParsedMessage,
};
#[cfg(feature = "client")]
use crate::proto::RequestHead;
use crate::proto::{BodyLength, MessageHead, RequestLine};

pub(crate) const DEFAULT_MAX_HEADERS: usize = 100;
const AVERAGE_HEADER_SIZE: usize = 30; // totally scientific
#[cfg(feature = "server")]
const MAX_URI_LEN: usize = (u16::MAX - 1) as usize;

macro_rules! header_name {
    ($bytes:expr) => {{
        {
            match HeaderName::from_bytes($bytes) {
                Ok(name) => name,
                Err(e) => maybe_panic!(e),
            }
        }
    }};
}

macro_rules! header_value {
    ($bytes:expr) => {{
        {
            unsafe { HeaderValue::from_maybe_shared_unchecked($bytes) }
        }
    }};
}

macro_rules! maybe_panic {
    ($($arg:tt)*) => ({
        let _err = ($($arg)*);
        if cfg!(debug_assertions) {
            panic!("{:?}", _err);
        } else {
            error!("Internal Hyper error, please report {:?}", _err);
            return Err(Parse::Internal)
        }
    })
}

pub(super) fn parse_headers<T>(
    bytes: &mut BytesMut,
    prev_len: Option<usize>,
    ctx: ParseContext<'_>,
) -> ParseResult<T::Incoming>
where
    T: Http1Transaction,
{
    // If the buffer is empty, don't bother entering the span, it's just noise.
    if bytes.is_empty() {
        return Ok(None);
    }

    let _entered = trace_span!("parse_headers");

    if let Some(prev_len) = prev_len {
        if !is_complete_fast(bytes, prev_len) {
            return Ok(None);
        }
    }

    T::parse(bytes, ctx)
}

/// A fast scan for the end of a message.
/// Used when there was a partial read, to skip full parsing on a
/// a slow connection.
fn is_complete_fast(bytes: &[u8], prev_len: usize) -> bool {
    let start = if prev_len < 3 { 0 } else { prev_len - 3 };
    let bytes = &bytes[start..];

    for (i, b) in bytes.iter().copied().enumerate() {
        if b == b'\r' {
            if bytes[i + 1..].chunks(3).next() == Some(&b"\n\r\n"[..]) {
                return true;
            }
        } else if b == b'\n' && bytes.get(i + 1) == Some(&b'\n') {
            return true;
        }
    }

    false
}

pub(super) fn encode_headers<T>(
    enc: Encode<'_, T::Outgoing>,
    dst: &mut Vec<u8>,
) -> crate::Result<Encoder>
where
    T: Http1Transaction,
{
    let _entered = trace_span!("encode_headers");
    T::encode(enc, dst)
}

// There are 2 main roles, Client and Server.

#[cfg(feature = "client")]
pub(crate) enum Client {}

#[cfg(feature = "server")]
pub(crate) enum Server {}

#[cfg(feature = "server")]
impl Http1Transaction for Server {
    type Incoming = RequestLine;
    type Outgoing = StatusCode;
    #[cfg(feature = "tracing")]
    const LOG: &'static str = "{role=server}";

    fn parse(buf: &mut BytesMut, ctx: ParseContext<'_>) -> ParseResult<RequestLine> {
        debug_assert!(!buf.is_empty(), "parse called with empty buf");

        let mut keep_alive;
        let is_http_11;
        let subject;
        let version;
        let len;
        let headers_len;
        let method;
        let path_range;

        // Both headers_indices and headers are using uninitialized memory,
        // but we *never* read any of it until after httparse has assigned
        // values into it. By not zeroing out the stack memory, this saves
        // a good ~5% on pipeline benchmarks.
        let mut headers_indices: SmallVec<[MaybeUninit<HeaderIndices>; DEFAULT_MAX_HEADERS]> =
            match ctx.h1_max_headers {
                Some(cap) => smallvec![MaybeUninit::uninit(); cap],
                None => smallvec_inline![MaybeUninit::uninit(); DEFAULT_MAX_HEADERS],
            };
        {
            let mut headers: SmallVec<[MaybeUninit<httparse::Header<'_>>; DEFAULT_MAX_HEADERS]> =
                match ctx.h1_max_headers {
                    Some(cap) => smallvec![MaybeUninit::uninit(); cap],
                    None => smallvec_inline![MaybeUninit::uninit(); DEFAULT_MAX_HEADERS],
                };
            trace!(bytes = buf.len(), "Request.parse");
            let mut req = httparse::Request::new(&mut []);
            let bytes = buf.as_ref();
            match req.parse_with_uninit_headers(bytes, &mut headers) {
                Ok(httparse::Status::Complete(parsed_len)) => {
                    trace!("Request.parse Complete({})", parsed_len);
                    len = parsed_len;
                    let uri = req.path.unwrap();
                    if uri.len() > MAX_URI_LEN {
                        return Err(Parse::UriTooLong);
                    }
                    method = Method::from_bytes(req.method.unwrap().as_bytes())?;
                    path_range = Server::record_path_range(bytes, uri);
                    version = if req.version.unwrap() == 1 {
                        keep_alive = true;
                        is_http_11 = true;
                        Version::HTTP_11
                    } else {
                        keep_alive = false;
                        is_http_11 = false;
                        Version::HTTP_10
                    };

                    record_header_indices(bytes, req.headers, &mut headers_indices)?;
                    headers_len = req.headers.len();
                }
                Ok(httparse::Status::Partial) => return Ok(None),
                Err(err) => {
                    return Err(match err {
                        // if invalid Token, try to determine if for method or path
                        httparse::Error::Token => {
                            if req.method.is_none() {
                                Parse::Method
                            } else {
                                debug_assert!(req.path.is_none());
                                Parse::Uri
                            }
                        }
                        other => other.into(),
                    });
                }
            }
        };

        let slice = buf.split_to(len).freeze();
        let uri = {
            let uri_bytes = slice.slice_ref(&slice[path_range]);
            // TODO(lucab): switch to `Uri::from_shared()` once public.
            http::Uri::from_maybe_shared(uri_bytes)?
        };
        subject = RequestLine(method, uri);

        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. (irrelevant to Request)
        // 2. (irrelevant to Request)
        // 3. Transfer-Encoding: chunked has a chunked body.
        // 4. If multiple differing Content-Length headers or invalid, close connection.
        // 5. Content-Length header has a sized body.
        // 6. Length 0.
        // 7. (irrelevant to Request)

        let mut decoder = DecodedLength::ZERO;
        let mut expect_continue = false;
        let mut con_len = None;
        let mut is_te = false;
        let mut is_te_chunked = false;
        let mut wants_upgrade = subject.0 == Method::CONNECT;

        let mut header_case_map = if ctx.preserve_header_case {
            Some(HeaderCaseMap::default())
        } else {
            None
        };

        #[cfg(feature = "ffi")]
        let mut header_order = if ctx.preserve_header_order {
            Some(OriginalHeaderOrder::default())
        } else {
            None
        };

        let mut headers = ctx.cached_headers.take().unwrap_or_default();

        headers.reserve(headers_len);

        for header in &headers_indices[..headers_len] {
            // SAFETY: array is valid up to `headers_len`
            let header = unsafe { header.assume_init_ref() };
            let name = header_name!(&slice[header.name.0..header.name.1]);
            let value = header_value!(slice.slice(header.value.0..header.value.1));

            match name {
                header::TRANSFER_ENCODING => {
                    // https://tools.ietf.org/html/rfc7230#section-3.3.3
                    // If Transfer-Encoding header is present, and 'chunked' is
                    // not the final encoding, and this is a Request, then it is
                    // malformed. A server should respond with 400 Bad Request.
                    if !is_http_11 {
                        debug!("HTTP/1.0 cannot have Transfer-Encoding header");
                        return Err(Parse::transfer_encoding_unexpected());
                    }
                    is_te = true;
                    if headers::is_chunked_(&value) {
                        is_te_chunked = true;
                        decoder = DecodedLength::CHUNKED;
                    } else {
                        is_te_chunked = false;
                    }
                }
                header::CONTENT_LENGTH => {
                    if is_te {
                        continue;
                    }
                    let len = headers::content_length_parse(&value)
                        .ok_or_else(Parse::content_length_invalid)?;
                    if let Some(prev) = con_len {
                        if prev != len {
                            debug!(
                                "multiple Content-Length headers with different values: [{}, {}]",
                                prev, len,
                            );
                            return Err(Parse::content_length_invalid());
                        }
                        // we don't need to append this secondary length
                        continue;
                    }
                    decoder = DecodedLength::checked_new(len)?;
                    con_len = Some(len);
                }
                header::CONNECTION => {
                    // keep_alive was previously set to default for Version
                    if keep_alive {
                        // HTTP/1.1
                        keep_alive = !headers::connection_close(&value);
                    } else {
                        // HTTP/1.0
                        keep_alive = headers::connection_keep_alive(&value);
                    }
                }
                header::EXPECT => {
                    // According to https://datatracker.ietf.org/doc/html/rfc2616#section-14.20
                    // Comparison of expectation values is case-insensitive for unquoted tokens
                    // (including the 100-continue token)
                    expect_continue = value.as_bytes().eq_ignore_ascii_case(b"100-continue");
                }
                header::UPGRADE => {
                    // Upgrades are only allowed with HTTP/1.1
                    wants_upgrade = is_http_11;
                }

                _ => (),
            }

            if let Some(ref mut header_case_map) = header_case_map {
                header_case_map.append(&name, slice.slice(header.name.0..header.name.1));
            }

            #[cfg(feature = "ffi")]
            if let Some(ref mut header_order) = header_order {
                header_order.append(&name);
            }

            headers.append(name, value);
        }

        if is_te && !is_te_chunked {
            debug!("request with transfer-encoding header, but not chunked, bad request");
            return Err(Parse::transfer_encoding_invalid());
        }

        let mut extensions = http::Extensions::default();

        if let Some(header_case_map) = header_case_map {
            extensions.insert(header_case_map);
        }

        #[cfg(feature = "ffi")]
        if let Some(header_order) = header_order {
            extensions.insert(header_order);
        }

        *ctx.req_method = Some(subject.0.clone());

        Ok(Some(ParsedMessage {
            head: MessageHead {
                version,
                subject,
                headers,
                extensions,
            },
            decode: decoder,
            expect_continue,
            keep_alive,
            wants_upgrade,
        }))
    }

    fn encode(mut msg: Encode<'_, Self::Outgoing>, dst: &mut Vec<u8>) -> crate::Result<Encoder> {
        trace!(
            "Server::encode status={:?}, body={:?}, req_method={:?}",
            msg.head.subject,
            msg.body,
            msg.req_method
        );

        let mut wrote_len = false;

        // hyper currently doesn't support returning 1xx status codes as a Response
        // This is because Service only allows returning a single Response, and
        // so if you try to reply with a e.g. 100 Continue, you have no way of
        // replying with the latter status code response.
        let (ret, is_last) = if msg.head.subject == StatusCode::SWITCHING_PROTOCOLS {
            (Ok(()), true)
        } else if msg.req_method == &Some(Method::CONNECT) && msg.head.subject.is_success() {
            // Sending content-length or transfer-encoding header on 2xx response
            // to CONNECT is forbidden in RFC 7231.
            wrote_len = true;
            (Ok(()), true)
        } else if msg.head.subject.is_informational() {
            warn!("response with 1xx status code not supported");
            *msg.head = MessageHead::default();
            msg.head.subject = StatusCode::INTERNAL_SERVER_ERROR;
            msg.body = None;
            (Err(crate::Error::new_user_unsupported_status_code()), true)
        } else {
            (Ok(()), !msg.keep_alive)
        };

        // In some error cases, we don't know about the invalid message until already
        // pushing some bytes onto the `dst`. In those cases, we don't want to send
        // the half-pushed message, so rewind to before.
        let orig_len = dst.len();

        let init_cap = 30 + msg.head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);

        let custom_reason_phrase = msg.head.extensions.get::<crate::ext::ReasonPhrase>();

        if msg.head.version == Version::HTTP_11
            && msg.head.subject == StatusCode::OK
            && custom_reason_phrase.is_none()
        {
            extend(dst, b"HTTP/1.1 200 OK\r\n");
        } else {
            match msg.head.version {
                Version::HTTP_10 => extend(dst, b"HTTP/1.0 "),
                Version::HTTP_11 => extend(dst, b"HTTP/1.1 "),
                Version::HTTP_2 => {
                    debug!("response with HTTP2 version coerced to HTTP/1.1");
                    extend(dst, b"HTTP/1.1 ");
                }
                other => panic!("unexpected response version: {:?}", other),
            }

            extend(dst, msg.head.subject.as_str().as_bytes());
            extend(dst, b" ");

            if let Some(reason) = custom_reason_phrase {
                extend(dst, reason.as_bytes());
            } else {
                // a reason MUST be written, as many parsers will expect it.
                extend(
                    dst,
                    msg.head
                        .subject
                        .canonical_reason()
                        .unwrap_or("<none>")
                        .as_bytes(),
                );
            }

            extend(dst, b"\r\n");
        }

        let orig_headers;
        let extensions = std::mem::take(&mut msg.head.extensions);
        let orig_headers = match extensions.get::<HeaderCaseMap>() {
            None if msg.title_case_headers => {
                orig_headers = HeaderCaseMap::default();
                Some(&orig_headers)
            }
            orig_headers => orig_headers,
        };
        let encoder = if let Some(orig_headers) = orig_headers {
            Self::encode_headers_with_original_case(
                msg,
                dst,
                is_last,
                orig_len,
                wrote_len,
                orig_headers,
            )?
        } else {
            Self::encode_headers_with_lower_case(msg, dst, is_last, orig_len, wrote_len)?
        };

        ret.map(|()| encoder)
    }

    fn on_error(err: &crate::Error) -> Option<MessageHead<Self::Outgoing>> {
        use crate::error::Kind;
        let status = match *err.kind() {
            Kind::Parse(Parse::Method)
            | Kind::Parse(Parse::Header(_))
            | Kind::Parse(Parse::Uri)
            | Kind::Parse(Parse::Version) => StatusCode::BAD_REQUEST,
            Kind::Parse(Parse::TooLarge) => StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            Kind::Parse(Parse::UriTooLong) => StatusCode::URI_TOO_LONG,
            _ => return None,
        };

        debug!("sending automatic response ({}) for parse error", status);
        let msg = MessageHead {
            subject: status,
            ..Default::default()
        };
        Some(msg)
    }

    fn is_server() -> bool {
        true
    }

    fn update_date() {
        date::update();
    }
}

#[cfg(feature = "server")]
impl Server {
    fn can_have_body(method: &Option<Method>, status: StatusCode) -> bool {
        Server::can_chunked(method, status)
    }

    fn can_chunked(method: &Option<Method>, status: StatusCode) -> bool {
        if method == &Some(Method::HEAD)
            || method == &Some(Method::CONNECT) && status.is_success()
            || status.is_informational()
        {
            false
        } else {
            !matches!(status, StatusCode::NO_CONTENT | StatusCode::NOT_MODIFIED)
        }
    }

    fn can_have_content_length(method: &Option<Method>, status: StatusCode) -> bool {
        if status.is_informational() || method == &Some(Method::CONNECT) && status.is_success() {
            false
        } else {
            !matches!(status, StatusCode::NO_CONTENT | StatusCode::NOT_MODIFIED)
        }
    }

    fn can_have_implicit_zero_content_length(method: &Option<Method>, status: StatusCode) -> bool {
        Server::can_have_content_length(method, status) && method != &Some(Method::HEAD)
    }

    fn encode_headers_with_lower_case(
        msg: Encode<'_, StatusCode>,
        dst: &mut Vec<u8>,
        is_last: bool,
        orig_len: usize,
        wrote_len: bool,
    ) -> crate::Result<Encoder> {
        struct LowercaseWriter;

        impl HeaderNameWriter for LowercaseWriter {
            #[inline]
            fn write_full_header_line(
                &mut self,
                dst: &mut Vec<u8>,
                line: &str,
                _: (HeaderName, &str),
            ) {
                extend(dst, line.as_bytes())
            }

            #[inline]
            fn write_header_name_with_colon(
                &mut self,
                dst: &mut Vec<u8>,
                name_with_colon: &str,
                _: HeaderName,
            ) {
                extend(dst, name_with_colon.as_bytes())
            }

            #[inline]
            fn write_header_name(&mut self, dst: &mut Vec<u8>, name: &HeaderName) {
                extend(dst, name.as_str().as_bytes())
            }
        }

        Self::encode_headers(msg, dst, is_last, orig_len, wrote_len, LowercaseWriter)
    }

    #[cold]
    #[inline(never)]
    fn encode_headers_with_original_case(
        msg: Encode<'_, StatusCode>,
        dst: &mut Vec<u8>,
        is_last: bool,
        orig_len: usize,
        wrote_len: bool,
        orig_headers: &HeaderCaseMap,
    ) -> crate::Result<Encoder> {
        struct OrigCaseWriter<'map> {
            map: &'map HeaderCaseMap,
            current: Option<(HeaderName, ValueIter<'map, Bytes>)>,
            title_case_headers: bool,
        }

        impl HeaderNameWriter for OrigCaseWriter<'_> {
            #[inline]
            fn write_full_header_line(
                &mut self,
                dst: &mut Vec<u8>,
                _: &str,
                (name, rest): (HeaderName, &str),
            ) {
                self.write_header_name(dst, &name);
                extend(dst, rest.as_bytes());
            }

            #[inline]
            fn write_header_name_with_colon(
                &mut self,
                dst: &mut Vec<u8>,
                _: &str,
                name: HeaderName,
            ) {
                self.write_header_name(dst, &name);
                extend(dst, b": ");
            }

            #[inline]
            fn write_header_name(&mut self, dst: &mut Vec<u8>, name: &HeaderName) {
                let Self {
                    map,
                    ref mut current,
                    title_case_headers,
                } = *self;
                if current.as_ref().map_or(true, |(last, _)| last != name) {
                    *current = None;
                }
                let (_, values) =
                    current.get_or_insert_with(|| (name.clone(), map.get_all_internal(name)));

                if let Some(orig_name) = values.next() {
                    extend(dst, orig_name);
                } else if title_case_headers {
                    title_case(dst, name.as_str().as_bytes());
                } else {
                    extend(dst, name.as_str().as_bytes());
                }
            }
        }

        let header_name_writer = OrigCaseWriter {
            map: orig_headers,
            current: None,
            title_case_headers: msg.title_case_headers,
        };

        Self::encode_headers(msg, dst, is_last, orig_len, wrote_len, header_name_writer)
    }

    #[inline]
    fn encode_headers<W>(
        msg: Encode<'_, StatusCode>,
        dst: &mut Vec<u8>,
        mut is_last: bool,
        orig_len: usize,
        mut wrote_len: bool,
        mut header_name_writer: W,
    ) -> crate::Result<Encoder>
    where
        W: HeaderNameWriter,
    {
        // In some error cases, we don't know about the invalid message until already
        // pushing some bytes onto the `dst`. In those cases, we don't want to send
        // the half-pushed message, so rewind to before.
        let rewind = |dst: &mut Vec<u8>| {
            dst.truncate(orig_len);
        };

        let mut encoder = Encoder::length(0);
        let mut allowed_trailer_fields: Option<Vec<HeaderValue>> = None;
        let mut wrote_date = false;
        let mut cur_name = None;
        let mut is_name_written = false;
        let mut must_write_chunked = false;
        let mut prev_con_len = None;

        macro_rules! handle_is_name_written {
            () => {{
                if is_name_written {
                    // we need to clean up and write the newline
                    debug_assert_ne!(
                        &dst[dst.len() - 2..],
                        b"\r\n",
                        "previous header wrote newline but set is_name_written"
                    );

                    if must_write_chunked {
                        extend(dst, b", chunked\r\n");
                    } else {
                        extend(dst, b"\r\n");
                    }
                }
            }};
        }

        'headers: for (opt_name, value) in msg.head.headers.drain() {
            if let Some(n) = opt_name {
                cur_name = Some(n);
                handle_is_name_written!();
                is_name_written = false;
            }
            let name = cur_name.as_ref().expect("current header name");
            match *name {
                header::CONTENT_LENGTH => {
                    if wrote_len && !is_name_written {
                        warn!("unexpected content-length found, canceling");
                        rewind(dst);
                        return Err(crate::Error::new_user_header());
                    }
                    match msg.body {
                        Some(BodyLength::Known(known_len)) => {
                            // The Body claims to know a length, and
                            // the headers are already set. For performance
                            // reasons, we are just going to trust that
                            // the values match.
                            //
                            // In debug builds, we'll assert they are the
                            // same to help developers find bugs.
                            #[cfg(debug_assertions)]
                            {
                                if let Some(len) = headers::content_length_parse(&value) {
                                    if msg.req_method != &Some(Method::HEAD) || known_len != 0 {
                                        assert!(
                                        len == known_len,
                                        "payload claims content-length of {}, custom content-length header claims {}",
                                        known_len,
                                        len,
                                    );
                                    }
                                }
                            }

                            if !is_name_written {
                                encoder = Encoder::length(known_len);
                                header_name_writer.write_header_name_with_colon(
                                    dst,
                                    "content-length: ",
                                    header::CONTENT_LENGTH,
                                );
                                extend(dst, value.as_bytes());
                                wrote_len = true;
                                is_name_written = true;
                            }
                            continue 'headers;
                        }
                        Some(BodyLength::Unknown) => {
                            // The Body impl didn't know how long the
                            // body is, but a length header was included.
                            // We have to parse the value to return our
                            // Encoder...

                            if let Some(len) = headers::content_length_parse(&value) {
                                if let Some(prev) = prev_con_len {
                                    if prev != len {
                                        warn!(
                                            "multiple Content-Length values found: [{}, {}]",
                                            prev, len
                                        );
                                        rewind(dst);
                                        return Err(crate::Error::new_user_header());
                                    }
                                    debug_assert!(is_name_written);
                                    continue 'headers;
                                } else {
                                    // we haven't written content-length yet!
                                    encoder = Encoder::length(len);
                                    header_name_writer.write_header_name_with_colon(
                                        dst,
                                        "content-length: ",
                                        header::CONTENT_LENGTH,
                                    );
                                    extend(dst, value.as_bytes());
                                    wrote_len = true;
                                    is_name_written = true;
                                    prev_con_len = Some(len);
                                    continue 'headers;
                                }
                            } else {
                                warn!("illegal Content-Length value: {:?}", value);
                                rewind(dst);
                                return Err(crate::Error::new_user_header());
                            }
                        }
                        None => {
                            // We have no body to actually send,
                            // but the headers claim a content-length.
                            // There's only 2 ways this makes sense:
                            //
                            // - The header says the length is `0`.
                            // - This is a response to a `HEAD` request.
                            if msg.req_method == &Some(Method::HEAD) {
                                debug_assert_eq!(encoder, Encoder::length(0));
                            } else {
                                if value.as_bytes() != b"0" {
                                    warn!(
                                        "content-length value found, but empty body provided: {:?}",
                                        value
                                    );
                                }
                                continue 'headers;
                            }
                        }
                    }
                    wrote_len = true;
                }
                header::TRANSFER_ENCODING => {
                    if wrote_len && !is_name_written {
                        warn!("unexpected transfer-encoding found, canceling");
                        rewind(dst);
                        return Err(crate::Error::new_user_header());
                    }
                    // check that we actually can send a chunked body...
                    if msg.head.version == Version::HTTP_10
                        || !Server::can_chunked(msg.req_method, msg.head.subject)
                    {
                        continue;
                    }
                    wrote_len = true;
                    // Must check each value, because `chunked` needs to be the
                    // last encoding, or else we add it.
                    must_write_chunked = !headers::is_chunked_(&value);

                    if !is_name_written {
                        encoder = Encoder::chunked();
                        is_name_written = true;
                        header_name_writer.write_header_name_with_colon(
                            dst,
                            "transfer-encoding: ",
                            header::TRANSFER_ENCODING,
                        );
                        extend(dst, value.as_bytes());
                    } else {
                        extend(dst, b", ");
                        extend(dst, value.as_bytes());
                    }
                    continue 'headers;
                }
                header::CONNECTION => {
                    if !is_last && headers::connection_close(&value) {
                        is_last = true;
                    }
                    if !is_name_written {
                        is_name_written = true;
                        header_name_writer.write_header_name_with_colon(
                            dst,
                            "connection: ",
                            header::CONNECTION,
                        );
                        extend(dst, value.as_bytes());
                    } else {
                        extend(dst, b", ");
                        extend(dst, value.as_bytes());
                    }
                    continue 'headers;
                }
                header::DATE => {
                    wrote_date = true;
                }
                header::TRAILER => {
                    // check that we actually can send a chunked body...
                    if msg.head.version == Version::HTTP_10
                        || !Server::can_chunked(msg.req_method, msg.head.subject)
                    {
                        continue;
                    }

                    if !is_name_written {
                        is_name_written = true;
                        header_name_writer.write_header_name_with_colon(
                            dst,
                            "trailer: ",
                            header::TRAILER,
                        );
                        extend(dst, value.as_bytes());
                    } else {
                        extend(dst, b", ");
                        extend(dst, value.as_bytes());
                    }

                    match allowed_trailer_fields {
                        Some(ref mut allowed_trailer_fields) => {
                            allowed_trailer_fields.push(value);
                        }
                        None => {
                            allowed_trailer_fields = Some(vec![value]);
                        }
                    }

                    continue 'headers;
                }
                _ => (),
            }
            //TODO: this should perhaps instead combine them into
            //single lines, as RFC7230 suggests is preferable.

            // non-special write Name and Value
            debug_assert!(
                !is_name_written,
                "{:?} set is_name_written and didn't continue loop",
                name,
            );
            header_name_writer.write_header_name(dst, name);
            extend(dst, b": ");
            extend(dst, value.as_bytes());
            extend(dst, b"\r\n");
        }

        handle_is_name_written!();

        if !wrote_len {
            encoder = match msg.body {
                Some(BodyLength::Unknown) => {
                    if msg.head.version == Version::HTTP_10
                        || !Server::can_chunked(msg.req_method, msg.head.subject)
                    {
                        Encoder::close_delimited()
                    } else {
                        header_name_writer.write_full_header_line(
                            dst,
                            "transfer-encoding: chunked\r\n",
                            (header::TRANSFER_ENCODING, ": chunked\r\n"),
                        );
                        Encoder::chunked()
                    }
                }
                None | Some(BodyLength::Known(0)) => {
                    if Server::can_have_implicit_zero_content_length(
                        msg.req_method,
                        msg.head.subject,
                    ) {
                        header_name_writer.write_full_header_line(
                            dst,
                            "content-length: 0\r\n",
                            (header::CONTENT_LENGTH, ": 0\r\n"),
                        )
                    }
                    Encoder::length(0)
                }
                Some(BodyLength::Known(len)) => {
                    if !Server::can_have_content_length(msg.req_method, msg.head.subject) {
                        Encoder::length(0)
                    } else {
                        header_name_writer.write_header_name_with_colon(
                            dst,
                            "content-length: ",
                            header::CONTENT_LENGTH,
                        );
                        extend(dst, ::itoa::Buffer::new().format(len).as_bytes());
                        extend(dst, b"\r\n");
                        Encoder::length(len)
                    }
                }
            };
        }

        if !Server::can_have_body(msg.req_method, msg.head.subject) {
            trace!(
                "server body forced to 0; method={:?}, status={:?}",
                msg.req_method,
                msg.head.subject
            );
            encoder = Encoder::length(0);
        }

        // cached date is much faster than formatting every request
        // don't force the write if disabled
        if !wrote_date && msg.date_header {
            dst.reserve(date::DATE_VALUE_LENGTH + 8);
            header_name_writer.write_header_name_with_colon(dst, "date: ", header::DATE);
            date::extend(dst);
            extend(dst, b"\r\n\r\n");
        } else {
            extend(dst, b"\r\n");
        }

        if encoder.is_chunked() {
            if let Some(allowed_trailer_fields) = allowed_trailer_fields {
                encoder = encoder.into_chunked_with_trailing_fields(allowed_trailer_fields);
            }
        }

        Ok(encoder.set_last(is_last))
    }

    /// Helper for zero-copy parsing of request path URI.
    #[inline]
    fn record_path_range(bytes: &[u8], req_path: &str) -> std::ops::Range<usize> {
        let bytes_ptr = bytes.as_ptr() as usize;
        let start = req_path.as_ptr() as usize - bytes_ptr;
        let end = start + req_path.len();
        std::ops::Range { start, end }
    }
}

#[cfg(feature = "server")]
trait HeaderNameWriter {
    fn write_full_header_line(
        &mut self,
        dst: &mut Vec<u8>,
        line: &str,
        name_value_pair: (HeaderName, &str),
    );
    fn write_header_name_with_colon(
        &mut self,
        dst: &mut Vec<u8>,
        name_with_colon: &str,
        name: HeaderName,
    );
    fn write_header_name(&mut self, dst: &mut Vec<u8>, name: &HeaderName);
}

#[cfg(feature = "client")]
impl Http1Transaction for Client {
    type Incoming = StatusCode;
    type Outgoing = RequestLine;
    #[cfg(feature = "tracing")]
    const LOG: &'static str = "{role=client}";

    fn parse(buf: &mut BytesMut, ctx: ParseContext<'_>) -> ParseResult<StatusCode> {
        debug_assert!(!buf.is_empty(), "parse called with empty buf");

        // Loop to skip information status code headers (100 Continue, etc).
        loop {
            let mut headers_indices: SmallVec<[MaybeUninit<HeaderIndices>; DEFAULT_MAX_HEADERS]> =
                match ctx.h1_max_headers {
                    Some(cap) => smallvec![MaybeUninit::uninit(); cap],
                    None => smallvec_inline![MaybeUninit::uninit(); DEFAULT_MAX_HEADERS],
                };
            let (len, status, reason, version, headers_len) = {
                let mut headers: SmallVec<
                    [MaybeUninit<httparse::Header<'_>>; DEFAULT_MAX_HEADERS],
                > = match ctx.h1_max_headers {
                    Some(cap) => smallvec![MaybeUninit::uninit(); cap],
                    None => smallvec_inline![MaybeUninit::uninit(); DEFAULT_MAX_HEADERS],
                };
                trace!(bytes = buf.len(), "Response.parse");
                let mut res = httparse::Response::new(&mut []);
                let bytes = buf.as_ref();
                match ctx.h1_parser_config.parse_response_with_uninit_headers(
                    &mut res,
                    bytes,
                    &mut headers,
                ) {
                    Ok(httparse::Status::Complete(len)) => {
                        trace!("Response.parse Complete({})", len);
                        let status = StatusCode::from_u16(res.code.unwrap())?;

                        let reason = {
                            let reason = res.reason.unwrap();
                            // Only save the reason phrase if it isn't the canonical reason
                            if Some(reason) != status.canonical_reason() {
                                Some(Bytes::copy_from_slice(reason.as_bytes()))
                            } else {
                                None
                            }
                        };

                        let version = if res.version.unwrap() == 1 {
                            Version::HTTP_11
                        } else {
                            Version::HTTP_10
                        };
                        record_header_indices(bytes, res.headers, &mut headers_indices)?;
                        let headers_len = res.headers.len();
                        (len, status, reason, version, headers_len)
                    }
                    Ok(httparse::Status::Partial) => return Ok(None),
                    Err(httparse::Error::Version) if ctx.h09_responses => {
                        trace!("Response.parse accepted HTTP/0.9 response");

                        (0, StatusCode::OK, None, Version::HTTP_09, 0)
                    }
                    Err(e) => return Err(e.into()),
                }
            };

            let mut slice = buf.split_to(len);

            if ctx
                .h1_parser_config
                .obsolete_multiline_headers_in_responses_are_allowed()
            {
                for header in &mut headers_indices[..headers_len] {
                    // SAFETY: array is valid up to `headers_len`
                    let header = unsafe { header.assume_init_mut() };
                    Client::obs_fold_line(&mut slice, header);
                }
            }

            let slice = slice.freeze();

            let mut headers = ctx.cached_headers.take().unwrap_or_default();

            let mut keep_alive = version == Version::HTTP_11;

            let mut header_case_map = if ctx.preserve_header_case {
                Some(HeaderCaseMap::default())
            } else {
                None
            };

            #[cfg(feature = "ffi")]
            let mut header_order = if ctx.preserve_header_order {
                Some(OriginalHeaderOrder::default())
            } else {
                None
            };

            headers.reserve(headers_len);
            for header in &headers_indices[..headers_len] {
                // SAFETY: array is valid up to `headers_len`
                let header = unsafe { header.assume_init_ref() };
                let name = header_name!(&slice[header.name.0..header.name.1]);
                let value = header_value!(slice.slice(header.value.0..header.value.1));

                if let header::CONNECTION = name {
                    // keep_alive was previously set to default for Version
                    if keep_alive {
                        // HTTP/1.1
                        keep_alive = !headers::connection_close(&value);
                    } else {
                        // HTTP/1.0
                        keep_alive = headers::connection_keep_alive(&value);
                    }
                }

                if let Some(ref mut header_case_map) = header_case_map {
                    header_case_map.append(&name, slice.slice(header.name.0..header.name.1));
                }

                #[cfg(feature = "ffi")]
                if let Some(ref mut header_order) = header_order {
                    header_order.append(&name);
                }

                headers.append(name, value);
            }

            let mut extensions = http::Extensions::default();

            if let Some(header_case_map) = header_case_map {
                extensions.insert(header_case_map);
            }

            #[cfg(feature = "ffi")]
            if let Some(header_order) = header_order {
                extensions.insert(header_order);
            }

            if let Some(reason) = reason {
                // Safety: httparse ensures that only valid reason phrase bytes are present in this
                // field.
                let reason = crate::ext::ReasonPhrase::from_bytes_unchecked(reason);
                extensions.insert(reason);
            }

            let head = MessageHead {
                version,
                subject: status,
                headers,
                extensions,
            };
            if let Some((decode, is_upgrade)) = Client::decoder(&head, ctx.req_method)? {
                return Ok(Some(ParsedMessage {
                    head,
                    decode,
                    expect_continue: false,
                    // a client upgrade means the connection can't be used
                    // again, as it is definitely upgrading.
                    keep_alive: keep_alive && !is_upgrade,
                    wants_upgrade: is_upgrade,
                }));
            }

            if head.subject.is_informational() {
                if let Some(callback) = ctx.on_informational {
                    callback.call(head.into_response(()));
                }
            }

            // Parsing a 1xx response could have consumed the buffer, check if
            // it is empty now...
            if buf.is_empty() {
                return Ok(None);
            }
        }
    }

    fn encode(msg: Encode<'_, Self::Outgoing>, dst: &mut Vec<u8>) -> crate::Result<Encoder> {
        trace!(
            "Client::encode method={:?}, body={:?}",
            msg.head.subject.0,
            msg.body
        );

        *msg.req_method = Some(msg.head.subject.0.clone());

        let body = Client::set_length(msg.head, msg.body);

        let init_cap = 30 + msg.head.headers.len() * AVERAGE_HEADER_SIZE;
        dst.reserve(init_cap);

        extend(dst, msg.head.subject.0.as_str().as_bytes());
        extend(dst, b" ");
        //TODO: add API to http::Uri to encode without std::fmt
        let _ = write!(FastWrite(dst), "{} ", msg.head.subject.1);

        match msg.head.version {
            Version::HTTP_10 => extend(dst, b"HTTP/1.0"),
            Version::HTTP_11 => extend(dst, b"HTTP/1.1"),
            Version::HTTP_2 => {
                debug!("request with HTTP2 version coerced to HTTP/1.1");
                extend(dst, b"HTTP/1.1");
            }
            other => panic!("unexpected request version: {:?}", other),
        }
        extend(dst, b"\r\n");

        if let Some(orig_headers) = msg.head.extensions.get::<HeaderCaseMap>() {
            write_headers_original_case(
                &msg.head.headers,
                orig_headers,
                dst,
                msg.title_case_headers,
            );
        } else if msg.title_case_headers {
            write_headers_title_case(&msg.head.headers, dst);
        } else {
            write_headers(&msg.head.headers, dst);
        }

        extend(dst, b"\r\n");
        msg.head.headers.clear(); //TODO: remove when switching to drain()

        Ok(body)
    }

    fn on_error(_err: &crate::Error) -> Option<MessageHead<Self::Outgoing>> {
        // we can't tell the server about any errors it creates
        None
    }

    fn is_client() -> bool {
        true
    }
}

#[cfg(feature = "client")]
impl Client {
    /// Returns Some(length, wants_upgrade) if successful.
    ///
    /// Returns None if this message head should be skipped (like a 100 status).
    fn decoder(
        inc: &MessageHead<StatusCode>,
        method: &mut Option<Method>,
    ) -> Result<Option<(DecodedLength, bool)>, Parse> {
        // According to https://tools.ietf.org/html/rfc7230#section-3.3.3
        // 1. HEAD responses, and Status 1xx, 204, and 304 cannot have a body.
        // 2. Status 2xx to a CONNECT cannot have a body.
        // 3. Transfer-Encoding: chunked has a chunked body.
        // 4. If multiple differing Content-Length headers or invalid, close connection.
        // 5. Content-Length header has a sized body.
        // 6. (irrelevant to Response)
        // 7. Read till EOF.

        match inc.subject.as_u16() {
            101 => {
                return Ok(Some((DecodedLength::ZERO, true)));
            }
            100 | 102..=199 => {
                trace!("ignoring informational response: {}", inc.subject.as_u16());
                return Ok(None);
            }
            204 | 304 => return Ok(Some((DecodedLength::ZERO, false))),
            _ => (),
        }
        match *method {
            Some(Method::HEAD) => {
                return Ok(Some((DecodedLength::ZERO, false)));
            }
            Some(Method::CONNECT) => {
                if let 200..=299 = inc.subject.as_u16() {
                    return Ok(Some((DecodedLength::ZERO, true)));
                }
            }
            Some(_) => {}
            None => {
                trace!("Client::decoder is missing the Method");
            }
        }

        if inc.headers.contains_key(header::TRANSFER_ENCODING) {
            // https://tools.ietf.org/html/rfc7230#section-3.3.3
            // If Transfer-Encoding header is present, and 'chunked' is
            // not the final encoding, and this is a Request, then it is
            // malformed. A server should respond with 400 Bad Request.
            if inc.version == Version::HTTP_10 {
                debug!("HTTP/1.0 cannot have Transfer-Encoding header");
                Err(Parse::transfer_encoding_unexpected())
            } else if headers::transfer_encoding_is_chunked(&inc.headers) {
                Ok(Some((DecodedLength::CHUNKED, false)))
            } else {
                trace!("not chunked, read till eof");
                Ok(Some((DecodedLength::CLOSE_DELIMITED, false)))
            }
        } else if let Some(len) = headers::content_length_parse_all(&inc.headers) {
            Ok(Some((DecodedLength::checked_new(len)?, false)))
        } else if inc.headers.contains_key(header::CONTENT_LENGTH) {
            debug!("illegal Content-Length header");
            Err(Parse::content_length_invalid())
        } else {
            trace!("neither Transfer-Encoding nor Content-Length");
            Ok(Some((DecodedLength::CLOSE_DELIMITED, false)))
        }
    }
    fn set_length(head: &mut RequestHead, body: Option<BodyLength>) -> Encoder {
        let body = if let Some(body) = body {
            body
        } else {
            head.headers.remove(header::TRANSFER_ENCODING);
            return Encoder::length(0);
        };

        // HTTP/1.0 doesn't know about chunked
        let can_chunked = head.version == Version::HTTP_11;
        let headers = &mut head.headers;

        // If the user already set specific headers, we should respect them, regardless
        // of what the Body knows about itself. They set them for a reason.

        // Because of the borrow checker, we can't check the for an existing
        // Content-Length header while holding an `Entry` for the Transfer-Encoding
        // header, so unfortunately, we must do the check here, first.

        let existing_con_len = headers::content_length_parse_all(headers);
        let mut should_remove_con_len = false;

        if !can_chunked {
            // Chunked isn't legal, so if it is set, we need to remove it.
            if headers.remove(header::TRANSFER_ENCODING).is_some() {
                trace!("removing illegal transfer-encoding header");
            }

            return if let Some(len) = existing_con_len {
                Encoder::length(len)
            } else if let BodyLength::Known(len) = body {
                set_content_length(headers, len)
            } else {
                // HTTP/1.0 client requests without a content-length
                // cannot have any body at all.
                Encoder::length(0)
            };
        }

        // If the user set a transfer-encoding, respect that. Let's just
        // make sure `chunked` is the final encoding.
        let encoder = match headers.entry(header::TRANSFER_ENCODING) {
            Entry::Occupied(te) => {
                should_remove_con_len = true;
                if headers::is_chunked(te.iter()) {
                    Some(Encoder::chunked())
                } else {
                    warn!("user provided transfer-encoding does not end in 'chunked'");

                    // There's a Transfer-Encoding, but it doesn't end in 'chunked'!
                    // An example that could trigger this:
                    //
                    //     Transfer-Encoding: gzip
                    //
                    // This can be bad, depending on if this is a request or a
                    // response.
                    //
                    // - A request is illegal if there is a `Transfer-Encoding`
                    //   but it doesn't end in `chunked`.
                    // - A response that has `Transfer-Encoding` but doesn't
                    //   end in `chunked` isn't illegal, it just forces this
                    //   to be close-delimited.
                    //
                    // We can try to repair this, by adding `chunked` ourselves.

                    headers::add_chunked(te);
                    Some(Encoder::chunked())
                }
            }
            Entry::Vacant(te) => {
                if let Some(len) = existing_con_len {
                    Some(Encoder::length(len))
                } else if let BodyLength::Unknown = body {
                    // GET, HEAD, and CONNECT almost never have bodies.
                    //
                    // So instead of sending a "chunked" body with a 0-chunk,
                    // assume no body here. If you *must* send a body,
                    // set the headers explicitly.
                    match head.subject.0 {
                        Method::GET | Method::HEAD | Method::CONNECT => Some(Encoder::length(0)),
                        _ => {
                            te.insert(HeaderValue::from_static("chunked"));
                            Some(Encoder::chunked())
                        }
                    }
                } else {
                    None
                }
            }
        };

        let encoder = encoder.map(|enc| {
            if enc.is_chunked() {
                let allowed_trailer_fields: Vec<HeaderValue> =
                    headers.get_all(header::TRAILER).iter().cloned().collect();

                if !allowed_trailer_fields.is_empty() {
                    return enc.into_chunked_with_trailing_fields(allowed_trailer_fields);
                }
            }

            enc
        });

        // This is because we need a second mutable borrow to remove
        // content-length header.
        if let Some(encoder) = encoder {
            if should_remove_con_len && existing_con_len.is_some() {
                headers.remove(header::CONTENT_LENGTH);
            }
            return encoder;
        }

        // User didn't set transfer-encoding, AND we know body length,
        // so we can just set the Content-Length automatically.

        let len = if let BodyLength::Known(len) = body {
            len
        } else {
            unreachable!("BodyLength::Unknown would set chunked");
        };

        set_content_length(headers, len)
    }

    fn obs_fold_line(all: &mut [u8], idx: &mut HeaderIndices) {
        // If the value has obs-folded text, then in-place shift the bytes out
        // of here.
        //
        // https://httpwg.org/specs/rfc9112.html#line.folding
        //
        // > A user agent that receives an obs-fold MUST replace each received
        // > obs-fold with one or more SP octets prior to interpreting the
        // > field value.
        //
        // This means strings like "\r\n\t foo" must replace the "\r\n\t " with
        // a single space.

        let buf = &mut all[idx.value.0..idx.value.1];

        // look for a newline, otherwise bail out
        let first_nl = match buf.iter().position(|b| *b == b'\n') {
            Some(i) => i,
            None => return,
        };

        // not on standard slices because whatever, sigh
        fn trim_start(mut s: &[u8]) -> &[u8] {
            while let [first, rest @ ..] = s {
                if first.is_ascii_whitespace() {
                    s = rest;
                } else {
                    break;
                }
            }
            s
        }

        fn trim_end(mut s: &[u8]) -> &[u8] {
            while let [rest @ .., last] = s {
                if last.is_ascii_whitespace() {
                    s = rest;
                } else {
                    break;
                }
            }
            s
        }

        fn trim(s: &[u8]) -> &[u8] {
            trim_start(trim_end(s))
        }

        // TODO(perf): we could do the moves in-place, but this is so uncommon
        // that it shouldn't matter.
        let mut unfolded = trim_end(&buf[..first_nl]).to_vec();
        for line in buf[first_nl + 1..].split(|b| *b == b'\n') {
            unfolded.push(b' ');
            unfolded.extend_from_slice(trim(line));
        }
        buf[..unfolded.len()].copy_from_slice(&unfolded);
        idx.value.1 = idx.value.0 + unfolded.len();
    }
}

#[cfg(feature = "client")]
fn set_content_length(headers: &mut HeaderMap, len: u64) -> Encoder {
    // At this point, there should not be a valid Content-Length
    // header. However, since we'll be indexing in anyways, we can
    // warn the user if there was an existing illegal header.
    //
    // Or at least, we can in theory. It's actually a little bit slower,
    // so perhaps only do that while the user is developing/testing.

    if cfg!(debug_assertions) {
        match headers.entry(header::CONTENT_LENGTH) {
            Entry::Occupied(mut cl) => {
                // Internal sanity check, we should have already determined
                // that the header was illegal before calling this function.
                debug_assert!(headers::content_length_parse_all_values(cl.iter()).is_none());
                // Uh oh, the user set `Content-Length` headers, but set bad ones.
                // This would be an illegal message anyways, so let's try to repair
                // with our known good length.
                error!("user provided content-length header was invalid");

                cl.insert(HeaderValue::from(len));
                Encoder::length(len)
            }
            Entry::Vacant(cl) => {
                cl.insert(HeaderValue::from(len));
                Encoder::length(len)
            }
        }
    } else {
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
        Encoder::length(len)
    }
}

#[derive(Clone, Copy)]
struct HeaderIndices {
    name: (usize, usize),
    value: (usize, usize),
}

fn record_header_indices(
    bytes: &[u8],
    headers: &[httparse::Header<'_>],
    indices: &mut [MaybeUninit<HeaderIndices>],
) -> Result<(), crate::error::Parse> {
    let bytes_ptr = bytes.as_ptr() as usize;

    for (header, indices) in headers.iter().zip(indices.iter_mut()) {
        if header.name.len() >= (1 << 16) {
            debug!("header name larger than 64kb: {:?}", header.name);
            return Err(crate::error::Parse::TooLarge);
        }
        let name_start = header.name.as_ptr() as usize - bytes_ptr;
        let name_end = name_start + header.name.len();
        let value_start = header.value.as_ptr() as usize - bytes_ptr;
        let value_end = value_start + header.value.len();

        indices.write(HeaderIndices {
            name: (name_start, name_end),
            value: (value_start, value_end),
        });
    }

    Ok(())
}

// Write header names as title case. The header name is assumed to be ASCII.
fn title_case(dst: &mut Vec<u8>, name: &[u8]) {
    dst.reserve(name.len());

    // Ensure first character is uppercased
    let mut prev = b'-';
    for &(mut c) in name {
        if prev == b'-' {
            c.make_ascii_uppercase();
        }
        dst.push(c);
        prev = c;
    }
}

pub(crate) fn write_headers_title_case(headers: &HeaderMap, dst: &mut Vec<u8>) {
    for (name, value) in headers {
        title_case(dst, name.as_str().as_bytes());
        extend(dst, b": ");
        extend(dst, value.as_bytes());
        extend(dst, b"\r\n");
    }
}

pub(crate) fn write_headers(headers: &HeaderMap, dst: &mut Vec<u8>) {
    for (name, value) in headers {
        extend(dst, name.as_str().as_bytes());
        extend(dst, b": ");
        extend(dst, value.as_bytes());
        extend(dst, b"\r\n");
    }
}

#[cold]
#[cfg(feature = "client")]
fn write_headers_original_case(
    headers: &HeaderMap,
    orig_case: &HeaderCaseMap,
    dst: &mut Vec<u8>,
    title_case_headers: bool,
) {
    // For each header name/value pair, there may be a value in the casemap
    // that corresponds to the HeaderValue. So, we iterator all the keys,
    // and for each one, try to pair the originally cased name with the value.
    //
    // TODO: consider adding http::HeaderMap::entries() iterator
    for name in headers.keys() {
        let mut names = orig_case.get_all(name);

        for value in headers.get_all(name) {
            if let Some(orig_name) = names.next() {
                extend(dst, orig_name.as_ref());
            } else if title_case_headers {
                title_case(dst, name.as_str().as_bytes());
            } else {
                extend(dst, name.as_str().as_bytes());
            }

            // Wanted for curl test cases that send `X-Custom-Header:\r\n`
            if value.is_empty() {
                extend(dst, b":\r\n");
            } else {
                extend(dst, b": ");
                extend(dst, value.as_bytes());
                extend(dst, b"\r\n");
            }
        }
    }
}

#[cfg(feature = "client")]
struct FastWrite<'a>(&'a mut Vec<u8>);

#[cfg(feature = "client")]
impl fmt::Write for FastWrite<'_> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        extend(self.0, s.as_bytes());
        Ok(())
    }

    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        fmt::write(self, args)
    }
}

#[inline]
fn extend(dst: &mut Vec<u8>, data: &[u8]) {
    dst.extend_from_slice(data);
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use super::*;

    #[cfg(feature = "server")]
    #[test]
    fn test_parse_request() {
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from("GET /echo HTTP/1.1\r\nHost: hyper.rs\r\n\r\n");
        let mut method = None;
        let msg = Server::parse(
            &mut raw,
            ParseContext {
                cached_headers: &mut None,
                req_method: &mut method,
                h1_parser_config: Default::default(),
                h1_max_headers: None,
                preserve_header_case: false,
                #[cfg(feature = "ffi")]
                preserve_header_order: false,
                h09_responses: false,
                #[cfg(feature = "client")]
                on_informational: &mut None,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(raw.len(), 0);
        assert_eq!(msg.head.subject.0, crate::Method::GET);
        assert_eq!(msg.head.subject.1, "/echo");
        assert_eq!(msg.head.version, crate::Version::HTTP_11);
        assert_eq!(msg.head.headers.len(), 1);
        assert_eq!(msg.head.headers["Host"], "hyper.rs");
        assert_eq!(method, Some(crate::Method::GET));
    }

    #[test]
    fn test_parse_response() {
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut Some(crate::Method::GET),
            h1_parser_config: Default::default(),
            h1_max_headers: None,
            preserve_header_case: false,
            #[cfg(feature = "ffi")]
            preserve_header_order: false,
            h09_responses: false,
            #[cfg(feature = "client")]
            on_informational: &mut None,
        };
        let msg = Client::parse(&mut raw, ctx).unwrap().unwrap();
        assert_eq!(raw.len(), 0);
        assert_eq!(msg.head.subject, crate::StatusCode::OK);
        assert_eq!(msg.head.version, crate::Version::HTTP_11);
        assert_eq!(msg.head.headers.len(), 1);
        assert_eq!(msg.head.headers["Content-Length"], "0");
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_parse_request_errors() {
        let mut raw = BytesMut::from("GET htt:p// HTTP/1.1\r\nHost: hyper.rs\r\n\r\n");
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut None,
            h1_parser_config: Default::default(),
            h1_max_headers: None,
            preserve_header_case: false,
            #[cfg(feature = "ffi")]
            preserve_header_order: false,
            h09_responses: false,
            #[cfg(feature = "client")]
            on_informational: &mut None,
        };
        Server::parse(&mut raw, ctx).unwrap_err();
    }

    const H09_RESPONSE: &str = "Baguettes are super delicious, don't you agree?";

    #[test]
    fn test_parse_response_h09_allowed() {
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(H09_RESPONSE);
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut Some(crate::Method::GET),
            h1_parser_config: Default::default(),
            h1_max_headers: None,
            preserve_header_case: false,
            #[cfg(feature = "ffi")]
            preserve_header_order: false,
            h09_responses: true,
            #[cfg(feature = "client")]
            on_informational: &mut None,
        };
        let msg = Client::parse(&mut raw, ctx).unwrap().unwrap();
        assert_eq!(raw, H09_RESPONSE);
        assert_eq!(msg.head.subject, crate::StatusCode::OK);
        assert_eq!(msg.head.version, crate::Version::HTTP_09);
        assert_eq!(msg.head.headers.len(), 0);
    }

    #[test]
    fn test_parse_response_h09_rejected() {
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(H09_RESPONSE);
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut Some(crate::Method::GET),
            h1_parser_config: Default::default(),
            h1_max_headers: None,
            preserve_header_case: false,
            #[cfg(feature = "ffi")]
            preserve_header_order: false,
            h09_responses: false,
            #[cfg(feature = "client")]
            on_informational: &mut None,
        };
        Client::parse(&mut raw, ctx).unwrap_err();
        assert_eq!(raw, H09_RESPONSE);
    }

    const RESPONSE_WITH_WHITESPACE_BETWEEN_HEADER_NAME_AND_COLON: &str =
        "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Credentials : true\r\n\r\n";

    #[test]
    fn test_parse_allow_response_with_spaces_before_colons() {
        use httparse::ParserConfig;

        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(RESPONSE_WITH_WHITESPACE_BETWEEN_HEADER_NAME_AND_COLON);
        let mut h1_parser_config = ParserConfig::default();
        h1_parser_config.allow_spaces_after_header_name_in_responses(true);
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut Some(crate::Method::GET),
            h1_parser_config,
            h1_max_headers: None,
            preserve_header_case: false,
            #[cfg(feature = "ffi")]
            preserve_header_order: false,
            h09_responses: false,
            #[cfg(feature = "client")]
            on_informational: &mut None,
        };
        let msg = Client::parse(&mut raw, ctx).unwrap().unwrap();
        assert_eq!(raw.len(), 0);
        assert_eq!(msg.head.subject, crate::StatusCode::OK);
        assert_eq!(msg.head.version, crate::Version::HTTP_11);
        assert_eq!(msg.head.headers.len(), 1);
        assert_eq!(msg.head.headers["Access-Control-Allow-Credentials"], "true");
    }

    #[test]
    fn test_parse_reject_response_with_spaces_before_colons() {
        let _ = pretty_env_logger::try_init();
        let mut raw = BytesMut::from(RESPONSE_WITH_WHITESPACE_BETWEEN_HEADER_NAME_AND_COLON);
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut Some(crate::Method::GET),
            h1_parser_config: Default::default(),
            h1_max_headers: None,
            preserve_header_case: false,
            #[cfg(feature = "ffi")]
            preserve_header_order: false,
            h09_responses: false,
            #[cfg(feature = "client")]
            on_informational: &mut None,
        };
        Client::parse(&mut raw, ctx).unwrap_err();
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_parse_preserve_header_case_in_request() {
        let mut raw =
            BytesMut::from("GET / HTTP/1.1\r\nHost: hyper.rs\r\nX-BREAD: baguette\r\n\r\n");
        let ctx = ParseContext {
            cached_headers: &mut None,
            req_method: &mut None,
            h1_parser_config: Default::default(),
            h1_max_headers: None,
            preserve_header_case: true,
            #[cfg(feature = "ffi")]
            preserve_header_order: false,
            h09_responses: false,
            #[cfg(feature = "client")]
            on_informational: &mut None,
        };
        let parsed_message = Server::parse(&mut raw, ctx).unwrap().unwrap();
        let orig_headers = parsed_message
            .head
            .extensions
            .get::<HeaderCaseMap>()
            .unwrap();
        assert_eq!(
            orig_headers
                .get_all_internal(&HeaderName::from_static("host"))
                .collect::<Vec<_>>(),
            vec![&Bytes::from("Host")]
        );
        assert_eq!(
            orig_headers
                .get_all_internal(&HeaderName::from_static("x-bread"))
                .collect::<Vec<_>>(),
            vec![&Bytes::from("X-BREAD")]
        );
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_decoder_request() {
        fn parse(s: &str) -> ParsedMessage<RequestLine> {
            let mut bytes = BytesMut::from(s);
            Server::parse(
                &mut bytes,
                ParseContext {
                    cached_headers: &mut None,
                    req_method: &mut None,
                    h1_parser_config: Default::default(),
                    h1_max_headers: None,
                    preserve_header_case: false,
                    #[cfg(feature = "ffi")]
                    preserve_header_order: false,
                    h09_responses: false,
                    #[cfg(feature = "client")]
                    on_informational: &mut None,
                },
            )
            .expect("parse ok")
            .expect("parse complete")
        }

        fn parse_err(s: &str, comment: &str) -> crate::error::Parse {
            let mut bytes = BytesMut::from(s);
            Server::parse(
                &mut bytes,
                ParseContext {
                    cached_headers: &mut None,
                    req_method: &mut None,
                    h1_parser_config: Default::default(),
                    h1_max_headers: None,
                    preserve_header_case: false,
                    #[cfg(feature = "ffi")]
                    preserve_header_order: false,
                    h09_responses: false,
                    #[cfg(feature = "client")]
                    on_informational: &mut None,
                },
            )
            .expect_err(comment)
        }

        // no length or transfer-encoding means 0-length body
        assert_eq!(
            parse(
                "\
                 GET / HTTP/1.1\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::ZERO
        );

        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::ZERO
        );

        // transfer-encoding: chunked
        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 transfer-encoding: chunked\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CHUNKED
        );

        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 transfer-encoding: gzip, chunked\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CHUNKED
        );

        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 transfer-encoding: gzip\r\n\
                 transfer-encoding: chunked\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CHUNKED
        );

        // content-length
        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 content-length: 10\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::new(10)
        );

        // transfer-encoding and content-length = chunked
        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 content-length: 10\r\n\
                 transfer-encoding: chunked\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CHUNKED
        );

        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 transfer-encoding: chunked\r\n\
                 content-length: 10\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CHUNKED
        );

        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 transfer-encoding: gzip\r\n\
                 content-length: 10\r\n\
                 transfer-encoding: chunked\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CHUNKED
        );

        // multiple content-lengths of same value are fine
        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.1\r\n\
                 content-length: 10\r\n\
                 content-length: 10\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::new(10)
        );

        // multiple content-lengths with different values is an error
        parse_err(
            "\
             POST / HTTP/1.1\r\n\
             content-length: 10\r\n\
             content-length: 11\r\n\
             \r\n\
             ",
            "multiple content-lengths",
        );

        // content-length with prefix is not allowed
        parse_err(
            "\
             POST / HTTP/1.1\r\n\
             content-length: +10\r\n\
             \r\n\
             ",
            "prefixed content-length",
        );

        // transfer-encoding that isn't chunked is an error
        parse_err(
            "\
             POST / HTTP/1.1\r\n\
             transfer-encoding: gzip\r\n\
             \r\n\
             ",
            "transfer-encoding but not chunked",
        );

        parse_err(
            "\
             POST / HTTP/1.1\r\n\
             transfer-encoding: chunked, gzip\r\n\
             \r\n\
             ",
            "transfer-encoding doesn't end in chunked",
        );

        parse_err(
            "\
             POST / HTTP/1.1\r\n\
             transfer-encoding: chunked\r\n\
             transfer-encoding: afterlol\r\n\
             \r\n\
             ",
            "transfer-encoding multiple lines doesn't end in chunked",
        );

        // http/1.0

        assert_eq!(
            parse(
                "\
                 POST / HTTP/1.0\r\n\
                 content-length: 10\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::new(10)
        );

        // 1.0 doesn't understand chunked, so its an error
        parse_err(
            "\
             POST / HTTP/1.0\r\n\
             transfer-encoding: chunked\r\n\
             \r\n\
             ",
            "1.0 chunked",
        );
    }

    #[test]
    fn test_decoder_response() {
        fn parse(s: &str) -> ParsedMessage<StatusCode> {
            parse_with_method(s, Method::GET)
        }

        fn parse_ignores(s: &str) {
            let mut bytes = BytesMut::from(s);
            assert!(Client::parse(
                &mut bytes,
                ParseContext {
                    cached_headers: &mut None,
                    req_method: &mut Some(Method::GET),
                    h1_parser_config: Default::default(),
                    h1_max_headers: None,
                    preserve_header_case: false,
                    #[cfg(feature = "ffi")]
                    preserve_header_order: false,
                    h09_responses: false,
                    #[cfg(feature = "client")]
                    on_informational: &mut None,
                }
            )
            .expect("parse ok")
            .is_none())
        }

        fn parse_with_method(s: &str, m: Method) -> ParsedMessage<StatusCode> {
            let mut bytes = BytesMut::from(s);
            Client::parse(
                &mut bytes,
                ParseContext {
                    cached_headers: &mut None,
                    req_method: &mut Some(m),
                    h1_parser_config: Default::default(),
                    h1_max_headers: None,
                    preserve_header_case: false,
                    #[cfg(feature = "ffi")]
                    preserve_header_order: false,
                    h09_responses: false,
                    #[cfg(feature = "client")]
                    on_informational: &mut None,
                },
            )
            .expect("parse ok")
            .expect("parse complete")
        }

        fn parse_err(s: &str) -> crate::error::Parse {
            let mut bytes = BytesMut::from(s);
            Client::parse(
                &mut bytes,
                ParseContext {
                    cached_headers: &mut None,
                    req_method: &mut Some(Method::GET),
                    h1_parser_config: Default::default(),
                    h1_max_headers: None,
                    preserve_header_case: false,
                    #[cfg(feature = "ffi")]
                    preserve_header_order: false,
                    h09_responses: false,
                    #[cfg(feature = "client")]
                    on_informational: &mut None,
                },
            )
            .expect_err("parse should err")
        }

        // no content-length or transfer-encoding means close-delimited
        assert_eq!(
            parse(
                "\
                 HTTP/1.1 200 OK\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CLOSE_DELIMITED
        );

        // 204 and 304 never have a body
        assert_eq!(
            parse(
                "\
                 HTTP/1.1 204 No Content\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::ZERO
        );

        assert_eq!(
            parse(
                "\
                 HTTP/1.1 304 Not Modified\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::ZERO
        );

        // content-length
        assert_eq!(
            parse(
                "\
                 HTTP/1.1 200 OK\r\n\
                 content-length: 8\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::new(8)
        );

        assert_eq!(
            parse(
                "\
                 HTTP/1.1 200 OK\r\n\
                 content-length: 8\r\n\
                 content-length: 8\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::new(8)
        );

        parse_err(
            "\
             HTTP/1.1 200 OK\r\n\
             content-length: 8\r\n\
             content-length: 9\r\n\
             \r\n\
             ",
        );

        parse_err(
            "\
             HTTP/1.1 200 OK\r\n\
             content-length: +8\r\n\
             \r\n\
             ",
        );

        // transfer-encoding: chunked
        assert_eq!(
            parse(
                "\
                 HTTP/1.1 200 OK\r\n\
                 transfer-encoding: chunked\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CHUNKED
        );

        // transfer-encoding not-chunked is close-delimited
        assert_eq!(
            parse(
                "\
                 HTTP/1.1 200 OK\r\n\
                 transfer-encoding: yolo\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CLOSE_DELIMITED
        );

        // transfer-encoding and content-length = chunked
        assert_eq!(
            parse(
                "\
                 HTTP/1.1 200 OK\r\n\
                 content-length: 10\r\n\
                 transfer-encoding: chunked\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CHUNKED
        );

        // HEAD can have content-length, but not body
        assert_eq!(
            parse_with_method(
                "\
                 HTTP/1.1 200 OK\r\n\
                 content-length: 8\r\n\
                 \r\n\
                 ",
                Method::HEAD
            )
            .decode,
            DecodedLength::ZERO
        );

        // CONNECT with 200 never has body
        {
            let msg = parse_with_method(
                "\
                 HTTP/1.1 200 OK\r\n\
                 \r\n\
                 ",
                Method::CONNECT,
            );
            assert_eq!(msg.decode, DecodedLength::ZERO);
            assert!(!msg.keep_alive, "should be upgrade");
            assert!(msg.wants_upgrade, "should be upgrade");
        }

        // CONNECT receiving non 200 can have a body
        assert_eq!(
            parse_with_method(
                "\
                 HTTP/1.1 400 Bad Request\r\n\
                 \r\n\
                 ",
                Method::CONNECT
            )
            .decode,
            DecodedLength::CLOSE_DELIMITED
        );

        // 1xx status codes
        parse_ignores(
            "\
             HTTP/1.1 100 Continue\r\n\
             \r\n\
             ",
        );

        parse_ignores(
            "\
             HTTP/1.1 103 Early Hints\r\n\
             \r\n\
             ",
        );

        // 101 upgrade not supported yet
        {
            let msg = parse(
                "\
                 HTTP/1.1 101 Switching Protocols\r\n\
                 \r\n\
                 ",
            );
            assert_eq!(msg.decode, DecodedLength::ZERO);
            assert!(!msg.keep_alive, "should be last");
            assert!(msg.wants_upgrade, "should be upgrade");
        }

        // http/1.0
        assert_eq!(
            parse(
                "\
                 HTTP/1.0 200 OK\r\n\
                 \r\n\
                 "
            )
            .decode,
            DecodedLength::CLOSE_DELIMITED
        );

        // 1.0 doesn't understand chunked
        parse_err(
            "\
             HTTP/1.0 200 OK\r\n\
             transfer-encoding: chunked\r\n\
             \r\n\
             ",
        );

        // keep-alive
        assert!(
            parse(
                "\
                 HTTP/1.1 200 OK\r\n\
                 content-length: 0\r\n\
                 \r\n\
                 "
            )
            .keep_alive,
            "HTTP/1.1 keep-alive is default"
        );

        assert!(
            !parse(
                "\
                 HTTP/1.1 200 OK\r\n\
                 content-length: 0\r\n\
                 connection: foo, close, bar\r\n\
                 \r\n\
                 "
            )
            .keep_alive,
            "connection close is always close"
        );

        assert!(
            !parse(
                "\
                 HTTP/1.0 200 OK\r\n\
                 content-length: 0\r\n\
                 \r\n\
                 "
            )
            .keep_alive,
            "HTTP/1.0 close is default"
        );

        assert!(
            parse(
                "\
                 HTTP/1.0 200 OK\r\n\
                 content-length: 0\r\n\
                 connection: foo, keep-alive, bar\r\n\
                 \r\n\
                 "
            )
            .keep_alive,
            "connection keep-alive is always keep-alive"
        );
    }

    #[cfg(feature = "client")]
    #[test]
    fn test_client_obs_fold_line() {
        fn unfold(src: &str) -> String {
            let mut buf = src.as_bytes().to_vec();
            let mut idx = HeaderIndices {
                name: (0, 0),
                value: (0, buf.len()),
            };
            Client::obs_fold_line(&mut buf, &mut idx);
            String::from_utf8(buf[idx.value.0..idx.value.1].to_vec()).unwrap()
        }

        assert_eq!(unfold("a normal line"), "a normal line",);

        assert_eq!(unfold("obs\r\n fold\r\n\t line"), "obs fold line",);
    }

    #[test]
    fn test_client_request_encode_title_case() {
        use crate::proto::BodyLength;
        use http::header::HeaderValue;

        let mut head = MessageHead::default();
        head.headers
            .insert("content-length", HeaderValue::from_static("10"));
        head.headers
            .insert("content-type", HeaderValue::from_static("application/json"));
        head.headers.insert("*-*", HeaderValue::from_static("o_o"));

        let mut vec = Vec::new();
        Client::encode(
            Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                #[cfg(feature = "server")]
                keep_alive: true,
                req_method: &mut None,
                title_case_headers: true,
                #[cfg(feature = "server")]
                date_header: true,
            },
            &mut vec,
        )
        .unwrap();

        assert_eq!(vec, b"GET / HTTP/1.1\r\nContent-Length: 10\r\nContent-Type: application/json\r\n*-*: o_o\r\n\r\n".to_vec());
    }

    #[test]
    fn test_client_request_encode_orig_case() {
        use crate::proto::BodyLength;
        use http::header::{HeaderValue, CONTENT_LENGTH};

        let mut head = MessageHead::default();
        head.headers
            .insert("content-length", HeaderValue::from_static("10"));
        head.headers
            .insert("content-type", HeaderValue::from_static("application/json"));

        let mut orig_headers = HeaderCaseMap::default();
        orig_headers.insert(CONTENT_LENGTH, "CONTENT-LENGTH".into());
        head.extensions.insert(orig_headers);

        let mut vec = Vec::new();
        Client::encode(
            Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                #[cfg(feature = "server")]
                keep_alive: true,
                req_method: &mut None,
                title_case_headers: false,
                #[cfg(feature = "server")]
                date_header: true,
            },
            &mut vec,
        )
        .unwrap();

        assert_eq!(
            &*vec,
            b"GET / HTTP/1.1\r\nCONTENT-LENGTH: 10\r\ncontent-type: application/json\r\n\r\n"
                .as_ref(),
        );
    }
    #[test]
    fn test_client_request_encode_orig_and_title_case() {
        use crate::proto::BodyLength;
        use http::header::{HeaderValue, CONTENT_LENGTH};

        let mut head = MessageHead::default();
        head.headers
            .insert("content-length", HeaderValue::from_static("10"));
        head.headers
            .insert("content-type", HeaderValue::from_static("application/json"));

        let mut orig_headers = HeaderCaseMap::default();
        orig_headers.insert(CONTENT_LENGTH, "CONTENT-LENGTH".into());
        head.extensions.insert(orig_headers);

        let mut vec = Vec::new();
        Client::encode(
            Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                #[cfg(feature = "server")]
                keep_alive: true,
                req_method: &mut None,
                title_case_headers: true,
                #[cfg(feature = "server")]
                date_header: true,
            },
            &mut vec,
        )
        .unwrap();

        assert_eq!(
            &*vec,
            b"GET / HTTP/1.1\r\nCONTENT-LENGTH: 10\r\nContent-Type: application/json\r\n\r\n"
                .as_ref(),
        );
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_server_encode_connect_method() {
        let mut head = MessageHead::default();

        let mut vec = Vec::new();
        let encoder = Server::encode(
            Encode {
                head: &mut head,
                body: None,
                keep_alive: true,
                req_method: &mut Some(Method::CONNECT),
                title_case_headers: false,
                date_header: true,
            },
            &mut vec,
        )
        .unwrap();

        assert!(encoder.is_last());
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_server_response_encode_title_case() {
        use crate::proto::BodyLength;
        use http::header::HeaderValue;

        let mut head = MessageHead::default();
        head.headers
            .insert("content-length", HeaderValue::from_static("10"));
        head.headers
            .insert("content-type", HeaderValue::from_static("application/json"));
        head.headers
            .insert("weird--header", HeaderValue::from_static(""));

        let mut vec = Vec::new();
        Server::encode(
            Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                keep_alive: true,
                req_method: &mut None,
                title_case_headers: true,
                date_header: true,
            },
            &mut vec,
        )
        .unwrap();

        let expected_response =
            b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nContent-Type: application/json\r\nWeird--Header: \r\n";

        assert_eq!(&vec[..expected_response.len()], &expected_response[..]);
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_server_response_encode_orig_case() {
        use crate::proto::BodyLength;
        use http::header::{HeaderValue, CONTENT_LENGTH};

        let mut head = MessageHead::default();
        head.headers
            .insert("content-length", HeaderValue::from_static("10"));
        head.headers
            .insert("content-type", HeaderValue::from_static("application/json"));

        let mut orig_headers = HeaderCaseMap::default();
        orig_headers.insert(CONTENT_LENGTH, "CONTENT-LENGTH".into());
        head.extensions.insert(orig_headers);

        let mut vec = Vec::new();
        Server::encode(
            Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                keep_alive: true,
                req_method: &mut None,
                title_case_headers: false,
                date_header: true,
            },
            &mut vec,
        )
        .unwrap();

        let expected_response =
            b"HTTP/1.1 200 OK\r\nCONTENT-LENGTH: 10\r\ncontent-type: application/json\r\ndate: ";

        assert_eq!(&vec[..expected_response.len()], &expected_response[..]);
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_server_response_encode_orig_and_title_case() {
        use crate::proto::BodyLength;
        use http::header::{HeaderValue, CONTENT_LENGTH};

        let mut head = MessageHead::default();
        head.headers
            .insert("content-length", HeaderValue::from_static("10"));
        head.headers
            .insert("content-type", HeaderValue::from_static("application/json"));

        let mut orig_headers = HeaderCaseMap::default();
        orig_headers.insert(CONTENT_LENGTH, "CONTENT-LENGTH".into());
        head.extensions.insert(orig_headers);

        let mut vec = Vec::new();
        Server::encode(
            Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                keep_alive: true,
                req_method: &mut None,
                title_case_headers: true,
                date_header: true,
            },
            &mut vec,
        )
        .unwrap();

        // this will also test that the date does exist
        let expected_response =
            b"HTTP/1.1 200 OK\r\nCONTENT-LENGTH: 10\r\nContent-Type: application/json\r\nDate: ";

        assert_eq!(&vec[..expected_response.len()], &expected_response[..]);
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_disabled_date_header() {
        use crate::proto::BodyLength;
        use http::header::{HeaderValue, CONTENT_LENGTH};

        let mut head = MessageHead::default();
        head.headers
            .insert("content-length", HeaderValue::from_static("10"));
        head.headers
            .insert("content-type", HeaderValue::from_static("application/json"));

        let mut orig_headers = HeaderCaseMap::default();
        orig_headers.insert(CONTENT_LENGTH, "CONTENT-LENGTH".into());
        head.extensions.insert(orig_headers);

        let mut vec = Vec::new();
        Server::encode(
            Encode {
                head: &mut head,
                body: Some(BodyLength::Known(10)),
                keep_alive: true,
                req_method: &mut None,
                title_case_headers: true,
                date_header: false,
            },
            &mut vec,
        )
        .unwrap();

        let expected_response =
            b"HTTP/1.1 200 OK\r\nCONTENT-LENGTH: 10\r\nContent-Type: application/json\r\n\r\n";

        assert_eq!(&vec, &expected_response);
    }

    #[test]
    fn parse_header_htabs() {
        let mut bytes = BytesMut::from("HTTP/1.1 200 OK\r\nserver: hello\tworld\r\n\r\n");
        let parsed = Client::parse(
            &mut bytes,
            ParseContext {
                cached_headers: &mut None,
                req_method: &mut Some(Method::GET),
                h1_parser_config: Default::default(),
                h1_max_headers: None,
                preserve_header_case: false,
                #[cfg(feature = "ffi")]
                preserve_header_order: false,
                h09_responses: false,
                #[cfg(feature = "client")]
                on_informational: &mut None,
            },
        )
        .expect("parse ok")
        .expect("parse complete");

        assert_eq!(parsed.head.headers["server"], "hello\tworld");
    }

    #[cfg(feature = "server")]
    #[test]
    fn parse_too_large_headers() {
        fn gen_req_with_headers(num: usize) -> String {
            let mut req = String::from("GET / HTTP/1.1\r\n");
            for i in 0..num {
                req.push_str(&format!("key{i}: val{i}\r\n"));
            }
            req.push_str("\r\n");
            req
        }
        fn gen_resp_with_headers(num: usize) -> String {
            let mut req = String::from("HTTP/1.1 200 OK\r\n");
            for i in 0..num {
                req.push_str(&format!("key{i}: val{i}\r\n"));
            }
            req.push_str("\r\n");
            req
        }
        fn parse(max_headers: Option<usize>, gen_size: usize, should_success: bool) {
            {
                // server side
                let mut bytes = BytesMut::from(gen_req_with_headers(gen_size).as_str());
                let result = Server::parse(
                    &mut bytes,
                    ParseContext {
                        cached_headers: &mut None,
                        req_method: &mut None,
                        h1_parser_config: Default::default(),
                        h1_max_headers: max_headers,
                        preserve_header_case: false,
                        #[cfg(feature = "ffi")]
                        preserve_header_order: false,
                        h09_responses: false,
                        #[cfg(feature = "client")]
                        on_informational: &mut None,
                    },
                );
                if should_success {
                    result.expect("parse ok").expect("parse complete");
                } else {
                    result.expect_err("parse should err");
                }
            }
            {
                // client side
                let mut bytes = BytesMut::from(gen_resp_with_headers(gen_size).as_str());
                let result = Client::parse(
                    &mut bytes,
                    ParseContext {
                        cached_headers: &mut None,
                        req_method: &mut None,
                        h1_parser_config: Default::default(),
                        h1_max_headers: max_headers,
                        preserve_header_case: false,
                        #[cfg(feature = "ffi")]
                        preserve_header_order: false,
                        h09_responses: false,
                        #[cfg(feature = "client")]
                        on_informational: &mut None,
                    },
                );
                if should_success {
                    result.expect("parse ok").expect("parse complete");
                } else {
                    result.expect_err("parse should err");
                }
            }
        }

        // check generator
        assert_eq!(
            gen_req_with_headers(0),
            String::from("GET / HTTP/1.1\r\n\r\n")
        );
        assert_eq!(
            gen_req_with_headers(1),
            String::from("GET / HTTP/1.1\r\nkey0: val0\r\n\r\n")
        );
        assert_eq!(
            gen_req_with_headers(2),
            String::from("GET / HTTP/1.1\r\nkey0: val0\r\nkey1: val1\r\n\r\n")
        );
        assert_eq!(
            gen_req_with_headers(3),
            String::from("GET / HTTP/1.1\r\nkey0: val0\r\nkey1: val1\r\nkey2: val2\r\n\r\n")
        );

        // default max_headers is 100, so
        //
        // - less than or equal to 100, accepted
        //
        parse(None, 0, true);
        parse(None, 1, true);
        parse(None, 50, true);
        parse(None, 99, true);
        parse(None, 100, true);
        //
        // - more than 100, rejected
        //
        parse(None, 101, false);
        parse(None, 102, false);
        parse(None, 200, false);

        // max_headers is 0, parser will reject any headers
        //
        // - without header, accepted
        //
        parse(Some(0), 0, true);
        //
        // - with header(s), rejected
        //
        parse(Some(0), 1, false);
        parse(Some(0), 100, false);

        // max_headers is 200
        //
        // - less than or equal to 200, accepted
        //
        parse(Some(200), 0, true);
        parse(Some(200), 1, true);
        parse(Some(200), 100, true);
        parse(Some(200), 200, true);
        //
        // - more than 200, rejected
        //
        parse(Some(200), 201, false);
        parse(Some(200), 210, false);
    }

    #[test]
    fn test_is_complete_fast() {
        let s = b"GET / HTTP/1.1\r\na: b\r\n\r\n";
        for n in 0..s.len() {
            assert!(is_complete_fast(s, n), "{:?}; {}", s, n);
        }
        let s = b"GET / HTTP/1.1\na: b\n\n";
        for n in 0..s.len() {
            assert!(is_complete_fast(s, n));
        }

        // Not
        let s = b"GET / HTTP/1.1\r\na: b\r\n\r";
        for n in 0..s.len() {
            assert!(!is_complete_fast(s, n));
        }
        let s = b"GET / HTTP/1.1\na: b\n";
        for n in 0..s.len() {
            assert!(!is_complete_fast(s, n));
        }
    }

    #[test]
    fn test_write_headers_orig_case_empty_value() {
        let mut headers = HeaderMap::new();
        let name = http::header::HeaderName::from_static("x-empty");
        headers.insert(&name, "".parse().expect("parse empty"));
        let mut orig_cases = HeaderCaseMap::default();
        orig_cases.insert(name, Bytes::from_static(b"X-EmptY"));

        let mut dst = Vec::new();
        super::write_headers_original_case(&headers, &orig_cases, &mut dst, false);

        assert_eq!(
            dst, b"X-EmptY:\r\n",
            "there should be no space between the colon and CRLF"
        );
    }

    #[test]
    fn test_write_headers_orig_case_multiple_entries() {
        let mut headers = HeaderMap::new();
        let name = http::header::HeaderName::from_static("x-empty");
        headers.insert(&name, "a".parse().unwrap());
        headers.append(&name, "b".parse().unwrap());

        let mut orig_cases = HeaderCaseMap::default();
        orig_cases.insert(name.clone(), Bytes::from_static(b"X-Empty"));
        orig_cases.append(name, Bytes::from_static(b"X-EMPTY"));

        let mut dst = Vec::new();
        super::write_headers_original_case(&headers, &orig_cases, &mut dst, false);

        assert_eq!(dst, b"X-Empty: a\r\nX-EMPTY: b\r\n");
    }

    #[cfg(feature = "nightly")]
    use test::Bencher;

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_parse_incoming(b: &mut Bencher) {
        let mut raw = BytesMut::from(
            &b"GET /super_long_uri/and_whatever?what_should_we_talk_about/\
            I_wonder/Hard_to_write_in_an_uri_after_all/you_have_to_make\
            _up_the_punctuation_yourself/how_fun_is_that?test=foo&test1=\
            foo1&test2=foo2&test3=foo3&test4=foo4 HTTP/1.1\r\nHost: \
            hyper.rs\r\nAccept: a lot of things\r\nAccept-Charset: \
            utf8\r\nAccept-Encoding: *\r\nAccess-Control-Allow-\
            Credentials: None\r\nAccess-Control-Allow-Origin: None\r\n\
            Access-Control-Allow-Methods: None\r\nAccess-Control-Allow-\
            Headers: None\r\nContent-Encoding: utf8\r\nContent-Security-\
            Policy: None\r\nContent-Type: text/html\r\nOrigin: hyper\
            \r\nSec-Websocket-Extensions: It looks super important!\r\n\
            Sec-Websocket-Origin: hyper\r\nSec-Websocket-Version: 4.3\r\
            \nStrict-Transport-Security: None\r\nUser-Agent: hyper\r\n\
            X-Content-Duration: None\r\nX-Content-Security-Policy: None\
            \r\nX-DNSPrefetch-Control: None\r\nX-Frame-Options: \
            Something important obviously\r\nX-Requested-With: Nothing\
            \r\n\r\n"[..],
        );
        let len = raw.len();
        let mut headers = Some(HeaderMap::new());

        b.bytes = len as u64;
        b.iter(|| {
            let mut msg = Server::parse(
                &mut raw,
                ParseContext {
                    cached_headers: &mut headers,
                    req_method: &mut None,
                    h1_parser_config: Default::default(),
                    h1_max_headers: None,
                    preserve_header_case: false,
                    #[cfg(feature = "ffi")]
                    preserve_header_order: false,
                    h09_responses: false,
                    #[cfg(feature = "client")]
                    on_informational: &mut None,
                },
            )
            .unwrap()
            .unwrap();
            ::test::black_box(&msg);

            // Remove all references pointing into BytesMut.
            msg.head.headers.clear();
            headers = Some(msg.head.headers);
            std::mem::take(&mut msg.head.subject);

            restart(&mut raw, len);
        });

        fn restart(b: &mut BytesMut, len: usize) {
            b.reserve(1);
            unsafe {
                b.set_len(len);
            }
        }
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_parse_short(b: &mut Bencher) {
        let s = &b"GET / HTTP/1.1\r\nHost: localhost:8080\r\n\r\n"[..];
        let mut raw = BytesMut::from(s);
        let len = raw.len();
        let mut headers = Some(HeaderMap::new());

        b.bytes = len as u64;
        b.iter(|| {
            let mut msg = Server::parse(
                &mut raw,
                ParseContext {
                    cached_headers: &mut headers,
                    req_method: &mut None,
                    h1_parser_config: Default::default(),
                    h1_max_headers: None,
                    preserve_header_case: false,
                    #[cfg(feature = "ffi")]
                    preserve_header_order: false,
                    h09_responses: false,
                    #[cfg(feature = "client")]
                    on_informational: &mut None,
                },
            )
            .unwrap()
            .unwrap();
            ::test::black_box(&msg);
            msg.head.headers.clear();
            headers = Some(msg.head.headers);
            restart(&mut raw, len);
        });

        fn restart(b: &mut BytesMut, len: usize) {
            b.reserve(1);
            unsafe {
                b.set_len(len);
            }
        }
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_server_encode_headers_preset(b: &mut Bencher) {
        use crate::proto::BodyLength;
        use http::header::HeaderValue;

        let len = 108;
        b.bytes = len as u64;

        let mut head = MessageHead::default();
        let mut headers = HeaderMap::new();
        headers.insert("content-length", HeaderValue::from_static("10"));
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        b.iter(|| {
            let mut vec = Vec::new();
            head.headers = headers.clone();
            Server::encode(
                Encode {
                    head: &mut head,
                    body: Some(BodyLength::Known(10)),
                    keep_alive: true,
                    req_method: &mut Some(Method::GET),
                    title_case_headers: false,
                    date_header: true,
                },
                &mut vec,
            )
            .unwrap();
            assert_eq!(vec.len(), len);
            ::test::black_box(vec);
        })
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_server_encode_no_headers(b: &mut Bencher) {
        use crate::proto::BodyLength;

        let len = 76;
        b.bytes = len as u64;

        let mut head = MessageHead::default();
        let mut vec = Vec::with_capacity(128);

        b.iter(|| {
            Server::encode(
                Encode {
                    head: &mut head,
                    body: Some(BodyLength::Known(10)),
                    keep_alive: true,
                    req_method: &mut Some(Method::GET),
                    title_case_headers: false,
                    date_header: true,
                },
                &mut vec,
            )
            .unwrap();
            assert_eq!(vec.len(), len);
            ::test::black_box(&vec);

            vec.clear();
        })
    }
}
