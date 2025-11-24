use std::borrow::{Borrow, Cow};
use std::fmt::{self, Debug, Formatter};
use std::io;
use std::ops::Deref;

#[cfg(feature = "async-tokio")]
use std::{
    pin::Pin,
    task::{Context, Poll},
};

#[cfg(feature = "serialize")]
use serde::de::{Deserialize, Deserializer, Error, Visitor};
#[cfg(feature = "serialize")]
use serde::ser::{Serialize, Serializer};

#[allow(clippy::ptr_arg)]
pub fn write_cow_string(f: &mut Formatter, cow_string: &Cow<[u8]>) -> fmt::Result {
    match cow_string {
        Cow::Owned(s) => {
            write!(f, "Owned(")?;
            write_byte_string(f, s)?;
        }
        Cow::Borrowed(s) => {
            write!(f, "Borrowed(")?;
            write_byte_string(f, s)?;
        }
    }
    write!(f, ")")
}

pub fn write_byte_string(f: &mut Formatter, byte_string: &[u8]) -> fmt::Result {
    write!(f, "\"")?;
    for b in byte_string {
        match *b {
            32..=33 | 35..=126 => write!(f, "{}", *b as char)?,
            34 => write!(f, "\\\"")?,
            _ => write!(f, "{:#02X}", b)?,
        }
    }
    write!(f, "\"")?;
    Ok(())
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A version of [`Cow`] that can borrow from two different buffers, one of them
/// is a deserializer input.
///
/// # Lifetimes
///
/// - `'i`: lifetime of the data that deserializer borrow from the parsed input
/// - `'s`: lifetime of the data that owned by a deserializer
pub enum CowRef<'i, 's, B>
where
    B: ToOwned + ?Sized,
{
    /// An input borrowed from the parsed data
    Input(&'i B),
    /// An input borrowed from the buffer owned by another deserializer
    Slice(&'s B),
    /// An input taken from an external deserializer, owned by that deserializer
    Owned(<B as ToOwned>::Owned),
}
impl<'i, 's, B> Deref for CowRef<'i, 's, B>
where
    B: ToOwned + ?Sized,
    B::Owned: Borrow<B>,
{
    type Target = B;

    fn deref(&self) -> &B {
        match *self {
            Self::Input(borrowed) => borrowed,
            Self::Slice(borrowed) => borrowed,
            Self::Owned(ref owned) => owned.borrow(),
        }
    }
}

impl<'i, 's, B> Debug for CowRef<'i, 's, B>
where
    B: ToOwned + ?Sized + Debug,
    B::Owned: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Self::Input(borrowed) => Debug::fmt(borrowed, f),
            Self::Slice(borrowed) => Debug::fmt(borrowed, f),
            Self::Owned(ref owned) => Debug::fmt(owned, f),
        }
    }
}

impl<'i, 's> CowRef<'i, 's, str> {
    /// Supply to the visitor a borrowed string, a string slice, or an owned
    /// string depending on the kind of input. Unlike [`Self::deserialize_all`],
    /// only part of [`Self::Owned`] string will be passed to the visitor.
    ///
    /// Calls
    /// - `visitor.visit_borrowed_str` if data borrowed from the input
    /// - `visitor.visit_str` if data borrowed from another source
    /// - `visitor.visit_string` if data owned by this type
    #[cfg(feature = "serialize")]
    pub fn deserialize_str<V, E>(self, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'i>,
        E: Error,
    {
        match self {
            Self::Input(s) => visitor.visit_borrowed_str(s),
            Self::Slice(s) => visitor.visit_str(s),
            Self::Owned(s) => visitor.visit_string(s),
        }
    }

    /// Calls [`Visitor::visit_bool`] with `true` or `false` if text contains
    /// [valid] boolean representation, otherwise calls [`Self::deserialize_str`].
    ///
    /// The valid boolean representations are only `"true"`, `"false"`, `"1"`, and `"0"`.
    ///
    /// [valid]: https://www.w3.org/TR/xmlschema11-2/#boolean
    #[cfg(feature = "serialize")]
    pub fn deserialize_bool<V, E>(self, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'i>,
        E: Error,
    {
        match self.as_ref() {
            "1" | "true" => visitor.visit_bool(true),
            "0" | "false" => visitor.visit_bool(false),
            _ => self.deserialize_str(visitor),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Wrapper around `Vec<u8>` that has a human-readable debug representation:
/// printable ASCII symbols output as is, all other output in HEX notation.
///
/// Also, when [`serialize`] feature is on, this type deserialized using
/// [`deserialize_byte_buf`](serde::Deserializer::deserialize_byte_buf) instead
/// of vector's generic [`deserialize_seq`](serde::Deserializer::deserialize_seq)
///
/// [`serialize`]: ../index.html#serialize
#[derive(PartialEq, Eq)]
pub struct ByteBuf(pub Vec<u8>);

impl Debug for ByteBuf {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write_byte_string(f, &self.0)
    }
}

#[cfg(feature = "serialize")]
impl<'de> Deserialize<'de> for ByteBuf {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = ByteBuf;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                f.write_str("byte data")
            }

            fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                Ok(ByteBuf(v.to_vec()))
            }

            fn visit_byte_buf<E: Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                Ok(ByteBuf(v))
            }
        }

