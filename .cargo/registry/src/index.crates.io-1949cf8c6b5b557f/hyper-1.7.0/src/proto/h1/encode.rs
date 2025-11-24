use std::collections::HashMap;
use std::fmt;
use std::io::IoSlice;

use bytes::buf::{Chain, Take};
use bytes::{Buf, Bytes};
use http::{
    header::{
        AUTHORIZATION, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE,
        CONTENT_TYPE, HOST, MAX_FORWARDS, SET_COOKIE, TE, TRAILER, TRANSFER_ENCODING,
    },
    HeaderMap, HeaderName, HeaderValue,
};

use super::io::WriteBuf;
use super::role::{write_headers, write_headers_title_case};

type StaticBuf = &'static [u8];

/// Encoders to handle different Transfer-Encodings.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Encoder {
    kind: Kind,
    is_last: bool,
}

#[derive(Debug)]
pub(crate) struct EncodedBuf<B> {
    kind: BufKind<B>,
}

#[derive(Debug)]
pub(crate) struct NotEof(u64);

#[derive(Debug, PartialEq, Clone)]
enum Kind {
    /// An Encoder for when Transfer-Encoding includes `chunked`.
    Chunked(Option<Vec<HeaderValue>>),
    /// An Encoder for when Content-Length is set.
    ///
    /// Enforces that the body is not longer than the Content-Length header.
    Length(u64),
    /// An Encoder for when neither Content-Length nor Chunked encoding is set.
    ///
    /// This is mostly only used with HTTP/1.0 with a length. This kind requires
    /// the connection to be closed when the body is finished.
    #[cfg(feature = "server")]
    CloseDelimited,
}

#[derive(Debug)]
enum BufKind<B> {
    Exact(B),
    Limited(Take<B>),
    Chunked(Chain<Chain<ChunkSize, B>, StaticBuf>),
    ChunkedEnd(StaticBuf),
    Trailers(Chain<Chain<StaticBuf, Bytes>, StaticBuf>),
}

impl Encoder {
    fn new(kind: Kind) -> Encoder {
        Encoder {
            kind,
            is_last: false,
        }
    }
    pub(crate) fn chunked() -> Encoder {
        Encoder::new(Kind::Chunked(None))
    }

    pub(crate) fn length(len: u64) -> Encoder {
        Encoder::new(Kind::Length(len))
    }

    #[cfg(feature = "server")]
    pub(crate) fn close_delimited() -> Encoder {
        Encoder::new(Kind::CloseDelimited)
    }

    pub(crate) fn into_chunked_with_trailing_fields(self, trailers: Vec<HeaderValue>) -> Encoder {
        match self.kind {
            Kind::Chunked(_) => Encoder {
                kind: Kind::Chunked(Some(trailers)),
                is_last: self.is_last,
            },
            _ => self,
        }
    }

    pub(crate) fn is_eof(&self) -> bool {
        matches!(self.kind, Kind::Length(0))
    }

    #[cfg(feature = "server")]
    pub(crate) fn set_last(mut self, is_last: bool) -> Self {
        self.is_last = is_last;
        self
    }

    pub(crate) fn is_last(&self) -> bool {
        self.is_last
    }

    pub(crate) fn is_close_delimited(&self) -> bool {
        match self.kind {
            #[cfg(feature = "server")]
            Kind::CloseDelimited => true,
            _ => false,
        }
    }

    pub(crate) fn is_chunked(&self) -> bool {
        matches!(self.kind, Kind::Chunked(_))
    }

    pub(crate) fn end<B>(&self) -> Result<Option<EncodedBuf<B>>, NotEof> {
        match self.kind {
            Kind::Length(0) => Ok(None),
            Kind::Chunked(_) => Ok(Some(EncodedBuf {
                kind: BufKind::ChunkedEnd(b"0\r\n\r\n"),
            })),
            #[cfg(feature = "server")]
            Kind::CloseDelimited => Ok(None),
            Kind::Length(n) => Err(NotEof(n)),
        }
    }

