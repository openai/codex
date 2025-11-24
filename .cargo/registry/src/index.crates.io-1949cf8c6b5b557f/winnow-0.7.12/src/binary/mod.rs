//! Parsers recognizing numbers

#![allow(clippy::match_same_arms)]

pub mod bits;

#[cfg(test)]
mod tests;

use crate::combinator::repeat;
use crate::combinator::trace;
use crate::error::Needed;
use crate::error::ParserError;
use crate::lib::std::ops::{Add, Shl};
use crate::stream::Accumulate;
use crate::stream::{Stream, StreamIsPartial};
use crate::stream::{ToUsize, UpdateSlice};
use crate::Parser;
use crate::Result;

/// Configurable endianness
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Endianness {
    /// Big endian
    Big,
    /// Little endian
    Little,
    /// Will match the host's endianness
    Native,
}

/// Recognizes an unsigned 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_u8;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u8> {
///     be_u8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_u8;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u8> {
///     be_u8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_u8<Input, Error>(input: &mut Input) -> Result<u8, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    u8(input)
}

/// Recognizes a big endian unsigned 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_u16;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u16> {
///     be_u16.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_u16;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u16> {
///     be_u16.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0001)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_u16<Input, Error>(input: &mut Input) -> Result<u16, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_u16", move |input: &mut Input| be_uint(input, 2)).parse_next(input)
}

/// Recognizes a big endian unsigned 3 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_u24;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u32> {
///     be_u24.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_u24;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u32> {
///     be_u24.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x000102)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn be_u24<Input, Error>(input: &mut Input) -> Result<u32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_u23", move |input: &mut Input| be_uint(input, 3)).parse_next(input)
}

/// Recognizes a big endian unsigned 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_u32;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u32> {
///     be_u32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_u32;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u32> {
///     be_u32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x00010203)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_u32<Input, Error>(input: &mut Input) -> Result<u32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_u32", move |input: &mut Input| be_uint(input, 4)).parse_next(input)
}

/// Recognizes a big endian unsigned 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_u64;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u64> {
///     be_u64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_u64;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u64> {
///     be_u64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0001020304050607)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_u64<Input, Error>(input: &mut Input) -> Result<u64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_u64", move |input: &mut Input| be_uint(input, 8)).parse_next(input)
}

/// Recognizes a big endian unsigned 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_u128;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u128> {
///     be_u128.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_u128;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u128> {
///     be_u128.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x00010203040506070809101112131415)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn be_u128<Input, Error>(input: &mut Input) -> Result<u128, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_u128", move |input: &mut Input| be_uint(input, 16)).parse_next(input)
}

#[inline]
fn be_uint<Input, Uint, Error>(input: &mut Input, bound: usize) -> Result<Uint, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Uint: Default + Shl<u8, Output = Uint> + Add<Uint, Output = Uint> + From<u8>,
    Error: ParserError<Input>,
{
    debug_assert_ne!(bound, 1, "to_be_uint needs extra work to avoid overflow");
    match input.offset_at(bound) {
        Ok(offset) => {
            let res = to_be_uint(input, offset);
            input.next_slice(offset);
            Ok(res)
        }
        Err(e) if <Input as StreamIsPartial>::is_partial_supported() && input.is_partial() => {
            Err(ParserError::incomplete(input, e))
        }
        Err(_needed) => Err(ParserError::from_input(input)),
    }
}

#[inline]
fn to_be_uint<Input, Uint>(number: &Input, offset: usize) -> Uint
where
    Input: Stream,
    Uint: Default
        + Shl<u8, Output = Uint>
        + Add<Uint, Output = Uint>
        + From<<Input as Stream>::Token>,
{
    let mut res = Uint::default();
    for (_, byte) in number.iter_offsets().take(offset) {
        res = (res << 8) + byte.into();
    }

    res
}

/// Recognizes a signed 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_i8;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i8> {
///     be_i8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_i8;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i8> {
///       be_i8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn be_i8<Input, Error>(input: &mut Input) -> Result<i8, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    i8(input)
}

/// Recognizes a big endian signed 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_i16;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i16> {
///     be_i16.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_i16;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i16> {
///       be_i16.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0001)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn be_i16<Input, Error>(input: &mut Input) -> Result<i16, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_i16", move |input: &mut Input| {
        be_uint::<_, u16, _>(input, 2).map(|n| n as i16)
    })
    .parse_next(input)
}

