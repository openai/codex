/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use core::cmp::min;

use crate::bytestream::traits::ZReaderTrait;

const ERROR_MSG: &str = "No more bytes";

/// An encapsulation of a byte stream reader
///
/// This provides an interface similar to [std::io::Cursor] but
/// it provides fine grained options for reading different integer data types from
/// the underlying buffer.
///
/// There are two variants mainly error and non error variants,
/// the error variants are useful for cases where you need bytes
/// from the underlying stream, and cannot do with zero result.
/// the non error variants are useful when you may have proved data already exists
/// eg by using [`has`] method or you are okay with returning zero if the underlying
/// buffer has been completely read.
///
/// [std::io::Cursor]: https://doc.rust-lang.org/std/io/struct.Cursor.html
/// [`has`]: Self::has
pub struct ZByteReader<T: ZReaderTrait> {
    /// Data stream
    stream:   T,
    position: usize
}

enum Mode {
    // Big endian
    BE,
    // Little Endian
    LE
}
#[cfg(feature = "std")]
impl<T: ZReaderTrait> std::io::Read for ZByteReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(self.read(buf).unwrap())
    }
}

impl<T: ZReaderTrait> ZByteReader<T> {
    /// Create a new instance of the byte stream
    ///
    /// Bytes will be read from the start of `buf`.
    ///
    /// `buf` is expected to live as long as this and
    /// all references to it live
    ///
    /// # Returns
    /// A byte reader which will pull bits from bye
    pub const fn new(buf: T) -> ZByteReader<T> {
        ZByteReader {
            stream:   buf,
            position: 0
        }
    }
    /// Destroy this reader returning
    /// the underlying source of the bytes
    /// from which we were decoding
    pub fn consume(self) -> T {
        self.stream
    }
    /// Skip `num` bytes ahead of the stream.
    ///
    /// This bumps up the internal cursor wit a wrapping addition
    /// The bytes between current position and `num` will be skipped
    ///
    /// # Arguments
    /// `num`: How many bytes to skip
    ///
    /// # Note
    /// This does not consider length of the buffer, so skipping more bytes
    /// than possible and then reading bytes will return an error if using error variants
    /// or zero if using non-error variants
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZByteReader;
    /// let zero_to_hundred:Vec<u8> = (0..100).collect();
    /// let mut stream = ZByteReader::new(&zero_to_hundred);
    /// // skip 37 bytes
    /// stream.skip(37);
    ///
    /// assert_eq!(stream.get_u8(),37);
    /// ```
    ///
    /// See [`rewind`](ZByteReader::rewind) for moving the internal cursor back
    pub fn skip(&mut self, num: usize) {
        // Can this overflow ??
        self.position = self.position.wrapping_add(num);
    }
    /// Undo a buffer read by moving the position pointer `num`
    /// bytes behind.
    ///
    /// This operation will saturate at zero
    pub fn rewind(&mut self, num: usize) {
        self.position = self.position.saturating_sub(num);
    }

