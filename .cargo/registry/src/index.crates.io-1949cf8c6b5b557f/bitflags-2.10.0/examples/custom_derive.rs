//! An example of implementing the `BitFlags` trait manually for a flags type.

use std::str;

use bitflags::bitflags;

// Define a flags type outside of the `bitflags` macro as a newtype
// It can accept custom derives for libraries `bitflags` doesn't support natively
#[derive(zerocopy::IntoBytes, zerocopy::FromBytes, zerocopy::KnownLayout, zerocopy::Immutable)]
#[repr(transparent)]
pub struct ManualFlags(u32);

// Next: use `impl Flags` instead of `struct Flags`
bitflags! {
    impl ManualFlags: u32 {
        const A = 0b00000001;
        const B = 0b00000010;
        const C = 0b00000100;
        const ABC = Self::A.bits() | Self::B.bits() | Self::C.bits();
    }
}

fn main() {}
