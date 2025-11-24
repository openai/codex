//! This crate provides types for identifiers of object files, such as executables, dynamic
//! libraries or debug companion files. The concept originates in Google Breakpad and defines two
//! types:
//!
//!  - [`CodeId`]: Identifies the file containing source code, i.e. the actual library or
//!    executable. The identifier is platform dependent and implementation defined. Thus, there is
//!    no canonical representation.
//!  - [`DebugId`]: Identifies a debug information file, which may or may not use information from
//!    the Code ID. The contents are also implementation defined, but as opposed to `CodeId`, the
//!    structure is streamlined across platforms. It is also guaranteed to be 32 bytes in size.
//!
//! [`CodeId`]: struct.CodeId.html
//! [`DebugId`]: struct.DebugId.html

#![warn(missing_docs)]

use std::error;
use std::fmt;
use std::fmt::Write;
use std::str;

use uuid::{Bytes, Uuid};

/// Indicates an error parsing a [`DebugId`](struct.DebugId.html).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseDebugIdError;

impl error::Error for ParseDebugIdError {}

impl fmt::Display for ParseDebugIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid debug identifier")
    }
}

#[derive(Clone, Copy, Debug)]
struct ParseOptions {
    allow_hyphens: bool,
    require_appendix: bool,
    allow_tail: bool,
}

/// Unique identifier for debug information files and their debug information.
///
/// This type is analogous to [`CodeId`], except that it identifies a debug file instead of the
/// actual library or executable. One some platforms, a `DebugId` is an alias for a `CodeId` but the
/// exact rules around this are complex. On Windows, the identifiers are completely different and
/// refer to separate files.
///
/// The string representation must be between 33 and 40 characters long and consist of:
///
/// 1. 36 character hyphenated hex representation of the UUID field
/// 2. 1-16 character lowercase hex representation of the u32 appendix
///
/// The debug identifier is compatible to Google Breakpad. Use [`DebugId::breakpad`] to get a
/// breakpad string representation of this debug identifier.
///
/// There is one exception to this: for the old PDB 2.0 format the debug identifier consists
/// of only a 32-bit integer + age resulting in a string representation of between 9 and 16
/// hex characters.
///
/// # Example
///
/// ```
/// # extern crate debugid;
/// use std::str::FromStr;
/// use debugid::DebugId;
///
/// # fn foo() -> Result<(), ::debugid::ParseDebugIdError> {
/// let id = DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75-a")?;
/// assert_eq!("dfb8e43a-f242-3d73-a453-aeb6a777ef75-a".to_string(), id.to_string());
/// # Ok(())
/// # }
///
/// # fn main() { foo().unwrap() }
/// ```
///
/// # In-memory representation
///
/// The in-memory representation takes up 32 bytes and can be directly written to storage
/// and mapped back into an object reference.
///
/// ```
/// use std::str::FromStr;
/// use debugid::DebugId;
///
/// let debug_id = DebugId::from_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75-a").unwrap();
///
/// let slice = &[debug_id];
/// let ptr = slice.as_ptr() as *const u8;
/// let len = std::mem::size_of_val(slice);
/// let buf: &[u8] = unsafe { std::slice::from_raw_parts(ptr, len) };
///
/// let mut new_buf: Vec<u8> = Vec::new();
/// std::io::copy(&mut std::io::Cursor::new(buf), &mut new_buf).unwrap();
///
/// let ptr = new_buf.as_ptr() as *const DebugId;
/// let new_debug_id = unsafe { &*ptr };
///
/// assert_eq!(*new_debug_id, debug_id);
/// ```
///
/// As long the bytes were written using the same major version of this crate you will be
/// able to read it again like this.
///
/// [`CodeId`]: struct.CodeId.html
/// [`DebugId::breakpad`]: struct.DebugId.html#method.breakpad
// This needs to be backwards compatible also in its exact in-memory byte-layout since this
// struct is directly mapped from disk in e.g. Symbolic SymCache formats.  The first version
// of this struct was defined as:
//
// ```rust
// struct DebugId {
//     uuid: Uuid,
//     appendix: u32,
//     _padding: [u8; 12],
// }
// ```
//
// For this reason the current `typ` byte represents the type of `DebugId` stored in the
// `Bytes`:
//
// - `0u8`: The `bytes` field contains a UUID.
// - `1u8`: The first 4 bytes of the `bytes` field contain a big-endian u32, the remaining
//   bytes are 0.
#[repr(C, packed)]
#[derive(Default, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy)]
pub struct DebugId {
    bytes: Bytes,
    appendix: u32,
    _padding: [u8; 11],
    typ: u8,
}