    /// Return whether the underlying buffer
    /// has `num` bytes available for reading
    ///
    /// # Example
    ///
    /// ```
    /// use zune_core::bytestream::ZByteReader;
    /// let data = [0_u8;120];
    /// let reader = ZByteReader::new(data.as_slice());
    /// assert!(reader.has(3));
    /// assert!(!reader.has(121));
    /// ```
    #[inline]
    pub fn has(&self, num: usize) -> bool {
        self.position.saturating_add(num) <= self.stream.get_len()
    }
    /// Get number of bytes available in the stream
    #[inline]
    pub fn get_bytes_left(&self) -> usize {
        // Must be saturating to prevent underflow
        self.stream.get_len().saturating_sub(self.position)
    }
    /// Get length of the underlying buffer.
    ///
    /// To get the number of bytes left in the buffer,
    /// use [remaining] method
    ///
    /// [remaining]: Self::remaining
    #[inline]
    pub fn len(&self) -> usize {
        self.stream.get_len()
    }
    /// Return true if the underlying buffer stream is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.stream.get_len() == 0
    }
    /// Get current position of the buffer.
    #[inline]
    pub const fn get_position(&self) -> usize {
        self.position
    }
    /// Return true whether or not we read to the end of the
    /// buffer and have no more bytes left.
    #[inline]
    pub fn eof(&self) -> bool {
        self.position >= self.len()
    }
    /// Get number of bytes unread inside this
    /// stream.
    ///
    /// To get the length of the underlying stream,
    /// use [len] method
    ///
    /// [len]: Self::len()
    #[inline]
    pub fn remaining(&self) -> usize {
        self.stream.get_len().saturating_sub(self.position)
    }
    /// Get a part of the bytestream as a reference.
    ///
    /// This increments the position to point past the bytestream
    /// if position+num is in bounds
    pub fn get(&mut self, num: usize) -> Result<&[u8], &'static str> {
        match self.stream.get_slice(self.position..self.position + num) {
            Some(bytes) => {
                self.position += num;
                Ok(bytes)
            }
            None => Err(ERROR_MSG)
        }
    }
    /// Look ahead position bytes and return a reference
    /// to num_bytes from that position, or an error if the
    /// peek would be out of bounds.
    ///
    /// This doesn't increment the position, bytes would have to be discarded
    /// at a later point.
    #[inline]
    pub fn peek_at(&self, position: usize, num_bytes: usize) -> Result<&[u8], &'static str> {
        let start = self.position + position;
        let end = self.position + position + num_bytes;

        match self.stream.get_slice(start..end) {
            Some(bytes) => Ok(bytes),
            None => Err(ERROR_MSG)
        }
    }
    /// Get a fixed amount of bytes or return an error if we cant
    /// satisfy the read
    ///
    /// This should be combined with [`has`] since if there are no
    /// more bytes you get an error.
    ///
    /// But it's useful for cases where you expect bytes but they are not present
    ///
    /// For the zero  variant see, [`get_fixed_bytes_or_zero`]
    ///
    /// # Example
    /// ```rust
    /// use zune_core::bytestream::ZByteReader;
    /// let mut stream = ZByteReader::new([0x0,0x5,0x3,0x2].as_slice());
    /// let first_bytes = stream.get_fixed_bytes_or_err::<10>(); // not enough bytes
    /// assert!(first_bytes.is_err());
    /// ```
    ///
    /// [`has`]:Self::has
    /// [`get_fixed_bytes_or_zero`]: Self::get_fixed_bytes_or_zero
    #[inline]
    pub fn get_fixed_bytes_or_err<const N: usize>(&mut self) -> Result<[u8; N], &'static str> {
        let mut byte_store: [u8; N] = [0; N];

        match self.stream.get_slice(self.position..self.position + N) {
            Some(bytes) => {
                self.position += N;
                byte_store.copy_from_slice(bytes);

                Ok(byte_store)
            }
            None => Err(ERROR_MSG)
        }
    }

    /// Get a fixed amount of bytes or return a zero array size
    /// if we can't satisfy the read
    ///
    /// This should be combined with [`has`] since if there are no
    /// more bytes you get a zero initialized array
    ///
    /// For the error variant see, [`get_fixed_bytes_or_err`]
    ///
    /// # Example
    /// ```rust
    /// use zune_core::bytestream::ZByteReader;
    /// let mut stream = ZByteReader::new([0x0,0x5,0x3,0x2].as_slice());
    /// let first_bytes = stream.get_fixed_bytes_or_zero::<2>();
    /// assert_eq!(first_bytes,[0x0,0x5]);
    /// ```
    ///
    /// [`has`]:Self::has
    /// [`get_fixed_bytes_or_err`]: Self::get_fixed_bytes_or_err
    #[inline]
    pub fn get_fixed_bytes_or_zero<const N: usize>(&mut self) -> [u8; N] {
        let mut byte_store: [u8; N] = [0; N];

        match self.stream.get_slice(self.position..self.position + N) {
            Some(bytes) => {
                self.position += N;
                byte_store.copy_from_slice(bytes);

                byte_store
            }
            None => byte_store
        }
    }
    #[inline]
    /// Skip bytes until a condition becomes false or the stream runs out of bytes
    ///
    /// # Example
    ///
    /// ```rust
    /// use zune_core::bytestream::ZByteReader;
    /// let mut stream = ZByteReader::new([0;10].as_slice());
    /// stream.skip_until_false(|x| x.is_ascii()) // skip until we meet a non ascii character
    /// ```
    pub fn skip_until_false<F: Fn(u8) -> bool>(&mut self, func: F) {
        // iterate until we have no more bytes
        while !self.eof() {
            // get a byte from stream
            let byte = self.get_u8();

            if !(func)(byte) {
                // function returned false meaning we stop skipping
                self.rewind(1);
                break;
            }
        }
    }
    /// Return the remaining unread bytes in this byte reader
    pub fn remaining_bytes(&self) -> &[u8] {
        debug_assert!(self.position <= self.len());
        self.stream.get_slice(self.position..self.len()).unwrap()
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        let buf_length = buf.len();
        let start = self.position;
        let end = min(self.len(), self.position + buf_length);
        let diff = end - start;

        buf[0..diff].copy_from_slice(self.stream.get_slice(start..end).unwrap());

        self.skip(diff);

        Ok(diff)
    }

    /// Read enough bytes to fill in
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), &'static str> {
        let size = self.read(buf)?;

        if size != buf.len() {
            return Err("Could not read into the whole buffer");
        }
        Ok(())
    }

    /// Set the cursor position
    ///
    /// After this, all reads will proceed from the position as an anchor
    /// point
    pub fn set_position(&mut self, position: usize) {
        self.position = position;
    }
}

