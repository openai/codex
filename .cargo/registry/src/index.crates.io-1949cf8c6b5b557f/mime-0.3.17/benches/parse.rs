#![feature(test)]

extern crate mime;
extern crate test;

use mime::Mime;
use test::Bencher;


#[bench]
fn bench_from_str(b: &mut Bencher) {
    let s = "text/plain";
    b.bytes = s.as_bytes().len() as u64;
    b.iter(|| s.parse::<Mime>())
}

#[bench]
fn bench_from_str_charset_utf8(b: &mut Bencher) {
    let s = "text/plain; charset=utf-8";
    b.bytes = s.as_bytes().len() as u64;
    b.iter(|| s.parse::<Mime>())
}

#[bench]
fn bench_from_str_extended(b: &mut Bencher) {
    let s = "text/plain; charset=utf-8; foo=bar";
    b.bytes = s.as_bytes().len() as u64;
    b.iter(|| s.parse::<Mime>())
}
