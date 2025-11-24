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

//! Generate and parse universally unique identifiers (UUIDs).
//!
//! Here's an example of a UUID:
//!
//! ```text
//! 67e55044-10b1-426f-9247-bb680e5fe0c8
//! ```
//!
//! A UUID is a unique 128-bit value, stored as 16 octets, and regularly
//! formatted as a hex string in five groups. UUIDs are used to assign unique
//! identifiers to entities without requiring a central allocating authority.
//!
//! They are particularly useful in distributed systems, though can be used in
//! disparate areas, such as databases and network protocols.  Typically a UUID
//! is displayed in a readable string form as a sequence of hexadecimal digits,
//! separated into groups by hyphens.
//!
//! The uniqueness property is not strictly guaranteed, however for all
//! practical purposes, it can be assumed that an unintentional collision would
//! be extremely unlikely.
//!
//! UUIDs have a number of standardized encodings that are specified in [RFC 9562](https://www.ietf.org/rfc/rfc9562.html).
//!
//! # Getting started
//!
//! Add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies.uuid]
//! version = "1.18.1"
//! # Lets you generate random UUIDs
//! features = [
//!     "v4",
//! ]
//! ```
//!
//! When you want a UUID, you can generate one:
//!
//! ```
//! # fn main() {
//! # #[cfg(feature = "v4")]
//! # {
//! use uuid::Uuid;
//!
//! let id = Uuid::new_v4();
//! # }
//! # }
//! ```
//!
//! If you have a UUID value, you can use its string literal form inline:
//!
//! ```
//! use uuid::{uuid, Uuid};
//!
//! const ID: Uuid = uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8");
//! ```
//!
//! # Working with different UUID versions
//!
//! This library supports all standardized methods for generating UUIDs through individual Cargo features.
//!
//! By default, this crate depends on nothing but the Rust standard library and can parse and format
//! UUIDs, but cannot generate them. Depending on the kind of UUID you'd like to work with, there
//! are Cargo features that enable generating them:
//!
//! * `v1` - Version 1 UUIDs using a timestamp and monotonic counter.
//! * `v3` - Version 3 UUIDs based on the MD5 hash of some data.
//! * `v4` - Version 4 UUIDs with random data.
//! * `v5` - Version 5 UUIDs based on the SHA1 hash of some data.
//! * `v6` - Version 6 UUIDs using a timestamp and monotonic counter.
//! * `v7` - Version 7 UUIDs using a Unix timestamp.
//! * `v8` - Version 8 UUIDs using user-defined data.
//!
//! This library also includes a [`Builder`] type that can be used to help construct UUIDs of any
//! version without any additional dependencies or features. It's a lower-level API than [`Uuid`]
//! that can be used when you need control over implicit requirements on things like a source
//! of randomness.
//!
//! ## Which UUID version should I use?
//!
//! If you just want to generate unique identifiers then consider version 4 (`v4`) UUIDs. If you want
//! to use UUIDs as database keys or need to sort them then consider version 7 (`v7`) UUIDs.
//! Other versions should generally be avoided unless there's an existing need for them.
//!
//! Some UUID versions supersede others. Prefer version 6 over version 1 and version 5 over version 3.
//!
//! # Other features
//!
//! Other crate features can also be useful beyond the version support:
//!
//! * `macro-diagnostics` - enhances the diagnostics of `uuid!` macro.
//! * `serde` - adds the ability to serialize and deserialize a UUID using
//!   `serde`.
//! * `borsh` - adds the ability to serialize and deserialize a UUID using
//!   `borsh`.
//! * `arbitrary` - adds an `Arbitrary` trait implementation to `Uuid` for
//!   fuzzing.
//! * `fast-rng` - uses a faster algorithm for generating random UUIDs when available.
//!   This feature requires more dependencies to compile, but is just as suitable for
//!   UUIDs as the default algorithm.
//! * `rng-rand` - forces `rand` as the backend for randomness.
//! * `rng-getrandom` - forces `getrandom` as the backend for randomness.
//! * `bytemuck` - adds a `Pod` trait implementation to `Uuid` for byte manipulation
//!
//! # Unstable features
//!
//! Some features are unstable. They may be incomplete or depend on other
//! unstable libraries. These include:
//!
//! * `zerocopy` - adds support for zero-copy deserialization using the
//!   `zerocopy` library.
//!
//! Unstable features may break between minor releases.
//!
//! To allow unstable features, you'll need to enable the Cargo feature as
//! normal, but also pass an additional flag through your environment to opt-in
//! to unstable `uuid` features:
//!
//! ```text
//! RUSTFLAGS="--cfg uuid_unstable"
//! ```
//!
//! # Building for other targets
//!
//! ## WebAssembly
//!
//! For WebAssembly, enable the `js` feature:
//!
//! ```toml
//! [dependencies.uuid]
//! version = "1.18.1"
//! features = [
//!     "v4",
//!     "v7",
//!     "js",
//! ]
//! ```
//!
//! ## Embedded
//!
//! For embedded targets without the standard library, you'll need to
//! disable default features when building `uuid`:
//!
//! ```toml
//! [dependencies.uuid]
//! version = "1.18.1"
//! default-features = false
//! ```
//!
//! Some additional features are supported in no-std environments:
//!
//! * `v1`, `v3`, `v5`, `v6`, and `v8`.
//! * `serde`.
//!
//! If you need to use `v4` or `v7` in a no-std environment, you'll need to
//! produce random bytes yourself and then pass them to [`Builder::from_random_bytes`]
//! without enabling the `v4` or `v7` features.
//!
//! If you're using `getrandom`, you can specify the `rng-getrandom` or `rng-rand`
//! features of `uuid` and configure `getrandom`'s provider per its docs. `uuid`
//! may upgrade its version of `getrandom` in minor releases.
//!
//! # Examples
//!
//! Parse a UUID given in the simple format and print it as a URN:
//!
//! ```
//! # use uuid::Uuid;
//! # fn main() -> Result<(), uuid::Error> {
//! let my_uuid = Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8")?;
//!
//! println!("{}", my_uuid.urn());
//! # Ok(())
//! # }
//! ```
//!
//! Generate a random UUID and print it out in hexadecimal form:
//!
//! ```
//! // Note that this requires the `v4` feature to be enabled.
//! # use uuid::Uuid;
//! # fn main() {
//! # #[cfg(feature = "v4")] {
//! let my_uuid = Uuid::new_v4();
//!
//! println!("{}", my_uuid);
//! # }
//! # }
//! ```
//!
//! # References
//!
//! * [Wikipedia: Universally Unique Identifier](http://en.wikipedia.org/wiki/Universally_unique_identifier)
//! * [RFC 9562: Universally Unique IDentifiers (UUID)](https://www.ietf.org/rfc/rfc9562.html).
//!
//! [`wasm-bindgen`]: https://crates.io/crates/wasm-bindgen

