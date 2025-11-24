// SPDX-License-Identifier: Apache-2.0 OR MIT

// Refs: https://github.com/rust-lang/rust/issues/82523

#[repr(packed)]
struct Packed {
    f: u32,
}

#[repr(packed(2))]
struct PackedN {
    f: u32,
}

fn main() {
    let a = Packed { f: 1 };
    let _ = &a.f; //~ ERROR reference to packed field is unaligned

    let b = PackedN { f: 1 };
    let _ = &b.f; //~ ERROR reference to packed field is unaligned
}
