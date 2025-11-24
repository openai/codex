#![warn(rust_2018_idioms)]

use ::bytes::{Buf, Bytes, BytesMut};
use core::{cmp, mem};
use std::collections::VecDeque;
#[cfg(feature = "std")]
use std::io::IoSlice;

// A random 64-byte ascii string, with the first 8 bytes altered to
// give valid representations of f32 and f64 (making them easier to compare)
// and negative signed numbers when interpreting as big endian
// (testing Sign Extension for `Buf::get_int' and `Buf::get_int_ne`).
const INPUT: &[u8] = b"\xffFqrjrDqPhvTc45vvq33f6bJrUtyHESuTeklWKgYd64xgzxJwvAkpYYnpNJyZSRn";

macro_rules! e {
    ($big_endian_val:expr, $little_endian_val:expr) => {
        if cfg!(target_endian = "big") {
            $big_endian_val
        } else {
            $little_endian_val
        }
    };
}

macro_rules! buf_tests {
    ($make_input:ident) => {
        buf_tests!($make_input, true);
    };
    ($make_input:ident, $checks_vectored_is_complete:expr) => {
        use super::*;

        #[test]
        fn empty_state() {
            let buf = $make_input(&[]);
            assert_eq!(buf.remaining(), 0);
            assert!(!buf.has_remaining());
            assert!(buf.chunk().is_empty());
        }

        #[test]
        fn fresh_state() {
            let buf = $make_input(INPUT);
            assert_eq!(buf.remaining(), 64);
            assert!(buf.has_remaining());

            let chunk = buf.chunk();
            assert!(chunk.len() <= 64);
            assert!(INPUT.starts_with(chunk));
        }

        #[test]
        fn advance() {
            let mut buf = $make_input(INPUT);
            buf.advance(8);
            assert_eq!(buf.remaining(), 64 - 8);
            assert!(buf.has_remaining());

            let chunk = buf.chunk();
            assert!(chunk.len() <= 64 - 8);
            assert!(INPUT[8..].starts_with(chunk));
        }

        #[test]
        fn advance_to_end() {
            let mut buf = $make_input(INPUT);
            buf.advance(64);
            assert_eq!(buf.remaining(), 0);
            assert!(!buf.has_remaining());

            let chunk = buf.chunk();
            assert!(chunk.is_empty());
        }

        #[test]
        #[should_panic]
        fn advance_past_end() {
            let  mut buf = $make_input(INPUT);
            buf.advance(65);
        }

        #[test]
        #[cfg(feature = "std")]
        fn chunks_vectored_empty() {
            let  buf = $make_input(&[]);
            let mut bufs = [IoSlice::new(&[]); 16];

            let n = buf.chunks_vectored(&mut bufs);
            assert_eq!(n, 0);
            assert!(bufs.iter().all(|buf| buf.is_empty()));
        }

        #[test]
        #[cfg(feature = "std")]
        fn chunks_vectored_is_complete() {
            let buf = $make_input(INPUT);
            let mut bufs = [IoSlice::new(&[]); 16];

            let n = buf.chunks_vectored(&mut bufs);
            assert!(n > 0);
            assert!(n <= 16);

            let bufs_concat = bufs[..n]
                .iter()
                .flat_map(|b| b.iter().copied())
                .collect::<Vec<u8>>();
            if $checks_vectored_is_complete {
                assert_eq!(bufs_concat, INPUT);
            } else {
                // If this panics then `buf` implements `chunks_vectored`.
                // Remove the `false` argument from `buf_tests!` for that type.
                assert!(bufs_concat.len() < INPUT.len());
                assert!(INPUT.starts_with(&bufs_concat));
            }

            for i in n..16 {
                assert!(bufs[i].is_empty());
            }
        }

        #[test]
        fn copy_to_slice() {
            let mut buf = $make_input(INPUT);

            let mut chunk = [0u8; 8];
            buf.copy_to_slice(&mut chunk);
            assert_eq!(buf.remaining(), 64 - 8);
            assert!(buf.has_remaining());
            assert_eq!(chunk, INPUT[..8]);

            let chunk = buf.chunk();
            assert!(chunk.len() <= 64 - 8);
            assert!(INPUT[8..].starts_with(chunk));
        }

        #[test]
        fn copy_to_slice_big() {
            let mut buf = $make_input(INPUT);

            let mut chunk = [0u8; 56];
            buf.copy_to_slice(&mut chunk);
            assert_eq!(buf.remaining(), 64 - 56);
            assert!(buf.has_remaining());
            assert_eq!(chunk, INPUT[..56]);

            let chunk = buf.chunk();
            assert!(chunk.len() <= 64 - 56);
            assert!(INPUT[56..].starts_with(chunk));
        }

        #[test]
        fn copy_to_slice_to_end() {
            let mut buf = $make_input(INPUT);

            let mut chunk = [0u8; 64];
            buf.copy_to_slice(&mut chunk);
            assert_eq!(buf.remaining(), 0);
            assert!(!buf.has_remaining());
            assert_eq!(chunk, INPUT);

            assert!(buf.chunk().is_empty());
        }

        #[test]
        #[should_panic]
        fn copy_to_slice_overflow() {
            let mut buf = $make_input(INPUT);

            let mut chunk = [0u8; 65];
            buf.copy_to_slice(&mut chunk);
        }

        #[test]
        fn copy_to_bytes() {
            let mut buf = $make_input(INPUT);

            let chunk = buf.copy_to_bytes(8);
            assert_eq!(buf.remaining(), 64 - 8);
            assert!(buf.has_remaining());
            assert_eq!(chunk, INPUT[..8]);

            let chunk = buf.chunk();
            assert!(chunk.len() <= 64 - 8);
            assert!(INPUT[8..].starts_with(chunk));
        }

        #[test]
        fn copy_to_bytes_big() {
            let mut buf = $make_input(INPUT);

            let chunk = buf.copy_to_bytes(56);
            assert_eq!(buf.remaining(), 64 - 56);
            assert!(buf.has_remaining());
            assert_eq!(chunk, INPUT[..56]);

            let chunk = buf.chunk();
            assert!(chunk.len() <= 64 - 56);
            assert!(INPUT[56..].starts_with(chunk));
        }

        #[test]
        fn copy_to_bytes_to_end() {
            let mut buf = $make_input(INPUT);

            let chunk = buf.copy_to_bytes(64);
            assert_eq!(buf.remaining(), 0);
            assert!(!buf.has_remaining());
            assert_eq!(chunk, INPUT);

            assert!(buf.chunk().is_empty());
        }

        #[test]
        #[should_panic]
        fn copy_to_bytes_overflow() {
            let mut buf = $make_input(INPUT);

            let _ = buf.copy_to_bytes(65);
        }

        buf_tests!(number $make_input, get_u8, get_u8_overflow, u8, get_u8, 0xff);
        buf_tests!(number $make_input, get_i8, get_i8_overflow, i8, get_i8, 0xffu8 as i8);
        buf_tests!(number $make_input, get_u16_be, get_u16_be_overflow, u16, get_u16, 0xff46);
        buf_tests!(number $make_input, get_u16_le, get_u16_le_overflow, u16, get_u16_le, 0x46ff);
        buf_tests!(number $make_input, get_u16_ne, get_u16_ne_overflow, u16, get_u16_ne, e!(0xff46, 0x46ff));
        buf_tests!(number $make_input, get_i16_be, get_i16_be_overflow, i16, get_i16, 0xff46u16 as i16);
        buf_tests!(number $make_input, get_i16_le, get_i16_le_overflow, i16, get_i16_le, 0x46ff);
        buf_tests!(number $make_input, get_i16_ne, get_i16_ne_overflow, i16, get_i16_ne, e!(0xff46u16 as i16, 0x46ff));
        buf_tests!(number $make_input, get_u32_be, get_u32_be_overflow, u32, get_u32, 0xff467172);
        buf_tests!(number $make_input, get_u32_le, get_u32_le_overflow, u32, get_u32_le, 0x727146ff);
        buf_tests!(number $make_input, get_u32_ne, get_u32_ne_overflow, u32, get_u32_ne, e!(0xff467172, 0x727146ff));
        buf_tests!(number $make_input, get_i32_be, get_i32_be_overflow, i32, get_i32, 0xff467172u32 as i32);
        buf_tests!(number $make_input, get_i32_le, get_i32_le_overflow, i32, get_i32_le, 0x727146ff);
        buf_tests!(number $make_input, get_i32_ne, get_i32_ne_overflow, i32, get_i32_ne, e!(0xff467172u32 as i32, 0x727146ff));
        buf_tests!(number $make_input, get_u64_be, get_u64_be_overflow, u64, get_u64, 0xff4671726a724471);
        buf_tests!(number $make_input, get_u64_le, get_u64_le_overflow, u64, get_u64_le, 0x7144726a727146ff);
        buf_tests!(number $make_input, get_u64_ne, get_u64_ne_overflow, u64, get_u64_ne, e!(0xff4671726a724471, 0x7144726a727146ff));
        buf_tests!(number $make_input, get_i64_be, get_i64_be_overflow, i64, get_i64, 0xff4671726a724471u64 as i64);
        buf_tests!(number $make_input, get_i64_le, get_i64_le_overflow, i64, get_i64_le, 0x7144726a727146ff);
        buf_tests!(number $make_input, get_i64_ne, get_i64_ne_overflow, i64, get_i64_ne, e!(0xff4671726a724471u64 as i64, 0x7144726a727146ff));
        buf_tests!(number $make_input, get_u128_be, get_u128_be_overflow, u128, get_u128, 0xff4671726a7244715068765463343576);
        buf_tests!(number $make_input, get_u128_le, get_u128_le_overflow, u128, get_u128_le, 0x76353463547668507144726a727146ff);
        buf_tests!(number $make_input, get_u128_ne, get_u128_ne_overflow, u128, get_u128_ne, e!(0xff4671726a7244715068765463343576, 0x76353463547668507144726a727146ff));
        buf_tests!(number $make_input, get_i128_be, get_i128_be_overflow, i128, get_i128, 0xff4671726a7244715068765463343576u128 as i128);
        buf_tests!(number $make_input, get_i128_le, get_i128_le_overflow, i128, get_i128_le, 0x76353463547668507144726a727146ff);
        buf_tests!(number $make_input, get_i128_ne, get_i128_ne_overflow, i128, get_i128_ne, e!(0xff4671726a7244715068765463343576u128 as i128, 0x76353463547668507144726a727146ff));
        buf_tests!(number $make_input, get_f32_be, get_f32_be_overflow, f32, get_f32, f32::from_bits(0xff467172));
        buf_tests!(number $make_input, get_f32_le, get_f32_le_overflow, f32, get_f32_le, f32::from_bits(0x727146ff));
        buf_tests!(number $make_input, get_f32_ne, get_f32_ne_overflow, f32, get_f32_ne, f32::from_bits(e!(0xff467172, 0x727146ff)));
        buf_tests!(number $make_input, get_f64_be, get_f64_be_overflow, f64, get_f64, f64::from_bits(0xff4671726a724471));
        buf_tests!(number $make_input, get_f64_le, get_f64_le_overflow, f64, get_f64_le, f64::from_bits(0x7144726a727146ff));
        buf_tests!(number $make_input, get_f64_ne, get_f64_ne_overflow, f64, get_f64_ne, f64::from_bits(e!(0xff4671726a724471, 0x7144726a727146ff)));

        buf_tests!(var_number $make_input, get_uint_be, get_uint_be_overflow, u64, get_uint, 3, 0xff4671);
        buf_tests!(var_number $make_input, get_uint_le, get_uint_le_overflow, u64, get_uint_le, 3, 0x7146ff);
        buf_tests!(var_number $make_input, get_uint_ne, get_uint_ne_overflow, u64, get_uint_ne, 3, e!(0xff4671, 0x7146ff));
        buf_tests!(var_number $make_input, get_int_be, get_int_be_overflow, i64, get_int, 3, 0xffffffffffff4671u64 as i64);
        buf_tests!(var_number $make_input, get_int_le, get_int_le_overflow, i64, get_int_le, 3, 0x7146ff);
        buf_tests!(var_number $make_input, get_int_ne, get_int_ne_overflow, i64, get_int_ne, 3, e!(0xffffffffffff4671u64 as i64, 0x7146ff));
    };
    (number $make_input:ident, $ok_name:ident, $panic_name:ident, $number:ty, $method:ident, $value:expr) => {
        #[test]
        fn $ok_name() {
            let mut buf = $make_input(INPUT);

            let value = buf.$method();
            assert_eq!(buf.remaining(), 64 - mem::size_of::<$number>());
            assert!(buf.has_remaining());
            assert_eq!(value, $value);
        }

        #[test]
        #[should_panic]
        fn $panic_name() {
            let mut buf = $make_input(&[]);

            let _ = buf.$method();
        }
    };
    (var_number $make_input:ident, $ok_name:ident, $panic_name:ident, $number:ty, $method:ident, $len:expr, $value:expr) => {
        #[test]
        fn $ok_name() {
            let mut buf = $make_input(INPUT);

            let value = buf.$method($len);
            assert_eq!(buf.remaining(), 64 - $len);
            assert!(buf.has_remaining());
            assert_eq!(value, $value);
        }

        #[test]
        #[should_panic]
        fn $panic_name() {
            let mut buf = $make_input(&[]);

            let _ = buf.$method($len);
        }
    };
}