/// Recognizes a big endian signed 3 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_i24;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i32> {
///     be_i24.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_i24;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i32> {
///       be_i24.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x000102)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_i24<Input, Error>(input: &mut Input) -> Result<i32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_i24", move |input: &mut Input| {
        be_uint::<_, u32, _>(input, 3).map(|n| {
            // Same as the unsigned version but we need to sign-extend manually here
            let n = if n & 0x80_00_00 != 0 {
                (n | 0xff_00_00_00) as i32
            } else {
                n as i32
            };
            n
        })
    })
    .parse_next(input)
}

/// Recognizes a big endian signed 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_i32;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i32> {
///       be_i32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_i32;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i32> {
///       be_i32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x00010203)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(4))));
/// ```
#[inline(always)]
pub fn be_i32<Input, Error>(input: &mut Input) -> Result<i32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_i32", move |input: &mut Input| {
        be_uint::<_, u32, _>(input, 4).map(|n| n as i32)
    })
    .parse_next(input)
}

/// Recognizes a big endian signed 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_i64;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i64> {
///       be_i64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_i64;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i64> {
///       be_i64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0001020304050607)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_i64<Input, Error>(input: &mut Input) -> Result<i64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_i64", move |input: &mut Input| {
        be_uint::<_, u64, _>(input, 8).map(|n| n as i64)
    })
    .parse_next(input)
}

/// Recognizes a big endian signed 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_i128;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i128> {
///       be_i128.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_i128;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i128> {
///       be_i128.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x00010203040506070809101112131415)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn be_i128<Input, Error>(input: &mut Input) -> Result<i128, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_i128", move |input: &mut Input| {
        be_uint::<_, u128, _>(input, 16).map(|n| n as i128)
    })
    .parse_next(input)
}

/// Recognizes an unsigned 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_u8;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u8> {
///       le_u8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_u8;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u8> {
///       le_u8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_u8<Input, Error>(input: &mut Input) -> Result<u8, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    u8(input)
}

/// Recognizes a little endian unsigned 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_u16;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u16> {
///       le_u16.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_u16;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u16> {
///       le_u16.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_u16<Input, Error>(input: &mut Input) -> Result<u16, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_u16", move |input: &mut Input| le_uint(input, 2)).parse_next(input)
}

/// Recognizes a little endian unsigned 3 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_u24;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u32> {
///       le_u24.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_u24;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u32> {
///       le_u24.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x020100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn le_u24<Input, Error>(input: &mut Input) -> Result<u32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_u24", move |input: &mut Input| le_uint(input, 3)).parse_next(input)
}

/// Recognizes a little endian unsigned 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_u32;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u32> {
///       le_u32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_u32;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u32> {
///       le_u32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x03020100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_u32<Input, Error>(input: &mut Input) -> Result<u32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_u32", move |input: &mut Input| le_uint(input, 4)).parse_next(input)
}

/// Recognizes a little endian unsigned 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_u64;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u64> {
///       le_u64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_u64;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u64> {
///       le_u64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0706050403020100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_u64<Input, Error>(input: &mut Input) -> Result<u64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_u64", move |input: &mut Input| le_uint(input, 8)).parse_next(input)
}

/// Recognizes a little endian unsigned 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_u128;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u128> {
///       le_u128.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_u128;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u128> {
///       le_u128.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x15141312111009080706050403020100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn le_u128<Input, Error>(input: &mut Input) -> Result<u128, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_u128", move |input: &mut Input| le_uint(input, 16)).parse_next(input)
}

#[inline]
fn le_uint<Input, Uint, Error>(input: &mut Input, bound: usize) -> Result<Uint, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Uint: Default + Shl<u8, Output = Uint> + Add<Uint, Output = Uint> + From<u8>,
    Error: ParserError<Input>,
{
    match input.offset_at(bound) {
        Ok(offset) => {
            let res = to_le_uint(input, offset);
            input.next_slice(offset);
            Ok(res)
        }
        Err(e) if <Input as StreamIsPartial>::is_partial_supported() && input.is_partial() => {
            Err(ParserError::incomplete(input, e))
        }
        Err(_needed) => Err(ParserError::from_input(input)),
    }
}