impl DebugId {
    /// Constructs an empty debug identifier, containing only zeros.
    pub fn nil() -> Self {
        Self::default()
    }

    /// Constructs a `DebugId` from its `uuid`.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self::from_parts(uuid, 0)
    }

    /// Constructs a `DebugId` from its `uuid` and `appendix` parts.
    pub fn from_parts(uuid: Uuid, appendix: u32) -> Self {
        DebugId {
            bytes: *uuid.as_bytes(),
            appendix,
            typ: 0,
            _padding: [0; 11],
        }
    }

    /// Constructs a `DebugId` from a Microsoft little-endian GUID and age.
    pub fn from_guid_age(guid: &[u8], age: u32) -> Result<Self, ParseDebugIdError> {
        if guid.len() != 16 {
            return Err(ParseDebugIdError);
        }

        let uuid = Uuid::from_bytes([
            guid[3], guid[2], guid[1], guid[0], guid[5], guid[4], guid[7], guid[6], guid[8],
            guid[9], guid[10], guid[11], guid[12], guid[13], guid[14], guid[15],
        ]);

        Ok(DebugId::from_parts(uuid, age))
    }

    /// Constructs a `DebugId` from a PDB 2.0 timestamp and age.
    pub fn from_pdb20(timestamp: u32, age: u32) -> Self {
        // The big-endian byte-order here has to match the one used to read this number in
        // the DebugId::timestamp method.
        DebugId {
            bytes: [
                (timestamp >> 24) as u8,
                (timestamp >> 16) as u8,
                (timestamp >> 8) as u8,
                timestamp as u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
            ],
            appendix: age,
            _padding: [0u8; 11],
            typ: 1u8,
        }
    }

    /// Parses a breakpad identifier from a string.
    pub fn from_breakpad(string: &str) -> Result<Self, ParseDebugIdError> {
        let options = ParseOptions {
            allow_hyphens: false,
            require_appendix: true,
            allow_tail: false,
        };
        Self::parse_str(string, options).ok_or(ParseDebugIdError)
    }

    /// Returns the UUID part of the code module's debug_identifier.
    ///
    /// If this is a debug identifier for the PDB 2.0 format an invalid UUID is returned
    /// where only the first 4 bytes are filled in and the remainder of the bytes are 0.
    /// This means the UUID has variant [`uuid::Variant::NCS`] and an unknown version,
    /// [`Uuid::get_version`] will return `None`, which is not a valid UUID.
    ///
    /// This may seem odd however does seem reasonable:
    ///
    /// - Every [`DebugId`] can be represented as [`Uuid`] and will still mostly look
    ///   reasonable e.g. in comparisons etc.
    /// - The PDB 2.0 format is very old and very unlikely to appear practically.
    pub fn uuid(&self) -> Uuid {
        Uuid::from_bytes(self.bytes)
    }

    /// Returns the appendix part of the code module's debug identifier.
    ///
    /// On Windows, this is an incrementing counter to identify the build.
    /// On all other platforms, this value will always be zero.
    pub fn appendix(&self) -> u32 {
        self.appendix
    }

    /// Returns whether this identifier is nil, i.e. it consists only of zeros.
    pub fn is_nil(&self) -> bool {
        self.bytes == [0u8; 16] && self.appendix == 0
    }

    /// Returns whether this identifier is from the PDB 2.0 format.
    pub fn is_pdb20(&self) -> bool {
        self.typ == 1
    }

    /// Returns a wrapper which when formatted via `fmt::Display` will format a
    /// a breakpad identifier.
    pub fn breakpad(&self) -> BreakpadFormat<'_> {
        BreakpadFormat { inner: self }
    }

    fn parse_str(string: &str, options: ParseOptions) -> Option<Self> {
        let is_hyphenated = string.get(8..9) == Some("-");
        if is_hyphenated && !options.allow_hyphens || !string.is_ascii() {
            return None;
        }

        // Can the PDB 2.0 format match?  This can never be true for a valid UUID.
        let min_len = if is_hyphenated { 10 } else { 9 };
        let max_len = if is_hyphenated { 17 } else { 16 };
        if min_len <= string.len() && string.len() <= max_len {
            let timestamp_str = string.get(..8)?;
            let timestamp = u32::from_str_radix(timestamp_str, 16).ok()?;
            let appendix_str = match is_hyphenated {
                true => string.get(9..)?,
                false => string.get(8..)?,
            };
            let appendix = u32::from_str_radix(appendix_str, 16).ok()?;
            return Some(Self::from_pdb20(timestamp, appendix));
        }

        let uuid_len = if is_hyphenated { 36 } else { 32 };
        let uuid = string.get(..uuid_len)?.parse().ok()?;
        if !options.require_appendix && string.len() == uuid_len {
            return Some(Self::from_parts(uuid, 0));
        }

        let mut appendix_str = &string[uuid_len..];
        if is_hyphenated ^ appendix_str.starts_with('-') {
            return None; // Require a hyphen if and only if we're hyphenated.
        } else if is_hyphenated {
            appendix_str = &appendix_str[1..]; // Skip the hyphen for parsing.
        }

        if options.allow_tail && appendix_str.len() > 8 {
            appendix_str = &appendix_str[..8];
        }

        // Parse the appendix, which fails on empty strings.
        let appendix = u32::from_str_radix(appendix_str, 16).ok()?;
        Some(Self::from_parts(uuid, appendix))
    }

    /// Returns the PDB 2.0 timestamp.
    ///
    /// Only valid if you know this is a PDB 2.0 debug identifier.
    fn timestamp(&self) -> u32 {
        u32::from_be_bytes([self.bytes[0], self.bytes[1], self.bytes[2], self.bytes[3]])
    }
}

