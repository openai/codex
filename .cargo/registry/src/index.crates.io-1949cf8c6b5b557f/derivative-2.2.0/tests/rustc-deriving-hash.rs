// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_camel_case_types)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[derive(Derivative)]
#[derivative(Hash)]
struct Person {
    id: u16,
    name: String,
    phone: u64,
}

// test for hygiene name collisions
#[derive(Derivative)]
#[derivative(Hash)] struct __H__H;
#[derive(Derivative)]
#[allow(dead_code)] #[derivative(Hash)] struct Collision<__H> ( __H );
// TODO(rustc) #[derivative(Hash)] enum Collision<__H> { __H { __H__H: __H } }

#[derive(Derivative)]
#[derivative(Hash)]
enum E { A=1, B }

fn hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

struct FakeHasher<'a>(&'a mut Vec<u8>);
impl<'a> Hasher for FakeHasher<'a> {
    fn finish(&self) -> u64 {
        unimplemented!()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.extend(bytes);
    }
}

fn fake_hash(v: &mut Vec<u8>, e: E) {
    e.hash(&mut FakeHasher(v));
}

#[test]
fn main() {
    let person1 = Person {
        id: 5,
        name: "Janet".to_string(),
        phone: 555_666_777,
    };
    let person2 = Person {
        id: 5,
        name: "Bob".to_string(),
        phone: 555_666_777,
    };
    assert_eq!(hash(&person1), hash(&person1));
    assert!(hash(&person1) != hash(&person2));

    // test #21714
    let mut va = vec![];
    let mut vb = vec![];
    fake_hash(&mut va, E::A);
    fake_hash(&mut vb, E::B);
    assert!(va != vb);
}
