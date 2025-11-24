mod v5;
pub use v5::{SocksV5, SocksV5Error};

mod v4;
pub use v4::{SocksV4, SocksV4Error};

use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BytesMut;

use hyper::rt::Read;

#[derive(Debug)]
pub enum SocksError<C> {
    Inner(C),
    Io(std::io::Error),

    DnsFailure,
    MissingHost,
    MissingPort,

    V4(SocksV4Error),
    V5(SocksV5Error),

    Parsing(ParsingError),
    Serialize(SerializeError),
}

#[derive(Debug)]
pub enum ParsingError {
    Incomplete,
    WouldOverflow,
    Other,
}

#[derive(Debug)]
pub enum SerializeError {
    WouldOverflow,
}

async fn read_message<T, M, C>(mut conn: &mut T, buf: &mut BytesMut) -> Result<M, SocksError<C>>
where
    T: Read + Unpin,
    M: for<'a> TryFrom<&'a mut BytesMut, Error = ParsingError>,
{
    let mut tmp = [0; 513];

    loop {
        let n = crate::rt::read(&mut conn, &mut tmp).await?;
        buf.extend_from_slice(&tmp[..n]);

        match M::try_from(buf) {
            Err(ParsingError::Incomplete) => {
                if n == 0 {
                    if buf.spare_capacity_mut().is_empty() {
                        return Err(SocksError::Parsing(ParsingError::WouldOverflow));
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "unexpected eof",
                        )
                        .into());
                    }
                }
            }
            Err(err) => return Err(err.into()),
            Ok(res) => return Ok(res),
        }
    }
}

impl<C> std::fmt::Display for SocksError<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SOCKS error: ")?;

        match self {
            Self::Inner(_) => f.write_str("failed to create underlying connection"),
            Self::Io(_) => f.write_str("io error during SOCKS handshake"),

            Self::DnsFailure => f.write_str("could not resolve to acceptable address type"),
            Self::MissingHost => f.write_str("missing destination host"),
            Self::MissingPort => f.write_str("missing destination port"),

            Self::Parsing(_) => f.write_str("failed parsing server response"),
            Self::Serialize(_) => f.write_str("failed serialize request"),

            Self::V4(e) => e.fmt(f),
            Self::V5(e) => e.fmt(f),
        }
    }
}

impl<C: std::fmt::Debug + std::fmt::Display> std::error::Error for SocksError<C> {}

impl<C> From<std::io::Error> for SocksError<C> {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl<C> From<ParsingError> for SocksError<C> {
    fn from(err: ParsingError) -> Self {
        Self::Parsing(err)
    }
}

impl<C> From<SerializeError> for SocksError<C> {
    fn from(err: SerializeError) -> Self {
        Self::Serialize(err)
    }
}

impl<C> From<SocksV4Error> for SocksError<C> {
    fn from(err: SocksV4Error) -> Self {
        Self::V4(err)
    }
}

impl<C> From<SocksV5Error> for SocksError<C> {
    fn from(err: SocksV5Error) -> Self {
        Self::V5(err)
    }
}

pin_project! {
    // Not publicly exported (so missing_docs doesn't trigger).
    //
    // We return this `Future` instead of the `Pin<Box<dyn Future>>` directly
    // so that users don't rely on it fitting in a `Pin<Box<dyn Future>>` slot
    // (and thus we can change the type in the future).
    #[must_use = "futures do nothing unless polled"]
    #[allow(missing_debug_implementations)]
    pub struct Handshaking<F, T, E> {
        #[pin]
        fut: BoxHandshaking<T, E>,
        _marker: std::marker::PhantomData<F>
    }
}

type BoxHandshaking<T, E> = Pin<Box<dyn Future<Output = Result<T, SocksError<E>>> + Send>>;

impl<F, T, E> Future for Handshaking<F, T, E>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, SocksError<E>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().fut.poll(cx)
    }
}