    pub(crate) fn encode<B>(&mut self, msg: B) -> EncodedBuf<B>
    where
        B: Buf,
    {
        let len = msg.remaining();
        debug_assert!(len > 0, "encode() called with empty buf");

        let kind = match self.kind {
            Kind::Chunked(_) => {
                trace!("encoding chunked {}B", len);
                let buf = ChunkSize::new(len)
                    .chain(msg)
                    .chain(b"\r\n" as &'static [u8]);
                BufKind::Chunked(buf)
            }
            Kind::Length(ref mut remaining) => {
                trace!("sized write, len = {}", len);
                if len as u64 > *remaining {
                    let limit = *remaining as usize;
                    *remaining = 0;
                    BufKind::Limited(msg.take(limit))
                } else {
                    *remaining -= len as u64;
                    BufKind::Exact(msg)
                }
            }
            #[cfg(feature = "server")]
            Kind::CloseDelimited => {
                trace!("close delimited write {}B", len);
                BufKind::Exact(msg)
            }
        };
        EncodedBuf { kind }
    }

    pub(crate) fn encode_trailers<B>(
        &self,
        trailers: HeaderMap,
        title_case_headers: bool,
    ) -> Option<EncodedBuf<B>> {
        trace!("encoding trailers");
        match &self.kind {
            Kind::Chunked(Some(allowed_trailer_fields)) => {
                let allowed_trailer_field_map = allowed_trailer_field_map(allowed_trailer_fields);

                let mut cur_name = None;
                let mut allowed_trailers = HeaderMap::new();

                for (opt_name, value) in trailers {
                    if let Some(n) = opt_name {
                        cur_name = Some(n);
                    }
                    let name = cur_name.as_ref().expect("current header name");

                    if allowed_trailer_field_map.contains_key(name.as_str()) {
                        if is_valid_trailer_field(name) {
                            allowed_trailers.insert(name, value);
                        } else {
                            debug!("trailer field is not valid: {}", &name);
                        }
                    } else {
                        debug!("trailer header name not found in trailer header: {}", &name);
                    }
                }

                let mut buf = Vec::new();
                if title_case_headers {
                    write_headers_title_case(&allowed_trailers, &mut buf);
                } else {
                    write_headers(&allowed_trailers, &mut buf);
                }

                if buf.is_empty() {
                    return None;
                }

                Some(EncodedBuf {
                    kind: BufKind::Trailers(b"0\r\n".chain(Bytes::from(buf)).chain(b"\r\n")),
                })
            }
            Kind::Chunked(None) => {
                debug!("attempted to encode trailers, but the trailer header is not set");
                None
            }
            _ => {
                debug!("attempted to encode trailers for non-chunked response");
                None
            }
        }
    }

    pub(super) fn encode_and_end<B>(&self, msg: B, dst: &mut WriteBuf<EncodedBuf<B>>) -> bool
    where
        B: Buf,
    {
        let len = msg.remaining();
        debug_assert!(len > 0, "encode() called with empty buf");

        match self.kind {
            Kind::Chunked(_) => {
                trace!("encoding chunked {}B", len);
                let buf = ChunkSize::new(len)
                    .chain(msg)
                    .chain(b"\r\n0\r\n\r\n" as &'static [u8]);
                dst.buffer(buf);
                !self.is_last
            }
            Kind::Length(remaining) => {
                use std::cmp::Ordering;

                trace!("sized write, len = {}", len);
                match (len as u64).cmp(&remaining) {
                    Ordering::Equal => {
                        dst.buffer(msg);
                        !self.is_last
                    }
                    Ordering::Greater => {
                        dst.buffer(msg.take(remaining as usize));
                        !self.is_last
                    }
                    Ordering::Less => {
                        dst.buffer(msg);
                        false
                    }
                }
            }
            #[cfg(feature = "server")]
            Kind::CloseDelimited => {
                trace!("close delimited write {}B", len);
                dst.buffer(msg);
                false
            }
        }
    }
}

fn is_valid_trailer_field(name: &HeaderName) -> bool {
    !matches!(
        *name,
        AUTHORIZATION
            | CACHE_CONTROL
            | CONTENT_ENCODING
            | CONTENT_LENGTH
            | CONTENT_RANGE
            | CONTENT_TYPE
            | HOST
            | MAX_FORWARDS
            | SET_COOKIE
            | TRAILER
            | TRANSFER_ENCODING
            | TE
    )
}

fn allowed_trailer_field_map(allowed_trailer_fields: &Vec<HeaderValue>) -> HashMap<String, ()> {
    let mut trailer_map = HashMap::new();

    for header_value in allowed_trailer_fields {
        if let Ok(header_str) = header_value.to_str() {
            let items: Vec<&str> = header_str.split(',').map(|item| item.trim()).collect();

            for item in items {
                trailer_map.entry(item.to_string()).or_insert(());
            }
        }
    }

    trailer_map
}

impl<B> Buf for EncodedBuf<B>
where
    B: Buf,
{
    #[inline]
    fn remaining(&self) -> usize {
        match self.kind {
            BufKind::Exact(ref b) => b.remaining(),
            BufKind::Limited(ref b) => b.remaining(),
            BufKind::Chunked(ref b) => b.remaining(),
            BufKind::ChunkedEnd(ref b) => b.remaining(),
            BufKind::Trailers(ref b) => b.remaining(),
        }
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        match self.kind {
            BufKind::Exact(ref b) => b.chunk(),
            BufKind::Limited(ref b) => b.chunk(),
            BufKind::Chunked(ref b) => b.chunk(),
            BufKind::ChunkedEnd(ref b) => b.chunk(),
            BufKind::Trailers(ref b) => b.chunk(),
        }
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        match self.kind {
            BufKind::Exact(ref mut b) => b.advance(cnt),
            BufKind::Limited(ref mut b) => b.advance(cnt),
            BufKind::Chunked(ref mut b) => b.advance(cnt),
            BufKind::ChunkedEnd(ref mut b) => b.advance(cnt),
            BufKind::Trailers(ref mut b) => b.advance(cnt),
        }
    }

    #[inline]
    fn chunks_vectored<'t>(&'t self, dst: &mut [IoSlice<'t>]) -> usize {
        match self.kind {
            BufKind::Exact(ref b) => b.chunks_vectored(dst),
            BufKind::Limited(ref b) => b.chunks_vectored(dst),
            BufKind::Chunked(ref b) => b.chunks_vectored(dst),
            BufKind::ChunkedEnd(ref b) => b.chunks_vectored(dst),
            BufKind::Trailers(ref b) => b.chunks_vectored(dst),
        }
    }
}

#[cfg(target_pointer_width = "32")]
const USIZE_BYTES: usize = 4;

#[cfg(target_pointer_width = "64")]
const USIZE_BYTES: usize = 8;

// each byte will become 2 hex
const CHUNK_SIZE_MAX_BYTES: usize = USIZE_BYTES * 2;

#[derive(Clone, Copy)]
struct ChunkSize {
    bytes: [u8; CHUNK_SIZE_MAX_BYTES + 2],
    pos: u8,
    len: u8,
}

impl ChunkSize {
    fn new(len: usize) -> ChunkSize {
        use std::fmt::Write;
        let mut size = ChunkSize {
            bytes: [0; CHUNK_SIZE_MAX_BYTES + 2],
            pos: 0,
            len: 0,
        };
        write!(&mut size, "{:X}\r\n", len).expect("CHUNK_SIZE_MAX_BYTES should fit any usize");
        size
    }
}

impl Buf for ChunkSize {
    #[inline]
    fn remaining(&self) -> usize {
        (self.len - self.pos).into()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        &self.bytes[self.pos.into()..self.len.into()]
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.remaining());
        self.pos += cnt as u8; // just asserted cnt fits in u8
    }
}

impl fmt::Debug for ChunkSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkSize")
            .field("bytes", &&self.bytes[..self.len.into()])
            .field("pos", &self.pos)
            .finish()
    }
}

