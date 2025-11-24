// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(clippy::derive_hash_xor_eq)]

#[cfg(feature = "use_core")]
extern crate core;

use std::collections::HashSet;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Hash)]
#[derive(Copy, Clone, PartialEq, Eq)]
struct XYZ {
    x: isize,
    y: isize,
    z: isize
}

#[test]
fn main() {
    let mut connected = HashSet::new();
    let mut border = HashSet::new();

    let middle = XYZ{x: 0, y: 0, z: 0};
    border.insert(middle);

    while !border.is_empty() && connected.len() < 10000 {
        let choice = *(border.iter().next().unwrap());
        border.remove(&choice);
        connected.insert(choice);

        let cxp = XYZ{x: choice.x + 1, y: choice.y, z: choice.z};
        let cxm = XYZ{x: choice.x - 1, y: choice.y, z: choice.z};
        let cyp = XYZ{x: choice.x, y: choice.y + 1, z: choice.z};
        let cym = XYZ{x: choice.x, y: choice.y - 1, z: choice.z};
        let czp = XYZ{x: choice.x, y: choice.y, z: choice.z + 1};
        let czm = XYZ{x: choice.x, y: choice.y, z: choice.z - 1};

        if !connected.contains(&cxp) {
            border.insert(cxp);
        }
        if  !connected.contains(&cxm){
            border.insert(cxm);
        }
        if !connected.contains(&cyp){
            border.insert(cyp);
        }
        if !connected.contains(&cym) {
            border.insert(cym);
        }
        if !connected.contains(&czp){
            border.insert(czp);
        }
        if !connected.contains(&czm) {
            border.insert(czm);
        }
    }
}
