#![feature(test)]

extern crate test;

use std::{
    fmt::{self, Display},
    str::FromStr,
};

bitflags::bitflags! {
    struct Flags10: u32 {
        const A = 0b0000_0000_0000_0001;
        const B = 0b0000_0000_0000_0010;
        const C = 0b0000_0000_0000_0100;
        const D = 0b0000_0000_0000_1000;
        const E = 0b0000_0000_0001_0000;
        const F = 0b0000_0000_0010_0000;
        const G = 0b0000_0000_0100_0000;
        const H = 0b0000_0000_1000_0000;
        const I = 0b0000_0001_0000_0000;
        const J = 0b0000_0010_0000_0000;
    }
}

impl FromStr for Flags10 {
    type Err = bitflags::parser::ParseError;

    fn from_str(flags: &str) -> Result<Self, Self::Err> {
        Ok(Flags10(flags.parse()?))
    }
}

impl Display for Flags10 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[bench]
fn format_flags_1_present(b: &mut test::Bencher) {
    b.iter(|| Flags10::J.to_string())
}

#[bench]
fn format_flags_5_present(b: &mut test::Bencher) {
    b.iter(|| (Flags10::F | Flags10::G | Flags10::H | Flags10::I | Flags10::J).to_string())
}

#[bench]
fn format_flags_10_present(b: &mut test::Bencher) {
    b.iter(|| {
        (Flags10::A
            | Flags10::B
            | Flags10::C
            | Flags10::D
            | Flags10::E
            | Flags10::F
            | Flags10::G
            | Flags10::H
            | Flags10::I
            | Flags10::J)
            .to_string()
    })
}

#[bench]
fn parse_flags_1_10(b: &mut test::Bencher) {
    b.iter(|| {
        let flags: Flags10 = "J".parse().unwrap();
        flags
    })
}

#[bench]
fn parse_flags_5_10(b: &mut test::Bencher) {
    b.iter(|| {
        let flags: Flags10 = "F | G | H | I | J".parse().unwrap();
        flags
    })
}

#[bench]
fn parse_flags_10_10(b: &mut test::Bencher) {
    b.iter(|| {
        let flags: Flags10 = "A | B | C | D | E | F | G | H | I | J".parse().unwrap();
        flags
    })
}

#[bench]
fn parse_flags_1_10_hex(b: &mut test::Bencher) {
    b.iter(|| {
        let flags: Flags10 = "0xFF".parse().unwrap();
        flags
    })
}