impl fmt::Debug for DebugId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let uuid = self.uuid();
        f.debug_struct("DebugId")
            .field("uuid", &uuid.hyphenated().to_string())
            .field("appendix", &self.appendix())
            .finish()
    }
}

impl fmt::Display for DebugId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.is_pdb20() {
            true => {
                let timestamp = self.timestamp();
                write!(f, "{:08X}", timestamp)?;
            }
            false => {
                let uuid = self.uuid();
                uuid.fmt(f)?;
            }
        }
        if self.appendix > 0 {
            write!(f, "-{:x}", { self.appendix })?;
        }
        Ok(())
    }
}

impl str::FromStr for DebugId {
    type Err = ParseDebugIdError;

    fn from_str(string: &str) -> Result<Self, ParseDebugIdError> {
        let options = ParseOptions {
            allow_hyphens: true,
            require_appendix: false,
            allow_tail: true,
        };
        Self::parse_str(string, options).ok_or(ParseDebugIdError)
    }
}

impl From<Uuid> for DebugId {
    fn from(uuid: Uuid) -> Self {
        DebugId::from_uuid(uuid)
    }
}

impl From<(Uuid, u32)> for DebugId {
    fn from(tuple: (Uuid, u32)) -> Self {
        let (uuid, appendix) = tuple;
        DebugId::from_parts(uuid, appendix)
    }
}