#[inline]
fn to_le_uint<Input, Uint>(number: &Input, offset: usize) -> Uint
where
    Input: Stream,
    Uint: Default
        + Shl<u8, Output = Uint>
        + Add<Uint, Output = Uint>
        + From<<Input as Stream>::Token>,
{
    let mut res = Uint::default();
    for (index, byte) in number.iter_offsets().take(offset) {
        res = res + (Uint::from(byte) << (8 * index as u8));
    }

    res
}

/// Recognizes a signed 1 byte integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_i8;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i8> {
///       le_i8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_i8;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i8> {
///       le_i8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"\x01abcd"[..]), 0x00)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_i8<Input, Error>(input: &mut Input) -> Result<i8, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    i8(input)
}

/// Recognizes a little endian signed 2 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_i16;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i16> {
///       le_i16.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_i16;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i16> {
///       le_i16.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn le_i16<Input, Error>(input: &mut Input) -> Result<i16, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_i16", move |input: &mut Input| {
        le_uint::<_, u16, _>(input, 2).map(|n| n as i16)
    })
    .parse_next(input)
}

/// Recognizes a little endian signed 3 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_i24;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i32> {
///       le_i24.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_i24;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i32> {
///       le_i24.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x020100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn le_i24<Input, Error>(input: &mut Input) -> Result<i32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_i24", move |input: &mut Input| {
        le_uint::<_, u32, _>(input, 3).map(|n| {
            // Same as the unsigned version but we need to sign-extend manually here
            let n = if n & 0x80_00_00 != 0 {
                (n | 0xff_00_00_00) as i32
            } else {
                n as i32
            };
            n
        })
    })
    .parse_next(input)
}

/// Recognizes a little endian signed 4 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_i32;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i32> {
///       le_i32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_i32;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i32> {
///       le_i32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x03020100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_i32<Input, Error>(input: &mut Input) -> Result<i32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_i32", move |input: &mut Input| {
        le_uint::<_, u32, _>(input, 4).map(|n| n as i32)
    })
    .parse_next(input)
}

/// Recognizes a little endian signed 8 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_i64;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i64> {
///       le_i64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_i64;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i64> {
///       le_i64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x0706050403020100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_i64<Input, Error>(input: &mut Input) -> Result<i64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_i64", move |input: &mut Input| {
        le_uint::<_, u64, _>(input, 8).map(|n| n as i64)
    })
    .parse_next(input)
}

/// Recognizes a little endian signed 16 bytes integer.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_i128;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i128> {
///       le_i128.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert!(parser.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_i128;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i128> {
///       le_i128.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x10\x11\x12\x13\x14\x15abcd"[..])), Ok((Partial::new(&b"abcd"[..]), 0x15141312111009080706050403020100)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn le_i128<Input, Error>(input: &mut Input) -> Result<i128, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_i128", move |input: &mut Input| {
        le_uint::<_, u128, _>(input, 16).map(|n| n as i128)
    })
    .parse_next(input)
}

/// Recognizes an unsigned 1 byte integer
///
/// <div class="warning">
///
/// **Note:** that endianness does not apply to 1 byte numbers.
///
/// </div>
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::u8;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<u8> {
///       u8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::u8;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<u8> {
///       u8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"\x03abcefg"[..]), 0x00)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn u8<Input, Error>(input: &mut Input) -> Result<u8, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("u8", move |input: &mut Input| {
        if <Input as StreamIsPartial>::is_partial_supported() {
            u8_::<_, _, true>(input)
        } else {
            u8_::<_, _, false>(input)
        }
    })
    .parse_next(input)
}

fn u8_<Input, Error, const PARTIAL: bool>(input: &mut Input) -> Result<u8, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    input.next_token().ok_or_else(|| {
        if PARTIAL && input.is_partial() {
            ParserError::incomplete(input, Needed::new(1))
        } else {
            ParserError::from_input(input)
        }
    })
}

/// Recognizes an unsigned 2 bytes integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian u16 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian u16 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::u16;
///
/// fn be_u16(input: &mut &[u8]) -> ModalResult<u16> {
///     u16(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u16.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert!(be_u16.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_u16(input: &mut &[u8]) -> ModalResult<u16> {
///     u16(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u16.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert!(le_u16.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::u16;
///
/// fn be_u16(input: &mut Partial<&[u8]>) -> ModalResult<u16> {
///     u16(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u16.parse_peek(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0003)));
/// assert_eq!(be_u16.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// fn le_u16(input: &mut Partial<&[u8]>) -> ModalResult< u16> {
///     u16(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u16.parse_peek(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0300)));
/// assert_eq!(le_u16.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn u16<Input, Error>(endian: Endianness) -> impl Parser<Input, u16, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_u16,
            Endianness::Little => le_u16,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_u16,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_u16,
        }
    }(input)
}

