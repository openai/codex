// Copyright 2012-2024 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// https://www.rust-lang.org/policies/licenses.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(feature = "bench")]
use std::iter;
#[cfg(feature = "bench")]
use std::prelude::v1::*;
#[cfg(feature = "bench")]
use test::Bencher;

#[cfg(feature = "bench")]
use UnicodeXID;

#[cfg(feature = "bench")]
#[bench]
fn cargo_is_xid_start(b: &mut Bencher) {
    let string = iter::repeat('a').take(4096).collect::<String>();

    b.bytes = string.len() as u64;
    b.iter(|| string.chars().all(super::UnicodeXID::is_xid_start));
}

#[cfg(feature = "bench")]
#[bench]
fn stdlib_is_xid_start(b: &mut Bencher) {
    let string = iter::repeat('a').take(4096).collect::<String>();

    b.bytes = string.len() as u64;
    b.iter(|| string.chars().all(char::is_xid_start));
}

#[cfg(feature = "bench")]
#[bench]
fn cargo_xid_continue(b: &mut Bencher) {
    let string = iter::repeat('a').take(4096).collect::<String>();

    b.bytes = string.len() as u64;
    b.iter(|| string.chars().all(super::UnicodeXID::is_xid_continue));
}

#[cfg(feature = "bench")]
#[bench]
fn stdlib_xid_continue(b: &mut Bencher) {
    let string = iter::repeat('a').take(4096).collect::<String>();

    b.bytes = string.len() as u64;
    b.iter(|| string.chars().all(char::is_xid_continue));
}

#[test]
fn test_is_xid_start() {
    let chars = ['A', 'Z', 'a', 'z', '\u{1000d}', '\u{10026}'];

    for ch in &chars {
        assert!(super::UnicodeXID::is_xid_start(*ch), "{}", ch);
    }
}

#[test]
fn test_is_not_xid_start() {
    let chars = [
        '\x00', '\x01', '0', '9', ' ', '[', '<', '{', '(', '\u{02c2}', '\u{ffff}',
    ];

    for ch in &chars {
        assert!(!super::UnicodeXID::is_xid_start(*ch), "{}", ch);
    }
}

#[test]
fn test_is_xid_continue() {
    let chars = ['0', '9', 'A', 'Z', 'a', 'z', '_', '\u{1000d}', '\u{10026}'];

    for ch in &chars {
        assert!(super::UnicodeXID::is_xid_continue(*ch), "{}", ch);
    }
}

#[test]
fn test_is_not_xid_continue() {
    let chars = [
        '\x00', '\x01', ' ', '[', '<', '{', '(', '\u{02c2}', '\u{ffff}',
    ];

    for &ch in &chars {
        assert!(!super::UnicodeXID::is_xid_continue(ch), "{}", ch);
    }
}
