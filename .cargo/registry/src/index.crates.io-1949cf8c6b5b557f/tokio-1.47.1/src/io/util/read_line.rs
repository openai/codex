use crate::io::util::read_until::read_until_internal;
use crate::io::AsyncBufRead;

use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::marker::PhantomPinned;
use std::mem;
use std::pin::Pin;
use std::string::FromUtf8Error;
use std::task::{ready, Context, Poll};

pin_project! {
    /// Future for the [`read_line`](crate::io::AsyncBufReadExt::read_line) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ReadLine<'a, R: ?Sized> {
        reader: &'a mut R,
        // This is the buffer we were provided. It will be replaced with an empty string
        // while reading to postpone utf-8 handling until after reading.
        output: &'a mut String,
        // The actual allocation of the string is moved into this vector instead.
        buf: Vec<u8>,
        // The number of bytes appended to buf. This can be less than buf.len() if
        // the buffer was not empty when the operation was started.
        read: usize,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

pub(crate) fn read_line<'a, R>(reader: &'a mut R, string: &'a mut String) -> ReadLine<'a, R>
where
    R: AsyncBufRead + ?Sized + Unpin,
{
    ReadLine {
        reader,
        buf: mem::take(string).into_bytes(),
        output: string,
        read: 0,
        _pin: PhantomPinned,
    }
}

fn put_back_original_data(output: &mut String, mut vector: Vec<u8>, num_bytes_read: usize) {
    let original_len = vector.len() - num_bytes_read;
    vector.truncate(original_len);
    *output = String::from_utf8(vector).expect("The original data must be valid utf-8.");
}

/// This handles the various failure cases and puts the string back into `output`.
///
/// The `truncate_on_io_error` `bool` is necessary because `read_to_string` and `read_line`
/// disagree on what should happen when an IO error occurs.
pub(super) fn finish_string_read(
    io_res: io::Result<usize>,
    utf8_res: Result<String, FromUtf8Error>,
    read: usize,
    output: &mut String,
    truncate_on_io_error: bool,
) -> Poll<io::Result<usize>> {
    match (io_res, utf8_res) {
        (Ok(num_bytes), Ok(string)) => {
            debug_assert_eq!(read, 0);
            *output = string;
            Poll::Ready(Ok(num_bytes))
        }
        (Err(io_err), Ok(string)) => {
            *output = string;
            if truncate_on_io_error {
                let original_len = output.len() - read;
                output.truncate(original_len);
            }
            Poll::Ready(Err(io_err))
        }
        (Ok(num_bytes), Err(utf8_err)) => {
            debug_assert_eq!(read, 0);
            put_back_original_data(output, utf8_err.into_bytes(), num_bytes);

            Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            )))
        }
        (Err(io_err), Err(utf8_err)) => {
            put_back_original_data(output, utf8_err.into_bytes(), read);

            Poll::Ready(Err(io_err))
        }
    }
}

pub(super) fn read_line_internal<R: AsyncBufRead + ?Sized>(
    reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    output: &mut String,
    buf: &mut Vec<u8>,
    read: &mut usize,
) -> Poll<io::Result<usize>> {
    let io_res = ready!(read_until_internal(reader, cx, b'\n', buf, read));
    let utf8_res = String::from_utf8(mem::take(buf));

    // At this point both buf and output are empty. The allocation is in utf8_res.

    debug_assert!(buf.is_empty());
    debug_assert!(output.is_empty());
    finish_string_read(io_res, utf8_res, *read, output, false)
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for ReadLine<'_, R> {
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        read_line_internal(Pin::new(*me.reader), cx, me.output, me.buf, me.read)
    }
}
