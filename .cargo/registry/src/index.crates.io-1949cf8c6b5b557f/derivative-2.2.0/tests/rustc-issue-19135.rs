// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(feature = "use_core")]
extern crate core;

use std::marker::PhantomData;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Debug)]
struct LifetimeStruct<'a>(PhantomData<&'a ()>);

#[test]
fn main() {
    takes_hrtb_closure(|lts| println!("{:?}", lts));
}

fn takes_hrtb_closure<F: for<'a>FnMut(LifetimeStruct<'a>)>(mut f: F) {
    f(LifetimeStruct(PhantomData));
}
