// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
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

trait noisy {
    fn speak(&mut self);
}

#[derive(Derivative)]
#[derivative(Clone)]
struct cat {
    meows : usize,

    how_hungry : isize,
    name : String,
}

impl cat {
    fn meow(&mut self) {
        println!("Meow");
        self.meows += 1_usize;
        if self.meows % 5_usize == 0_usize {
            self.how_hungry += 1;
        }
    }
}

impl cat {
    pub fn eat(&mut self) -> bool {
        if self.how_hungry > 0 {
            println!("OM NOM NOM");
            self.how_hungry -= 2;
            true
        } else {
            println!("Not hungry!");
            false
        }
    }
}

impl noisy for cat {
    fn speak(&mut self) { self.meow(); }
}

fn cat(in_x : usize, in_y : isize, in_name: String) -> cat {
    cat {
        meows: in_x,
        how_hungry: in_y,
        name: in_name,
    }
}


fn make_speak<C:noisy>(mut c: C) {
    c.speak();
}

#[test]
fn main() {
    let mut nyan = cat(0_usize, 2, "nyan".to_string());
    nyan.eat();
    assert!((!nyan.eat()));
    for _ in 1_usize..10_usize {
        make_speak(nyan.clone());
    }
}
