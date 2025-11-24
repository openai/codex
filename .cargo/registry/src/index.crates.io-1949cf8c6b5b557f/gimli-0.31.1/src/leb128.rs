//! Read and write DWARF's "Little Endian Base 128" (LEB128) variable length
//! integer encoding.
//!
//! The implementation is a direct translation of the psuedocode in the DWARF 4
//! standard's appendix C.
//!
//! Read and write signed integers:
//!
//! ```
//! # #[cfg(all(feature = "read", feature = "write"))] {
//! use gimli::{EndianSlice, NativeEndian, leb128};
//!
//! let mut buf = [0; 1024];
//!
//! // Write to anything that implements `std::io::Write`.
//! {
//!     let mut writable = &mut buf[..];
//!     leb128::write::signed(&mut writable, -12345).expect("Should write number");
//! }
//!
//! // Read from anything that implements `gimli::Reader`.
//! let mut readable = EndianSlice::new(&buf[..], NativeEndian);
//! let val = leb128::read::signed(&mut readable).expect("Should read number");
//! assert_eq!(val, -12345);
//! # }
//! ```
//!
//! Or read and write unsigned integers:
//!
//! ```
//! # #[cfg(all(feature = "read", feature = "write"))] {
//! use gimli::{EndianSlice, NativeEndian, leb128};
//!
//! let mut buf = [0; 1024];
//!
//! {
//!     let mut writable = &mut buf[..];
//!     leb128::write::unsigned(&mut writable, 98765).expect("Should write number");
//! }
//!
//! let mut readable = EndianSlice::new(&buf[..], NativeEndian);
//! let val = leb128::read::unsigned(&mut readable).expect("Should read number");
//! assert_eq!(val, 98765);
//! # }
//! ```

const CONTINUATION_BIT: u8 = 1 << 7;
#[cfg(feature = "read-core")]
const SIGN_BIT: u8 = 1 << 6;

#[inline]
fn low_bits_of_byte(byte: u8) -> u8 {
    byte & !CONTINUATION_BIT
}

#[inline]
#[allow(dead_code)]
fn low_bits_of_u64(val: u64) -> u8 {
    let byte = val & u64::from(u8::MAX);
    low_bits_of_byte(byte as u8)
}

/// A module for reading signed and unsigned integers that have been LEB128
/// encoded.
#[cfg(feature = "read-core")]
pub mod read {
    use super::{low_bits_of_byte, CONTINUATION_BIT, SIGN_BIT};
    use crate::read::{Error, Reader, Result};

    /// Read bytes until the LEB128 continuation bit is not set.
    pub fn skip<R: Reader>(r: &mut R) -> Result<()> {
        loop {
            let byte = r.read_u8()?;
            if byte & CONTINUATION_BIT == 0 {
                return Ok(());
            }
        }
    }

    /// Read an unsigned LEB128 number from the given `Reader` and
    /// return it or an error if reading failed.
    pub fn unsigned<R: Reader>(r: &mut R) -> Result<u64> {
        let mut result = 0;
        let mut shift = 0;

        loop {
            let byte = r.read_u8()?;
            if shift == 63 && byte != 0x00 && byte != 0x01 {
                return Err(Error::BadUnsignedLeb128);
            }

            let low_bits = u64::from(low_bits_of_byte(byte));
            result |= low_bits << shift;

            if byte & CONTINUATION_BIT == 0 {
                return Ok(result);
            }

            shift += 7;
        }
    }

    /// Read an LEB128 u16 from the given `Reader` and
    /// return it or an error if reading failed.
    pub fn u16<R: Reader>(r: &mut R) -> Result<u16> {
        let byte = r.read_u8()?;
        let mut result = u16::from(low_bits_of_byte(byte));
        if byte & CONTINUATION_BIT == 0 {
            return Ok(result);
        }

        let byte = r.read_u8()?;
        result |= u16::from(low_bits_of_byte(byte)) << 7;
        if byte & CONTINUATION_BIT == 0 {
            return Ok(result);
        }

        let byte = r.read_u8()?;
        if byte > 0x03 {
            return Err(Error::BadUnsignedLeb128);
        }
        result += u16::from(byte) << 14;
        Ok(result)
    }

    /// Read a signed LEB128 number from the given `Reader` and
    /// return it or an error if reading failed.
    pub fn signed<R: Reader>(r: &mut R) -> Result<i64> {
        let mut result = 0;
        let mut shift = 0;
        let size = 64;
        let mut byte;

        loop {
            byte = r.read_u8()?;
            if shift == 63 && byte != 0x00 && byte != 0x7f {
                return Err(Error::BadSignedLeb128);
            }

            let low_bits = i64::from(low_bits_of_byte(byte));
            result |= low_bits << shift;
            shift += 7;

            if byte & CONTINUATION_BIT == 0 {
                break;
            }
        }

        if shift < size && (SIGN_BIT & byte) == SIGN_BIT {
            // Sign extend the result.
            result |= !0 << shift;
        }

        Ok(result)
    }
}

/// A module for writing integers encoded as LEB128.
#[cfg(feature = "write")]
pub mod write {
    use super::{low_bits_of_u64, CONTINUATION_BIT};
    use std::io;

