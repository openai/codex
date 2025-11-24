#![warn(rust_2018_idioms)]

use bytes::{Buf, BufMut, Bytes};
#[cfg(feature = "std")]
use std::io::IoSlice;

#[test]
fn collect_two_bufs() {
    let a = Bytes::from(&b"hello"[..]);
    let b = Bytes::from(&b"world"[..]);

    let res = a.chain(b).copy_to_bytes(10);
    assert_eq!(res, &b"helloworld"[..]);
}

#[test]
fn writing_chained() {
    let mut a = [0u8; 64];
    let mut b = [0u8; 64];

    {
        let mut buf = (&mut a[..]).chain_mut(&mut b[..]);

        for i in 0u8..128 {
            buf.put_u8(i);
        }
    }

    for i in 0..64 {
        let expect = i as u8;
        assert_eq!(expect, a[i]);
        assert_eq!(expect + 64, b[i]);
    }
}

#[test]
fn iterating_two_bufs() {
    let a = Bytes::from(&b"hello"[..]);
    let b = Bytes::from(&b"world"[..]);

    let res: Vec<u8> = a.chain(b).into_iter().collect();
    assert_eq!(res, &b"helloworld"[..]);
}

#[cfg(feature = "std")]
#[test]
fn vectored_read() {
    let a = Bytes::from(&b"hello"[..]);
    let b = Bytes::from(&b"world"[..]);

    let mut buf = a.chain(b);

    {
        let b1: &[u8] = &mut [];
        let b2: &[u8] = &mut [];
        let b3: &[u8] = &mut [];
        let b4: &[u8] = &mut [];
        let mut iovecs = [
            IoSlice::new(b1),
            IoSlice::new(b2),
            IoSlice::new(b3),
            IoSlice::new(b4),
        ];

        assert_eq!(2, buf.chunks_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"hello"[..]);
        assert_eq!(iovecs[1][..], b"world"[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }

    buf.advance(2);

    {
        let b1: &[u8] = &mut [];
        let b2: &[u8] = &mut [];
        let b3: &[u8] = &mut [];
        let b4: &[u8] = &mut [];
        let mut iovecs = [
            IoSlice::new(b1),
            IoSlice::new(b2),
            IoSlice::new(b3),
            IoSlice::new(b4),
        ];

        assert_eq!(2, buf.chunks_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"llo"[..]);
        assert_eq!(iovecs[1][..], b"world"[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }

    buf.advance(3);

    {
        let b1: &[u8] = &mut [];
        let b2: &[u8] = &mut [];
        let b3: &[u8] = &mut [];
        let b4: &[u8] = &mut [];
        let mut iovecs = [
            IoSlice::new(b1),
            IoSlice::new(b2),
            IoSlice::new(b3),
            IoSlice::new(b4),
        ];

        assert_eq!(1, buf.chunks_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"world"[..]);
        assert_eq!(iovecs[1][..], b""[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }

    buf.advance(3);

    {
        let b1: &[u8] = &mut [];
        let b2: &[u8] = &mut [];
        let b3: &[u8] = &mut [];
        let b4: &[u8] = &mut [];
        let mut iovecs = [
            IoSlice::new(b1),
            IoSlice::new(b2),
            IoSlice::new(b3),
            IoSlice::new(b4),
        ];

        assert_eq!(1, buf.chunks_vectored(&mut iovecs));
        assert_eq!(iovecs[0][..], b"ld"[..]);
        assert_eq!(iovecs[1][..], b""[..]);
        assert_eq!(iovecs[2][..], b""[..]);
        assert_eq!(iovecs[3][..], b""[..]);
    }
}

#[test]
fn chain_growing_buffer() {
    let mut buff = [' ' as u8; 10];
    let mut vec = b"wassup".to_vec();

    let mut chained = (&mut buff[..]).chain_mut(&mut vec).chain_mut(Vec::new()); // Required for potential overflow because remaining_mut for Vec is isize::MAX - vec.len(), but for chain_mut is usize::MAX

    chained.put_slice(b"hey there123123");

    assert_eq!(&buff, b"hey there1");
    assert_eq!(&vec, b"wassup23123");
}

#[test]
fn chain_overflow_remaining_mut() {
    let mut chained = Vec::<u8>::new().chain_mut(Vec::new()).chain_mut(Vec::new());

    assert_eq!(chained.remaining_mut(), usize::MAX);
    chained.put_slice(&[0; 256]);
    assert_eq!(chained.remaining_mut(), usize::MAX);
}

#[test]
fn chain_get_bytes() {
    let mut ab = Bytes::copy_from_slice(b"ab");
    let mut cd = Bytes::copy_from_slice(b"cd");
    let ab_ptr = ab.as_ptr();
    let cd_ptr = cd.as_ptr();
    let mut chain = (&mut ab).chain(&mut cd);
    let a = chain.copy_to_bytes(1);
    let bc = chain.copy_to_bytes(2);
    let d = chain.copy_to_bytes(1);

    assert_eq!(Bytes::copy_from_slice(b"a"), a);
    assert_eq!(Bytes::copy_from_slice(b"bc"), bc);
    assert_eq!(Bytes::copy_from_slice(b"d"), d);

    // assert `get_bytes` did not allocate
    assert_eq!(ab_ptr, a.as_ptr());
    // assert `get_bytes` did not allocate
    assert_eq!(cd_ptr.wrapping_offset(1), d.as_ptr());
}
