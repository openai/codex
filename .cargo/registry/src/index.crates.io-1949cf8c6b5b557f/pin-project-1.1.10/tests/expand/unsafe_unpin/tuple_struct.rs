// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::{pin_project, UnsafeUnpin};

#[pin_project(UnsafeUnpin)]
struct TupleStruct<T, U>(#[pin] T, U);

unsafe impl<T: Unpin, U> UnsafeUnpin for TupleStruct<T, U> {}

fn main() {}