    /// Write the given unsigned number using the LEB128 encoding to the given
    /// `std::io::Write`able. Returns the number of bytes written to `w`, or an
    /// error if writing failed.
    pub fn unsigned<W>(w: &mut W, mut val: u64) -> Result<usize, io::Error>
    where
        W: io::Write,
    {
        let mut bytes_written = 0;
        loop {
            let mut byte = low_bits_of_u64(val);
            val >>= 7;
            if val != 0 {
                // More bytes to come, so set the continuation bit.
                byte |= CONTINUATION_BIT;
            }

            let buf = [byte];
            w.write_all(&buf)?;
            bytes_written += 1;

            if val == 0 {
                return Ok(bytes_written);
            }
        }
    }

    /// Return the size of the LEB128 encoding of the given unsigned number.
    pub fn uleb128_size(mut val: u64) -> usize {
        let mut size = 0;
        loop {
            val >>= 7;
            size += 1;
            if val == 0 {
                return size;
            }
        }
    }

    /// Write the given signed number using the LEB128 encoding to the given
    /// `std::io::Write`able. Returns the number of bytes written to `w`, or an
    /// error if writing failed.
    pub fn signed<W>(w: &mut W, mut val: i64) -> Result<usize, io::Error>
    where
        W: io::Write,
    {
        let mut bytes_written = 0;
        loop {
            let mut byte = val as u8;
            // Keep the sign bit for testing
            val >>= 6;
            let done = val == 0 || val == -1;
            if done {
                byte &= !CONTINUATION_BIT;
            } else {
                // Remove the sign bit
                val >>= 1;
                // More bytes to come, so set the continuation bit.
                byte |= CONTINUATION_BIT;
            }

            let buf = [byte];
            w.write_all(&buf)?;
            bytes_written += 1;

            if done {
                return Ok(bytes_written);
            }
        }
    }

    /// Return the size of the LEB128 encoding of the given signed number.
    pub fn sleb128_size(mut val: i64) -> usize {
        let mut size = 0;
        loop {
            val >>= 6;
            let done = val == 0 || val == -1;
            val >>= 1;
            size += 1;
            if done {
                return size;
            }
        }
    }
}

#[cfg(test)]
#[cfg(all(feature = "read", feature = "write"))]
mod tests {
    use super::{low_bits_of_byte, low_bits_of_u64, read, write, CONTINUATION_BIT};
    use crate::endianity::NativeEndian;
    use crate::read::{EndianSlice, Error, ReaderOffsetId};

    trait ResultExt {
        fn map_eof(self, input: &[u8]) -> Self;
    }

    impl<T> ResultExt for Result<T, Error> {
        fn map_eof(self, input: &[u8]) -> Self {
            match self {
                Err(Error::UnexpectedEof(id)) => {
                    let id = ReaderOffsetId(id.0 - input.as_ptr() as u64);
                    Err(Error::UnexpectedEof(id))
                }
                r => r,
            }
        }
    }

    #[test]
    fn test_low_bits_of_byte() {
        for i in 0..127 {
            assert_eq!(i, low_bits_of_byte(i));
            assert_eq!(i, low_bits_of_byte(i | CONTINUATION_BIT));
        }
    }

    #[test]
    fn test_low_bits_of_u64() {
        for i in 0u64..127 {
            assert_eq!(i as u8, low_bits_of_u64(1 << 16 | i));
            assert_eq!(
                i as u8,
                low_bits_of_u64(i << 16 | i | (u64::from(CONTINUATION_BIT)))
            );
        }
    }

    // Examples from the DWARF 4 standard, section 7.6, figure 22.
    #[test]
    fn test_read_unsigned() {
        let buf = [2u8];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            2,
            read::unsigned(&mut readable).expect("Should read number")
        );

