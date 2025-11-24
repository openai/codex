#[doc(hidden)]
pub const MZ_ADLER32_INIT: u32 = 1;

#[doc(hidden)]
pub const MZ_DEFAULT_WINDOW_BITS: i32 = 15;

pub const HUFFMAN_LENGTH_ORDER: [u8; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

#[doc(hidden)]
#[cfg(not(feature = "simd"))]
pub fn update_adler32(adler: u32, data: &[u8]) -> u32 {
    let mut hash = adler2::Adler32::from_checksum(adler);
    hash.write_slice(data);
    hash.checksum()
}

#[doc(hidden)]
#[cfg(feature = "simd")]
pub fn update_adler32(adler: u32, data: &[u8]) -> u32 {
    let mut hash = simd_adler32::Adler32::from_checksum(adler);
    hash.write(data);
    hash.finish()
}
