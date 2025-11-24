// Copyright 2013-2014 The Rust Project Developers.
// Copyright 2018 The Uuid Project Developers.
//
// See the COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Adapters for alternative string formats.

use core::{convert::TryInto as _, str::FromStr};

use crate::{
    std::{borrow::Borrow, fmt, str},
    Error, Uuid, Variant,
};

#[cfg(feature = "std")]
use crate::std::string::{String, ToString};

impl std::fmt::Debug for Uuid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(self, f)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(self, f)
    }
}

#[cfg(feature = "std")]
impl From<Uuid> for String {
    fn from(uuid: Uuid) -> Self {
        uuid.to_string()
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Variant::NCS => write!(f, "NCS"),
            Variant::RFC4122 => write!(f, "RFC4122"),
            Variant::Microsoft => write!(f, "Microsoft"),
            Variant::Future => write!(f, "Future"),
        }
    }
}

impl fmt::LowerHex for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(self.as_hyphenated(), f)
    }
}

impl fmt::UpperHex for Uuid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(self.as_hyphenated(), f)
    }
}

/// Format a [`Uuid`] as a hyphenated string, like
/// `67e55044-10b1-426f-9247-bb680e5fe0c8`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
#[cfg_attr(
    all(uuid_unstable, feature = "zerocopy"),
    derive(
        zerocopy::IntoBytes,
        zerocopy::FromBytes,
        zerocopy::KnownLayout,
        zerocopy::Immutable,
        zerocopy::Unaligned
    )
)]
#[repr(transparent)]
pub struct Hyphenated(Uuid);

/// Format a [`Uuid`] as a simple string, like
/// `67e5504410b1426f9247bb680e5fe0c8`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
#[cfg_attr(
    all(uuid_unstable, feature = "zerocopy"),
    derive(
        zerocopy::IntoBytes,
        zerocopy::FromBytes,
        zerocopy::KnownLayout,
        zerocopy::Immutable,
        zerocopy::Unaligned
    )
)]
#[repr(transparent)]
pub struct Simple(Uuid);

/// Format a [`Uuid`] as a URN string, like
/// `urn:uuid:67e55044-10b1-426f-9247-bb680e5fe0c8`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
#[cfg_attr(
    all(uuid_unstable, feature = "zerocopy"),
    derive(
        zerocopy::IntoBytes,
        zerocopy::FromBytes,
        zerocopy::KnownLayout,
        zerocopy::Immutable,
        zerocopy::Unaligned
    )
)]
#[repr(transparent)]
pub struct Urn(Uuid);

/// Format a [`Uuid`] as a braced hyphenated string, like
/// `{67e55044-10b1-426f-9247-bb680e5fe0c8}`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
#[cfg_attr(
    all(uuid_unstable, feature = "zerocopy"),
    derive(
        zerocopy::IntoBytes,
        zerocopy::FromBytes,
        zerocopy::KnownLayout,
        zerocopy::Immutable,
        zerocopy::Unaligned
    )
)]
#[repr(transparent)]
pub struct Braced(Uuid);

impl Uuid {
    /// Get a [`Hyphenated`] formatter.
    #[inline]
    pub const fn hyphenated(self) -> Hyphenated {
        Hyphenated(self)
    }

    /// Get a borrowed [`Hyphenated`] formatter.
    #[inline]
    pub fn as_hyphenated(&self) -> &Hyphenated {
        unsafe_transmute_ref!(self)
    }

    /// Get a [`Simple`] formatter.
    #[inline]
    pub const fn simple(self) -> Simple {
        Simple(self)
    }

    /// Get a borrowed [`Simple`] formatter.
    #[inline]
    pub fn as_simple(&self) -> &Simple {
        unsafe_transmute_ref!(self)
    }

    /// Get a [`Urn`] formatter.
    #[inline]
    pub const fn urn(self) -> Urn {
        Urn(self)
    }

    /// Get a borrowed [`Urn`] formatter.
    #[inline]
    pub fn as_urn(&self) -> &Urn {
        unsafe_transmute_ref!(self)
    }