#![no_std]
#![deny(missing_debug_implementations, missing_docs)]
#![allow(clippy::mixed_attributes_style)]
#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico",
    html_root_url = "https://docs.rs/uuid/1.18.1"
)]

#[cfg(any(feature = "std", test))]
#[macro_use]
extern crate std;

#[cfg(all(not(feature = "std"), not(test)))]
#[macro_use]
extern crate core as std;

#[macro_use]
mod macros;

mod builder;
mod error;
mod non_nil;
mod parser;

pub mod fmt;
pub mod timestamp;

use core::hash::{Hash, Hasher};
pub use timestamp::{context::NoContext, ClockSequence, Timestamp};

#[cfg(any(feature = "v1", feature = "v6"))]
pub use timestamp::context::Context;

#[cfg(feature = "v7")]
pub use timestamp::context::ContextV7;

#[cfg(feature = "v1")]
#[doc(hidden)]
// Soft-deprecated (Rust doesn't support deprecating re-exports)
// Use `Context` from the crate root instead
pub mod v1;
#[cfg(feature = "v3")]
mod v3;
#[cfg(feature = "v4")]
mod v4;
#[cfg(feature = "v5")]
mod v5;
#[cfg(feature = "v6")]
mod v6;
#[cfg(feature = "v7")]
mod v7;
#[cfg(feature = "v8")]
mod v8;

#[cfg(feature = "md5")]
mod md5;
#[cfg(feature = "rng")]
mod rng;
#[cfg(feature = "sha1")]
mod sha1;

mod external;

#[doc(hidden)]
#[cfg(feature = "macro-diagnostics")]
pub extern crate uuid_macro_internal;

#[doc(hidden)]
pub mod __macro_support {
    pub use crate::std::result::Result::{Err, Ok};
}

use crate::std::convert;

pub use crate::{builder::Builder, error::Error, non_nil::NonNilUuid};

/// A 128-bit (16 byte) buffer containing the UUID.
///
/// # ABI
///
/// The `Bytes` type is always guaranteed to be have the same ABI as [`Uuid`].
pub type Bytes = [u8; 16];

/// The version of the UUID, denoting the generating algorithm.
///
/// # References
///
/// * [Version Field in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-4.2)
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
#[repr(u8)]
pub enum Version {
    /// The "nil" (all zeros) UUID.
    Nil = 0u8,
    /// Version 1: Timestamp and node ID.
    Mac = 1,
    /// Version 2: DCE Security.
    Dce = 2,
    /// Version 3: MD5 hash.
    Md5 = 3,
    /// Version 4: Random.
    Random = 4,
    /// Version 5: SHA-1 hash.
    Sha1 = 5,
    /// Version 6: Sortable Timestamp and node ID.
    SortMac = 6,
    /// Version 7: Timestamp and random.
    SortRand = 7,
    /// Version 8: Custom.
    Custom = 8,
    /// The "max" (all ones) UUID.
    Max = 0xff,
}

/// The reserved variants of UUIDs.
///
/// # References
///
/// * [Variant Field in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-4.1)
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
#[repr(u8)]
pub enum Variant {
    /// Reserved by the NCS for backward compatibility.
    NCS = 0u8,
    /// As described in the RFC 9562 Specification (default).
    /// (for backward compatibility it is not yet renamed)
    RFC4122,
    /// Reserved by Microsoft for backward compatibility.
    Microsoft,
    /// Reserved for future expansion.
    Future,
}