/// Recognizes an unsigned 3 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian u24 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian u24 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::u24;
///
/// fn be_u24(input: &mut &[u8]) -> ModalResult<u32> {
///     u24(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u24.parse_peek(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert!(be_u24.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_u24(input: &mut &[u8]) -> ModalResult<u32> {
///     u24(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u24.parse_peek(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert!(le_u24.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::u24;
///
/// fn be_u24(input: &mut Partial<&[u8]>) -> ModalResult<u32> {
///     u24(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u24.parse_peek(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x000305)));
/// assert_eq!(be_u24.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
///
/// fn le_u24(input: &mut Partial<&[u8]>) -> ModalResult<u32> {
///     u24(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u24.parse_peek(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x050300)));
/// assert_eq!(le_u24.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn u24<Input, Error>(endian: Endianness) -> impl Parser<Input, u32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_u24,
            Endianness::Little => le_u24,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_u24,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_u24,
        }
    }(input)
}

/// Recognizes an unsigned 4 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian u32 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian u32 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::u32;
///
/// fn be_u32(input: &mut &[u8]) -> ModalResult<u32> {
///     u32(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u32.parse_peek(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert!(be_u32.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_u32(input: &mut &[u8]) -> ModalResult<u32> {
///     u32(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u32.parse_peek(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert!(le_u32.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::u32;
///
/// fn be_u32(input: &mut Partial<&[u8]>) -> ModalResult<u32> {
///     u32(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u32.parse_peek(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00030507)));
/// assert_eq!(be_u32.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
///
/// fn le_u32(input: &mut Partial<&[u8]>) -> ModalResult<u32> {
///     u32(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u32.parse_peek(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07050300)));
/// assert_eq!(le_u32.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn u32<Input, Error>(endian: Endianness) -> impl Parser<Input, u32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_u32,
            Endianness::Little => le_u32,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_u32,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_u32,
        }
    }(input)
}

/// Recognizes an unsigned 8 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian u64 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian u64 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::u64;
///
/// fn be_u64(input: &mut &[u8]) -> ModalResult<u64> {
///     u64(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u64.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert!(be_u64.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_u64(input: &mut &[u8]) -> ModalResult<u64> {
///     u64(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u64.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert!(le_u64.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::u64;
///
/// fn be_u64(input: &mut Partial<&[u8]>) -> ModalResult<u64> {
///     u64(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u64.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0001020304050607)));
/// assert_eq!(be_u64.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
///
/// fn le_u64(input: &mut Partial<&[u8]>) -> ModalResult<u64> {
///     u64(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u64.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0706050403020100)));
/// assert_eq!(le_u64.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn u64<Input, Error>(endian: Endianness) -> impl Parser<Input, u64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_u64,
            Endianness::Little => le_u64,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_u64,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_u64,
        }
    }(input)
}

/// Recognizes an unsigned 16 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian u128 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian u128 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::u128;
///
/// fn be_u128(input: &mut &[u8]) -> ModalResult<u128> {
///     u128(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u128.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert!(be_u128.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_u128(input: &mut &[u8]) -> ModalResult<u128> {
///     u128(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u128.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert!(le_u128.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::u128;
///
/// fn be_u128(input: &mut Partial<&[u8]>) -> ModalResult<u128> {
///     u128(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_u128.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00010203040506070001020304050607)));
/// assert_eq!(be_u128.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
///
/// fn le_u128(input: &mut Partial<&[u8]>) -> ModalResult<u128> {
///     u128(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_u128.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07060504030201000706050403020100)));
/// assert_eq!(le_u128.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn u128<Input, Error>(endian: Endianness) -> impl Parser<Input, u128, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_u128,
            Endianness::Little => le_u128,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_u128,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_u128,
        }
    }(input)
}