    /// Get a [`Braced`] formatter.
    #[inline]
    pub const fn braced(self) -> Braced {
        Braced(self)
    }

    /// Get a borrowed [`Braced`] formatter.
    #[inline]
    pub fn as_braced(&self) -> &Braced {
        unsafe_transmute_ref!(self)
    }
}

const UPPER: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C', b'D', b'E', b'F',
];
const LOWER: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'a', b'b', b'c', b'd', b'e', b'f',
];

#[inline]
const fn format_simple(src: &[u8; 16], upper: bool) -> [u8; 32] {
    let lut = if upper { &UPPER } else { &LOWER };
    let mut dst = [0; 32];
    let mut i = 0;
    while i < 16 {
        let x = src[i];
        dst[i * 2] = lut[(x >> 4) as usize];
        dst[i * 2 + 1] = lut[(x & 0x0f) as usize];
        i += 1;
    }
    dst
}

#[inline]
const fn format_hyphenated(src: &[u8; 16], upper: bool) -> [u8; 36] {
    let lut = if upper { &UPPER } else { &LOWER };
    let groups = [(0, 8), (9, 13), (14, 18), (19, 23), (24, 36)];
    let mut dst = [0; 36];

    let mut group_idx = 0;
    let mut i = 0;
    while group_idx < 5 {
        let (start, end) = groups[group_idx];
        let mut j = start;
        while j < end {
            let x = src[i];
            i += 1;

            dst[j] = lut[(x >> 4) as usize];
            dst[j + 1] = lut[(x & 0x0f) as usize];
            j += 2;
        }
        if group_idx < 4 {
            dst[end] = b'-';
        }
        group_idx += 1;
    }
    dst
}

#[inline]
fn encode_simple<'b>(src: &[u8; 16], buffer: &'b mut [u8], upper: bool) -> &'b mut str {
    let buf = &mut buffer[..Simple::LENGTH];
    let buf: &mut [u8; Simple::LENGTH] = buf.try_into().unwrap();
    *buf = format_simple(src, upper);

    // SAFETY: The encoded buffer is ASCII encoded
    unsafe { str::from_utf8_unchecked_mut(buf) }
}

#[inline]
fn encode_hyphenated<'b>(src: &[u8; 16], buffer: &'b mut [u8], upper: bool) -> &'b mut str {
    let buf = &mut buffer[..Hyphenated::LENGTH];
    let buf: &mut [u8; Hyphenated::LENGTH] = buf.try_into().unwrap();
    *buf = format_hyphenated(src, upper);

    // SAFETY: The encoded buffer is ASCII encoded
    unsafe { str::from_utf8_unchecked_mut(buf) }
}

#[inline]
fn encode_braced<'b>(src: &[u8; 16], buffer: &'b mut [u8], upper: bool) -> &'b mut str {
    let buf = &mut buffer[..Hyphenated::LENGTH + 2];
    let buf: &mut [u8; Hyphenated::LENGTH + 2] = buf.try_into().unwrap();

    #[cfg_attr(
        all(uuid_unstable, feature = "zerocopy"),
        derive(zerocopy::IntoBytes)
    )]
    #[repr(C)]
    struct Braced {
        open_curly: u8,
        hyphenated: [u8; Hyphenated::LENGTH],
        close_curly: u8,
    }

    let braced = Braced {
        open_curly: b'{',
        hyphenated: format_hyphenated(src, upper),
        close_curly: b'}',
    };

    *buf = unsafe_transmute!(braced);

    // SAFETY: The encoded buffer is ASCII encoded
    unsafe { str::from_utf8_unchecked_mut(buf) }
}

#[inline]
fn encode_urn<'b>(src: &[u8; 16], buffer: &'b mut [u8], upper: bool) -> &'b mut str {
    let buf = &mut buffer[..Urn::LENGTH];
    buf[..9].copy_from_slice(b"urn:uuid:");

    let dst = &mut buf[9..(9 + Hyphenated::LENGTH)];
    let dst: &mut [u8; Hyphenated::LENGTH] = dst.try_into().unwrap();
    *dst = format_hyphenated(src, upper);

    // SAFETY: The encoded buffer is ASCII encoded
    unsafe { str::from_utf8_unchecked_mut(buf) }
}

