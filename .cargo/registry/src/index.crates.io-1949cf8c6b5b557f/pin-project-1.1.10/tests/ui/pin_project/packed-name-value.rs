// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::pin_project;

// #[repr(packed = "")] is not valid format of #[repr(packed)] and will be
// rejected by rustc.
// However, we should not rely on the behavior of rustc that rejects this.
// https://github.com/taiki-e/pin-project/pull/324#discussion_r612388001

// https://github.com/taiki-e/pin-project/pull/324#discussion_r612388001
// https://github.com/rust-lang/rust/issues/83921
// #[repr(packed = "")] //~ ERROR E0552
// struct S1 {
//     f: (),
// }

#[pin_project]
#[repr(packed = "")] //~ ERROR attribute should not be name-value pair
struct S2 {
    f: (),
}

#[repr(packed = "")] //~ ERROR attribute should not be name-value pair
#[pin_project]
struct S3 {
    f: (),
}

fn main() {}
