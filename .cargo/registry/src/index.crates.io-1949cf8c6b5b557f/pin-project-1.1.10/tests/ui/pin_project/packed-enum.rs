// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::pin_project;

// #[repr(packed)] cannot be apply on enums and will be rejected by rustc.
// However, we should not rely on the behavior of rustc that rejects this.
// https://github.com/taiki-e/pin-project/pull/324#discussion_r612388001

#[repr(packed)] //~ ERROR E0517
enum E1 {
    V(()),
}

#[pin_project]
#[repr(packed)] //~ ERROR E0517
enum E2 {
    V(()),
}

#[repr(packed)] //~ ERROR E0517
#[pin_project]
enum E3 {
    V(()),
}

fn main() {}