impl Hyphenated {
    /// The length of a hyphenated [`Uuid`] string.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    pub const LENGTH: usize = 36;

    /// Creates a [`Hyphenated`] from a [`Uuid`].
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    /// [`Hyphenated`]: struct.Hyphenated.html
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Hyphenated(uuid)
    }

    /// Writes the [`Uuid`] as a lower-case hyphenated string to
    /// `buffer`, and returns the subslice of the buffer that contains the
    /// encoded UUID.
    ///
    /// This is slightly more efficient than using the formatting
    /// infrastructure as it avoids virtual calls, and may avoid
    /// double buffering.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not large enough: it must have length at least
    /// [`LENGTH`]. [`Uuid::encode_buffer`] can be used to get a
    /// sufficiently-large temporary buffer.
    ///
    /// [`LENGTH`]: #associatedconstant.LENGTH
    /// [`Uuid::encode_buffer`]: ../struct.Uuid.html#method.encode_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// fn main() -> Result<(), uuid::Error> {
    ///     let uuid = Uuid::parse_str("936DA01f9abd4d9d80c702af85c822a8")?;
    ///
    ///     // the encoded portion is returned
    ///     assert_eq!(
    ///         uuid.hyphenated()
    ///             .encode_lower(&mut Uuid::encode_buffer()),
    ///         "936da01f-9abd-4d9d-80c7-02af85c822a8"
    ///     );
    ///
    ///     // the buffer is mutated directly, and trailing contents remains
    ///     let mut buf = [b'!'; 40];
    ///     uuid.hyphenated().encode_lower(&mut buf);
    ///     assert_eq!(
    ///         &buf as &[_],
    ///         b"936da01f-9abd-4d9d-80c7-02af85c822a8!!!!" as &[_]
    ///     );
    ///
    ///     Ok(())
    /// }
    /// ```
    /// */
    #[inline]
    pub fn encode_lower<'buf>(&self, buffer: &'buf mut [u8]) -> &'buf mut str {
        encode_hyphenated(self.0.as_bytes(), buffer, false)
    }

    /// Writes the [`Uuid`] as an upper-case hyphenated string to
    /// `buffer`, and returns the subslice of the buffer that contains the
    /// encoded UUID.
    ///
    /// This is slightly more efficient than using the formatting
    /// infrastructure as it avoids virtual calls, and may avoid
    /// double buffering.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not large enough: it must have length at least
    /// [`LENGTH`]. [`Uuid::encode_buffer`] can be used to get a
    /// sufficiently-large temporary buffer.
    ///
    /// [`LENGTH`]: #associatedconstant.LENGTH
    /// [`Uuid::encode_buffer`]: ../struct.Uuid.html#method.encode_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// fn main() -> Result<(), uuid::Error> {
    ///     let uuid = Uuid::parse_str("936da01f9abd4d9d80c702af85c822a8")?;
    ///
    ///     // the encoded portion is returned
    ///     assert_eq!(
    ///         uuid.hyphenated()
    ///             .encode_upper(&mut Uuid::encode_buffer()),
    ///         "936DA01F-9ABD-4D9D-80C7-02AF85C822A8"
    ///     );
    ///
    ///     // the buffer is mutated directly, and trailing contents remains
    ///     let mut buf = [b'!'; 40];
    ///     uuid.hyphenated().encode_upper(&mut buf);
    ///     assert_eq!(
    ///         &buf as &[_],
    ///         b"936DA01F-9ABD-4D9D-80C7-02AF85C822A8!!!!" as &[_]
    ///     );
    ///
    ///     Ok(())
    /// }
    /// ```
    /// */
    #[inline]
    pub fn encode_upper<'buf>(&self, buffer: &'buf mut [u8]) -> &'buf mut str {
        encode_hyphenated(self.0.as_bytes(), buffer, true)
    }

    /// Get a reference to the underlying [`Uuid`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// let hyphenated = Uuid::nil().hyphenated();
    /// assert_eq!(*hyphenated.as_uuid(), Uuid::nil());
    /// ```
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Consumes the [`Hyphenated`], returning the underlying [`Uuid`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// let hyphenated = Uuid::nil().hyphenated();
    /// assert_eq!(hyphenated.into_uuid(), Uuid::nil());
    /// ```
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Braced {
    /// The length of a braced [`Uuid`] string.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    pub const LENGTH: usize = 38;

    /// Creates a [`Braced`] from a [`Uuid`].
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    /// [`Braced`]: struct.Braced.html
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Braced(uuid)
    }

    /// Writes the [`Uuid`] as a lower-case hyphenated string surrounded by
    /// braces to `buffer`, and returns the subslice of the buffer that contains
    /// the encoded UUID.
    ///
    /// This is slightly more efficient than using the formatting
    /// infrastructure as it avoids virtual calls, and may avoid
    /// double buffering.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not large enough: it must have length at least
    /// [`LENGTH`]. [`Uuid::encode_buffer`] can be used to get a
    /// sufficiently-large temporary buffer.
    ///
    /// [`LENGTH`]: #associatedconstant.LENGTH
    /// [`Uuid::encode_buffer`]: ../struct.Uuid.html#method.encode_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// fn main() -> Result<(), uuid::Error> {
    ///     let uuid = Uuid::parse_str("936DA01f9abd4d9d80c702af85c822a8")?;
    ///
    ///     // the encoded portion is returned
    ///     assert_eq!(
    ///         uuid.braced()
    ///             .encode_lower(&mut Uuid::encode_buffer()),
    ///         "{936da01f-9abd-4d9d-80c7-02af85c822a8}"
    ///     );
    ///
    ///     // the buffer is mutated directly, and trailing contents remains
    ///     let mut buf = [b'!'; 40];
    ///     uuid.braced().encode_lower(&mut buf);
    ///     assert_eq!(
    ///         &buf as &[_],
    ///         b"{936da01f-9abd-4d9d-80c7-02af85c822a8}!!" as &[_]
    ///     );
    ///
    ///     Ok(())
    /// }
    /// ```
    /// */
    #[inline]
    pub fn encode_lower<'buf>(&self, buffer: &'buf mut [u8]) -> &'buf mut str {
        encode_braced(self.0.as_bytes(), buffer, false)
    }

    /// Writes the [`Uuid`] as an upper-case hyphenated string surrounded by
    /// braces to `buffer`, and returns the subslice of the buffer that contains
    /// the encoded UUID.
    ///
    /// This is slightly more efficient than using the formatting
    /// infrastructure as it avoids virtual calls, and may avoid
    /// double buffering.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not large enough: it must have length at least
    /// [`LENGTH`]. [`Uuid::encode_buffer`] can be used to get a
    /// sufficiently-large temporary buffer.
    ///
    /// [`LENGTH`]: #associatedconstant.LENGTH
    /// [`Uuid::encode_buffer`]: ../struct.Uuid.html#method.encode_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// fn main() -> Result<(), uuid::Error> {
    ///     let uuid = Uuid::parse_str("936da01f9abd4d9d80c702af85c822a8")?;
    ///
    ///     // the encoded portion is returned
    ///     assert_eq!(
    ///         uuid.braced()
    ///             .encode_upper(&mut Uuid::encode_buffer()),
    ///         "{936DA01F-9ABD-4D9D-80C7-02AF85C822A8}"
    ///     );
    ///
    ///     // the buffer is mutated directly, and trailing contents remains
    ///     let mut buf = [b'!'; 40];
    ///     uuid.braced().encode_upper(&mut buf);
    ///     assert_eq!(
    ///         &buf as &[_],
    ///         b"{936DA01F-9ABD-4D9D-80C7-02AF85C822A8}!!" as &[_]
    ///     );
    ///
    ///     Ok(())
    /// }
    /// ```
    /// */
    #[inline]
    pub fn encode_upper<'buf>(&self, buffer: &'buf mut [u8]) -> &'buf mut str {
        encode_braced(self.0.as_bytes(), buffer, true)
    }

    /// Get a reference to the underlying [`Uuid`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// let braced = Uuid::nil().braced();
    /// assert_eq!(*braced.as_uuid(), Uuid::nil());
    /// ```
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Consumes the [`Braced`], returning the underlying [`Uuid`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// let braced = Uuid::nil().braced();
    /// assert_eq!(braced.into_uuid(), Uuid::nil());
    /// ```
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Simple {
    /// The length of a simple [`Uuid`] string.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    pub const LENGTH: usize = 32;

    /// Creates a [`Simple`] from a [`Uuid`].
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    /// [`Simple`]: struct.Simple.html
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Simple(uuid)
    }

    /// Writes the [`Uuid`] as a lower-case simple string to `buffer`,
    /// and returns the subslice of the buffer that contains the encoded UUID.
    ///
    /// This is slightly more efficient than using the formatting
    /// infrastructure as it avoids virtual calls, and may avoid
    /// double buffering.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not large enough: it must have length at least
    /// [`LENGTH`]. [`Uuid::encode_buffer`] can be used to get a
    /// sufficiently-large temporary buffer.
    ///
    /// [`LENGTH`]: #associatedconstant.LENGTH
    /// [`Uuid::encode_buffer`]: ../struct.Uuid.html#method.encode_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// fn main() -> Result<(), uuid::Error> {
    ///     let uuid = Uuid::parse_str("936DA01f9abd4d9d80c702af85c822a8")?;
    ///
    ///     // the encoded portion is returned
    ///     assert_eq!(
    ///         uuid.simple().encode_lower(&mut Uuid::encode_buffer()),
    ///         "936da01f9abd4d9d80c702af85c822a8"
    ///     );
    ///
    ///     // the buffer is mutated directly, and trailing contents remains
    ///     let mut buf = [b'!'; 36];
    ///     assert_eq!(
    ///         uuid.simple().encode_lower(&mut buf),
    ///         "936da01f9abd4d9d80c702af85c822a8"
    ///     );
    ///     assert_eq!(
    ///         &buf as &[_],
    ///         b"936da01f9abd4d9d80c702af85c822a8!!!!" as &[_]
    ///     );
    ///
    ///     Ok(())
    /// }
    /// ```
    /// */
    #[inline]
    pub fn encode_lower<'buf>(&self, buffer: &'buf mut [u8]) -> &'buf mut str {
        encode_simple(self.0.as_bytes(), buffer, false)
    }

    /// Writes the [`Uuid`] as an upper-case simple string to `buffer`,
    /// and returns the subslice of the buffer that contains the encoded UUID.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not large enough: it must have length at least
    /// [`LENGTH`]. [`Uuid::encode_buffer`] can be used to get a
    /// sufficiently-large temporary buffer.
    ///
    /// [`LENGTH`]: #associatedconstant.LENGTH
    /// [`Uuid::encode_buffer`]: ../struct.Uuid.html#method.encode_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// fn main() -> Result<(), uuid::Error> {
    ///     let uuid = Uuid::parse_str("936da01f9abd4d9d80c702af85c822a8")?;
    ///
    ///     // the encoded portion is returned
    ///     assert_eq!(
    ///         uuid.simple().encode_upper(&mut Uuid::encode_buffer()),
    ///         "936DA01F9ABD4D9D80C702AF85C822A8"
    ///     );
    ///
    ///     // the buffer is mutated directly, and trailing contents remains
    ///     let mut buf = [b'!'; 36];
    ///     assert_eq!(
    ///         uuid.simple().encode_upper(&mut buf),
    ///         "936DA01F9ABD4D9D80C702AF85C822A8"
    ///     );
    ///     assert_eq!(
    ///         &buf as &[_],
    ///         b"936DA01F9ABD4D9D80C702AF85C822A8!!!!" as &[_]
    ///     );
    ///
    ///     Ok(())
    /// }
    /// ```
    /// */
    #[inline]
    pub fn encode_upper<'buf>(&self, buffer: &'buf mut [u8]) -> &'buf mut str {
        encode_simple(self.0.as_bytes(), buffer, true)
    }

    /// Get a reference to the underlying [`Uuid`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// let simple = Uuid::nil().simple();
    /// assert_eq!(*simple.as_uuid(), Uuid::nil());
    /// ```
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Consumes the [`Simple`], returning the underlying [`Uuid`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// let simple = Uuid::nil().simple();
    /// assert_eq!(simple.into_uuid(), Uuid::nil());
    /// ```
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Urn {
    /// The length of a URN [`Uuid`] string.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    pub const LENGTH: usize = 45;

    /// Creates a [`Urn`] from a [`Uuid`].
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    /// [`Urn`]: struct.Urn.html
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Urn(uuid)
    }

    /// Writes the [`Uuid`] as a lower-case URN string to
    /// `buffer`, and returns the subslice of the buffer that contains the
    /// encoded UUID.
    ///
    /// This is slightly more efficient than using the formatting
    /// infrastructure as it avoids virtual calls, and may avoid
    /// double buffering.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not large enough: it must have length at least
    /// [`LENGTH`]. [`Uuid::encode_buffer`] can be used to get a
    /// sufficiently-large temporary buffer.
    ///
    /// [`LENGTH`]: #associatedconstant.LENGTH
    /// [`Uuid::encode_buffer`]: ../struct.Uuid.html#method.encode_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// fn main() -> Result<(), uuid::Error> {
    ///     let uuid = Uuid::parse_str("936DA01f9abd4d9d80c702af85c822a8")?;
    ///
    ///     // the encoded portion is returned
    ///     assert_eq!(
    ///         uuid.urn().encode_lower(&mut Uuid::encode_buffer()),
    ///         "urn:uuid:936da01f-9abd-4d9d-80c7-02af85c822a8"
    ///     );
    ///
    ///     // the buffer is mutated directly, and trailing contents remains
    ///     let mut buf = [b'!'; 49];
    ///     uuid.urn().encode_lower(&mut buf);
    ///     assert_eq!(
    ///         uuid.urn().encode_lower(&mut buf),
    ///         "urn:uuid:936da01f-9abd-4d9d-80c7-02af85c822a8"
    ///     );
    ///     assert_eq!(
    ///         &buf as &[_],
    ///         b"urn:uuid:936da01f-9abd-4d9d-80c7-02af85c822a8!!!!" as &[_]
    ///     );
    ///     
    ///     Ok(())
    /// }
    /// ```
    /// */
    #[inline]
    pub fn encode_lower<'buf>(&self, buffer: &'buf mut [u8]) -> &'buf mut str {
        encode_urn(self.0.as_bytes(), buffer, false)
    }

    /// Writes the [`Uuid`] as an upper-case URN string to
    /// `buffer`, and returns the subslice of the buffer that contains the
    /// encoded UUID.
    ///
    /// This is slightly more efficient than using the formatting
    /// infrastructure as it avoids virtual calls, and may avoid
    /// double buffering.
    ///
    /// [`Uuid`]: ../struct.Uuid.html
    ///
    /// # Panics
    ///
    /// Panics if the buffer is not large enough: it must have length at least
    /// [`LENGTH`]. [`Uuid::encode_buffer`] can be used to get a
    /// sufficiently-large temporary buffer.
    ///
    /// [`LENGTH`]: #associatedconstant.LENGTH
    /// [`Uuid::encode_buffer`]: ../struct.Uuid.html#method.encode_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// fn main() -> Result<(), uuid::Error> {
    ///     let uuid = Uuid::parse_str("936da01f9abd4d9d80c702af85c822a8")?;
    ///
    ///     // the encoded portion is returned
    ///     assert_eq!(
    ///         uuid.urn().encode_upper(&mut Uuid::encode_buffer()),
    ///         "urn:uuid:936DA01F-9ABD-4D9D-80C7-02AF85C822A8"
    ///     );
    ///
    ///     // the buffer is mutated directly, and trailing contents remains
    ///     let mut buf = [b'!'; 49];
    ///     assert_eq!(
    ///         uuid.urn().encode_upper(&mut buf),
    ///         "urn:uuid:936DA01F-9ABD-4D9D-80C7-02AF85C822A8"
    ///     );
    ///     assert_eq!(
    ///         &buf as &[_],
    ///         b"urn:uuid:936DA01F-9ABD-4D9D-80C7-02AF85C822A8!!!!" as &[_]
    ///     );
    ///
    ///     Ok(())
    /// }
    /// ```
    /// */
    #[inline]
    pub fn encode_upper<'buf>(&self, buffer: &'buf mut [u8]) -> &'buf mut str {
        encode_urn(self.0.as_bytes(), buffer, true)
    }

    /// Get a reference to the underlying [`Uuid`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// let urn = Uuid::nil().urn();
    /// assert_eq!(*urn.as_uuid(), Uuid::nil());
    /// ```
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Consumes the [`Urn`], returning the underlying [`Uuid`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use uuid::Uuid;
    ///
    /// let urn = Uuid::nil().urn();
    /// assert_eq!(urn.into_uuid(), Uuid::nil());
    /// ```
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl FromStr for Hyphenated {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parser::parse_hyphenated(s.as_bytes())
            .map(|b| Hyphenated(Uuid(b)))
            .map_err(|invalid| invalid.into_err())
    }
}

