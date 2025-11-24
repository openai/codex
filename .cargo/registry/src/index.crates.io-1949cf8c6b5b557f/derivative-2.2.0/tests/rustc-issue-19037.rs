// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// pretty-expanded FIXME #23616

#![allow(dead_code)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

struct Str([u8]);

#[derive(Derivative)]
#[derivative(Clone)]
struct CharSplits<'a, Sep> {
    string: &'a Str,
    sep: Sep,
    allow_trailing_empty: bool,
    only_ascii: bool,
    finished: bool,
}

fn clone(s: &Str) -> &Str {
    Clone::clone(&s)
}

#[test]
fn main() {}
