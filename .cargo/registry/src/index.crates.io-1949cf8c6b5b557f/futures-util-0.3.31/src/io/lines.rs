use super::read_line::read_line_internal;
use futures_core::ready;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_io::AsyncBufRead;
use pin_project_lite::pin_project;
use std::io;
use std::mem;
use std::pin::Pin;
use std::string::String;
use std::vec::Vec;

pin_project! {
    /// Stream for the [`lines`](super::AsyncBufReadExt::lines) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Lines<R> {
        #[pin]
        reader: R,
        buf: String,
        bytes: Vec<u8>,
        read: usize,
    }
}

impl<R: AsyncBufRead> Lines<R> {
    pub(super) fn new(reader: R) -> Self {
        Self { reader, buf: String::new(), bytes: Vec::new(), read: 0 }
    }
}

impl<R: AsyncBufRead> Stream for Lines<R> {
    type Item = io::Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let n = ready!(read_line_internal(this.reader, cx, this.buf, this.bytes, this.read))?;
        *this.read = 0;
        if n == 0 && this.buf.is_empty() {
            return Poll::Ready(None);
        }
        if this.buf.ends_with('\n') {
            this.buf.pop();
            if this.buf.ends_with('\r') {
                this.buf.pop();
            }
        }
        Poll::Ready(Some(Ok(mem::take(this.buf))))
    }
}
