use crate::{Buffer, Permutation};

const ROUNDS: usize = 12;

const RC: [u64; ROUNDS] = [
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

keccak_function!("`keccak-p[1600, 12]`", keccakp, ROUNDS, RC);

pub struct KeccakP;

impl Permutation for KeccakP {
    fn execute(buffer: &mut Buffer) {
        keccakp(buffer.words());
    }
}