/// A Universally Unique Identifier (UUID).
///
/// # Examples
///
/// Parse a UUID given in the simple format and print it as a urn:
///
/// ```
/// # use uuid::Uuid;
/// # fn main() -> Result<(), uuid::Error> {
/// let my_uuid = Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8")?;
///
/// println!("{}", my_uuid.urn());
/// # Ok(())
/// # }
/// ```
///
/// Create a new random (V4) UUID and print it out in hexadecimal form:
///
/// ```
/// // Note that this requires the `v4` feature enabled in the uuid crate.
/// # use uuid::Uuid;
/// # fn main() {
/// # #[cfg(feature = "v4")] {
/// let my_uuid = Uuid::new_v4();
///
/// println!("{}", my_uuid);
/// # }
/// # }
/// ```
///
/// # Formatting
///
/// A UUID can be formatted in one of a few ways:
///
/// * [`simple`](#method.simple): `a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8`.
/// * [`hyphenated`](#method.hyphenated):
///   `a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8`.
/// * [`urn`](#method.urn): `urn:uuid:A1A2A3A4-B1B2-C1C2-D1D2-D3D4D5D6D7D8`.
/// * [`braced`](#method.braced): `{a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8}`.
///
/// The default representation when formatting a UUID with `Display` is
/// hyphenated:
///
/// ```
/// # use uuid::Uuid;
/// # fn main() -> Result<(), uuid::Error> {
/// let my_uuid = Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8")?;
///
/// assert_eq!(
///     "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8",
///     my_uuid.to_string(),
/// );
/// # Ok(())
/// # }
/// ```
///
/// Other formats can be specified using adapter methods on the UUID:
///
/// ```
/// # use uuid::Uuid;
/// # fn main() -> Result<(), uuid::Error> {
/// let my_uuid = Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8")?;
///
/// assert_eq!(
///     "urn:uuid:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8",
///     my_uuid.urn().to_string(),
/// );
/// # Ok(())
/// # }
/// ```
///
/// # Endianness
///
/// The specification for UUIDs encodes the integer fields that make up the
/// value in big-endian order. This crate assumes integer inputs are already in
/// the correct order by default, regardless of the endianness of the
/// environment. Most methods that accept integers have a `_le` variant (such as
/// `from_fields_le`) that assumes any integer values will need to have their
/// bytes flipped, regardless of the endianness of the environment.
///
/// Most users won't need to worry about endianness unless they need to operate
/// on individual fields (such as when converting between Microsoft GUIDs). The
/// important things to remember are:
///
/// - The endianness is in terms of the fields of the UUID, not the environment.
/// - The endianness is assumed to be big-endian when there's no `_le` suffix
///   somewhere.
/// - Byte-flipping in `_le` methods applies to each integer.
/// - Endianness roundtrips, so if you create a UUID with `from_fields_le`
///   you'll get the same values back out with `to_fields_le`.
///
/// # ABI
///
/// The `Uuid` type is always guaranteed to be have the same ABI as [`Bytes`].
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
// NOTE: Also check `NonNilUuid` when ading new derives here
#[cfg_attr(
    feature = "borsh",
    derive(borsh_derive::BorshDeserialize, borsh_derive::BorshSerialize)
)]
#[cfg_attr(
    feature = "bytemuck",
    derive(bytemuck::Zeroable, bytemuck::Pod, bytemuck::TransparentWrapper)
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
pub struct Uuid(Bytes);

