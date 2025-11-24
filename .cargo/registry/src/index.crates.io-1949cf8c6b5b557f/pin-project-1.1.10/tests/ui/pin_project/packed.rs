// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project::pin_project;

#[pin_project]
#[repr(packed, C)] //~ ERROR may not be used on #[repr(packed)] types
struct Packed1 {
    #[pin]
    f: u8,
}

// Test putting 'repr' before the 'pin_project' attribute
#[repr(packed, C)] //~ ERROR may not be used on #[repr(packed)] types
#[pin_project]
struct Packed2 {
    #[pin]
    f: u8,
}

#[pin_project]
#[repr(packed(2))] //~ ERROR may not be used on #[repr(packed)] types
struct PackedN1 {
    #[pin]
    f: u32,
}

// Test putting 'repr' before the 'pin_project' attribute
#[repr(packed(2))] //~ ERROR may not be used on #[repr(packed)] types
#[pin_project]
struct PackedN2 {
    #[pin]
    f: u32,
}

fn main() {}