mod u8_slice {
    fn make_input(buf: &'static [u8]) -> &'static [u8] {
        buf
    }

    buf_tests!(make_input);
}

mod bytes {
    fn make_input(buf: &'static [u8]) -> impl Buf {
        Bytes::from_static(buf)
    }

    buf_tests!(make_input);
}

mod bytes_mut {
    fn make_input(buf: &'static [u8]) -> impl Buf {
        BytesMut::from(buf)
    }

    buf_tests!(make_input);
}

mod vec_deque {
    fn make_input(buf: &'static [u8]) -> impl Buf {
        let mut deque = VecDeque::new();

        if !buf.is_empty() {
            // Construct |b|some bytes|a| `VecDeque`
            let mid = buf.len() / 2;
            let (a, b) = buf.split_at(mid);

            deque.reserve_exact(buf.len() + 1);

            let extra_space = deque.capacity() - b.len() - 1;
            deque.resize(extra_space, 0);

            deque.extend(a);
            deque.drain(..extra_space);
            deque.extend(b);

            let (a, b) = deque.as_slices();
            assert!(
                !a.is_empty(),
                "could not setup test - attempt to create discontiguous VecDeque failed"
            );
            assert!(
                !b.is_empty(),
                "could not setup test - attempt to create discontiguous VecDeque failed"
            );
        }

        deque
    }