impl Uuid {
    /// UUID namespace for Domain Name System (DNS).
    pub const NAMESPACE_DNS: Self = Uuid([
        0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);

    /// UUID namespace for ISO Object Identifiers (OIDs).
    pub const NAMESPACE_OID: Self = Uuid([
        0x6b, 0xa7, 0xb8, 0x12, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);

    /// UUID namespace for Uniform Resource Locators (URLs).
    pub const NAMESPACE_URL: Self = Uuid([
        0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);

    /// UUID namespace for X.500 Distinguished Names (DNs).
    pub const NAMESPACE_X500: Self = Uuid([
        0x6b, 0xa7, 0xb8, 0x14, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ]);

    /// Returns the variant of the UUID structure.
    ///
    /// This determines the interpretation of the structure of the UUID.
    /// This method simply reads the value of the variant byte. It doesn't
    /// validate the rest of the UUID as conforming to that variant.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use uuid::{Uuid, Variant};
    /// # fn main() -> Result<(), uuid::Error> {
    /// let my_uuid = Uuid::parse_str("02f09a3f-1624-3b1d-8409-44eff7708208")?;
    ///
    /// assert_eq!(Variant::RFC4122, my_uuid.get_variant());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// * [Variant Field in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-4.1)
    pub const fn get_variant(&self) -> Variant {
        match self.as_bytes()[8] {
            x if x & 0x80 == 0x00 => Variant::NCS,
            x if x & 0xc0 == 0x80 => Variant::RFC4122,
            x if x & 0xe0 == 0xc0 => Variant::Microsoft,
            x if x & 0xe0 == 0xe0 => Variant::Future,
            // The above match arms are actually exhaustive
            // We just return `Future` here because we can't
            // use `unreachable!()` in a `const fn`
            _ => Variant::Future,
        }
    }

    /// Returns the version number of the UUID.
    ///
    /// This represents the algorithm used to generate the value.
    /// This method is the future-proof alternative to [`Uuid::get_version`].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use uuid::Uuid;
    /// # fn main() -> Result<(), uuid::Error> {
    /// let my_uuid = Uuid::parse_str("02f09a3f-1624-3b1d-8409-44eff7708208")?;
    ///
    /// assert_eq!(3, my_uuid.get_version_num());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// * [Version Field in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-4.2)
    pub const fn get_version_num(&self) -> usize {
        (self.as_bytes()[6] >> 4) as usize
    }

    /// Returns the version of the UUID.
    ///
    /// This represents the algorithm used to generate the value.
    /// If the version field doesn't contain a recognized version then `None`
    /// is returned. If you're trying to read the version for a future extension
    /// you can also use [`Uuid::get_version_num`] to unconditionally return a
    /// number. Future extensions may start to return `Some` once they're
    /// standardized and supported.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use uuid::{Uuid, Version};
    /// # fn main() -> Result<(), uuid::Error> {
    /// let my_uuid = Uuid::parse_str("02f09a3f-1624-3b1d-8409-44eff7708208")?;
    ///
    /// assert_eq!(Some(Version::Md5), my_uuid.get_version());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # References
    ///
    /// * [Version Field in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-4.2)
    pub const fn get_version(&self) -> Option<Version> {
        match self.get_version_num() {
            0 if self.is_nil() => Some(Version::Nil),
            1 => Some(Version::Mac),
            2 => Some(Version::Dce),
            3 => Some(Version::Md5),
            4 => Some(Version::Random),
            5 => Some(Version::Sha1),
            6 => Some(Version::SortMac),
            7 => Some(Version::SortRand),
            8 => Some(Version::Custom),
            0xf => Some(Version::Max),
            _ => None,
        }
    }

    /// Returns the four field values of the UUID.
    ///
    /// These values can be passed to the [`Uuid::from_fields`] method to get
    /// the original `Uuid` back.
    ///
    /// * The first field value represents the first group of (eight) hex
    ///   digits, taken as a big-endian `u32` value.  For V1 UUIDs, this field
    ///   represents the low 32 bits of the timestamp.
    /// * The second field value represents the second group of (four) hex
    ///   digits, taken as a big-endian `u16` value.  For V1 UUIDs, this field
    ///   represents the middle 16 bits of the timestamp.
    /// * The third field value represents the third group of (four) hex digits,
    ///   taken as a big-endian `u16` value.  The 4 most significant bits give
    ///   the UUID version, and for V1 UUIDs, the last 12 bits represent the
    ///   high 12 bits of the timestamp.
    /// * The last field value represents the last two groups of four and twelve
    ///   hex digits, taken in order.  The first 1-3 bits of this indicate the
    ///   UUID variant, and for V1 UUIDs, the next 13-15 bits indicate the clock
    ///   sequence and the last 48 bits indicate the node ID.
    ///
    /// # Examples
    ///
    /// ```
    /// # use uuid::Uuid;
    /// # fn main() -> Result<(), uuid::Error> {
    /// let uuid = Uuid::nil();
    ///
    /// assert_eq!(uuid.as_fields(), (0, 0, 0, &[0u8; 8]));
    ///
    /// let uuid = Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8")?;
    ///
    /// assert_eq!(
    ///     uuid.as_fields(),
    ///     (
    ///         0xa1a2a3a4,
    ///         0xb1b2,
    ///         0xc1c2,
    ///         &[0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8],
    ///     )
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn as_fields(&self) -> (u32, u16, u16, &[u8; 8]) {
        let bytes = self.as_bytes();

        let d1 = (bytes[0] as u32) << 24
            | (bytes[1] as u32) << 16
            | (bytes[2] as u32) << 8
            | (bytes[3] as u32);

        let d2 = (bytes[4] as u16) << 8 | (bytes[5] as u16);

        let d3 = (bytes[6] as u16) << 8 | (bytes[7] as u16);

        let d4: &[u8; 8] = convert::TryInto::try_into(&bytes[8..16]).unwrap();
        (d1, d2, d3, d4)
    }

    /// Returns the four field values of the UUID in little-endian order.
    ///
    /// The bytes in the returned integer fields will be converted from
    /// big-endian order. This is based on the endianness of the UUID,
    /// rather than the target environment so bytes will be flipped on both
    /// big and little endian machines.
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// # fn main() -> Result<(), uuid::Error> {
    /// let uuid = Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8")?;
    ///
    /// assert_eq!(
    ///     uuid.to_fields_le(),
    ///     (
    ///         0xa4a3a2a1,
    ///         0xb2b1,
    ///         0xc2c1,
    ///         &[0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8],
    ///     )
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_fields_le(&self) -> (u32, u16, u16, &[u8; 8]) {
        let d1 = (self.as_bytes()[0] as u32)
            | (self.as_bytes()[1] as u32) << 8
            | (self.as_bytes()[2] as u32) << 16
            | (self.as_bytes()[3] as u32) << 24;

        let d2 = (self.as_bytes()[4] as u16) | (self.as_bytes()[5] as u16) << 8;

        let d3 = (self.as_bytes()[6] as u16) | (self.as_bytes()[7] as u16) << 8;

        let d4: &[u8; 8] = convert::TryInto::try_into(&self.as_bytes()[8..16]).unwrap();
        (d1, d2, d3, d4)
    }

    /// Returns a 128bit value containing the value.
    ///
    /// The bytes in the UUID will be packed directly into a `u128`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use uuid::Uuid;
    /// # fn main() -> Result<(), uuid::Error> {
    /// let uuid = Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8")?;
    ///
    /// assert_eq!(
    ///     uuid.as_u128(),
    ///     0xa1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8,
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub const fn as_u128(&self) -> u128 {
        u128::from_be_bytes(*self.as_bytes())
    }

    /// Returns a 128bit little-endian value containing the value.
    ///
    /// The bytes in the `u128` will be flipped to convert into big-endian
    /// order. This is based on the endianness of the UUID, rather than the
    /// target environment so bytes will be flipped on both big and little
    /// endian machines.
    ///
    /// Note that this will produce a different result than
    /// [`Uuid::to_fields_le`], because the entire UUID is reversed, rather
    /// than reversing the individual fields in-place.
    ///
    /// # Examples
    ///
    /// ```
    /// # use uuid::Uuid;
    /// # fn main() -> Result<(), uuid::Error> {
    /// let uuid = Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8")?;
    ///
    /// assert_eq!(
    ///     uuid.to_u128_le(),
    ///     0xd8d7d6d5d4d3d2d1c2c1b2b1a4a3a2a1,
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub const fn to_u128_le(&self) -> u128 {
        u128::from_le_bytes(*self.as_bytes())
    }

    /// Returns two 64bit values containing the value.
    ///
    /// The bytes in the UUID will be split into two `u64`.
    /// The first u64 represents the 64 most significant bits,
    /// the second one represents the 64 least significant.
    ///
    /// # Examples
    ///
    /// ```
    /// # use uuid::Uuid;
    /// # fn main() -> Result<(), uuid::Error> {
    /// let uuid = Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8")?;
    /// assert_eq!(
    ///     uuid.as_u64_pair(),
    ///     (0xa1a2a3a4b1b2c1c2, 0xd1d2d3d4d5d6d7d8),
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub const fn as_u64_pair(&self) -> (u64, u64) {
        let value = self.as_u128();
        ((value >> 64) as u64, value as u64)
    }

    /// Returns a slice of 16 octets containing the value.
    ///
    /// This method borrows the underlying byte value of the UUID.
    ///
    /// # Examples
    ///
    /// ```
    /// # use uuid::Uuid;
    /// let bytes1 = [
    ///     0xa1, 0xa2, 0xa3, 0xa4,
    ///     0xb1, 0xb2,
    ///     0xc1, 0xc2,
    ///     0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8,
    /// ];
    /// let uuid1 = Uuid::from_bytes_ref(&bytes1);
    ///
    /// let bytes2 = uuid1.as_bytes();
    /// let uuid2 = Uuid::from_bytes_ref(bytes2);
    ///
    /// assert_eq!(uuid1, uuid2);
    ///
    /// assert!(std::ptr::eq(
    ///     uuid2 as *const Uuid as *const u8,
    ///     &bytes1 as *const [u8; 16] as *const u8,
    /// ));
    /// ```
    #[inline]
    pub const fn as_bytes(&self) -> &Bytes {
        &self.0
    }

    /// Consumes self and returns the underlying byte value of the UUID.
    ///
    /// # Examples
    ///
    /// ```
    /// # use uuid::Uuid;
    /// let bytes = [
    ///     0xa1, 0xa2, 0xa3, 0xa4,
    ///     0xb1, 0xb2,
    ///     0xc1, 0xc2,
    ///     0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8,
    /// ];
    /// let uuid = Uuid::from_bytes(bytes);
    /// assert_eq!(bytes, uuid.into_bytes());
    /// ```
    #[inline]
    pub const fn into_bytes(self) -> Bytes {
        self.0
    }

    /// Returns the bytes of the UUID in little-endian order.
    ///
    /// The bytes will be flipped to convert into little-endian order. This is
    /// based on the endianness of the UUID, rather than the target environment
    /// so bytes will be flipped on both big and little endian machines.
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// # fn main() -> Result<(), uuid::Error> {
    /// let uuid = Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8")?;
    ///
    /// assert_eq!(
    ///     uuid.to_bytes_le(),
    ///     ([
    ///         0xa4, 0xa3, 0xa2, 0xa1, 0xb2, 0xb1, 0xc2, 0xc1, 0xd1, 0xd2,
    ///         0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8
    ///     ])
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub const fn to_bytes_le(&self) -> Bytes {
        [
            self.0[3], self.0[2], self.0[1], self.0[0], self.0[5], self.0[4], self.0[7], self.0[6],
            self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14],
            self.0[15],
        ]
    }

    /// Tests if the UUID is nil (all zeros).
    pub const fn is_nil(&self) -> bool {
        self.as_u128() == u128::MIN
    }

    /// Tests if the UUID is max (all ones).
    pub const fn is_max(&self) -> bool {
        self.as_u128() == u128::MAX
    }

    /// A buffer that can be used for `encode_...` calls, that is
    /// guaranteed to be long enough for any of the format adapters.
    ///
    /// # Examples
    ///
    /// ```
    /// # use uuid::Uuid;
    /// let uuid = Uuid::nil();
    ///
    /// assert_eq!(
    ///     uuid.simple().encode_lower(&mut Uuid::encode_buffer()),
    ///     "00000000000000000000000000000000"
    /// );
    ///
    /// assert_eq!(
    ///     uuid.hyphenated()
    ///         .encode_lower(&mut Uuid::encode_buffer()),
    ///     "00000000-0000-0000-0000-000000000000"
    /// );
    ///
    /// assert_eq!(
    ///     uuid.urn().encode_lower(&mut Uuid::encode_buffer()),
    ///     "urn:uuid:00000000-0000-0000-0000-000000000000"
    /// );
    /// ```
    pub const fn encode_buffer() -> [u8; fmt::Urn::LENGTH] {
        [0; fmt::Urn::LENGTH]
    }

    /// If the UUID is the correct version (v1, v6, or v7) this will return
    /// the timestamp in a version-agnostic [`Timestamp`]. For other versions
    /// this will return `None`.
    ///
    /// # Roundtripping
    ///
    /// This method is unlikely to roundtrip a timestamp in a UUID due to the way
    /// UUIDs encode timestamps. The timestamp returned from this method will be truncated to
    /// 100ns precision for version 1 and 6 UUIDs, and to millisecond precision for version 7 UUIDs.
    pub const fn get_timestamp(&self) -> Option<Timestamp> {
        match self.get_version() {
            Some(Version::Mac) => {
                let (ticks, counter) = timestamp::decode_gregorian_timestamp(self);

                Some(Timestamp::from_gregorian(ticks, counter))
            }
            Some(Version::SortMac) => {
                let (ticks, counter) = timestamp::decode_sorted_gregorian_timestamp(self);

                Some(Timestamp::from_gregorian(ticks, counter))
            }
            Some(Version::SortRand) => {
                let millis = timestamp::decode_unix_timestamp_millis(self);

                let seconds = millis / 1000;
                let nanos = ((millis % 1000) * 1_000_000) as u32;

                Some(Timestamp::from_unix_time(seconds, nanos, 0, 0))
            }
            _ => None,
        }
    }

    /// If the UUID is the correct version (v1, or v6) this will return the
    /// node value as a 6-byte array. For other versions this will return `None`.
    pub const fn get_node_id(&self) -> Option<[u8; 6]> {
        match self.get_version() {
            Some(Version::Mac) | Some(Version::SortMac) => {
                let mut node_id = [0; 6];

                node_id[0] = self.0[10];
                node_id[1] = self.0[11];
                node_id[2] = self.0[12];
                node_id[3] = self.0[13];
                node_id[4] = self.0[14];
                node_id[5] = self.0[15];

                Some(node_id)
            }
            _ => None,
        }
    }
}

impl Hash for Uuid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0);
    }
}

