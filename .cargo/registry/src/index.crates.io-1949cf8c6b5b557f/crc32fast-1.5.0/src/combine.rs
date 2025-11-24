const POLY: u32 = 0xedb88320;

static X2N_TABLE: [u32; 32] = [
    0x00800000, 0x00008000, 0xedb88320, 0xb1e6b092, 0xa06a2517, 0xed627dae, 0x88d14467, 0xd7bbfe6a,
    0xec447f11, 0x8e7ea170, 0x6427800e, 0x4d47bae0, 0x09fe548f, 0x83852d0f, 0x30362f1a, 0x7b5a9cc3,
    0x31fec169, 0x9fec022a, 0x6c8dedc4, 0x15d6874d, 0x5fde7a4e, 0xbad90e37, 0x2e4e5eef, 0x4eaba214,
    0xa8a472c0, 0x429a969e, 0x148d302a, 0xc40ba6d0, 0xc4e22c3c, 0x40000000, 0x20000000, 0x08000000,
];

// Calculates a(x) multiplied by b(x) modulo p(x), where p(x) is the CRC polynomial,
// reflected. For speed, this requires that a not be zero.
fn multiply(a: u32, mut b: u32) -> u32 {
    let mut p = 0u32;

    for i in 0..32 {
        p ^= b & ((a >> (31 - i)) & 1).wrapping_neg();
        b = (b >> 1) ^ ((b & 1).wrapping_neg() & POLY);
    }

    p
}

pub(crate) fn combine(crc1: u32, crc2: u32, len2: u64) -> u32 {
    // Special case: If the length of the second chunk is zero, return the hash
    // of the first chunk.
    if len2 == 0 {
        return crc1;
    }

    // We are padding the first checksum with len2-amount of zeroes. For efficiency,
    // this is done in powers-of-two via a lookup table rather than one by one.
    let mut p = crc1;
    let n = 64 - len2.leading_zeros();
    for i in 0..n {
        if (len2 >> i & 1) != 0 {
            p = multiply(X2N_TABLE[(i & 0x1F) as usize], p);
        }
    }

    p ^ crc2
}

#[test]
fn golden() {
    assert_eq!(combine(0x0, 0x1, 0x0), 0x0);
    assert_eq!(combine(0xc401f8c9, 0x00000000, 0x0), 0xc401f8c9);
    assert_eq!(combine(0x7cba3d5e, 0xe7466d39, 0xb), 0x76365c4f);
    assert_eq!(combine(0x576c62d6, 0x123256e1, 0x47), 0x579a636);
    assert_eq!(combine(0x4f626f9a, 0x9e5ccbf5, 0xa59d), 0x98d43168);
    assert_eq!(combine(0xa09b8a88, 0x815b0f48, 0x40f39511), 0xd7a5f79);
    assert_eq!(
        combine(0x7f6a4306, 0xbc929646, 0x828cde72b3e25301),
        0xef922dda
    );
}
