/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use core::mem::size_of;

enum Mode {
    // Big endian
    BE,
    // Little Endian
    LE
}

static ERROR_MSG: &str = "No more space";

/// Encapsulates a simple Byte writer with
/// support for Endian aware writes
pub struct ZByteWriter<'a> {
    buffer:   &'a mut [u8],
    position: usize
}

impl<'a> ZByteWriter<'a> {
    /// Write bytes from the buf into the bytestream
    /// and return how many bytes were written
    ///
    /// # Arguments
    /// - `buf`: The bytes to be written to the bytestream
    ///
    /// # Returns
    /// - `Ok(usize)` - Number of bytes written
    /// This number may be less than `buf.len()` if the length of the buffer is greater
    /// than the internal bytestream length
    ///  
    /// If you want to be sure that all bytes were written, see [`write_all`](Self::write_all)
    ///
    #[inline]
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        let min = buf.len().min(self.bytes_left());
        // write
        self.buffer[self.position..self.position + min].copy_from_slice(&buf[0..min]);
        self.position += min;

        Ok(min)
    }
    /// Write all bytes from `buf` into the bytestream and return
    /// and panic if not all bytes were written to the bytestream
    ///
    /// # Arguments
    /// - `buf`: The bytes to be written into the bytestream
    ///
    ///# Returns
    /// - `Ok(())`: Indicates all bytes were written into the bytestream
    /// - `Err(&static str)`: In case all the bytes could not be written
    /// to the stream
    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), &'static str> {
        let size = self.write(buf)?;

        if size != buf.len() {
            return Err("Could not write the whole buffer");
        }
        Ok(())
    }
    /// Create a new bytestream writer
    /// Bytes are written from the start to the end and not assumptions
    /// are made of the nature of the underlying stream
    ///
    /// # Arguments
    pub fn new(data: &'a mut [u8]) -> ZByteWriter<'a> {
        ZByteWriter {
            buffer:   data,
            position: 0
        }
    }
    /// Return number of unwritten bytes in this stream
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut storage = [0;10];
    ///
    /// let writer = ZByteWriter::new(&mut storage);
    /// assert_eq!(writer.bytes_left(),10); // no bytes were written
    /// ```
    pub const fn bytes_left(&self) -> usize {
        self.buffer.len().saturating_sub(self.position)
    }

    /// Return the number of bytes the writer has written
    ///
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut stream = ZByteWriter::new(&mut []);
    /// assert_eq!(stream.position(),0);
    /// ```
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Write a single byte into the bytestream or error out
    /// if there is not enough space
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut buf = [0;10];
    /// let mut stream  =  ZByteWriter::new(&mut buf);
    /// assert!(stream.write_u8_err(34).is_ok());
    /// ```
    /// No space
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut stream = ZByteWriter::new(&mut []);
    /// assert!(stream.write_u8_err(32).is_err());
    /// ```
    ///
    pub fn write_u8_err(&mut self, byte: u8) -> Result<(), &'static str> {
        match self.buffer.get_mut(self.position) {
            Some(m_byte) => {
                self.position += 1;
                *m_byte = byte;

                Ok(())
            }
            None => Err(ERROR_MSG)
        }
    }

    /// Write a single byte in the stream or don't write
    /// anything if the buffer is full and cannot support the byte read
    ///
    /// Should be combined with [`has`](Self::has)
    pub fn write_u8(&mut self, byte: u8) {
        if let Some(m_byte) = self.buffer.get_mut(self.position) {
            self.position += 1;
            *m_byte = byte;
        }
    }
    /// Check if the byte writer can support
    /// the following write
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut data = [0;10];
    /// let mut stream = ZByteWriter::new(&mut data);
    /// assert!(stream.has(5));
    /// assert!(!stream.has(100));
    /// ```
    pub const fn has(&self, bytes: usize) -> bool {
        self.position.saturating_add(bytes) <= self.buffer.len()
    }

    /// Get length of the underlying buffer.
    #[inline]
    pub const fn len(&self) -> usize {
        self.buffer.len()
    }
    /// Return true if the underlying buffer stream is empty
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return true whether or not we read to the end of the
    /// buffer and have no more bytes left.
    ///
    /// If this is true, all non error variants will silently discard the
    /// byte and all error variants will return an error on writing a byte
    /// if any write occurs
    ///
    ///
    #[inline]
    pub const fn eof(&self) -> bool {
        self.position >= self.len()
    }

    /// Rewind the position of the internal cursor back by `by` bytes
    ///
    /// The position saturates at zero
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let bytes = &mut [1,2,4];
    /// let mut stream = ZByteWriter::new(bytes);
    /// stream.write_u16_be(23);
    /// // now internal cursor is at position 2.
    /// // lets rewind it
    /// stream.rewind(usize::MAX);
    /// assert_eq!(stream.position(),0);
    /// ```
    #[inline]
    pub fn rewind(&mut self, by: usize) {
        self.position = self.position.saturating_sub(by);
    }
    /// Move the internal cursor forward some bytes
    ///
    ///
    /// This saturates at maximum value of usize in your platform.
    #[inline]
    pub fn skip(&mut self, by: usize) {
        self.position = self.position.saturating_add(by);
    }

    /// Look ahead position bytes and return a reference
    /// to num_bytes from that position, or an error if the
    /// peek would be out of bounds.
    ///
    /// This doesn't increment the position, bytes would have to be discarded
    /// at a later point.
    #[inline]
    pub fn peek_at(&'a self, position: usize, num_bytes: usize) -> Result<&'a [u8], &'static str> {
        let start = self.position + position;
        let end = self.position + position + num_bytes;

        match self.buffer.get(start..end) {
            Some(bytes) => Ok(bytes),
            None => Err(ERROR_MSG)
        }
    }

    /// Set position for the internal cursor
    ///
    /// Further calls to write bytes will proceed from the
    /// position set
    pub fn set_position(&mut self, position: usize) {
        self.position = position;
    }
}