impl Default for Uuid {
    #[inline]
    fn default() -> Self {
        Uuid::nil()
    }
}

impl AsRef<Uuid> for Uuid {
    #[inline]
    fn as_ref(&self) -> &Uuid {
        self
    }
}

impl AsRef<[u8]> for Uuid {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(feature = "std")]
impl From<Uuid> for std::vec::Vec<u8> {
    fn from(value: Uuid) -> Self {
        value.0.to_vec()
    }
}

#[cfg(feature = "std")]
impl std::convert::TryFrom<std::vec::Vec<u8>> for Uuid {
    type Error = Error;

    fn try_from(value: std::vec::Vec<u8>) -> Result<Self, Self::Error> {
        Uuid::from_slice(&value)
    }
}

#[cfg(feature = "serde")]
pub mod serde {
    //! Adapters for alternative `serde` formats.
    //!
    //! This module contains adapters you can use with [`#[serde(with)]`](https://serde.rs/field-attrs.html#with)
    //! to change the way a [`Uuid`](../struct.Uuid.html) is serialized
    //! and deserialized.

    pub use crate::external::serde_support::{braced, compact, simple, urn};
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::std::string::{String, ToString};

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::*;

    macro_rules! check {
        ($buf:ident, $format:expr, $target:expr, $len:expr, $cond:expr) => {
            $buf.clear();
            write!($buf, $format, $target).unwrap();
            assert!($buf.len() == $len);
            assert!($buf.chars().all($cond), "{}", $buf);
        };
    }