/// Recognizes a signed 1 byte integer
///
/// <div class="warning">
///
/// **Note:** that endianness does not apply to 1 byte numbers.
///
/// </div>
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::i8;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<i8> {
///       i8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"\x03abcefg"[..], 0x00)));
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::i8;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<i8> {
///       i8.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"\x03abcefg"[..]), 0x00)));
/// assert_eq!(parser.parse_peek(Partial::new(&b""[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn i8<Input, Error>(input: &mut Input) -> Result<i8, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("i8", move |input: &mut Input| {
        if <Input as StreamIsPartial>::is_partial_supported() {
            u8_::<_, _, true>(input)
        } else {
            u8_::<_, _, false>(input)
        }
        .map(|n| n as i8)
    })
    .parse_next(input)
}

/// Recognizes a signed 2 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian i16 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian i16 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::i16;
///
/// fn be_i16(input: &mut &[u8]) -> ModalResult<i16> {
///     i16(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i16.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0003)));
/// assert!(be_i16.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_i16(input: &mut &[u8]) -> ModalResult<i16> {
///     i16(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i16.parse_peek(&b"\x00\x03abcefg"[..]), Ok((&b"abcefg"[..], 0x0300)));
/// assert!(le_i16.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::i16;
///
/// fn be_i16(input: &mut Partial<&[u8]>) -> ModalResult<i16> {
///     i16(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i16.parse_peek(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0003)));
/// assert_eq!(be_i16.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// fn le_i16(input: &mut Partial<&[u8]>) -> ModalResult<i16> {
///     i16(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i16.parse_peek(Partial::new(&b"\x00\x03abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0300)));
/// assert_eq!(le_i16.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn i16<Input, Error>(endian: Endianness) -> impl Parser<Input, i16, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_i16,
            Endianness::Little => le_i16,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_i16,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_i16,
        }
    }(input)
}

/// Recognizes a signed 3 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian i24 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian i24 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::i24;
///
/// fn be_i24(input: &mut &[u8]) -> ModalResult<i32> {
///     i24(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i24.parse_peek(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x000305)));
/// assert!(be_i24.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_i24(input: &mut &[u8]) -> ModalResult<i32> {
///     i24(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i24.parse_peek(&b"\x00\x03\x05abcefg"[..]), Ok((&b"abcefg"[..], 0x050300)));
/// assert!(le_i24.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::i24;
///
/// fn be_i24(input: &mut Partial<&[u8]>) -> ModalResult<i32> {
///     i24(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i24.parse_peek(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x000305)));
/// assert_eq!(be_i24.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
///
/// fn le_i24(input: &mut Partial<&[u8]>) -> ModalResult<i32> {
///     i24(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i24.parse_peek(Partial::new(&b"\x00\x03\x05abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x050300)));
/// assert_eq!(le_i24.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
#[inline(always)]
pub fn i24<Input, Error>(endian: Endianness) -> impl Parser<Input, i32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_i24,
            Endianness::Little => le_i24,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_i24,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_i24,
        }
    }(input)
}

/// Recognizes a signed 4 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian i32 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian i32 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::i32;
///
/// fn be_i32(input: &mut &[u8]) -> ModalResult<i32> {
///     i32(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i32.parse_peek(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00030507)));
/// assert!(be_i32.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_i32(input: &mut &[u8]) -> ModalResult<i32> {
///     i32(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i32.parse_peek(&b"\x00\x03\x05\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07050300)));
/// assert!(le_i32.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::i32;
///
/// fn be_i32(input: &mut Partial<&[u8]>) -> ModalResult<i32> {
///     i32(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i32.parse_peek(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00030507)));
/// assert_eq!(be_i32.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
///
/// fn le_i32(input: &mut Partial<&[u8]>) -> ModalResult<i32> {
///     i32(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i32.parse_peek(Partial::new(&b"\x00\x03\x05\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07050300)));
/// assert_eq!(le_i32.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn i32<Input, Error>(endian: Endianness) -> impl Parser<Input, i32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_i32,
            Endianness::Little => le_i32,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_i32,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_i32,
        }
    }(input)
}

