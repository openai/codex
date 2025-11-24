use winnow::combinator::seq;
use winnow::prelude::*;
use winnow::token::take_while;
use winnow::Result;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Color {
    pub(crate) red: u8,
    pub(crate) green: u8,
    pub(crate) blue: u8,
}

impl std::str::FromStr for Color {
    // The error must be owned
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        hex_color.parse(s).map_err(|e| e.to_string())
    }
}

pub(crate) fn hex_color(input: &mut &str) -> Result<Color> {
    seq!(Color {
        _: '#',
        red: hex_primary,
        green: hex_primary,
        blue: hex_primary
    })
    .parse_next(input)
}

fn hex_primary(input: &mut &str) -> Result<u8> {
    take_while(2, |c: char| c.is_ascii_hexdigit())
        .try_map(|input| u8::from_str_radix(input, 16))
        .parse_next(input)
}
