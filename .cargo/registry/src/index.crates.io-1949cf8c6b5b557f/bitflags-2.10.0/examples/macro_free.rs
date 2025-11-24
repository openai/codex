//! An example of implementing the `BitFlags` trait manually for a flags type.
//!
//! This example doesn't use any macros.

use std::{fmt, str};

use bitflags::{Flag, Flags};

// First: Define your flags type. It just needs to be `Sized + 'static`.
pub struct ManualFlags(u32);

// Not required: Define some constants for valid flags
impl ManualFlags {
    pub const A: ManualFlags = ManualFlags(0b00000001);
    pub const B: ManualFlags = ManualFlags(0b00000010);
    pub const C: ManualFlags = ManualFlags(0b00000100);
    pub const ABC: ManualFlags = ManualFlags(0b00000111);
}

// Next: Implement the `BitFlags` trait, specifying your set of valid flags
// and iterators
impl Flags for ManualFlags {
    const FLAGS: &'static [Flag<Self>] = &[
        Flag::new("A", Self::A),
        Flag::new("B", Self::B),
        Flag::new("C", Self::C),
    ];

    type Bits = u32;

    fn bits(&self) -> u32 {
        self.0
    }

    fn from_bits_retain(bits: u32) -> Self {
        Self(bits)
    }
}

// Not required: Add parsing support
impl str::FromStr for ManualFlags {
    type Err = bitflags::parser::ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        bitflags::parser::from_str(input)
    }
}

// Not required: Add formatting support
impl fmt::Display for ManualFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        bitflags::parser::to_writer(self, f)
    }
}

fn main() {
    println!(
        "{}",
        ManualFlags::A.union(ManualFlags::B).union(ManualFlags::C)
    );
}
