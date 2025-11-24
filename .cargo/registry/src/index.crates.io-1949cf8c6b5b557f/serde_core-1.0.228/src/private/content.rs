use crate::lib::*;

// Used from generated code to buffer the contents of the Deserializer when
// deserializing untagged enums and internally tagged enums.
//
// Not public API. Use serde-value instead.
//
// Obsoleted by format-specific buffer types (https://github.com/serde-rs/serde/pull/2912).
#[doc(hidden)]
pub enum Content<'de> {
    Bool(bool),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),

    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),

    F32(f32),
    F64(f64),

    Char(char),
    String(String),
    Str(&'de str),
    ByteBuf(Vec<u8>),
    Bytes(&'de [u8]),

    None,
    Some(Box<Content<'de>>),

    Unit,
    Newtype(Box<Content<'de>>),
    Seq(Vec<Content<'de>>),
    Map(Vec<(Content<'de>, Content<'de>)>),
}
