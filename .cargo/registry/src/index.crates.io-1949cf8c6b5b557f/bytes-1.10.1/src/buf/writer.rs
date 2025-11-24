use crate::BufMut;

use std::{cmp, io};

/// A `BufMut` adapter which implements `io::Write` for the inner value.
///
/// This struct is generally created by calling `writer()` on `BufMut`. See
/// documentation of [`writer()`](BufMut::writer) for more
/// details.
#[derive(Debug)]
pub struct Writer<B> {
    buf: B,
}

pub fn new<B>(buf: B) -> Writer<B> {
    Writer { buf }
}

impl<B: BufMut> Writer<B> {
    /// Gets a reference to the underlying `BufMut`.
    ///
    /// It is inadvisable to directly write to the underlying `BufMut`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::BufMut;
    ///
    /// let buf = Vec::with_capacity(1024).writer();
    ///
    /// assert_eq!(1024, buf.get_ref().capacity());
    /// ```
    pub fn get_ref(&self) -> &B {
        &self.buf
    }

    /// Gets a mutable reference to the underlying `BufMut`.
    ///
    /// It is inadvisable to directly write to the underlying `BufMut`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::BufMut;
    ///
    /// let mut buf = vec![].writer();
    ///
    /// buf.get_mut().reserve(1024);
    ///
    /// assert_eq!(1024, buf.get_ref().capacity());
    /// ```
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.buf
    }

    /// Consumes this `Writer`, returning the underlying value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use bytes::BufMut;
    /// use std::io;
    ///
    /// let mut buf = vec![].writer();
    /// let mut src = &b"hello world"[..];
    ///
    /// io::copy(&mut src, &mut buf).unwrap();
    ///
    /// let buf = buf.into_inner();
    /// assert_eq!(*buf, b"hello world"[..]);
    /// ```
    pub fn into_inner(self) -> B {
        self.buf
    }
}

impl<B: BufMut + Sized> io::Write for Writer<B> {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        let n = cmp::min(self.buf.remaining_mut(), src.len());

        self.buf.put_slice(&src[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
