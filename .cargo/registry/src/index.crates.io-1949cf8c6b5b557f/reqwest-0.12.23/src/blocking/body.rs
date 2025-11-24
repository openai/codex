use std::fmt;
use std::fs::File;
use std::future::Future;
#[cfg(feature = "multipart")]
use std::io::Cursor;
use std::io::{self, Read};
use std::mem::{self, MaybeUninit};
use std::ptr;

use bytes::Bytes;
use futures_channel::mpsc;

use crate::async_impl;

/// The body of a `Request`.
///
/// In most cases, this is not needed directly, as the
/// [`RequestBuilder.body`][builder] method uses `Into<Body>`, which allows
/// passing many things (like a string or vector of bytes).
///
/// [builder]: ./struct.RequestBuilder.html#method.body
#[derive(Debug)]
pub struct Body {
    kind: Kind,
}

impl Body {
    /// Instantiate a `Body` from a reader.
    ///
    /// # Note
    ///
    /// While allowing for many types to be used, these bodies do not have
    /// a way to reset to the beginning and be reused. This means that when
    /// encountering a 307 or 308 status code, instead of repeating the
    /// request at the new location, the `Response` will be returned with
    /// the redirect status code set.
    ///
    /// ```rust
    /// # use std::fs::File;
    /// # use reqwest::blocking::Body;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let file = File::open("national_secrets.txt")?;
    /// let body = Body::new(file);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// If you have a set of bytes, like `String` or `Vec<u8>`, using the
    /// `From` implementations for `Body` will store the data in a manner
    /// it can be reused.
    ///
    /// ```rust
    /// # use reqwest::blocking::Body;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let s = "A stringy body";
    /// let body = Body::from(s);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<R: Read + Send + 'static>(reader: R) -> Body {
        Body {
            kind: Kind::Reader(Box::from(reader), None),
        }
    }

    /// Create a `Body` from a `Read` where the size is known in advance
    /// but the data should not be fully loaded into memory. This will
    /// set the `Content-Length` header and stream from the `Read`.
    ///
    /// ```rust
    /// # use std::fs::File;
    /// # use reqwest::blocking::Body;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let file = File::open("a_large_file.txt")?;
    /// let file_size = file.metadata()?.len();
    /// let body = Body::sized(file, file_size);
    /// # Ok(())
    /// # }
    /// ```
    pub fn sized<R: Read + Send + 'static>(reader: R, len: u64) -> Body {
        Body {
            kind: Kind::Reader(Box::from(reader), Some(len)),
        }
    }

    /// Returns the body as a byte slice if the body is already buffered in
    /// memory. For streamed requests this method returns `None`.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self.kind {
            Kind::Reader(_, _) => None,
            Kind::Bytes(ref bytes) => Some(bytes.as_ref()),
        }
    }

    /// Converts streamed requests to their buffered equivalent and
    /// returns a reference to the buffer. If the request is already
    /// buffered, this has no effect.
    ///
    /// Be aware that for large requests this method is expensive
    /// and may cause your program to run out of memory.
    pub fn buffer(&mut self) -> Result<&[u8], crate::Error> {
        match self.kind {
            Kind::Reader(ref mut reader, maybe_len) => {
                let mut bytes = if let Some(len) = maybe_len {
                    Vec::with_capacity(len as usize)
                } else {
                    Vec::new()
                };
                io::copy(reader, &mut bytes).map_err(crate::error::builder)?;
                self.kind = Kind::Bytes(bytes.into());
                self.buffer()
            }
            Kind::Bytes(ref bytes) => Ok(bytes.as_ref()),
        }
    }

    #[cfg(feature = "multipart")]
    pub(crate) fn len(&self) -> Option<u64> {
        match self.kind {
            Kind::Reader(_, len) => len,
            Kind::Bytes(ref bytes) => Some(bytes.len() as u64),
        }
    }

    #[cfg(feature = "multipart")]
    pub(crate) fn into_reader(self) -> Reader {
        match self.kind {
            Kind::Reader(r, _) => Reader::Reader(r),
            Kind::Bytes(b) => Reader::Bytes(Cursor::new(b)),
        }
    }

    pub(crate) fn into_async(self) -> (Option<Sender>, async_impl::Body, Option<u64>) {
        match self.kind {
            Kind::Reader(read, len) => {
                let (tx, rx) = mpsc::channel(0);
                let tx = Sender {
                    body: (read, len),
                    tx,
                };
                (Some(tx), async_impl::Body::stream(rx), len)
            }
            Kind::Bytes(chunk) => {
                let len = chunk.len() as u64;
                (None, async_impl::Body::reusable(chunk), Some(len))
            }
        }
    }

    pub(crate) fn try_clone(&self) -> Option<Body> {
        self.kind.try_clone().map(|kind| Body { kind })
    }
}

enum Kind {
    Reader(Box<dyn Read + Send>, Option<u64>),
    Bytes(Bytes),
}