/// Recognizes a signed 8 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian i64 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian i64 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::i64;
///
/// fn be_i64(input: &mut &[u8]) -> ModalResult<i64> {
///     i64(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i64.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0001020304050607)));
/// assert!(be_i64.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_i64(input: &mut &[u8]) -> ModalResult<i64> {
///     i64(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i64.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x0706050403020100)));
/// assert!(le_i64.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::i64;
///
/// fn be_i64(input: &mut Partial<&[u8]>) -> ModalResult<i64> {
///     i64(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i64.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0001020304050607)));
/// assert_eq!(be_i64.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
///
/// fn le_i64(input: &mut Partial<&[u8]>) -> ModalResult<i64> {
///     i64(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i64.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x0706050403020100)));
/// assert_eq!(le_i64.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn i64<Input, Error>(endian: Endianness) -> impl Parser<Input, i64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_i64,
            Endianness::Little => le_i64,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_i64,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_i64,
        }
    }(input)
}

/// Recognizes a signed 16 byte integer
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian i128 integer,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian i128 integer.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::i128;
///
/// fn be_i128(input: &mut &[u8]) -> ModalResult<i128> {
///     i128(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i128.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x00010203040506070001020304050607)));
/// assert!(be_i128.parse_peek(&b"\x01"[..]).is_err());
///
/// fn le_i128(input: &mut &[u8]) -> ModalResult<i128> {
///     i128(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i128.parse_peek(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..]), Ok((&b"abcefg"[..], 0x07060504030201000706050403020100)));
/// assert!(le_i128.parse_peek(&b"\x01"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::i128;
///
/// fn be_i128(input: &mut Partial<&[u8]>) -> ModalResult<i128> {
///     i128(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_i128.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x00010203040506070001020304050607)));
/// assert_eq!(be_i128.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
///
/// fn le_i128(input: &mut Partial<&[u8]>) -> ModalResult<i128> {
///     i128(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_i128.parse_peek(Partial::new(&b"\x00\x01\x02\x03\x04\x05\x06\x07\x00\x01\x02\x03\x04\x05\x06\x07abcefg"[..])), Ok((Partial::new(&b"abcefg"[..]), 0x07060504030201000706050403020100)));
/// assert_eq!(le_i128.parse_peek(Partial::new(&b"\x01"[..])), Err(ErrMode::Incomplete(Needed::new(15))));
/// ```
#[inline(always)]
pub fn i128<Input, Error>(endian: Endianness) -> impl Parser<Input, i128, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_i128,
            Endianness::Little => le_i128,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_i128,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_i128,
        }
    }(input)
}

/// Recognizes a big endian 4 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_f32;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<f32> {
///       be_f32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert!(parser.parse_peek(&b"abc"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_f32;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<f32> {
///       be_f32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&[0x40, 0x29, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 2.640625)));
/// assert_eq!(parser.parse_peek(Partial::new(&[0x01][..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn be_f32<Input, Error>(input: &mut Input) -> Result<f32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_f32", move |input: &mut Input| {
        be_uint::<_, u32, _>(input, 4).map(f32::from_bits)
    })
    .parse_next(input)
}

/// Recognizes a big endian 8 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::be_f64;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<f64> {
///       be_f64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert!(parser.parse_peek(&b"abc"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::be_f64;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<f64> {
///       be_f64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(parser.parse_peek(Partial::new(&[0x01][..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn be_f64<Input, Error>(input: &mut Input) -> Result<f64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_f64", move |input: &mut Input| {
        be_uint::<_, u64, _>(input, 8).map(f64::from_bits)
    })
    .parse_next(input)
}

/// Recognizes a little endian 4 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_f32;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<f32> {
///       le_f32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert!(parser.parse_peek(&b"abc"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_f32;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<f32> {
///       le_f32.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&[0x00, 0x00, 0x48, 0x41][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(parser.parse_peek(Partial::new(&[0x01][..])), Err(ErrMode::Incomplete(Needed::new(3))));
/// ```
#[inline(always)]
pub fn le_f32<Input, Error>(input: &mut Input) -> Result<f32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("le_f32", move |input: &mut Input| {
        le_uint::<_, u32, _>(input, 4).map(f32::from_bits)
    })
    .parse_next(input)
}

