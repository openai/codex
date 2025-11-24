#![feature(test)]

extern crate mime;
extern crate test;

use test::Bencher;

#[bench]
fn bench_fmt(b: &mut Bencher) {
    use std::fmt::Write;
    let mime = ::mime::TEXT_PLAIN_UTF_8;
    b.bytes = mime.to_string().as_bytes().len() as u64;
    let mut s = String::with_capacity(64);
    b.iter(|| {
        let _ = write!(s, "{}", mime);
        ::test::black_box(&s);
        unsafe { s.as_mut_vec().set_len(0); }
    })
}