/// Wrapper around [`DebugId`] for Breakpad formatting.
///
/// **Example:**
///
/// ```
/// # extern crate debugid;
/// use std::str::FromStr;
/// use debugid::DebugId;
///
/// # fn foo() -> Result<(), debugid::ParseDebugIdError> {
/// let id = DebugId::from_breakpad("DFB8E43AF2423D73A453AEB6A777EF75a")?;
/// assert_eq!("DFB8E43AF2423D73A453AEB6A777EF75a".to_string(), id.breakpad().to_string());
/// # Ok(())
/// # }
///
/// # fn main() { foo().unwrap() }
/// ```
///
/// [`DebugId`]: struct.DebugId.html
#[derive(Debug)]
pub struct BreakpadFormat<'a> {
    inner: &'a DebugId,
}

impl<'a> fmt::Display for BreakpadFormat<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.is_pdb20() {
            true => {
                let timestamp = self.inner.timestamp();
                write!(f, "{:08X}{:x}", timestamp, self.inner.appendix())
            }
            false => {
                let uuid = self.inner.uuid();
                write!(f, "{:X}{:x}", uuid.simple(), self.inner.appendix())
            }
        }
    }
}

/// Indicates an error parsing a [`CodeId`](struct.CodeId.html).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseCodeIdError;

impl error::Error for ParseCodeIdError {}

impl fmt::Display for ParseCodeIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid code identifier")
    }
}

/// Unique platform-dependent identifier of code files.
///
/// This identifier assumes a string representation that depends on the platform and compiler used.
/// The representation only retains hex characters and canonically stores lower case.
///
/// There are the following known formats:
///
///  - **MachO UUID**: The unique identifier of a Mach binary, specified in the `LC_UUID` load
///    command header.
///  - **GNU Build ID**: Contents of the `.gnu.build-id` note or section contents formatted as
///    lowercase hex string.
///  - **PE Timestamp**: Timestamp and size of image values from a Windows PE header. The size of
///    image value is truncated, so the length of the `CodeId` might not be a multiple of 2.
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CodeId {
    inner: String,
}

impl CodeId {
    /// Constructs an empty code identifier.
    pub fn nil() -> Self {
        Self::default()
    }

    /// Constructs a `CodeId` from its string representation.
    pub fn new(mut string: String) -> Self {
        string.retain(|c| c.is_ascii_hexdigit());
        string.make_ascii_lowercase();
        CodeId { inner: string }
    }

    /// Constructs a `CodeId` from a binary slice.
    pub fn from_binary(slice: &[u8]) -> Self {
        let mut string = String::with_capacity(slice.len() * 2);

        for byte in slice {
            write!(&mut string, "{:02x}", byte).expect("");
        }

        Self::new(string)
    }

    /// Returns whether this identifier is nil, i.e. it is empty.
    pub fn is_nil(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the string representation of this code identifier.
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }
}

impl fmt::Display for CodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl fmt::Debug for CodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CodeId({})", self)
    }
}

impl From<String> for CodeId {
    fn from(string: String) -> Self {
        Self::new(string)
    }
}

impl From<&'_ str> for CodeId {
    fn from(string: &str) -> Self {
        Self::new(string.into())
    }
}

impl AsRef<str> for CodeId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl str::FromStr for CodeId {
    type Err = ParseCodeIdError;

    fn from_str(string: &str) -> Result<Self, ParseCodeIdError> {
        Ok(Self::new(string.into()))
    }
}

#[cfg(feature = "serde")]
mod serde_support {
    use serde::de::{self, Deserialize, Deserializer, Unexpected, Visitor};
    use serde::ser::{Serialize, Serializer};

    use super::*;

    impl Serialize for CodeId {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(self.as_str())
        }
    }

    impl<'de> Deserialize<'de> for CodeId {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let string = String::deserialize(deserializer)?;
            Ok(CodeId::new(string))
        }
    }

    impl<'de> Deserialize<'de> for DebugId {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct V;

            impl<'de> Visitor<'de> for V {
                type Value = DebugId;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("DebugId")
                }

                fn visit_str<E: de::Error>(self, value: &str) -> Result<DebugId, E> {
                    value
                        .parse()
                        .map_err(|_| de::Error::invalid_value(Unexpected::Str(value), &self))
                }
            }

            deserializer.deserialize_str(V)
        }
    }

    impl Serialize for DebugId {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&self.to_string())
        }
    }
}