/// Recognizes a little endian 8 bytes floating point number.
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::le_f64;
///
/// fn parser(s: &mut &[u8]) -> ModalResult<f64> {
///       le_f64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert!(parser.parse_peek(&b"abc"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::binary::le_f64;
///
/// fn parser(s: &mut Partial<&[u8]>) -> ModalResult<f64> {
///       le_f64.parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x48, 0x41][..])), Ok((Partial::new(&b""[..]), 3145728.0)));
/// assert_eq!(parser.parse_peek(Partial::new(&[0x01][..])), Err(ErrMode::Incomplete(Needed::new(7))));
/// ```
#[inline(always)]
pub fn le_f64<Input, Error>(input: &mut Input) -> Result<f64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    trace("be_f64", move |input: &mut Input| {
        le_uint::<_, u64, _>(input, 8).map(f64::from_bits)
    })
    .parse_next(input)
}

/// Recognizes a 4 byte floating point number
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian f32 float,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian f32 float.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::f32;
///
/// fn be_f32(input: &mut &[u8]) -> ModalResult<f32> {
///     f32(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_f32.parse_peek(&[0x41, 0x48, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert!(be_f32.parse_peek(&b"abc"[..]).is_err());
///
/// fn le_f32(input: &mut &[u8]) -> ModalResult<f32> {
///     f32(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_f32.parse_peek(&[0x00, 0x00, 0x48, 0x41][..]), Ok((&b""[..], 12.5)));
/// assert!(le_f32.parse_peek(&b"abc"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::f32;
///
/// fn be_f32(input: &mut Partial<&[u8]>) -> ModalResult<f32> {
///     f32(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_f32.parse_peek(Partial::new(&[0x41, 0x48, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(be_f32.parse_peek(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// fn le_f32(input: &mut Partial<&[u8]>) -> ModalResult<f32> {
///     f32(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_f32.parse_peek(Partial::new(&[0x00, 0x00, 0x48, 0x41][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(le_f32.parse_peek(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn f32<Input, Error>(endian: Endianness) -> impl Parser<Input, f32, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_f32,
            Endianness::Little => le_f32,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_f32,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_f32,
        }
    }(input)
}

/// Recognizes an 8 byte floating point number
///
/// If the parameter is `winnow::binary::Endianness::Big`, parse a big endian f64 float,
/// otherwise if `winnow::binary::Endianness::Little` parse a little endian f64 float.
///
/// *Complete version*: returns an error if there is not enough input data
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::binary::f64;
///
/// fn be_f64(input: &mut &[u8]) -> ModalResult<f64> {
///     f64(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_f64.parse_peek(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]), Ok((&b""[..], 12.5)));
/// assert!(be_f64.parse_peek(&b"abc"[..]).is_err());
///
/// fn le_f64(input: &mut &[u8]) -> ModalResult<f64> {
///     f64(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_f64.parse_peek(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..]), Ok((&b""[..], 12.5)));
/// assert!(le_f64.parse_peek(&b"abc"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::binary::f64;
///
/// fn be_f64(input: &mut Partial<&[u8]>) -> ModalResult<f64> {
///     f64(winnow::binary::Endianness::Big).parse_next(input)
/// };
///
/// assert_eq!(be_f64.parse_peek(Partial::new(&[0x40, 0x29, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(be_f64.parse_peek(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(5))));
///
/// fn le_f64(input: &mut Partial<&[u8]>) -> ModalResult<f64> {
///     f64(winnow::binary::Endianness::Little).parse_next(input)
/// };
///
/// assert_eq!(le_f64.parse_peek(Partial::new(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x29, 0x40][..])), Ok((Partial::new(&b""[..]), 12.5)));
/// assert_eq!(le_f64.parse_peek(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(5))));
/// ```
#[inline(always)]
pub fn f64<Input, Error>(endian: Endianness) -> impl Parser<Input, f64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input>,
{
    move |input: &mut Input| {
        match endian {
            Endianness::Big => be_f64,
            Endianness::Little => le_f64,
            #[cfg(target_endian = "big")]
            Endianness::Native => be_f64,
            #[cfg(target_endian = "little")]
            Endianness::Native => le_f64,
        }
    }(input)
}

