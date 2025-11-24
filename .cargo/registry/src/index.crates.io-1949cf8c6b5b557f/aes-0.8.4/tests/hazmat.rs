//! Tests for low-level "hazmat" AES functions.

// TODO(tarcieri): support for using the hazmat functions with the `soft` backend
#![cfg(feature = "hazmat")]

use aes::{Block, Block8};
use hex_literal::hex;

/// Round function tests vectors.
struct RoundTestVector {
    /// State at start of `round[r]`.
    start: [u8; 16],

    /// Key schedule value for `round[r]`.
    k_sch: [u8; 16],

    /// Cipher output.
    output: [u8; 16],
}

/// Cipher round function test vectors from FIPS 197 Appendix C.1.
const CIPHER_ROUND_TEST_VECTORS: &[RoundTestVector] = &[
    // round 1
    RoundTestVector {
        start: hex!("00102030405060708090a0b0c0d0e0f0"),
        k_sch: hex!("d6aa74fdd2af72fadaa678f1d6ab76fe"),
        output: hex!("89d810e8855ace682d1843d8cb128fe4"),
    },
    // round 2
    RoundTestVector {
        start: hex!("89d810e8855ace682d1843d8cb128fe4"),
        k_sch: hex!("b692cf0b643dbdf1be9bc5006830b3fe"),
        output: hex!("4915598f55e5d7a0daca94fa1f0a63f7"),
    },
    // round 3
    RoundTestVector {
        start: hex!("4915598f55e5d7a0daca94fa1f0a63f7"),
        k_sch: hex!("b6ff744ed2c2c9bf6c590cbf0469bf41"),
        output: hex!("fa636a2825b339c940668a3157244d17"),
    },
    // round 4
    RoundTestVector {
        start: hex!("fa636a2825b339c940668a3157244d17"),
        k_sch: hex!("47f7f7bc95353e03f96c32bcfd058dfd"),
        output: hex!("247240236966b3fa6ed2753288425b6c"),
    },
];

/// Equivalent Inverse Cipher round function test vectors from FIPS 197 Appendix C.1.
const EQUIV_INV_CIPHER_ROUND_TEST_VECTORS: &[RoundTestVector] = &[
    // round 1
    RoundTestVector {
        start: hex!("7ad5fda789ef4e272bca100b3d9ff59f"),
        k_sch: hex!("13aa29be9c8faff6f770f58000f7bf03"),
        output: hex!("54d990a16ba09ab596bbf40ea111702f"),
    },
    // round 2
    RoundTestVector {
        start: hex!("54d990a16ba09ab596bbf40ea111702f"),
        k_sch: hex!("1362a4638f2586486bff5a76f7874a83"),
        output: hex!("3e1c22c0b6fcbf768da85067f6170495"),
    },
    // round 3
    RoundTestVector {
        start: hex!("3e1c22c0b6fcbf768da85067f6170495"),
        k_sch: hex!("8d82fc749c47222be4dadc3e9c7810f5"),
        output: hex!("b458124c68b68a014b99f82e5f15554c"),
    },
    // round 4
    RoundTestVector {
        start: hex!("b458124c68b68a014b99f82e5f15554c"),
        k_sch: hex!("72e3098d11c5de5f789dfe1578a2cccb"),
        output: hex!("e8dab6901477d4653ff7f5e2e747dd4f"),
    },
];

#[test]
fn cipher_round_fips197_vectors() {
    for vector in CIPHER_ROUND_TEST_VECTORS {
        let mut block = Block::from(vector.start);
        aes::hazmat::cipher_round(&mut block, &vector.k_sch.into());
        assert_eq!(block.as_slice(), &vector.output);
    }
}

#[test]
fn cipher_round_par_fips197_vectors() {
    let mut blocks = Block8::default();
    let mut round_keys = Block8::default();

    for i in 0..4 {
        let vector = &CIPHER_ROUND_TEST_VECTORS[i];

        blocks[i] = Block::from(vector.start);
        blocks[i + 4] = Block::from(vector.start);

        round_keys[i] = Block::from(vector.k_sch);
        round_keys[i + 4] = Block::from(vector.k_sch);
    }

    aes::hazmat::cipher_round_par(&mut blocks, &round_keys);

    for i in 0..4 {
        let vector = &CIPHER_ROUND_TEST_VECTORS[i];
        assert_eq!(blocks[i].as_slice(), &vector.output);
        assert_eq!(blocks[i + 4].as_slice(), &vector.output);
    }
}

#[test]
fn equiv_inv_cipher_round_fips197_vectors() {
    for vector in EQUIV_INV_CIPHER_ROUND_TEST_VECTORS {
        let mut block = Block::from(vector.start);
        aes::hazmat::equiv_inv_cipher_round(&mut block, &vector.k_sch.into());
        assert_eq!(block.as_slice(), &vector.output);
    }
}

#[test]
fn equiv_inv_cipher_round_par_fips197_vectors() {
    let mut blocks = Block8::default();
    let mut round_keys = Block8::default();

    for i in 0..4 {
        let vector = &EQUIV_INV_CIPHER_ROUND_TEST_VECTORS[i];

        blocks[i] = Block::from(vector.start);
        blocks[i + 4] = Block::from(vector.start);

        round_keys[i] = Block::from(vector.k_sch);
        round_keys[i + 4] = Block::from(vector.k_sch);
    }

    aes::hazmat::equiv_inv_cipher_round_par(&mut blocks, &round_keys);

    for i in 0..4 {
        let vector = &EQUIV_INV_CIPHER_ROUND_TEST_VECTORS[i];
        assert_eq!(blocks[i].as_slice(), &vector.output);
        assert_eq!(blocks[i + 4].as_slice(), &vector.output);
    }
}

#[test]
fn mix_columns_fips197_vector() {
    let mut block = Block::from(hex!("6353e08c0960e104cd70b751bacad0e7"));
    aes::hazmat::mix_columns(&mut block);
    assert_eq!(block.as_slice(), &hex!("5f72641557f5bc92f7be3b291db9f91a"))
}

#[test]
fn inv_mix_columns_fips197_vector() {
    let mut block = Block::from(hex!("bd6e7c3df2b5779e0b61216e8b10b689"));
    aes::hazmat::inv_mix_columns(&mut block);
    assert_eq!(block.as_slice(), &hex!("4773b91ff72f354361cb018ea1e6cf2c"))
}
