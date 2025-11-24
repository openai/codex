use crate::limb::LeakyLimb;
use core::mem::size_of;

const fn parse_digit(d: u8) -> u8 {
    match d.to_ascii_lowercase() {
        b'0'..=b'9' => d - b'0',
        b'a'..=b'f' => d - b'a' + 10,
        _ => panic!(),
    }
}

// TODO: this would be nicer as a trait, but currently traits don't support const functions
pub const fn limbs_from_hex<const LIMBS: usize>(hex: &str) -> [LeakyLimb; LIMBS] {
    let hex = hex.as_bytes();
    let mut limbs = [0; LIMBS];
    let limb_nibbles = size_of::<LeakyLimb>() * 2;
    let mut i = 0;

    while i < hex.len() {
        let char = hex[hex.len() - 1 - i];
        let val = parse_digit(char);
        limbs[i / limb_nibbles] |= (val as LeakyLimb) << ((i % limb_nibbles) * 4);
        i += 1;
    }

    limbs
}