macro_rules! get_single_type {
    ($name:tt,$name2:tt,$name3:tt,$name4:tt,$name5:tt,$name6:tt,$int_type:tt) => {
        impl<T:ZReaderTrait> ZByteReader<T>
        {
            #[inline(always)]
            fn $name(&mut self, mode: Mode) -> $int_type
            {
                const SIZE_OF_VAL: usize = core::mem::size_of::<$int_type>();

                let mut space = [0; SIZE_OF_VAL];

                match self.stream.get_slice(self.position..self.position + SIZE_OF_VAL)
                {
                    Some(position) =>
                    {
                        space.copy_from_slice(position);
                        self.position += SIZE_OF_VAL;

                        match mode
                        {
                            Mode::LE => $int_type::from_le_bytes(space),
                            Mode::BE => $int_type::from_be_bytes(space),
                        }
                    }
                    None => 0,
                }
            }

            #[inline(always)]
            fn $name2(&mut self, mode: Mode) -> Result<$int_type, &'static str>
            {
                const SIZE_OF_VAL: usize = core::mem::size_of::<$int_type>();

                let mut space = [0; SIZE_OF_VAL];

                match self.stream.get_slice(self.position..self.position + SIZE_OF_VAL)
                {
                    Some(position) =>
                    {
                        space.copy_from_slice(position);
                        self.position += SIZE_OF_VAL;

                        match mode
                        {
                            Mode::LE => Ok($int_type::from_le_bytes(space)),
                            Mode::BE => Ok($int_type::from_be_bytes(space)),
                        }
                    }
                    None => Err(ERROR_MSG),
                }
            }
            #[doc=concat!("Read ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," read.")]
            #[inline]
            pub fn $name3(&mut self) -> Result<$int_type, &'static str>
            {
                self.$name2(Mode::BE)
            }

            #[doc=concat!("Read ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," read.")]
            #[inline]
            pub fn $name4(&mut self) -> Result<$int_type, &'static str>
            {
                self.$name2(Mode::LE)
            }
            #[doc=concat!("Read ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Returning 0 if the underlying  buffer does not have enough bytes for a ",stringify!($int_type)," read.")]
            #[inline(always)]
            pub fn $name5(&mut self) -> $int_type
            {
                self.$name(Mode::BE)
            }
            #[doc=concat!("Read ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Returning 0 if the underlying buffer does not have enough bytes for a ",stringify!($int_type)," read.")]
            #[inline(always)]
            pub fn $name6(&mut self) -> $int_type
            {
                self.$name(Mode::LE)
            }
        }
    };
}
// U8 implementation
// The benefit of our own unrolled u8 impl instead of macros is that this is sometimes used in some
// impls and is called multiple times, e.g jpeg during huffman decoding.
// we can make some functions leaner like get_u8 is branchless
impl<T> ZByteReader<T>
where
    T: ZReaderTrait
{
    /// Retrieve a byte from the underlying stream
    /// returning 0 if there are no more bytes available
    ///
    /// This means 0 might indicate a bit or an end of stream, but
    /// this is useful for some scenarios where one needs a byte.
    ///
    /// For the panicking one, see [`get_u8_err`]
    ///
    /// [`get_u8_err`]: Self::get_u8_err
    #[inline(always)]
    pub fn get_u8(&mut self) -> u8 {
        let byte = *self.stream.get_byte(self.position).unwrap_or(&0);

        self.position += usize::from(self.position < self.len());
        byte
    }

    /// Retrieve a byte from the underlying stream
    /// returning an error if there are no more bytes available
    ///
    /// For the non panicking one, see [`get_u8`]
    ///
    /// [`get_u8`]: Self::get_u8
    #[inline(always)]
    pub fn get_u8_err(&mut self) -> Result<u8, &'static str> {
        match self.stream.get_byte(self.position) {
            Some(byte) => {
                self.position += 1;
                Ok(*byte)
            }
            None => Err(ERROR_MSG)
        }
    }
}

// u16,u32,u64 -> macros
get_single_type!(
    get_u16_inner_or_default,
    get_u16_inner_or_die,
    get_u16_be_err,
    get_u16_le_err,
    get_u16_be,
    get_u16_le,
    u16
);
get_single_type!(
    get_u32_inner_or_default,
    get_u32_inner_or_die,
    get_u32_be_err,
    get_u32_le_err,
    get_u32_be,
    get_u32_le,
    u32
);
get_single_type!(
    get_u64_inner_or_default,
    get_u64_inner_or_die,
    get_u64_be_err,
    get_u64_le_err,
    get_u64_be,
    get_u64_le,
    u64
);