    buf_tests!(make_input, true);
}

#[cfg(feature = "std")]
mod cursor {
    use std::io::Cursor;

    fn make_input(buf: &'static [u8]) -> impl Buf {
        Cursor::new(buf)
    }

    buf_tests!(make_input);
}

mod box_bytes {
    fn make_input(buf: &'static [u8]) -> impl Buf {
        Box::new(Bytes::from_static(buf))
    }

    buf_tests!(make_input);
}

mod chain_u8_slice {
    fn make_input(buf: &'static [u8]) -> impl Buf {
        let (a, b) = buf.split_at(buf.len() / 2);
        Buf::chain(a, b)
    }

    buf_tests!(make_input);
}

mod chain_small_big_u8_slice {
    fn make_input(buf: &'static [u8]) -> impl Buf {
        let mid = cmp::min(1, buf.len());
        let (a, b) = buf.split_at(mid);
        Buf::chain(a, b)
    }

    buf_tests!(make_input);
}

mod chain_limited_slices {
    fn make_input(buf: &'static [u8]) -> impl Buf {
        let buf3 = &buf[cmp::min(buf.len(), 3)..];
        let a = Buf::take(buf3, 0);
        let b = Buf::take(buf, 3);
        let c = Buf::take(buf3, usize::MAX);
        let d = buf;
        Buf::take(Buf::chain(Buf::chain(a, b), Buf::chain(c, d)), buf.len())
    }

    buf_tests!(make_input, true);
}

#[allow(unused_allocation)] // This is intentional.
#[test]
fn test_deref_buf_forwards() {
    struct Special;

    impl Buf for Special {
        fn remaining(&self) -> usize {
            unreachable!("remaining");
        }

        fn chunk(&self) -> &[u8] {
            unreachable!("chunk");
        }

        fn advance(&mut self, _: usize) {
            unreachable!("advance");
        }

        fn get_u8(&mut self) -> u8 {
            // specialized!
            b'x'
        }
    }

    // these should all use the specialized method
    assert_eq!(Special.get_u8(), b'x');
    assert_eq!((&mut Special as &mut dyn Buf).get_u8(), b'x');
    assert_eq!((Box::new(Special) as Box<dyn Buf>).get_u8(), b'x');
    assert_eq!(Box::new(Special).get_u8(), b'x');
}
