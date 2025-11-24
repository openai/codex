// Copyright 2022 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

include!("../../zerocopy-derive/tests/include.rs");

extern crate zerocopy;

use util::NotZerocopy;
use zerocopy::{Immutable, IntoBytes};

fn main() {
    // This is adapted from #1296, which includes the following text:
    //
    //   The compiler errors when a type is missing Immutable are somewhat
    //   misleading, although I'm not sure there's much zerocopy can do about
    //   this. An example where the compiler recommends adding a reference
    //   rather than implementing Immutable (some were even more confusing than
    //   this):
    //
    //   error[E0277]: the trait bound `virtio::wl::CtrlVfdNewDmabuf: zerocopy::Immutable` is not satisfied
    //      --> devices/src/virtio/wl.rs:317:20
    //       |
    //   317 |         .write_obj(ctrl_vfd_new_dmabuf)
    //       |          --------- ^^^^^^^^^^^^^^^^^^^ the trait `zerocopy::Immutable` is not implemented for `virtio::wl::CtrlVfdNewDmabuf`
    //       |          |
    //       |          required by a bound introduced by this call
    //       |
    //   note: required by a bound in `virtio::descriptor_utils::Writer::write_obj`
    //      --> devices/src/virtio/descriptor_utils.rs:536:25
    //       |
    //   536 |     pub fn write_obj<T: Immutable + IntoBytes>(&mut self, val: T) -> io::Result<()> {
    //       |                         ^^^^^^^^^ required by this bound in `Writer::write_obj`
    //   help: consider borrowing here
    //       |
    //   317 |         .write_obj(&ctrl_vfd_new_dmabuf)
    //       |                    +
    //   317 |         .write_obj(&mut ctrl_vfd_new_dmabuf)
    //       |                    ++++
    //
    //   Taking the compiler's suggestion results in a different error with a
    //   recommendation to remove the reference (back to the original code).
    //
    // As of this writing, the described problem is still happening thanks to
    // https://github.com/rust-lang/rust/issues/130563. We include this test so
    // that we can capture the current behavior, but we will update it once that
    // Rust issue is fixed.
    Foo.write_obj(NotZerocopy(()));
}

struct Foo;

impl Foo {
    fn write_obj<T: Immutable + IntoBytes>(&mut self, _val: T) {}
}
