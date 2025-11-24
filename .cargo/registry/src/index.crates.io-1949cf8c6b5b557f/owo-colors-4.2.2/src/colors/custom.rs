use crate::Color;

const U8_TO_STR: [[u8; 3]; 256] = generate_lookup();

const fn generate_lookup() -> [[u8; 3]; 256] {
    let mut table = [[0, 0, 0]; 256];

    let mut i = 0;
    while i < 256 {
        table[i] = [
            b'0' + (i / 100) as u8,
            b'0' + (i / 10 % 10) as u8,
            b'0' + (i % 10) as u8,
        ];
        i += 1;
    }

    table
}

enum Plane {
    Fg,
    Bg,
}

const fn rgb_to_ansi(r: u8, g: u8, b: u8, plane: Plane) -> [u8; 19] {
    let mut buf = match plane {
        Plane::Fg => *b"\x1b[38;2;rrr;ggg;bbbm",
        Plane::Bg => *b"\x1b[48;2;rrr;ggg;bbbm",
    };

    let r = U8_TO_STR[r as usize];
    let g = U8_TO_STR[g as usize];
    let b = U8_TO_STR[b as usize];

    // r 7
    buf[7] = r[0];
    buf[8] = r[1];
    buf[9] = r[2];

    // g 11
    buf[11] = g[0];
    buf[12] = g[1];
    buf[13] = g[2];

    // b 15
    buf[15] = b[0];
    buf[16] = b[1];
    buf[17] = b[2];

    buf
}

const fn rgb_to_ansi_color(r: u8, g: u8, b: u8, plane: Plane) -> [u8; 16] {
    let mut buf = match plane {
        Plane::Fg => *b"38;2;rrr;ggg;bbb",
        Plane::Bg => *b"48;2;rrr;ggg;bbb",
    };

    let r = U8_TO_STR[r as usize];
    let g = U8_TO_STR[g as usize];
    let b = U8_TO_STR[b as usize];

    // r 5
    buf[5] = r[0];
    buf[6] = r[1];
    buf[7] = r[2];

    // g 9
    buf[9] = g[0];
    buf[10] = g[1];
    buf[11] = g[2];

    // b 13
    buf[13] = b[0];
    buf[14] = b[1];
    buf[15] = b[2];

    buf
}

/// A custom RGB color, determined at compile time
pub struct CustomColor<const R: u8, const G: u8, const B: u8>;

/// This exists since unwrap() isn't const-safe (it invokes formatting infrastructure)
const fn bytes_to_str(bytes: &'static [u8]) -> &'static str {
    match core::str::from_utf8(bytes) {
        Ok(o) => o,
        Err(_e) => panic!("Const parsing &[u8] to a string failed!"),
    }
}

impl<const R: u8, const G: u8, const B: u8> CustomColor<R, G, B> {
    const ANSI_FG_U8: [u8; 19] = rgb_to_ansi(R, G, B, Plane::Fg);
    const ANSI_BG_U8: [u8; 19] = rgb_to_ansi(R, G, B, Plane::Bg);
    const RAW_ANSI_FG_U8: [u8; 16] = rgb_to_ansi_color(R, G, B, Plane::Fg);
    const RAW_ANSI_BG_U8: [u8; 16] = rgb_to_ansi_color(R, G, B, Plane::Bg);
}

impl<const R: u8, const G: u8, const B: u8> crate::private::Sealed for CustomColor<R, G, B> {}

impl<const R: u8, const G: u8, const B: u8> Color for CustomColor<R, G, B> {
    const ANSI_FG: &'static str = bytes_to_str(&Self::ANSI_FG_U8);
    const ANSI_BG: &'static str = bytes_to_str(&Self::ANSI_BG_U8);
    const RAW_ANSI_FG: &'static str = bytes_to_str(&Self::RAW_ANSI_FG_U8);
    const RAW_ANSI_BG: &'static str = bytes_to_str(&Self::RAW_ANSI_BG_U8);

    #[doc(hidden)]
    type DynEquivalent = crate::Rgb;

    #[doc(hidden)]
    const DYN_EQUIVALENT: Self::DynEquivalent = crate::Rgb(R, G, B);

    #[doc(hidden)]
    const DYN_COLORS_EQUIVALENT: crate::DynColors = crate::DynColors::Rgb(R, G, B);
}