impl FromStr for Simple {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parser::parse_simple(s.as_bytes())
            .map(|b| Simple(Uuid(b)))
            .map_err(|invalid| invalid.into_err())
    }
}

impl FromStr for Urn {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parser::parse_urn(s.as_bytes())
            .map(|b| Urn(Uuid(b)))
            .map_err(|invalid| invalid.into_err())
    }
}

impl FromStr for Braced {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parser::parse_braced(s.as_bytes())
            .map(|b| Braced(Uuid(b)))
            .map_err(|invalid| invalid.into_err())
    }
}

macro_rules! impl_fmt_traits {
    ($($T:ident<$($a:lifetime),*>),+) => {$(
        impl<$($a),*> fmt::Display for $T<$($a),*> {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::LowerHex::fmt(self, f)
            }
        }

        impl<$($a),*> fmt::LowerHex for $T<$($a),*> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.encode_lower(&mut [0; Self::LENGTH]))
            }
        }

        impl<$($a),*> fmt::UpperHex for $T<$($a),*> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.encode_upper(&mut [0; Self::LENGTH]))
            }
        }

        impl_fmt_from!($T<$($a),*>);
    )+}
}

macro_rules! impl_fmt_from {
    ($T:ident<>) => {
        impl From<Uuid> for $T {
            #[inline]
            fn from(f: Uuid) -> Self {
                $T(f)
            }
        }

        impl From<$T> for Uuid {
            #[inline]
            fn from(f: $T) -> Self {
                f.into_uuid()
            }
        }

        impl AsRef<Uuid> for $T {
            #[inline]
            fn as_ref(&self) -> &Uuid {
                &self.0
            }
        }

        impl Borrow<Uuid> for $T {
            #[inline]
            fn borrow(&self) -> &Uuid {
                &self.0
            }
        }
    };
    ($T:ident<$a:lifetime>) => {
        impl<$a> From<&$a Uuid> for $T<$a> {
            #[inline]
            fn from(f: &$a Uuid) -> Self {
                $T::from_uuid_ref(f)
            }
        }

        impl<$a> From<$T<$a>> for &$a Uuid {
            #[inline]
            fn from(f: $T<$a>) -> &$a Uuid {
                f.0
            }
        }

        impl<$a> AsRef<Uuid> for $T<$a> {
            #[inline]
            fn as_ref(&self) -> &Uuid {
                self.0
            }
        }

        impl<$a> Borrow<Uuid> for $T<$a> {
            #[inline]
            fn borrow(&self) -> &Uuid {
                self.0
            }
        }
    };
}