/// Get a length-prefixed slice ([TLV](https://en.wikipedia.org/wiki/Type-length-value))
///
/// To apply a parser to the returned slice, see [`length_and_then`].
///
/// If the count is for something besides tokens, see [`length_repeat`].
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed, stream::Partial};
/// # use winnow::prelude::*;
/// use winnow::Bytes;
/// use winnow::binary::be_u16;
/// use winnow::binary::length_take;
///
/// type Stream<'i> = Partial<&'i Bytes>;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Partial::new(Bytes::new(b))
/// }
///
/// fn parser<'i>(s: &mut Stream<'i>) -> ModalResult<&'i [u8]> {
///   length_take(be_u16).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(stream(b"\x00\x03abcefg")), Ok((stream(&b"efg"[..]), &b"abc"[..])));
/// assert_eq!(parser.parse_peek(stream(b"\x00\x03a")), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
pub fn length_take<Input, Count, Error, CountParser>(
    mut count: CountParser,
) -> impl Parser<Input, <Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    Count: ToUsize,
    CountParser: Parser<Input, Count, Error>,
    Error: ParserError<Input>,
{
    trace("length_take", move |i: &mut Input| {
        let length = count.parse_next(i)?;

        crate::token::take(length).parse_next(i)
    })
}

/// Parse a length-prefixed slice ([TLV](https://en.wikipedia.org/wiki/Type-length-value))
///
/// *Complete version*: Returns an error if there is not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::InputError, error::Needed, stream::{Partial, StreamIsPartial}};
/// # use winnow::prelude::*;
/// use winnow::Bytes;
/// use winnow::binary::be_u16;
/// use winnow::binary::length_and_then;
///
/// type Stream<'i> = Partial<&'i Bytes>;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Partial::new(Bytes::new(b))
/// }
///
/// fn complete_stream(b: &[u8]) -> Stream<'_> {
///     let mut p = Partial::new(Bytes::new(b));
///     let _ = p.complete();
///     p
/// }
///
/// fn parser<'i>(s: &mut Stream<'i>) -> ModalResult<&'i [u8]> {
///   length_and_then(be_u16, "abc").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(stream(b"\x00\x03abcefg")), Ok((stream(&b"efg"[..]), &b"abc"[..])));
/// assert!(parser.parse_peek(stream(b"\x00\x03123123")).is_err());
/// assert_eq!(parser.parse_peek(stream(b"\x00\x03a")), Err(ErrMode::Incomplete(Needed::new(2))));
/// ```
pub fn length_and_then<Input, Output, Count, Error, CountParser, ParseNext>(
    mut count: CountParser,
    mut parser: ParseNext,
) -> impl Parser<Input, Output, Error>
where
    Input: StreamIsPartial + Stream + UpdateSlice + Clone,
    Count: ToUsize,
    CountParser: Parser<Input, Count, Error>,
    ParseNext: Parser<Input, Output, Error>,
    Error: ParserError<Input>,
{
    trace("length_and_then", move |i: &mut Input| {
        let data = length_take(count.by_ref()).parse_next(i)?;
        let mut data = Input::update_slice(i.clone(), data);
        let _ = data.complete();
        let o = parser.by_ref().complete_err().parse_next(&mut data)?;
        Ok(o)
    })
}

/// [`Accumulate`] a length-prefixed sequence of values ([TLV](https://en.wikipedia.org/wiki/Type-length-value))
///
/// If the length represents token counts, see instead [`length_take`]
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::InputError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::Bytes;
/// use winnow::binary::u8;
/// use winnow::binary::length_repeat;
///
/// type Stream<'i> = &'i Bytes;
///
/// fn stream(b: &[u8]) -> Stream<'_> {
///     Bytes::new(b)
/// }
///
/// fn parser<'i>(s: &mut Stream<'i>) -> ModalResult<Vec<&'i [u8]>> {
///   length_repeat(u8.map(|i| {
///      println!("got number: {}", i);
///      i
///   }), "abc").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(stream(b"\x02abcabcabc")), Ok((stream(b"abc"), vec![&b"abc"[..], &b"abc"[..]])));
/// assert!(parser.parse_peek(stream(b"\x03123123123")).is_err());
/// # }
/// ```
pub fn length_repeat<Input, Output, Accumulator, Count, Error, CountParser, ParseNext>(
    mut count: CountParser,
    mut parser: ParseNext,
) -> impl Parser<Input, Accumulator, Error>
where
    Input: Stream,
    Count: ToUsize,
    Accumulator: Accumulate<Output>,
    CountParser: Parser<Input, Count, Error>,
    ParseNext: Parser<Input, Output, Error>,
    Error: ParserError<Input>,
{
    trace("length_repeat", move |i: &mut Input| {
        let n = count.parse_next(i)?;
        let n = n.to_usize();
        repeat(n, parser.by_ref()).parse_next(i)
    })
}
