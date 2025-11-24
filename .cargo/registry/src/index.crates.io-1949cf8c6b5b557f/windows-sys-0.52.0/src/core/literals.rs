/// A literal UTF-8 string with a trailing null terminator.
#[macro_export]
macro_rules! s {
    ($s:literal) => {
        ::core::concat!($s, '\0').as_ptr()
    };
}

/// A literal UTF-16 wide string with a trailing null terminator.
#[macro_export]
macro_rules! w {
    ($s:literal) => {{
        const INPUT: &[u8] = $s.as_bytes();
        const OUTPUT_LEN: usize = $crate::core::utf16_len(INPUT) + 1;
        const OUTPUT: &[u16; OUTPUT_LEN] = {
            let mut buffer = [0; OUTPUT_LEN];
            let mut input_pos = 0;
            let mut output_pos = 0;
            while let Some((mut code_point, new_pos)) = $crate::core::decode_utf8_char(INPUT, input_pos) {
                input_pos = new_pos;
                if code_point <= 0xffff {
                    buffer[output_pos] = code_point as u16;
                    output_pos += 1;
                } else {
                    code_point -= 0x10000;
                    buffer[output_pos] = 0xd800 + (code_point >> 10) as u16;
                    output_pos += 1;
                    buffer[output_pos] = 0xdc00 + (code_point & 0x3ff) as u16;
                    output_pos += 1;
                }
            }
            &{ buffer }
        };
        OUTPUT.as_ptr()
    }};
}

pub use s;
pub use w;

#[doc(hidden)]
pub const fn decode_utf8_char(bytes: &[u8], mut pos: usize) -> Option<(u32, usize)> {
    if bytes.len() == pos {
        return None;
    }
    let ch = bytes[pos] as u32;
    pos += 1;
    if ch <= 0x7f {
        return Some((ch, pos));
    }
    if (ch & 0xe0) == 0xc0 {
        if bytes.len() - pos < 1 {
            return None;
        }
        let ch2 = bytes[pos] as u32;
        pos += 1;
        if (ch2 & 0xc0) != 0x80 {
            return None;
        }
        let result: u32 = ((ch & 0x1f) << 6) | (ch2 & 0x3f);
        if result <= 0x7f {
            return None;
        }
        return Some((result, pos));
    }
    if (ch & 0xf0) == 0xe0 {
        if bytes.len() - pos < 2 {
            return None;
        }
        let ch2 = bytes[pos] as u32;
        pos += 1;
        let ch3 = bytes[pos] as u32;
        pos += 1;
        if (ch2 & 0xc0) != 0x80 || (ch3 & 0xc0) != 0x80 {
            return None;
        }
        let result = ((ch & 0x0f) << 12) | ((ch2 & 0x3f) << 6) | (ch3 & 0x3f);
        if result <= 0x7ff || (0xd800 <= result && result <= 0xdfff) {
            return None;
        }
        return Some((result, pos));
    }
    if (ch & 0xf8) == 0xf0 {
        if bytes.len() - pos < 3 {
            return None;
        }
        let ch2 = bytes[pos] as u32;
        pos += 1;
        let ch3 = bytes[pos] as u32;
        pos += 1;
        let ch4 = bytes[pos] as u32;
        pos += 1;
        if (ch2 & 0xc0) != 0x80 || (ch3 & 0xc0) != 0x80 || (ch4 & 0xc0) != 0x80 {
            return None;
        }
        let result = ((ch & 0x07) << 18) | ((ch2 & 0x3f) << 12) | ((ch3 & 0x3f) << 6) | (ch4 & 0x3f);
        if result <= 0xffff || 0x10ffff < result {
            return None;
        }
        return Some((result, pos));
    }
    None
}

#[doc(hidden)]
pub const fn utf16_len(bytes: &[u8]) -> usize {
    let mut pos = 0;
    let mut len = 0;
    while let Some((code_point, new_pos)) = decode_utf8_char(bytes, pos) {
        pos = new_pos;
        len += if code_point <= 0xffff { 1 } else { 2 };
    }
    len
}