impl_fmt_traits! {
    Hyphenated<>,
    Simple<>,
    Urn<>,
    Braced<>
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphenated_trailing() {
        let mut buf = [b'x'; 100];
        let len = Uuid::nil().hyphenated().encode_lower(&mut buf).len();
        assert_eq!(len, super::Hyphenated::LENGTH);
        assert!(buf[len..].iter().all(|x| *x == b'x'));
    }

    #[test]
    fn hyphenated_ref_trailing() {
        let mut buf = [b'x'; 100];
        let len = Uuid::nil().as_hyphenated().encode_lower(&mut buf).len();
        assert_eq!(len, super::Hyphenated::LENGTH);
        assert!(buf[len..].iter().all(|x| *x == b'x'));
    }

    #[test]
    fn simple_trailing() {
        let mut buf = [b'x'; 100];
        let len = Uuid::nil().simple().encode_lower(&mut buf).len();
        assert_eq!(len, super::Simple::LENGTH);
        assert!(buf[len..].iter().all(|x| *x == b'x'));
    }

    #[test]
    fn simple_ref_trailing() {
        let mut buf = [b'x'; 100];
        let len = Uuid::nil().as_simple().encode_lower(&mut buf).len();
        assert_eq!(len, super::Simple::LENGTH);
        assert!(buf[len..].iter().all(|x| *x == b'x'));
    }

    #[test]
    fn urn_trailing() {
        let mut buf = [b'x'; 100];
        let len = Uuid::nil().urn().encode_lower(&mut buf).len();
        assert_eq!(len, super::Urn::LENGTH);
        assert!(buf[len..].iter().all(|x| *x == b'x'));
    }

    #[test]
    fn urn_ref_trailing() {
        let mut buf = [b'x'; 100];
        let len = Uuid::nil().as_urn().encode_lower(&mut buf).len();
        assert_eq!(len, super::Urn::LENGTH);
        assert!(buf[len..].iter().all(|x| *x == b'x'));
    }

    #[test]
    fn braced_trailing() {
        let mut buf = [b'x'; 100];
        let len = Uuid::nil().braced().encode_lower(&mut buf).len();
        assert_eq!(len, super::Braced::LENGTH);
        assert!(buf[len..].iter().all(|x| *x == b'x'));
    }

    #[test]
    fn braced_ref_trailing() {
        let mut buf = [b'x'; 100];
        let len = Uuid::nil().as_braced().encode_lower(&mut buf).len();
        assert_eq!(len, super::Braced::LENGTH);
        assert!(buf[len..].iter().all(|x| *x == b'x'));
    }

    #[test]
    #[should_panic]
    fn hyphenated_too_small() {
        Uuid::nil().hyphenated().encode_lower(&mut [0; 35]);
    }

    #[test]
    #[should_panic]
    fn simple_too_small() {
        Uuid::nil().simple().encode_lower(&mut [0; 31]);
    }

    #[test]
    #[should_panic]
    fn urn_too_small() {
        Uuid::nil().urn().encode_lower(&mut [0; 44]);
    }

    #[test]
    #[should_panic]
    fn braced_too_small() {
        Uuid::nil().braced().encode_lower(&mut [0; 37]);
    }

    #[test]
    fn hyphenated_to_inner() {
        let hyphenated = Uuid::nil().hyphenated();
        assert_eq!(Uuid::from(hyphenated), Uuid::nil());
    }

    #[test]
    fn simple_to_inner() {
        let simple = Uuid::nil().simple();
        assert_eq!(Uuid::from(simple), Uuid::nil());
    }

    #[test]
    fn urn_to_inner() {
        let urn = Uuid::nil().urn();
        assert_eq!(Uuid::from(urn), Uuid::nil());
    }

    #[test]
    fn braced_to_inner() {
        let braced = Uuid::nil().braced();
        assert_eq!(Uuid::from(braced), Uuid::nil());
    }
}