        d.deserialize_byte_buf(ValueVisitor)
    }
}

#[cfg(feature = "serialize")]
impl Serialize for ByteBuf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Wrapper around `&[u8]` that has a human-readable debug representation:
/// printable ASCII symbols output as is, all other output in HEX notation.
///
/// Also, when [`serialize`] feature is on, this type deserialized using
/// [`deserialize_bytes`](serde::Deserializer::deserialize_bytes) instead
/// of vector's generic [`deserialize_seq`](serde::Deserializer::deserialize_seq)
///
/// [`serialize`]: ../index.html#serialize
#[derive(PartialEq, Eq)]
pub struct Bytes<'de>(pub &'de [u8]);

impl<'de> Debug for Bytes<'de> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write_byte_string(f, self.0)
    }
}

#[cfg(feature = "serialize")]
impl<'de> Deserialize<'de> for Bytes<'de> {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Bytes<'de>;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                f.write_str("borrowed bytes")
            }

            fn visit_borrowed_bytes<E: Error>(self, v: &'de [u8]) -> Result<Self::Value, E> {
                Ok(Bytes(v))
            }
        }

        d.deserialize_bytes(ValueVisitor)
    }
}

#[cfg(feature = "serialize")]
impl<'de> Serialize for Bytes<'de> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(self.0)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A simple producer of infinite stream of bytes, useful in tests.
///
/// Will repeat `chunk` field indefinitely.
pub struct Fountain<'a> {
    /// That piece of data repeated infinitely...
    pub chunk: &'a [u8],
    /// Part of `chunk` that was consumed by BufRead impl
    pub consumed: usize,
    /// The overall count of read bytes
    pub overall_read: u64,
}

impl<'a> io::Read for Fountain<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let available = &self.chunk[self.consumed..];
        let len = buf.len().min(available.len());
        let (portion, _) = available.split_at(len);

        buf.copy_from_slice(portion);
        Ok(len)
    }
}

impl<'a> io::BufRead for Fountain<'a> {
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(&self.chunk[self.consumed..])
    }

    fn consume(&mut self, amt: usize) {
        self.consumed += amt;
        if self.consumed == self.chunk.len() {
            self.consumed = 0;
        }
        self.overall_read += amt as u64;
    }
}

#[cfg(feature = "async-tokio")]
impl<'a> tokio::io::AsyncRead for Fountain<'a> {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let available = &self.chunk[self.consumed..];
        let len = buf.remaining().min(available.len());
        let (portion, _) = available.split_at(len);

        buf.put_slice(portion);
        Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "async-tokio")]
impl<'a> tokio::io::AsyncBufRead for Fountain<'a> {
    #[inline]
    fn poll_fill_buf(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Poll::Ready(io::BufRead::fill_buf(self.get_mut()))
    }

