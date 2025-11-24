use crate::table::CRC32_TABLE;

#[derive(Clone)]
pub struct State {
    state: u32,
}

impl State {
    pub fn new(state: u32) -> Self {
        State { state }
    }

    pub fn update(&mut self, buf: &[u8]) {
        self.state = update_fast_16(self.state, buf);
    }

    pub fn finalize(self) -> u32 {
        self.state
    }

    pub fn reset(&mut self) {
        self.state = 0;
    }

    pub fn combine(&mut self, other: u32, amount: u64) {
        self.state = crate::combine::combine(self.state, other, amount);
    }
}

pub(crate) fn update_fast_16(prev: u32, mut buf: &[u8]) -> u32 {
    const UNROLL: usize = 4;
    const BYTES_AT_ONCE: usize = 16 * UNROLL;

    let mut crc = !prev;

    while buf.len() >= BYTES_AT_ONCE {
        for _ in 0..UNROLL {
            crc = CRC32_TABLE[0x0][buf[0xf] as usize]
                ^ CRC32_TABLE[0x1][buf[0xe] as usize]
                ^ CRC32_TABLE[0x2][buf[0xd] as usize]
                ^ CRC32_TABLE[0x3][buf[0xc] as usize]
                ^ CRC32_TABLE[0x4][buf[0xb] as usize]
                ^ CRC32_TABLE[0x5][buf[0xa] as usize]
                ^ CRC32_TABLE[0x6][buf[0x9] as usize]
                ^ CRC32_TABLE[0x7][buf[0x8] as usize]
                ^ CRC32_TABLE[0x8][buf[0x7] as usize]
                ^ CRC32_TABLE[0x9][buf[0x6] as usize]
                ^ CRC32_TABLE[0xa][buf[0x5] as usize]
                ^ CRC32_TABLE[0xb][buf[0x4] as usize]
                ^ CRC32_TABLE[0xc][buf[0x3] as usize ^ ((crc >> 0x18) & 0xFF) as usize]
                ^ CRC32_TABLE[0xd][buf[0x2] as usize ^ ((crc >> 0x10) & 0xFF) as usize]
                ^ CRC32_TABLE[0xe][buf[0x1] as usize ^ ((crc >> 0x08) & 0xFF) as usize]
                ^ CRC32_TABLE[0xf][buf[0x0] as usize ^ (crc & 0xFF) as usize];
            buf = &buf[16..];
        }
    }

    update_slow(!crc, buf)
}

pub(crate) fn update_slow(prev: u32, buf: &[u8]) -> u32 {
    let mut crc = !prev;

    for &byte in buf.iter() {
        crc = CRC32_TABLE[0][((crc as u8) ^ byte) as usize] ^ (crc >> 8);
    }

    !crc
}

#[cfg(test)]
mod test {
    #[test]
    fn slow() {
        assert_eq!(super::update_slow(0, b""), 0);

        // test vectors from the iPXE project (input and output are bitwise negated)
        assert_eq!(super::update_slow(!0x12345678, b""), !0x12345678);
        assert_eq!(super::update_slow(!0xffffffff, b"hello world"), !0xf2b5ee7a);
        assert_eq!(super::update_slow(!0xffffffff, b"hello"), !0xc9ef5979);
        assert_eq!(super::update_slow(!0xc9ef5979, b" world"), !0xf2b5ee7a);

        // Some vectors found on Rosetta code
        assert_eq!(super::update_slow(0, b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"), 0x190A55AD);
        assert_eq!(super::update_slow(0, b"\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF"), 0xFF6CAB0B);
        assert_eq!(super::update_slow(0, b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1A\x1B\x1C\x1D\x1E\x1F"), 0x91267E8A);
    }

    quickcheck::quickcheck! {
        fn fast_16_is_the_same_as_slow(crc: u32, bytes: Vec<u8>) -> bool {
            super::update_fast_16(crc, &bytes) == super::update_slow(crc, &bytes)
        }
    }
}
