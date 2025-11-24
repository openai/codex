mod table;

use self::table::{DECODE_TABLE, ENCODE_TABLE};
use crate::hpack::DecoderError;

use bytes::{BufMut, BytesMut};

// Constructed in the generated `table.rs` file
struct Decoder {
    state: usize,
    maybe_eos: bool,
}

// These flags must match the ones in genhuff.rs

const MAYBE_EOS: u8 = 1;
const DECODED: u8 = 2;
const ERROR: u8 = 4;

pub fn decode(src: &[u8], buf: &mut BytesMut) -> Result<BytesMut, DecoderError> {
    let mut decoder = Decoder::new();

    // Max compression ratio is >= 0.5
    buf.reserve(src.len() << 1);

    for b in src {
        if let Some(b) = decoder.decode4(b >> 4)? {
            buf.put_u8(b);
        }

        if let Some(b) = decoder.decode4(b & 0xf)? {
            buf.put_u8(b);
        }
    }

    if !decoder.is_final() {
        return Err(DecoderError::InvalidHuffmanCode);
    }

    Ok(buf.split())
}

pub fn encode(src: &[u8], dst: &mut BytesMut) {
    let mut bits: u64 = 0;
    let mut bits_left = 40;

    for &b in src {
        let (nbits, code) = ENCODE_TABLE[b as usize];

        bits |= code << (bits_left - nbits);
        bits_left -= nbits;

        while bits_left <= 32 {
            dst.put_u8((bits >> 32) as u8);

            bits <<= 8;
            bits_left += 8;
        }
    }

    if bits_left != 40 {
        // This writes the EOS token
        bits |= (1 << bits_left) - 1;
        dst.put_u8((bits >> 32) as u8);
    }
}

impl Decoder {
    fn new() -> Decoder {
        Decoder {
            state: 0,
            maybe_eos: false,
        }
    }

    // Decodes 4 bits
    fn decode4(&mut self, input: u8) -> Result<Option<u8>, DecoderError> {
        // (next-state, byte, flags)
        let (next, byte, flags) = DECODE_TABLE[self.state][input as usize];

        if flags & ERROR == ERROR {
            // Data followed the EOS marker
            return Err(DecoderError::InvalidHuffmanCode);
        }

        let mut ret = None;

        if flags & DECODED == DECODED {
            ret = Some(byte);
        }

        self.state = next;
        self.maybe_eos = flags & MAYBE_EOS == MAYBE_EOS;

        Ok(ret)
    }

    fn is_final(&self) -> bool {
        self.state == 0 || self.maybe_eos
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn decode(src: &[u8]) -> Result<BytesMut, DecoderError> {
        let mut buf = BytesMut::new();
        super::decode(src, &mut buf)
    }

    #[test]
    fn decode_single_byte() {
        assert_eq!("o", decode(&[0b00111111]).unwrap());
        assert_eq!("0", decode(&[7]).unwrap());
        assert_eq!("A", decode(&[(0x21 << 2) + 3]).unwrap());
    }

    #[test]
    fn single_char_multi_byte() {
        assert_eq!("#", decode(&[255, 160 + 15]).unwrap());
        assert_eq!("$", decode(&[255, 200 + 7]).unwrap());
        assert_eq!("\x0a", decode(&[255, 255, 255, 240 + 3]).unwrap());
    }

    #[test]
    fn multi_char() {
        assert_eq!("!0", decode(&[254, 1]).unwrap());
        assert_eq!(" !", decode(&[0b01010011, 0b11111000]).unwrap());
    }

    #[test]
    fn encode_single_byte() {
        let mut dst = BytesMut::with_capacity(1);

        encode(b"o", &mut dst);
        assert_eq!(&dst[..], &[0b00111111]);

        dst.clear();
        encode(b"0", &mut dst);
        assert_eq!(&dst[..], &[7]);

        dst.clear();
        encode(b"A", &mut dst);
        assert_eq!(&dst[..], &[(0x21 << 2) + 3]);
    }

    #[test]
    fn encode_decode_str() {
        const DATA: &[&str] = &[
            "hello world",
            ":method",
            ":scheme",
            ":authority",
            "yahoo.co.jp",
            "GET",
            "http",
            ":path",
            "/images/top/sp2/cmn/logo-ns-130528.png",
            "example.com",
            "hpack-test",
            "xxxxxxx1",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.8; rv:16.0) Gecko/20100101 Firefox/16.0",
            "accept",
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "cookie",
            "B=76j09a189a6h4&b=3&s=0b",
            "TE",
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Morbi non bibendum libero. \
             Etiam ultrices lorem ut.",
        ];

        for s in DATA {
            let mut dst = BytesMut::with_capacity(s.len());

            encode(s.as_bytes(), &mut dst);

            let decoded = decode(&dst).unwrap();

            assert_eq!(&decoded[..], s.as_bytes());
        }
    }

    #[test]
    fn encode_decode_u8() {
        const DATA: &[&[u8]] = &[b"\0", b"\0\0\0", b"\0\x01\x02\x03\x04\x05", b"\xFF\xF8"];

        for s in DATA {
            let mut dst = BytesMut::with_capacity(s.len());

            encode(s, &mut dst);

            let decoded = decode(&dst).unwrap();

            assert_eq!(&decoded[..], &s[..]);
        }
    }
}
