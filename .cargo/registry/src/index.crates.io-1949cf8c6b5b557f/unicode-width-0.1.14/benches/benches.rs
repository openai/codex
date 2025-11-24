// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
#![feature(test)]

extern crate test;

use std::iter;

use test::Bencher;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[bench]
fn cargo(b: &mut Bencher) {
    let string = iter::repeat('a').take(4096).collect::<String>();

    b.iter(|| {
        for c in string.chars() {
            test::black_box(UnicodeWidthChar::width(c));
        }
    });
}

#[bench]
#[allow(deprecated)]
fn stdlib(b: &mut Bencher) {
    let string = iter::repeat('a').take(4096).collect::<String>();

    b.iter(|| {
        for c in string.chars() {
            test::black_box(c.width());
        }
    });
}

#[bench]
fn simple_if(b: &mut Bencher) {
    let string = iter::repeat('a').take(4096).collect::<String>();

    b.iter(|| {
        for c in string.chars() {
            test::black_box(simple_width_if(c));
        }
    });
}

#[bench]
fn simple_match(b: &mut Bencher) {
    let string = iter::repeat('a').take(4096).collect::<String>();

    b.iter(|| {
        for c in string.chars() {
            test::black_box(simple_width_match(c));
        }
    });
}

#[inline]
fn simple_width_if(c: char) -> Option<usize> {
    let cu = c as u32;
    if cu < 127 {
        if cu > 31 {
            Some(1)
        } else if cu == 0 {
            Some(0)
        } else {
            None
        }
    } else {
        UnicodeWidthChar::width(c)
    }
}

#[inline]
fn simple_width_match(c: char) -> Option<usize> {
    match c as u32 {
        cu if cu == 0 => Some(0),
        cu if cu < 0x20 => None,
        cu if cu < 0x7f => Some(1),
        _ => UnicodeWidthChar::width(c),
    }
}

#[bench]
fn enwik8(b: &mut Bencher) {
    // To benchmark, download & unzip `enwik8` from https://data.deepai.org/enwik8.zip
    let data_path = "bench_data/enwik8";
    let string = std::fs::read_to_string(data_path).unwrap_or_default();
    b.iter(|| test::black_box(UnicodeWidthStr::width(string.as_str())));
}

#[bench]
fn jawiki(b: &mut Bencher) {
    // To benchmark, download & extract `jawiki-20220501-pages-articles-multistream-index.txt` from
    // https://dumps.wikimedia.org/jawiki/20220501/jawiki-20220501-pages-articles-multistream-index.txt.bz2
    let data_path = "bench_data/jawiki-20220501-pages-articles-multistream-index.txt";
    let string = std::fs::read_to_string(data_path).unwrap_or_default();
    b.iter(|| test::black_box(UnicodeWidthStr::width(string.as_str())));
}

#[bench]
fn emoji(b: &mut Bencher) {
    // To benchmark, download emoji-style.txt from https://www.unicode.org/emoji/charts/emoji-style.txt
    let data_path = "bench_data/emoji-style.txt";
    let string = std::fs::read_to_string(data_path).unwrap_or_default();
    b.iter(|| test::black_box(UnicodeWidthStr::width(string.as_str())));
}