macro_rules! write_single_type {
    ($name:tt,$name2:tt,$name3:tt,$name4:tt,$name5:tt,$name6:tt,$int_type:tt) => {
        impl<'a> ZByteWriter<'a>
        {
            #[inline(always)]
            fn $name(&mut self, byte: $int_type, mode: Mode) -> Result<(), &'static str>
            {
                const SIZE: usize = size_of::<$int_type>();

                match self.buffer.get_mut(self.position..self.position + SIZE)
                {
                    Some(m_byte) =>
                    {
                        self.position += SIZE;
                        // get bits, depending on mode.
                        // This should be inlined and not visible in
                        // the generated binary since mode is a compile
                        // time constant.
                        let bytes = match mode
                        {
                            Mode::BE => byte.to_be_bytes(),
                            Mode::LE => byte.to_le_bytes()
                        };

                        m_byte.copy_from_slice(&bytes);

                        Ok(())
                    }
                    None => Err(ERROR_MSG)
                }
            }
            #[inline(always)]
            fn $name2(&mut self, byte: $int_type, mode: Mode)
            {
                const SIZE: usize = size_of::<$int_type>();

                if let Some(m_byte) = self.buffer.get_mut(self.position..self.position + SIZE)
                {
                    self.position += SIZE;
                    // get bits, depending on mode.
                    // This should be inlined and not visible in
                    // the generated binary since mode is a compile
                    // time constant.
                    let bytes = match mode
                    {
                        Mode::BE => byte.to_be_bytes(),
                        Mode::LE => byte.to_le_bytes()
                    };

                    m_byte.copy_from_slice(&bytes);
                }
            }

            #[doc=concat!("Write ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," write.")]
            #[inline]
            pub fn $name3(&mut self, byte: $int_type) -> Result<(), &'static str>
            {
                self.$name(byte, Mode::BE)
            }

            #[doc=concat!("Write ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," write.")]
            #[inline]
            pub fn $name4(&mut self, byte: $int_type) -> Result<(), &'static str>
            {
                self.$name(byte, Mode::LE)
            }

            #[doc=concat!("Write ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Or don't write anything if the reader cannot support a ",stringify!($int_type)," write.")]
            #[doc=concat!("\nShould be combined with the [`has`](Self::has) method to ensure a write succeeds")]
            #[inline]
            pub fn $name5(&mut self, byte: $int_type)
            {
                self.$name2(byte, Mode::BE)
            }
            #[doc=concat!("Write ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Or don't write anything if the reader cannot support a ",stringify!($int_type)," write.")]
            #[doc=concat!("Should be combined with the [`has`](Self::has) method to ensure a write succeeds")]
            #[inline]
            pub fn $name6(&mut self, byte: $int_type)
            {
                self.$name2(byte, Mode::LE)
            }
        }
    };
}

write_single_type!(
    write_u64_inner_or_die,
    write_u64_inner_or_none,
    write_u64_be_err,
    write_u64_le_err,
    write_u64_be,
    write_u64_le,
    u64
);

write_single_type!(
    write_u32_inner_or_die,
    write_u32_inner_or_none,
    write_u32_be_err,
    write_u32_le_err,
    write_u32_be,
    write_u32_le,
    u32
);

write_single_type!(
    write_u16_inner_or_die,
    write_u16_inner_or_none,
    write_u16_be_err,
    write_u16_le_err,
    write_u16_be,
    write_u16_le,
    u16
);