impl fmt::Write for ChunkSize {
    fn write_str(&mut self, num: &str) -> fmt::Result {
        use std::io::Write;
        (&mut self.bytes[self.len.into()..])
            .write_all(num.as_bytes())
            .expect("&mut [u8].write() cannot error");
        self.len += num.len() as u8; // safe because bytes is never bigger than 256
        Ok(())
    }
}

impl<B: Buf> From<B> for EncodedBuf<B> {
    fn from(buf: B) -> Self {
        EncodedBuf {
            kind: BufKind::Exact(buf),
        }
    }
}

impl<B: Buf> From<Take<B>> for EncodedBuf<B> {
    fn from(buf: Take<B>) -> Self {
        EncodedBuf {
            kind: BufKind::Limited(buf),
        }
    }
}

impl<B: Buf> From<Chain<Chain<ChunkSize, B>, StaticBuf>> for EncodedBuf<B> {
    fn from(buf: Chain<Chain<ChunkSize, B>, StaticBuf>) -> Self {
        EncodedBuf {
            kind: BufKind::Chunked(buf),
        }
    }
}

impl fmt::Display for NotEof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "early end, expected {} more bytes", self.0)
    }
}

impl std::error::Error for NotEof {}

#[cfg(test)]
mod tests {
    use bytes::BufMut;
    use http::{
        header::{
            AUTHORIZATION, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE,
            CONTENT_TYPE, HOST, MAX_FORWARDS, SET_COOKIE, TE, TRAILER, TRANSFER_ENCODING,
        },
        HeaderMap, HeaderName, HeaderValue,
    };

