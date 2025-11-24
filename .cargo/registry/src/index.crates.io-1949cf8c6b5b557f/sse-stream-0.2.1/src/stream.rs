use std::{
    collections::VecDeque,
    num::ParseIntError,
    str::Utf8Error,
    task::{ready, Context, Poll},
};

use crate::Sse;
use bytes::Buf;
use futures_util::{stream::MapOk, Stream, TryStreamExt};
use http_body::{Body, Frame};
use http_body_util::{BodyDataStream, StreamBody};

pin_project_lite::pin_project! {
    pub struct SseStream<B: Body> {
        #[pin]
        body: BodyDataStream<B>,
        parsed: VecDeque<Sse>,
        current: Option<Sse>,
        unfinished_line: Vec<u8>,
    }
}

pub type ByteStreamBody<S, D> = StreamBody<MapOk<S, fn(D) -> Frame<D>>>;
impl<E, S, D> SseStream<ByteStreamBody<S, D>>
where
    S: Stream<Item = Result<D, E>>,
    E: std::error::Error,
    D: Buf,
    StreamBody<ByteStreamBody<S, D>>: Body,
{
    /// Create a new [`SseStream`] from a stream of [`Bytes`](bytes::Bytes).
    ///
    /// This is useful when you interact with clients don't provide response body directly list reqwest.
    pub fn from_byte_stream(stream: S) -> Self {
        let stream = stream.map_ok(http_body::Frame::data as fn(D) -> Frame<D>);
        let body = StreamBody::new(stream);
        Self {
            body: BodyDataStream::new(body),
            parsed: VecDeque::new(),
            current: None,
            unfinished_line: Vec::new(),
        }
    }
}

impl<B: Body> SseStream<B> {
    /// Create a new [`SseStream`] from a [`Body`].
    pub fn new(body: B) -> Self {
        Self {
            body: BodyDataStream::new(body),
            parsed: VecDeque::new(),
            current: None,
            unfinished_line: Vec::new(),
        }
    }
}

pub enum Error {
    Body(Box<dyn std::error::Error + Send + Sync>),
    InvalidLine,
    DuplicatedEventLine,
    DuplicatedIdLine,
    DuplicatedRetry,
    Utf8Parse(Utf8Error),
    IntParse(ParseIntError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Body(e) => write!(f, "body error: {}", e),
            Error::InvalidLine => write!(f, "invalid line"),
            Error::DuplicatedEventLine => write!(f, "duplicated event line"),
            Error::DuplicatedIdLine => write!(f, "duplicated id line"),
            Error::DuplicatedRetry => write!(f, "duplicated retry line"),
            Error::Utf8Parse(e) => write!(f, "utf8 parse error: {}", e),
            Error::IntParse(e) => write!(f, "int parse error: {}", e),
        }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Body(e) => write!(f, "Body({:?})", e),
            Error::InvalidLine => write!(f, "InvalidLine"),
            Error::DuplicatedEventLine => write!(f, "DuplicatedEventLine"),
            Error::DuplicatedIdLine => write!(f, "DuplicatedIdLine"),
            Error::DuplicatedRetry => write!(f, "DuplicatedRetry"),
            Error::Utf8Parse(e) => write!(f, "Utf8Parse({:?})", e),
            Error::IntParse(e) => write!(f, "IntParse({:?})", e),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Body(_) => "body error",
            Error::InvalidLine => "invalid line",
            Error::DuplicatedEventLine => "duplicated event line",
            Error::DuplicatedIdLine => "duplicated id line",
            Error::DuplicatedRetry => "duplicated retry line",
            Error::Utf8Parse(_) => "utf8 parse error",
            Error::IntParse(_) => "int parse error",
        }
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Body(e) => Some(e.as_ref()),
            Error::Utf8Parse(e) => Some(e),
            Error::IntParse(e) => Some(e),
            _ => None,
        }
    }
}