    #[inline]
    fn consume(self: Pin<&mut Self>, amt: usize) {
        io::BufRead::consume(self.get_mut(), amt);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A function to check whether the byte is a whitespace (blank, new line, carriage return or tab).
#[inline]
pub const fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\r' | b'\n' | b'\t')
}

/// Calculates name from an element-like content. Name is the first word in `content`,
/// where word boundaries is XML whitespace characters.
///
/// 'Whitespace' refers to the definition used by [`is_whitespace`].
#[inline]
pub const fn name_len(mut bytes: &[u8]) -> usize {
    // Note: A pattern matching based approach (instead of indexing) allows
    // making the function const.
    let mut len = 0;
    while let [first, rest @ ..] = bytes {
        if is_whitespace(*first) {
            break;
        }
        len += 1;
        bytes = rest;
    }
    len
}

/// Returns a byte slice with leading XML whitespace bytes removed.
///
/// 'Whitespace' refers to the definition used by [`is_whitespace`].
#[inline]
pub const fn trim_xml_start(mut bytes: &[u8]) -> &[u8] {
    // Note: A pattern matching based approach (instead of indexing) allows
    // making the function const.
    while let [first, rest @ ..] = bytes {
        if is_whitespace(*first) {
            bytes = rest;
        } else {
            break;
        }
    }
    bytes
}

/// Returns a byte slice with trailing XML whitespace bytes removed.
///
/// 'Whitespace' refers to the definition used by [`is_whitespace`].
#[inline]
pub const fn trim_xml_end(mut bytes: &[u8]) -> &[u8] {
    // Note: A pattern matching based approach (instead of indexing) allows
    // making the function const.
    while let [rest @ .., last] = bytes {
        if is_whitespace(*last) {
            bytes = rest;
        } else {
            break;
        }
    }
    bytes
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn write_byte_string0() {
        let bytes = ByteBuf(vec![10, 32, 32, 32, 32, 32, 32, 32, 32]);
        assert_eq!(format!("{:?}", bytes), "\"0xA        \"");
    }

    #[test]
    fn write_byte_string1() {
        let bytes = ByteBuf(vec![
            104, 116, 116, 112, 58, 47, 47, 119, 119, 119, 46, 119, 51, 46, 111, 114, 103, 47, 50,
            48, 48, 50, 47, 48, 55, 47, 111, 119, 108, 35,
        ]);
        assert_eq!(
            format!("{:?}", bytes),
            r##""http://www.w3.org/2002/07/owl#""##
        );
    }

    #[test]
    fn write_byte_string3() {
        let bytes = ByteBuf(vec![
            67, 108, 97, 115, 115, 32, 73, 82, 73, 61, 34, 35, 66, 34,
        ]);
        assert_eq!(format!("{:?}", bytes), r##""Class IRI=\"#B\"""##);
    }

    #[test]
    fn name_len() {
        assert_eq!(super::name_len(b""), 0);
        assert_eq!(super::name_len(b" abc"), 0);
        assert_eq!(super::name_len(b" \t\r\n"), 0);

        assert_eq!(super::name_len(b"abc"), 3);
        assert_eq!(super::name_len(b"abc "), 3);

        assert_eq!(super::name_len(b"a bc"), 1);
        assert_eq!(super::name_len(b"ab\tc"), 2);
        assert_eq!(super::name_len(b"ab\rc"), 2);
        assert_eq!(super::name_len(b"ab\nc"), 2);
    }

    #[test]
    fn trim_xml_start() {
        assert_eq!(Bytes(super::trim_xml_start(b"")), Bytes(b""));
        assert_eq!(Bytes(super::trim_xml_start(b"abc")), Bytes(b"abc"));
        assert_eq!(
            Bytes(super::trim_xml_start(b"\r\n\t ab \t\r\nc \t\r\n")),
            Bytes(b"ab \t\r\nc \t\r\n")
        );
    }

    #[test]
    fn trim_xml_end() {
        assert_eq!(Bytes(super::trim_xml_end(b"")), Bytes(b""));
        assert_eq!(Bytes(super::trim_xml_end(b"abc")), Bytes(b"abc"));
        assert_eq!(
            Bytes(super::trim_xml_end(b"\r\n\t ab \t\r\nc \t\r\n")),
            Bytes(b"\r\n\t ab \t\r\nc")
        );
    }
}
