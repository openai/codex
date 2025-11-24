#![feature(test)]
#![warn(rust_2018_idioms)]

extern crate test;

use bytes::Bytes;
use test::Bencher;

#[bench]
fn deref_unique(b: &mut Bencher) {
    let buf = Bytes::from(vec![0; 1024]);

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(&buf[..]);
        }
    })
}

#[bench]
fn deref_shared(b: &mut Bencher) {
    let buf = Bytes::from(vec![0; 1024]);
    let _b2 = buf.clone();

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(&buf[..]);
        }
    })
}

#[bench]
fn deref_static(b: &mut Bencher) {
    let buf = Bytes::from_static(b"hello world");

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(&buf[..]);
        }
    })
}

#[bench]
fn clone_static(b: &mut Bencher) {
    let bytes =
        Bytes::from_static("hello world 1234567890 and have a good byte 0987654321".as_bytes());

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(test::black_box(&bytes).clone());
        }
    })
}

#[bench]
fn clone_shared(b: &mut Bencher) {
    let bytes = Bytes::from(b"hello world 1234567890 and have a good byte 0987654321".to_vec());

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(test::black_box(&bytes).clone());
        }
    })
}

#[bench]
fn clone_arc_vec(b: &mut Bencher) {
    use std::sync::Arc;
    let bytes = Arc::new(b"hello world 1234567890 and have a good byte 0987654321".to_vec());

    b.iter(|| {
        for _ in 0..1024 {
            test::black_box(test::black_box(&bytes).clone());
        }
    })
}

#[bench]
fn from_long_slice(b: &mut Bencher) {
    let data = [0u8; 128];
    b.bytes = data.len() as u64;
    b.iter(|| {
        let buf = Bytes::copy_from_slice(&data[..]);
        test::black_box(buf);
    })
}

#[bench]
fn slice_empty(b: &mut Bencher) {
    b.iter(|| {
        // `clone` is to convert to ARC
        let b = Bytes::from(vec![17; 1024]).clone();
        for i in 0..1000 {
            test::black_box(b.slice(i % 100..i % 100));
        }
    })
}

#[bench]
fn slice_short_from_arc(b: &mut Bencher) {
    b.iter(|| {
        // `clone` is to convert to ARC
        let b = Bytes::from(vec![17; 1024]).clone();
        for i in 0..1000 {
            test::black_box(b.slice(1..2 + i % 10));
        }
    })
}

#[bench]
fn split_off_and_drop(b: &mut Bencher) {
    b.iter(|| {
        for _ in 0..1024 {
            let v = vec![10; 200];
            let mut b = Bytes::from(v);
            test::black_box(b.split_off(100));
            test::black_box(b);
        }
    })
}