    use super::super::io::Cursor;
    use super::Encoder;

    #[test]
    fn chunked() {
        let mut encoder = Encoder::chunked();
        let mut dst = Vec::new();

        let msg1 = b"foo bar".as_ref();
        let buf1 = encoder.encode(msg1);
        dst.put(buf1);
        assert_eq!(dst, b"7\r\nfoo bar\r\n");

        let msg2 = b"baz quux herp".as_ref();
        let buf2 = encoder.encode(msg2);
        dst.put(buf2);

        assert_eq!(dst, b"7\r\nfoo bar\r\nD\r\nbaz quux herp\r\n");

        let end = encoder.end::<Cursor<Vec<u8>>>().unwrap().unwrap();
        dst.put(end);

        assert_eq!(
            dst,
            b"7\r\nfoo bar\r\nD\r\nbaz quux herp\r\n0\r\n\r\n".as_ref()
        );
    }

    #[test]
    fn length() {
        let max_len = 8;
        let mut encoder = Encoder::length(max_len as u64);
        let mut dst = Vec::new();

        let msg1 = b"foo bar".as_ref();
        let buf1 = encoder.encode(msg1);
        dst.put(buf1);

        assert_eq!(dst, b"foo bar");
        assert!(!encoder.is_eof());
        encoder.end::<()>().unwrap_err();

        let msg2 = b"baz".as_ref();
        let buf2 = encoder.encode(msg2);
        dst.put(buf2);

        assert_eq!(dst.len(), max_len);
        assert_eq!(dst, b"foo barb");
        assert!(encoder.is_eof());
        assert!(encoder.end::<()>().unwrap().is_none());
    }

    #[cfg(feature = "server")]
    #[test]
    fn eof() {
        let mut encoder = Encoder::close_delimited();
        let mut dst = Vec::new();

        let msg1 = b"foo bar".as_ref();
        let buf1 = encoder.encode(msg1);
        dst.put(buf1);

        assert_eq!(dst, b"foo bar");
        assert!(!encoder.is_eof());
        encoder.end::<()>().unwrap();

        let msg2 = b"baz".as_ref();
        let buf2 = encoder.encode(msg2);
        dst.put(buf2);

        assert_eq!(dst, b"foo barbaz");
        assert!(!encoder.is_eof());
        encoder.end::<()>().unwrap();
    }

