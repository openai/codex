//! Simple CRC bindings backed by miniz.c

use std::io;
use std::io::prelude::*;

use crc32fast::Hasher;

/// The CRC calculated by a [`CrcReader`].
///
/// [`CrcReader`]: struct.CrcReader.html
#[derive(Debug, Default)]
pub struct Crc {
    amt: u32,
    hasher: Hasher,
}

/// A wrapper around a [`Read`] that calculates the CRC.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
#[derive(Debug)]
pub struct CrcReader<R> {
    inner: R,
    crc: Crc,
}

impl Crc {
    /// Create a new CRC.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current crc32 checksum.
    pub fn sum(&self) -> u32 {
        self.hasher.clone().finalize()
    }

    /// The number of bytes that have been used to calculate the CRC.
    /// This value is only accurate if the amount is lower than 2<sup>32</sup>.
    pub fn amount(&self) -> u32 {
        self.amt
    }

    /// Update the CRC with the bytes in `data`.
    pub fn update(&mut self, data: &[u8]) {
        self.amt = self.amt.wrapping_add(data.len() as u32);
        self.hasher.update(data);
    }

    /// Reset the CRC.
    pub fn reset(&mut self) {
        self.amt = 0;
        self.hasher.reset();
    }

    /// Combine the CRC with the CRC for the subsequent block of bytes.
    pub fn combine(&mut self, additional_crc: &Crc) {
        self.amt = self.amt.wrapping_add(additional_crc.amt);
        self.hasher.combine(&additional_crc.hasher);
    }
}

impl<R: Read> CrcReader<R> {
    /// Create a new `CrcReader`.
    pub fn new(r: R) -> CrcReader<R> {
        CrcReader {
            inner: r,
            crc: Crc::new(),
        }
    }
}

impl<R> CrcReader<R> {
    /// Get the Crc for this `CrcReader`.
    pub fn crc(&self) -> &Crc {
        &self.crc
    }

    /// Get the reader that is wrapped by this `CrcReader`.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Get the reader that is wrapped by this `CrcReader` by reference.
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Get a mutable reference to the reader that is wrapped by this `CrcReader`.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Reset the Crc in this `CrcReader`.
    pub fn reset(&mut self) {
        self.crc.reset();
    }
}

impl<R: Read> Read for CrcReader<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        let amt = self.inner.read(into)?;
        self.crc.update(&into[..amt]);
        Ok(amt)
    }
}

impl<R: BufRead> BufRead for CrcReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }
    fn consume(&mut self, amt: usize) {
        if let Ok(data) = self.inner.fill_buf() {
            self.crc.update(&data[..amt]);
        }
        self.inner.consume(amt);
    }
}

/// A wrapper around a [`Write`] that calculates the CRC.
///
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
#[derive(Debug)]
pub struct CrcWriter<W> {
    inner: W,
    crc: Crc,
}

impl<W> CrcWriter<W> {
    /// Get the Crc for this `CrcWriter`.
    pub fn crc(&self) -> &Crc {
        &self.crc
    }

    /// Get the writer that is wrapped by this `CrcWriter`.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Get the writer that is wrapped by this `CrcWriter` by reference.
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Get a mutable reference to the writer that is wrapped by this `CrcWriter`.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Reset the Crc in this `CrcWriter`.
    pub fn reset(&mut self) {
        self.crc.reset();
    }
}

impl<W: Write> CrcWriter<W> {
    /// Create a new `CrcWriter`.
    pub fn new(w: W) -> CrcWriter<W> {
        CrcWriter {
            inner: w,
            crc: Crc::new(),
        }
    }
}

impl<W: Write> Write for CrcWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let amt = self.inner.write(buf)?;
        self.crc.update(&buf[..amt]);
        Ok(amt)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
