#![allow(dead_code, clippy::unreadable_literal)]

pub const STATE_LEN: usize = 8;
pub const BLOCK_LEN: usize = 16;

pub type State256 = [u32; STATE_LEN];
pub type State512 = [u64; STATE_LEN];

/// Constants necessary for SHA-256 family of digests.
pub const K32: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Constants necessary for SHA-256 family of digests.
pub const K32X4: [[u32; 4]; 16] = [
    [K32[3], K32[2], K32[1], K32[0]],
    [K32[7], K32[6], K32[5], K32[4]],
    [K32[11], K32[10], K32[9], K32[8]],
    [K32[15], K32[14], K32[13], K32[12]],
    [K32[19], K32[18], K32[17], K32[16]],
    [K32[23], K32[22], K32[21], K32[20]],
    [K32[27], K32[26], K32[25], K32[24]],
    [K32[31], K32[30], K32[29], K32[28]],
    [K32[35], K32[34], K32[33], K32[32]],
    [K32[39], K32[38], K32[37], K32[36]],
    [K32[43], K32[42], K32[41], K32[40]],
    [K32[47], K32[46], K32[45], K32[44]],
    [K32[51], K32[50], K32[49], K32[48]],
    [K32[55], K32[54], K32[53], K32[52]],
    [K32[59], K32[58], K32[57], K32[56]],
    [K32[63], K32[62], K32[61], K32[60]],
];

/// Constants necessary for SHA-512 family of digests.
pub const K64: [u64; 80] = [
    0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc,
    0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118,
    0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2,
    0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694,
    0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
    0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5,
    0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70,
    0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df,
    0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
    0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30,
    0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8,
    0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
    0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b,
    0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178,
    0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b,
    0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c,
    0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
];

/// Constants necessary for SHA-512 family of digests.
pub const K64X2: [[u64; 2]; 40] = [
    [K64[1],  K64[0]],  [K64[3],  K64[2]],  [K64[5],  K64[4]],  [K64[7],  K64[6]],
    [K64[9],  K64[8]],  [K64[11], K64[10]], [K64[13], K64[12]], [K64[15], K64[14]],
    [K64[17], K64[16]], [K64[19], K64[18]], [K64[21], K64[20]], [K64[23], K64[22]],
    [K64[25], K64[24]], [K64[27], K64[26]], [K64[29], K64[28]], [K64[31], K64[30]],
    [K64[33], K64[32]], [K64[35], K64[34]], [K64[37], K64[36]], [K64[39], K64[38]],
    [K64[41], K64[40]], [K64[43], K64[42]], [K64[45], K64[44]], [K64[47], K64[46]],
    [K64[49], K64[48]], [K64[51], K64[50]], [K64[53], K64[52]], [K64[55], K64[54]],
    [K64[57], K64[56]], [K64[59], K64[58]], [K64[61], K64[60]], [K64[63], K64[62]],
    [K64[65], K64[64]], [K64[67], K64[66]], [K64[69], K64[68]], [K64[71], K64[70]],
    [K64[73], K64[72]], [K64[75], K64[74]], [K64[77], K64[76]], [K64[79], K64[78]],
];

pub const H256_224: State256 = [
    0xc1059ed8, 0x367cd507, 0x3070dd17, 0xf70e5939,
    0xffc00b31, 0x68581511, 0x64f98fa7, 0xbefa4fa4,
];

pub const H256_256: State256 = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

pub const H512_224: State512 = [
    0x8c3d37c819544da2, 0x73e1996689dcd4d6, 0x1dfab7ae32ff9c82, 0x679dd514582f9fcf,
    0x0f6d2b697bd44da8, 0x77e36f7304c48942, 0x3f9d85a86a1d36c8, 0x1112e6ad91d692a1,
];

pub const H512_256: State512 = [
    0x22312194fc2bf72c, 0x9f555fa3c84c64c2, 0x2393b86b6f53b151, 0x963877195940eabd,
    0x96283ee2a88effe3, 0xbe5e1e2553863992, 0x2b0199fc2c85b8aa, 0x0eb72ddc81c52ca2,
];

pub const H512_384: State512 = [
    0xcbbb9d5dc1059ed8, 0x629a292a367cd507, 0x9159015a3070dd17, 0x152fecd8f70e5939,
    0x67332667ffc00b31, 0x8eb44a8768581511, 0xdb0c2e0d64f98fa7, 0x47b5481dbefa4fa4,
];

pub const H512_512: State512 = [
    0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
    0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
];