impl Kind {
    fn try_clone(&self) -> Option<Kind> {
        match self {
            Kind::Reader(..) => None,
            Kind::Bytes(v) => Some(Kind::Bytes(v.clone())),
        }
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from(v: Vec<u8>) -> Body {
        Body {
            kind: Kind::Bytes(v.into()),
        }
    }
}

impl From<String> for Body {
    #[inline]
    fn from(s: String) -> Body {
        s.into_bytes().into()
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from(s: &'static [u8]) -> Body {
        Body {
            kind: Kind::Bytes(Bytes::from_static(s)),
        }
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(s: &'static str) -> Body {
        s.as_bytes().into()
    }
}

impl From<File> for Body {
    #[inline]
    fn from(f: File) -> Body {
        let len = f.metadata().map(|m| m.len()).ok();
        Body {
            kind: Kind::Reader(Box::new(f), len),
        }
    }
}
impl From<Bytes> for Body {
    #[inline]
    fn from(b: Bytes) -> Body {
        Body {
            kind: Kind::Bytes(b),
        }
    }
}

impl fmt::Debug for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Kind::Reader(_, ref v) => f
                .debug_struct("Reader")
                .field("length", &DebugLength(v))
                .finish(),
            Kind::Bytes(ref v) => fmt::Debug::fmt(v, f),
        }
    }
}

struct DebugLength<'a>(&'a Option<u64>);

impl<'a> fmt::Debug for DebugLength<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self.0 {
            Some(ref len) => fmt::Debug::fmt(len, f),
            None => f.write_str("Unknown"),
        }
    }
}

#[cfg(feature = "multipart")]
pub(crate) enum Reader {
    Reader(Box<dyn Read + Send>),
    Bytes(Cursor<Bytes>),
}

#[cfg(feature = "multipart")]
impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match *self {
            Reader::Reader(ref mut rdr) => rdr.read(buf),
            Reader::Bytes(ref mut rdr) => rdr.read(buf),
        }
    }
}

pub(crate) struct Sender {
    body: (Box<dyn Read + Send>, Option<u64>),
    tx: mpsc::Sender<Result<Bytes, Abort>>,
}

#[derive(Debug)]
struct Abort;

impl fmt::Display for Abort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("abort request body")
    }
}

impl std::error::Error for Abort {}

async fn send_future(sender: Sender) -> Result<(), crate::Error> {
    use bytes::{BufMut, BytesMut};
    use futures_util::SinkExt;
    use std::cmp;

    let con_len = sender.body.1;
    let cap = cmp::min(sender.body.1.unwrap_or(8192), 8192);
    let mut written = 0;
    let mut buf = BytesMut::zeroed(cap as usize);
    buf.clear();
    let mut body = sender.body.0;
    // Put in an option so that it can be consumed on error to call abort()
    let mut tx = Some(sender.tx);

    loop {
        if Some(written) == con_len {
            // Written up to content-length, so stop.
            return Ok(());
        }

        // The input stream is read only if the buffer is empty so
        // that there is only one read in the buffer at any time.
        //
        // We need to know whether there is any data to send before
        // we check the transmission channel (with poll_ready below)
        // because sometimes the receiver disappears as soon as it
        // considers the data is completely transmitted, which may
        // be true.
        //
        // The use case is a web server that closes its
        // input stream as soon as the data received is valid JSON.
        // This behaviour is questionable, but it exists and the
        // fact is that there is actually no remaining data to read.
        if buf.is_empty() {
            if buf.capacity() == buf.len() {
                buf.reserve(8192);
                // zero out the reserved memory
                let uninit = buf.spare_capacity_mut();
                let uninit_len = uninit.len();
                unsafe {
                    ptr::write_bytes(uninit.as_mut_ptr().cast::<u8>(), 0, uninit_len);
                }
            }

            let bytes = unsafe {
                mem::transmute::<&mut [MaybeUninit<u8>], &mut [u8]>(buf.spare_capacity_mut())
            };
            match body.read(bytes) {
                Ok(0) => {
                    // The buffer was empty and nothing's left to
                    // read. Return.
                    return Ok(());
                }
                Ok(n) => unsafe {
                    buf.advance_mut(n);
                },
                Err(e) => {
                    let _ = tx
                        .take()
                        .expect("tx only taken on error")
                        .clone()
                        .try_send(Err(Abort));
                    return Err(crate::error::body(e));
                }
            }
        }

        // The only way to get here is when the buffer is not empty.
        // We can check the transmission channel

        let buf_len = buf.len() as u64;
        tx.as_mut()
            .expect("tx only taken on error")
            .send(Ok(buf.split().freeze()))
            .await
            .map_err(crate::error::body)?;

        written += buf_len;
    }
}

impl Sender {
    // A `Future` that may do blocking read calls.
    // As a `Future`, this integrates easily with `wait::timeout`.
    pub(crate) fn send(self) -> impl Future<Output = Result<(), crate::Error>> {
        send_future(self)
    }
}

// useful for tests, but not publicly exposed
#[cfg(test)]
pub(crate) fn read_to_string(mut body: Body) -> io::Result<String> {
    let mut s = String::new();
    match body.kind {
        Kind::Reader(ref mut reader, _) => reader.read_to_string(&mut s),
        Kind::Bytes(ref mut bytes) => (&**bytes).read_to_string(&mut s),
    }
    .map(|_| s)
}