    #[test]
    fn chunked_with_valid_trailers() {
        let encoder = Encoder::chunked();
        let trailers = vec![HeaderValue::from_static("chunky-trailer")];
        let encoder = encoder.into_chunked_with_trailing_fields(trailers);

        let headers = HeaderMap::from_iter(vec![
            (
                HeaderName::from_static("chunky-trailer"),
                HeaderValue::from_static("header data"),
            ),
            (
                HeaderName::from_static("should-not-be-included"),
                HeaderValue::from_static("oops"),
            ),
        ]);

        let buf1 = encoder.encode_trailers::<&[u8]>(headers, false).unwrap();

        let mut dst = Vec::new();
        dst.put(buf1);
        assert_eq!(dst, b"0\r\nchunky-trailer: header data\r\n\r\n");
    }

    #[test]
    fn chunked_with_multiple_trailer_headers() {
        let encoder = Encoder::chunked();
        let trailers = vec![
            HeaderValue::from_static("chunky-trailer"),
            HeaderValue::from_static("chunky-trailer-2"),
        ];
        let encoder = encoder.into_chunked_with_trailing_fields(trailers);

        let headers = HeaderMap::from_iter(vec![
            (
                HeaderName::from_static("chunky-trailer"),
                HeaderValue::from_static("header data"),
            ),
            (
                HeaderName::from_static("chunky-trailer-2"),
                HeaderValue::from_static("more header data"),
            ),
        ]);

        let buf1 = encoder.encode_trailers::<&[u8]>(headers, false).unwrap();

        let mut dst = Vec::new();
        dst.put(buf1);
        assert_eq!(
            dst,
            b"0\r\nchunky-trailer: header data\r\nchunky-trailer-2: more header data\r\n\r\n"
        );
    }

    #[test]
    fn chunked_with_no_trailer_header() {
        let encoder = Encoder::chunked();

        let headers = HeaderMap::from_iter(vec![(
            HeaderName::from_static("chunky-trailer"),
            HeaderValue::from_static("header data"),
        )]);

        assert!(encoder
            .encode_trailers::<&[u8]>(headers.clone(), false)
            .is_none());

        let trailers = vec![];
        let encoder = encoder.into_chunked_with_trailing_fields(trailers);

        assert!(encoder.encode_trailers::<&[u8]>(headers, false).is_none());
    }

    #[test]
    fn chunked_with_invalid_trailers() {
        let encoder = Encoder::chunked();

        let trailers = format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            AUTHORIZATION,
            CACHE_CONTROL,
            CONTENT_ENCODING,
            CONTENT_LENGTH,
            CONTENT_RANGE,
            CONTENT_TYPE,
            HOST,
            MAX_FORWARDS,
            SET_COOKIE,
            TRAILER,
            TRANSFER_ENCODING,
            TE,
        );
        let trailers = vec![HeaderValue::from_str(&trailers).unwrap()];
        let encoder = encoder.into_chunked_with_trailing_fields(trailers);

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("header data"));
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("header data"));
        headers.insert(CONTENT_ENCODING, HeaderValue::from_static("header data"));
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("header data"));
        headers.insert(CONTENT_RANGE, HeaderValue::from_static("header data"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("header data"));
        headers.insert(HOST, HeaderValue::from_static("header data"));
        headers.insert(MAX_FORWARDS, HeaderValue::from_static("header data"));
        headers.insert(SET_COOKIE, HeaderValue::from_static("header data"));
        headers.insert(TRAILER, HeaderValue::from_static("header data"));
        headers.insert(TRANSFER_ENCODING, HeaderValue::from_static("header data"));
        headers.insert(TE, HeaderValue::from_static("header data"));

        assert!(encoder.encode_trailers::<&[u8]>(headers, true).is_none());
    }

    #[test]
    fn chunked_with_title_case_headers() {
        let encoder = Encoder::chunked();
        let trailers = vec![HeaderValue::from_static("chunky-trailer")];
        let encoder = encoder.into_chunked_with_trailing_fields(trailers);

        let headers = HeaderMap::from_iter(vec![(
            HeaderName::from_static("chunky-trailer"),
            HeaderValue::from_static("header data"),
        )]);
        let buf1 = encoder.encode_trailers::<&[u8]>(headers, true).unwrap();

        let mut dst = Vec::new();
        dst.put(buf1);
        assert_eq!(dst, b"0\r\nChunky-Trailer: header data\r\n\r\n");
    }
}