impl<B: Body> Stream for SseStream<B>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    type Item = Result<Sse, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().project();
        if let Some(sse) = this.parsed.pop_front() {
            return Poll::Ready(Some(Ok(sse)));
        }
        let next_data = ready!(this.body.poll_next(cx));
        match next_data {
            Some(Ok(data)) => {
                let chunk = data.chunk();

                if chunk.is_empty() {
                    return self.poll_next(cx);
                }
                let mut lines = chunk.chunk_by(|maybe_nl, _| *maybe_nl != b'\n');
                let first_line = lines.next().expect("frame is empty");
                let mut new_unfinished_line = Vec::new();
                let first_line = if !this.unfinished_line.is_empty() {
                    this.unfinished_line.extend(first_line);
                    std::mem::swap(&mut new_unfinished_line, this.unfinished_line);
                    new_unfinished_line.as_ref()
                } else {
                    first_line
                };
                let mut lines = std::iter::once(first_line).chain(lines);
                *this.unfinished_line = loop {
                    let Some(line) = lines.next() else {
                        break Vec::new();
                    };
                    let line = if line.ends_with(b"\r\n") {
                        &line[..line.len() - 2]
                    } else if line.ends_with(b"\n") || line.ends_with(b"\r") {
                        &line[..line.len() - 1]
                    } else {
                        break line.to_vec();
                    };

                    if line.is_empty() {
                        if let Some(sse) = this.current.take() {
                            this.parsed.push_back(sse);
                        }
                        continue;
                    }
                    // find comma
                    let Some(comma_index) = line.iter().position(|b| *b == b':') else {
                        #[cfg(feature = "tracing")]
                        tracing::warn!(?line, "invalid line, missing `:`");
                        return Poll::Ready(Some(Err(Error::InvalidLine)));
                    };
                    let field_name = &line[..comma_index];
                    let field_value = if line.len() > comma_index + 1 {
                        let field_value = &line[comma_index + 1..];
                        if field_value.starts_with(b" ") {
                            &field_value[1..]
                        } else {
                            field_value
                        }
                    } else {
                        b""
                    };
                    match field_name {
                        b"data" => {
                            let data_line =
                                std::str::from_utf8(field_value).map_err(Error::Utf8Parse)?;
                            // merge data lines
                            if let Some(Sse { data, .. }) = this.current.as_mut() {
                                if data.is_none() {
                                    data.replace(data_line.to_owned());
                                } else {
                                    let data = data.as_mut().unwrap();
                                    data.push('\n');
                                    data.push_str(data_line);
                                }
                            } else {
                                this.current.replace(Sse {
                                    event: None,
                                    data: Some(data_line.to_owned()),
                                    id: None,
                                    retry: None,
                                });
                            }
                        }
                        b"event" => {
                            let event_value =
                                std::str::from_utf8(field_value).map_err(Error::Utf8Parse)?;
                            if let Some(Sse { event, .. }) = this.current.as_mut() {
                                if event.is_some() {
                                    return Poll::Ready(Some(Err(Error::DuplicatedEventLine)));
                                } else {
                                    event.replace(event_value.to_owned());
                                }
                            } else {
                                this.current.replace(Sse {
                                    event: Some(event_value.to_owned()),
                                    ..Default::default()
                                });
                            }
                        }
                        b"id" => {
                            let id_value =
                                std::str::from_utf8(field_value).map_err(Error::Utf8Parse)?;
                            if let Some(Sse { id, .. }) = this.current.as_mut() {
                                if id.is_some() {
                                    return Poll::Ready(Some(Err(Error::DuplicatedIdLine)));
                                } else {
                                    id.replace(id_value.to_owned());
                                }
                            } else {
                                this.current.replace(Sse {
                                    id: Some(id_value.to_owned()),
                                    ..Default::default()
                                });
                            }
                        }
                        b"retry" => {
                            let retry_value = std::str::from_utf8(field_value)
                                .map_err(Error::Utf8Parse)?
                                .trim_ascii();
                            let retry_value =
                                retry_value.parse::<u64>().map_err(Error::IntParse)?;
                            if let Some(Sse { retry, .. }) = this.current.as_mut() {
                                if retry.is_some() {
                                    return Poll::Ready(Some(Err(Error::DuplicatedRetry)));
                                } else {
                                    retry.replace(retry_value);
                                }
                            } else {
                                this.current.replace(Sse {
                                    retry: Some(retry_value),
                                    ..Default::default()
                                });
                            }
                        }
                        b"" => {
                            #[cfg(feature = "tracing")]
                            if tracing::enabled!(tracing::Level::DEBUG) {
                                // a comment
                                let comment =
                                    std::str::from_utf8(field_value).map_err(Error::Utf8Parse)?;
                                tracing::debug!(?comment, "sse comment line");
                            }
                        }
                        _line => {
                            #[cfg(feature = "tracing")]
                            if tracing::enabled!(tracing::Level::WARN) {
                                tracing::warn!(line = ?_line, "invalid line: unknown field");
                            }
                            return Poll::Ready(Some(Err(Error::InvalidLine)));
                        }
                    }
                };
                self.poll_next(cx)
            }
            Some(Err(e)) => Poll::Ready(Some(Err(Error::Body(Box::new(e))))),
            None => {
                if let Some(sse) = this.current.take() {
                    Poll::Ready(Some(Ok(sse)))
                } else {
                    Poll::Ready(None)
                }
            }
        }
    }
}