    pub const fn new() -> Uuid {
        Uuid::from_bytes([
            0xF9, 0x16, 0x8C, 0x5E, 0xCE, 0xB2, 0x4F, 0xAA, 0xB6, 0xBF, 0x32, 0x9B, 0xF3, 0x9F,
            0xA1, 0xE4,
        ])
    }

    pub const fn new2() -> Uuid {
        Uuid::from_bytes([
            0xF9, 0x16, 0x8C, 0x5E, 0xCE, 0xB2, 0x4F, 0xAB, 0xB6, 0xBF, 0x32, 0x9B, 0xF3, 0x9F,
            0xA1, 0xE4,
        ])
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_uuid_compare() {
        let uuid1 = new();
        let uuid2 = new2();

        assert_eq!(uuid1, uuid1);
        assert_eq!(uuid2, uuid2);

        assert_ne!(uuid1, uuid2);
        assert_ne!(uuid2, uuid1);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_uuid_default() {
        let default_uuid = Uuid::default();
        let nil_uuid = Uuid::nil();

        assert_eq!(default_uuid, nil_uuid);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_uuid_display() {
        use crate::std::fmt::Write;

        let uuid = new();
        let s = uuid.to_string();
        let mut buffer = String::new();

        assert_eq!(s, uuid.hyphenated().to_string());

        check!(buffer, "{}", uuid, 36, |c| c.is_lowercase()
            || c.is_digit(10)
            || c == '-');
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_uuid_lowerhex() {
        use crate::std::fmt::Write;

        let mut buffer = String::new();
        let uuid = new();

        check!(buffer, "{:x}", uuid, 36, |c| c.is_lowercase()
            || c.is_digit(10)
            || c == '-');
    }

    // noinspection RsAssertEqual
    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_uuid_operator_eq() {
        let uuid1 = new();
        let uuid1_dup = uuid1.clone();
        let uuid2 = new2();

        assert!(uuid1 == uuid1);
        assert!(uuid1 == uuid1_dup);
        assert!(uuid1_dup == uuid1);

        assert!(uuid1 != uuid2);
        assert!(uuid2 != uuid1);
        assert!(uuid1_dup != uuid2);
        assert!(uuid2 != uuid1_dup);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_uuid_to_string() {
        use crate::std::fmt::Write;

        let uuid = new();
        let s = uuid.to_string();
        let mut buffer = String::new();

        assert_eq!(s.len(), 36);

        check!(buffer, "{}", s, 36, |c| c.is_lowercase()
            || c.is_digit(10)
            || c == '-');
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_non_conforming() {
        let from_bytes =
            Uuid::from_bytes([4, 54, 67, 12, 43, 2, 2, 76, 32, 50, 87, 5, 1, 33, 43, 87]);

        assert_eq!(from_bytes.get_version(), None);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_nil() {
        let nil = Uuid::nil();
        let not_nil = new();

        assert!(nil.is_nil());
        assert!(!not_nil.is_nil());

        assert_eq!(nil.get_version(), Some(Version::Nil));
        assert_eq!(not_nil.get_version(), Some(Version::Random));

        assert_eq!(
            nil,
            Builder::from_bytes([0; 16])
                .with_version(Version::Nil)
                .into_uuid()
        );
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_max() {
        let max = Uuid::max();
        let not_max = new();

        assert!(max.is_max());
        assert!(!not_max.is_max());

        assert_eq!(max.get_version(), Some(Version::Max));
        assert_eq!(not_max.get_version(), Some(Version::Random));

        assert_eq!(
            max,
            Builder::from_bytes([0xff; 16])
                .with_version(Version::Max)
                .into_uuid()
        );
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_predefined_namespaces() {
        assert_eq!(
            Uuid::NAMESPACE_DNS.hyphenated().to_string(),
            "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
        );
        assert_eq!(
            Uuid::NAMESPACE_URL.hyphenated().to_string(),
            "6ba7b811-9dad-11d1-80b4-00c04fd430c8"
        );
        assert_eq!(
            Uuid::NAMESPACE_OID.hyphenated().to_string(),
            "6ba7b812-9dad-11d1-80b4-00c04fd430c8"
        );
        assert_eq!(
            Uuid::NAMESPACE_X500.hyphenated().to_string(),
            "6ba7b814-9dad-11d1-80b4-00c04fd430c8"
        );
    }

    #[cfg(feature = "v3")]
    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_get_version_v3() {
        let uuid = Uuid::new_v3(&Uuid::NAMESPACE_DNS, "rust-lang.org".as_bytes());

        assert_eq!(uuid.get_version().unwrap(), Version::Md5);
        assert_eq!(uuid.get_version_num(), 3);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_get_timestamp_unsupported_version() {
        let uuid = new();

        assert_ne!(Version::Mac, uuid.get_version().unwrap());
        assert_ne!(Version::SortMac, uuid.get_version().unwrap());
        assert_ne!(Version::SortRand, uuid.get_version().unwrap());

        assert!(uuid.get_timestamp().is_none());
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_get_node_id_unsupported_version() {
        let uuid = new();

        assert_ne!(Version::Mac, uuid.get_version().unwrap());
        assert_ne!(Version::SortMac, uuid.get_version().unwrap());

        assert!(uuid.get_node_id().is_none());
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_get_variant() {
        let uuid1 = new();
        let uuid2 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let uuid3 = Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
        let uuid4 = Uuid::parse_str("936DA01F9ABD4d9dC0C702AF85C822A8").unwrap();
        let uuid5 = Uuid::parse_str("F9168C5E-CEB2-4faa-D6BF-329BF39FA1E4").unwrap();
        let uuid6 = Uuid::parse_str("f81d4fae-7dec-11d0-7765-00a0c91e6bf6").unwrap();

        assert_eq!(uuid1.get_variant(), Variant::RFC4122);
        assert_eq!(uuid2.get_variant(), Variant::RFC4122);
        assert_eq!(uuid3.get_variant(), Variant::RFC4122);
        assert_eq!(uuid4.get_variant(), Variant::Microsoft);
        assert_eq!(uuid5.get_variant(), Variant::Microsoft);
        assert_eq!(uuid6.get_variant(), Variant::NCS);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_to_simple_string() {
        let uuid1 = new();
        let s = uuid1.simple().to_string();

        assert_eq!(s.len(), 32);
        assert!(s.chars().all(|c| c.is_digit(16)));
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_hyphenated_string() {
        let uuid1 = new();
        let s = uuid1.hyphenated().to_string();

        assert_eq!(36, s.len());
        assert!(s.chars().all(|c| c.is_digit(16) || c == '-'));
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_upper_lower_hex() {
        use std::fmt::Write;

        let mut buf = String::new();
        let u = new();

        macro_rules! check {
            ($buf:ident, $format:expr, $target:expr, $len:expr, $cond:expr) => {
                $buf.clear();
                write!($buf, $format, $target).unwrap();
                assert_eq!($len, buf.len());
                assert!($buf.chars().all($cond), "{}", $buf);
            };
        }

        check!(buf, "{:x}", u, 36, |c| c.is_lowercase()
            || c.is_digit(10)
            || c == '-');
        check!(buf, "{:X}", u, 36, |c| c.is_uppercase()
            || c.is_digit(10)
            || c == '-');
        check!(buf, "{:#x}", u, 36, |c| c.is_lowercase()
            || c.is_digit(10)
            || c == '-');
        check!(buf, "{:#X}", u, 36, |c| c.is_uppercase()
            || c.is_digit(10)
            || c == '-');

        check!(buf, "{:X}", u.hyphenated(), 36, |c| c.is_uppercase()
            || c.is_digit(10)
            || c == '-');
        check!(buf, "{:X}", u.simple(), 32, |c| c.is_uppercase()
            || c.is_digit(10));
        check!(buf, "{:#X}", u.hyphenated(), 36, |c| c.is_uppercase()
            || c.is_digit(10)
            || c == '-');
        check!(buf, "{:#X}", u.simple(), 32, |c| c.is_uppercase()
            || c.is_digit(10));

        check!(buf, "{:x}", u.hyphenated(), 36, |c| c.is_lowercase()
            || c.is_digit(10)
            || c == '-');
        check!(buf, "{:x}", u.simple(), 32, |c| c.is_lowercase()
            || c.is_digit(10));
        check!(buf, "{:#x}", u.hyphenated(), 36, |c| c.is_lowercase()
            || c.is_digit(10)
            || c == '-');
        check!(buf, "{:#x}", u.simple(), 32, |c| c.is_lowercase()
            || c.is_digit(10));
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_to_urn_string() {
        let uuid1 = new();
        let ss = uuid1.urn().to_string();
        let s = &ss[9..];

        assert!(ss.starts_with("urn:uuid:"));
        assert_eq!(s.len(), 36);
        assert!(s.chars().all(|c| c.is_digit(16) || c == '-'));
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_to_simple_string_matching() {
        let uuid1 = new();

        let hs = uuid1.hyphenated().to_string();
        let ss = uuid1.simple().to_string();

        let hsn = hs.chars().filter(|&c| c != '-').collect::<String>();

        assert_eq!(hsn, ss);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_string_roundtrip() {
        let uuid = new();

        let hs = uuid.hyphenated().to_string();
        let uuid_hs = Uuid::parse_str(&hs).unwrap();
        assert_eq!(uuid_hs, uuid);

        let ss = uuid.to_string();
        let uuid_ss = Uuid::parse_str(&ss).unwrap();
        assert_eq!(uuid_ss, uuid);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_from_fields() {
        let d1: u32 = 0xa1a2a3a4;
        let d2: u16 = 0xb1b2;
        let d3: u16 = 0xc1c2;
        let d4 = [0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8];

        let u = Uuid::from_fields(d1, d2, d3, &d4);

        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";
        let result = u.simple().to_string();
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_from_fields_le() {
        let d1: u32 = 0xa4a3a2a1;
        let d2: u16 = 0xb2b1;
        let d3: u16 = 0xc2c1;
        let d4 = [0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8];

        let u = Uuid::from_fields_le(d1, d2, d3, &d4);

        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";
        let result = u.simple().to_string();
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_as_fields() {
        let u = new();
        let (d1, d2, d3, d4) = u.as_fields();

        assert_ne!(d1, 0);
        assert_ne!(d2, 0);
        assert_ne!(d3, 0);
        assert_eq!(d4.len(), 8);
        assert!(!d4.iter().all(|&b| b == 0));
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_fields_roundtrip() {
        let d1_in: u32 = 0xa1a2a3a4;
        let d2_in: u16 = 0xb1b2;
        let d3_in: u16 = 0xc1c2;
        let d4_in = &[0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8];

        let u = Uuid::from_fields(d1_in, d2_in, d3_in, d4_in);
        let (d1_out, d2_out, d3_out, d4_out) = u.as_fields();

        assert_eq!(d1_in, d1_out);
        assert_eq!(d2_in, d2_out);
        assert_eq!(d3_in, d3_out);
        assert_eq!(d4_in, d4_out);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_fields_le_roundtrip() {
        let d1_in: u32 = 0xa4a3a2a1;
        let d2_in: u16 = 0xb2b1;
        let d3_in: u16 = 0xc2c1;
        let d4_in = &[0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8];

        let u = Uuid::from_fields_le(d1_in, d2_in, d3_in, d4_in);
        let (d1_out, d2_out, d3_out, d4_out) = u.to_fields_le();

        assert_eq!(d1_in, d1_out);
        assert_eq!(d2_in, d2_out);
        assert_eq!(d3_in, d3_out);
        assert_eq!(d4_in, d4_out);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_fields_le_are_actually_le() {
        let d1_in: u32 = 0xa1a2a3a4;
        let d2_in: u16 = 0xb1b2;
        let d3_in: u16 = 0xc1c2;
        let d4_in = &[0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8];

        let u = Uuid::from_fields(d1_in, d2_in, d3_in, d4_in);
        let (d1_out, d2_out, d3_out, d4_out) = u.to_fields_le();

        assert_eq!(d1_in, d1_out.swap_bytes());
        assert_eq!(d2_in, d2_out.swap_bytes());
        assert_eq!(d3_in, d3_out.swap_bytes());
        assert_eq!(d4_in, d4_out);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_from_u128() {
        let v_in: u128 = 0xa1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8;

        let u = Uuid::from_u128(v_in);

        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";
        let result = u.simple().to_string();
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_from_u128_le() {
        let v_in: u128 = 0xd8d7d6d5d4d3d2d1c2c1b2b1a4a3a2a1;

        let u = Uuid::from_u128_le(v_in);

        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";
        let result = u.simple().to_string();
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_from_u64_pair() {
        let high_in: u64 = 0xa1a2a3a4b1b2c1c2;
        let low_in: u64 = 0xd1d2d3d4d5d6d7d8;

        let u = Uuid::from_u64_pair(high_in, low_in);

        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";
        let result = u.simple().to_string();
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_u128_roundtrip() {
        let v_in: u128 = 0xa1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8;

        let u = Uuid::from_u128(v_in);
        let v_out = u.as_u128();

        assert_eq!(v_in, v_out);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_u128_le_roundtrip() {
        let v_in: u128 = 0xd8d7d6d5d4d3d2d1c2c1b2b1a4a3a2a1;

        let u = Uuid::from_u128_le(v_in);
        let v_out = u.to_u128_le();

        assert_eq!(v_in, v_out);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_u64_pair_roundtrip() {
        let high_in: u64 = 0xa1a2a3a4b1b2c1c2;
        let low_in: u64 = 0xd1d2d3d4d5d6d7d8;

        let u = Uuid::from_u64_pair(high_in, low_in);
        let (high_out, low_out) = u.as_u64_pair();

        assert_eq!(high_in, high_out);
        assert_eq!(low_in, low_out);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_u128_le_is_actually_le() {
        let v_in: u128 = 0xa1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8;

        let u = Uuid::from_u128(v_in);
        let v_out = u.to_u128_le();

        assert_eq!(v_in, v_out.swap_bytes());
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_from_slice() {
        let b = [
            0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8,
        ];

        let u = Uuid::from_slice(&b).unwrap();
        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";

        assert_eq!(u.simple().to_string(), expected);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_from_bytes() {
        let b = [
            0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8,
        ];

        let u = Uuid::from_bytes(b);
        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";

        assert_eq!(u.simple().to_string(), expected);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_as_bytes() {
        let u = new();
        let ub = u.as_bytes();
        let ur: &[u8] = u.as_ref();

        assert_eq!(ub.len(), 16);
        assert_eq!(ur.len(), 16);
        assert!(!ub.iter().all(|&b| b == 0));
        assert!(!ur.iter().all(|&b| b == 0));
    }

    #[test]
    #[cfg(feature = "std")]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_convert_vec() {
        use crate::std::{convert::TryInto, vec::Vec};

        let u = new();
        let ub: &[u8] = u.as_ref();

        let v: Vec<u8> = u.into();

        assert_eq!(&v, ub);

        let uv: Uuid = v.try_into().unwrap();

        assert_eq!(uv, u);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_bytes_roundtrip() {
        let b_in: crate::Bytes = [
            0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8,
        ];

        let u = Uuid::from_slice(&b_in).unwrap();

        let b_out = u.as_bytes();

        assert_eq!(&b_in, b_out);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_bytes_le_roundtrip() {
        let b = [
            0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8,
        ];

        let u1 = Uuid::from_bytes(b);

        let b_le = u1.to_bytes_le();

        let u2 = Uuid::from_bytes_le(b_le);

        assert_eq!(u1, u2);
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_iterbytes_impl_for_uuid() {
        let mut set = std::collections::HashSet::new();
        let id1 = new();
        let id2 = new2();
        set.insert(id1.clone());

        assert!(set.contains(&id1));
        assert!(!set.contains(&id2));
    }
}
