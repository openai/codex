use std::ops::{BitAnd, BitOr, BitXor, Not};

use bitflags::{Bits, Flag, Flags};

// Define a custom container that can be used in flags types
// Note custom bits types can't be used in `bitflags!`
// without making the trait impls `const`. This is currently
// unstable
#[derive(Clone, Copy, Debug)]
pub struct CustomBits([bool; 3]);

impl Bits for CustomBits {
    const EMPTY: Self = CustomBits([false; 3]);

    const ALL: Self = CustomBits([true; 3]);
}

impl PartialEq for CustomBits {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl BitAnd for CustomBits {
    type Output = Self;

    fn bitand(self, other: Self) -> Self {
        CustomBits([
            self.0[0] & other.0[0],
            self.0[1] & other.0[1],
            self.0[2] & other.0[2],
        ])
    }
}

impl BitOr for CustomBits {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        CustomBits([
            self.0[0] | other.0[0],
            self.0[1] | other.0[1],
            self.0[2] | other.0[2],
        ])
    }
}

impl BitXor for CustomBits {
    type Output = Self;

    fn bitxor(self, other: Self) -> Self {
        CustomBits([
            self.0[0] & other.0[0],
            self.0[1] & other.0[1],
            self.0[2] & other.0[2],
        ])
    }
}

impl Not for CustomBits {
    type Output = Self;

    fn not(self) -> Self {
        CustomBits([!self.0[0], !self.0[1], !self.0[2]])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CustomFlags(CustomBits);

impl CustomFlags {
    pub const A: Self = CustomFlags(CustomBits([true, false, false]));
    pub const B: Self = CustomFlags(CustomBits([false, true, false]));
    pub const C: Self = CustomFlags(CustomBits([false, false, true]));
}

impl Flags for CustomFlags {
    const FLAGS: &'static [Flag<Self>] = &[
        Flag::new("A", Self::A),
        Flag::new("B", Self::B),
        Flag::new("C", Self::C),
    ];

    type Bits = CustomBits;

    fn bits(&self) -> Self::Bits {
        self.0
    }

    fn from_bits_retain(bits: Self::Bits) -> Self {
        CustomFlags(bits)
    }
}

fn main() {
    println!("{:?}", CustomFlags::A.union(CustomFlags::C));
}
