use self::table::{TABLE, TABLE16};

mod table;

/// Returns the "masked" CRC32 checksum of the slice using the Castagnoli
/// polynomial.
///
/// This "masked" checksum is the same one used by the Snappy frame format.
/// Masking is supposed to make the checksum robust with respect to data that
/// contains the checksum itself.
pub(crate) fn sum(buf: &[u8]) -> u32 {
    let sum = slice16(0, buf);
    (sum.wrapping_shr(15) | sum.wrapping_shl(17)).wrapping_add(0xA282EAD8)
}

/// Returns the CRC32 checksum of `buf` using the Castagnoli polynomial.
///
/// This computes the checksum by looking at 16 bytes from the given slice
/// per iteration.
fn slice16(prev: u32, mut buf: &[u8]) -> u32 {
    let mut crc: u32 = !prev;
    while buf.len() >= 16 {
        crc ^= u32::from_le_bytes(buf[..4].try_into().unwrap());
        crc = TABLE16[0][usize::from(buf[15])]
            ^ TABLE16[1][usize::from(buf[14])]
            ^ TABLE16[2][usize::from(buf[13])]
            ^ TABLE16[3][usize::from(buf[12])]
            ^ TABLE16[4][usize::from(buf[11])]
            ^ TABLE16[5][usize::from(buf[10])]
            ^ TABLE16[6][usize::from(buf[9])]
            ^ TABLE16[7][usize::from(buf[8])]
            ^ TABLE16[8][usize::from(buf[7])]
            ^ TABLE16[9][usize::from(buf[6])]
            ^ TABLE16[10][usize::from(buf[5])]
            ^ TABLE16[11][usize::from(buf[4])]
            ^ TABLE16[12][usize::from((crc >> 24) as u8)]
            ^ TABLE16[13][usize::from((crc >> 16) as u8)]
            ^ TABLE16[14][usize::from((crc >> 8) as u8)]
            ^ TABLE16[15][usize::from((crc) as u8)];
        buf = &buf[16..];
    }
    for &b in buf {
        crc = TABLE[usize::from((crc as u8) ^ b)] ^ (crc >> 8);
    }
    !crc
}