        let buf = [127u8];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            127,
            read::unsigned(&mut readable).expect("Should read number")
        );

        let buf = [CONTINUATION_BIT, 1];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            128,
            read::unsigned(&mut readable).expect("Should read number")
        );

        let buf = [1u8 | CONTINUATION_BIT, 1];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            129,
            read::unsigned(&mut readable).expect("Should read number")
        );

        let buf = [2u8 | CONTINUATION_BIT, 1];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            130,
            read::unsigned(&mut readable).expect("Should read number")
        );

        let buf = [57u8 | CONTINUATION_BIT, 100];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            12857,
            read::unsigned(&mut readable).expect("Should read number")
        );
    }

    // Examples from the DWARF 4 standard, section 7.6, figure 23.
    #[test]
    fn test_read_signed() {
        let buf = [2u8];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(2, read::signed(&mut readable).expect("Should read number"));

        let buf = [0x7eu8];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(-2, read::signed(&mut readable).expect("Should read number"));

        let buf = [127u8 | CONTINUATION_BIT, 0];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            127,
            read::signed(&mut readable).expect("Should read number")
        );

        let buf = [1u8 | CONTINUATION_BIT, 0x7f];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            -127,
            read::signed(&mut readable).expect("Should read number")
        );

        let buf = [CONTINUATION_BIT, 1];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            128,
            read::signed(&mut readable).expect("Should read number")
        );

        let buf = [CONTINUATION_BIT, 0x7f];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            -128,
            read::signed(&mut readable).expect("Should read number")
        );

        let buf = [1u8 | CONTINUATION_BIT, 1];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            129,
            read::signed(&mut readable).expect("Should read number")
        );

        let buf = [0x7fu8 | CONTINUATION_BIT, 0x7e];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            -129,
            read::signed(&mut readable).expect("Should read number")
        );
    }

    #[test]
    fn test_read_signed_63_bits() {
        let buf = [
            CONTINUATION_BIT,
            CONTINUATION_BIT,
            CONTINUATION_BIT,
            CONTINUATION_BIT,
            CONTINUATION_BIT,
            CONTINUATION_BIT,
            CONTINUATION_BIT,
            CONTINUATION_BIT,
            0x40,
        ];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            -0x4000_0000_0000_0000,
            read::signed(&mut readable).expect("Should read number")
        );
    }

    #[test]
    fn test_read_unsigned_not_enough_data() {
        let buf = [CONTINUATION_BIT];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            read::unsigned(&mut readable).map_eof(&buf),
            Err(Error::UnexpectedEof(ReaderOffsetId(1)))
        );
    }

    #[test]
    fn test_read_signed_not_enough_data() {
        let buf = [CONTINUATION_BIT];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            read::signed(&mut readable).map_eof(&buf),
            Err(Error::UnexpectedEof(ReaderOffsetId(1)))
        );
    }

    #[test]
    fn test_write_unsigned_not_enough_space() {
        let mut buf = [0; 1];
        let mut writable = &mut buf[..];
        match write::unsigned(&mut writable, 128) {
            Err(e) => assert_eq!(e.kind(), std::io::ErrorKind::WriteZero),
            otherwise => panic!("Unexpected: {:?}", otherwise),
        }
    }

    #[test]
    fn test_write_signed_not_enough_space() {
        let mut buf = [0; 1];
        let mut writable = &mut buf[..];
        match write::signed(&mut writable, 128) {
            Err(e) => assert_eq!(e.kind(), std::io::ErrorKind::WriteZero),
            otherwise => panic!("Unexpected: {:?}", otherwise),
        }
    }

    #[test]
    fn dogfood_signed() {
        fn inner(i: i64) {
            let mut buf = [0u8; 1024];

            {
                let mut writable = &mut buf[..];
                write::signed(&mut writable, i).expect("Should write signed number");
            }

            let mut readable = EndianSlice::new(&buf[..], NativeEndian);
            let result = read::signed(&mut readable).expect("Should be able to read it back again");
            assert_eq!(i, result);
        }
        for i in -513..513 {
            inner(i);
        }
        inner(i64::MIN);
    }

    #[test]
    fn dogfood_unsigned() {
        for i in 0..1025 {
            let mut buf = [0u8; 1024];

            {
                let mut writable = &mut buf[..];
                write::unsigned(&mut writable, i).expect("Should write signed number");
            }

            let mut readable = EndianSlice::new(&buf[..], NativeEndian);
            let result =
                read::unsigned(&mut readable).expect("Should be able to read it back again");
            assert_eq!(i, result);
        }
    }

    #[test]
    fn test_read_unsigned_overflow() {
        let buf = [
            2u8 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            1,
        ];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert!(read::unsigned(&mut readable).is_err());
    }

    #[test]
    fn test_read_signed_overflow() {
        let buf = [
            2u8 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            2 | CONTINUATION_BIT,
            1,
        ];
        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert!(read::signed(&mut readable).is_err());
    }

    #[test]
    fn test_read_multiple() {
        let buf = [2u8 | CONTINUATION_BIT, 1u8, 1u8];

        let mut readable = EndianSlice::new(&buf[..], NativeEndian);
        assert_eq!(
            read::unsigned(&mut readable).expect("Should read first number"),
            130u64
        );
        assert_eq!(
            read::unsigned(&mut readable).expect("Should read first number"),
            1u64
        );
    }

    #[test]
    fn test_read_u16() {
        for (buf, val) in [
            (&[2][..], 2),
            (&[0x7f][..], 0x7f),
            (&[0x80, 1][..], 0x80),
            (&[0x81, 1][..], 0x81),
            (&[0x82, 1][..], 0x82),
            (&[0xff, 0x7f][..], 0x3fff),
            (&[0x80, 0x80, 1][..], 0x4000),
            (&[0xff, 0xff, 1][..], 0x7fff),
            (&[0xff, 0xff, 3][..], 0xffff),
        ]
        .iter()
        {
            let mut readable = EndianSlice::new(buf, NativeEndian);
            assert_eq!(*val, read::u16(&mut readable).expect("Should read number"));
        }

        for buf in [
            &[0x80][..],
            &[0x80, 0x80][..],
            &[0x80, 0x80, 4][..],
            &[0x80, 0x80, 0x80, 3][..],
        ]
        .iter()
        {
            let mut readable = EndianSlice::new(buf, NativeEndian);
            assert!(read::u16(&mut readable).is_err(), "{:?}", buf);
        }
    }
}
