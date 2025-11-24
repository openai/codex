//! Utility functions for X11 things.
//!
//! The most important definitions in this module are the [`TryParse`], [`TryParseFd`] and
//! [`Serialize`] traits. These traits are used internally for parsing incoming data and producing
//! outgoing data when talking with the X11 server.

use alloc::string::String;
use alloc::vec::Vec;

use crate::errors::ParseError;
use crate::protocol::{request_name, ErrorKind};
use crate::utils::RawFdContainer;
use crate::BufWithFds;

/// Representation of an X11 error packet that was sent by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X11Error {
    /// The kind of error that occurred.
    pub error_kind: ErrorKind,
    /// The kind of error that occurred as it appears "on the wire".
    pub error_code: u8,
    /// The sequence number of the request that caused this error.
    pub sequence: u16,
    /// The value in the request that caused the error.
    pub bad_value: u32,
    /// The minor opcode of the request that caused this error.
    pub minor_opcode: u16,
    /// The major opcode of the request that caused this error.
    pub major_opcode: u8,
    /// Name of the extension that caused this error, if known.
    pub extension_name: Option<String>,
    /// Name of the request that caused this error, if known.
    pub request_name: Option<&'static str>,
}

impl X11Error {
    /// Parse an X11 error.
    pub fn try_parse(
        data: &[u8],
        ext_info_provider: &dyn ExtInfoProvider,
    ) -> Result<Self, ParseError> {
        let (response_type, remaining) = u8::try_parse(data)?;
        let (error_code, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (bad_value, remaining) = u32::try_parse(remaining)?;
        let (minor_opcode, remaining) = u16::try_parse(remaining)?;
        let (major_opcode, _) = u8::try_parse(remaining)?;
        if response_type != 0 {
            Err(ParseError::InvalidValue)
        } else {
            let error_kind = ErrorKind::from_wire_error_code(error_code, ext_info_provider);
            let (extension_name, request_name) =
                request_name(ext_info_provider, major_opcode, minor_opcode);
            Ok(X11Error {
                error_kind,
                error_code,
                sequence,
                bad_value,
                minor_opcode,
                major_opcode,
                extension_name,
                request_name,
            })
        }
    }
}

#[cfg(test)]
mod tryparse_x11error_test {
    use super::{ErrorKind, ExtInfoProvider, ParseError, X11Error};
    use crate::x11_utils::ExtensionInformation;

    struct Provider;

    impl ExtInfoProvider for Provider {
        fn get_from_major_opcode(&self, major_opcode: u8) -> Option<(&str, ExtensionInformation)> {
            assert_eq!(major_opcode, 10);
            None
        }
        fn get_from_event_code(&self, _event_code: u8) -> Option<(&str, ExtensionInformation)> {
            unimplemented!()
        }
        fn get_from_error_code(&self, _error_code: u8) -> Option<(&str, ExtensionInformation)> {
            unimplemented!()
        }
    }

    #[test]
    fn try_parse_error() {
        let input = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        let error = X11Error::try_parse(&input, &Provider);
        let expected = X11Error {
            error_kind: ErrorKind::Request,
            error_code: 1,
            sequence: u16::from_ne_bytes([2, 3]),
            bad_value: u32::from_ne_bytes([4, 5, 6, 7]),
            minor_opcode: u16::from_ne_bytes([8, 9]),
            major_opcode: 10,
            extension_name: None,
            request_name: Some("UnmapWindow"),
        };
        assert_eq!(error, Ok(expected));
    }

    #[test]
    fn reject_invalid_response_type() {
        let result = X11Error::try_parse(&[1; 32], &Provider);
        assert_eq!(Err(ParseError::InvalidValue), result);
    }
}

impl From<&X11Error> for [u8; 32] {
    fn from(input: &X11Error) -> Self {
        let sequence_bytes = input.sequence.serialize();
        let bad_value_bytes = input.bad_value.serialize();
        let minor_opcode_bytes = input.minor_opcode.serialize();
        [
            0,
            input.error_code,
            sequence_bytes[0],
            sequence_bytes[1],
            bad_value_bytes[0],
            bad_value_bytes[1],
            bad_value_bytes[2],
            bad_value_bytes[3],
            minor_opcode_bytes[0],
            minor_opcode_bytes[1],
            input.major_opcode,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<X11Error> for [u8; 32] {
    fn from(input: X11Error) -> Self {
        Self::from(&input)
    }
}

#[cfg(test)]
mod serialise_x11error_test {
    use super::{ErrorKind, X11Error};

    #[test]
    fn test_serialise() {
        let error = X11Error {
            error_kind: ErrorKind::Request,
            error_code: 1,
            sequence: u16::from_ne_bytes([2, 3]),
            bad_value: u32::from_ne_bytes([4, 5, 6, 7]),
            minor_opcode: u16::from_ne_bytes([8, 9]),
            major_opcode: 10,
            extension_name: None,
            request_name: None,
        };
        let expected = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        assert_eq!(expected, <[u8; 32]>::from(error));
    }
}

/// Information about a X11 extension.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExtensionInformation {
    /// Major opcode used in request
    pub major_opcode: u8,
    /// Lowest event number used by the extension.
    pub first_event: u8,
    /// Lowest error number used by the extension.
    pub first_error: u8,
}

/// Trait to provide information about extensions.
pub trait ExtInfoProvider {
    /// Returns the information of the extension that whose
    /// opcode is `major_opcode`.
    fn get_from_major_opcode(&self, major_opcode: u8) -> Option<(&str, ExtensionInformation)>;

    /// Returns the information of the extension that whose
    /// event number range includes `event_number`.
    fn get_from_event_code(&self, event_code: u8) -> Option<(&str, ExtensionInformation)>;

    /// Returns the information of the extension that whose
    /// error number range includes `error_number`.
    fn get_from_error_code(&self, error_code: u8) -> Option<(&str, ExtensionInformation)>;
}

/// A type implementing this trait can be parsed from some raw bytes.
pub trait TryParse: Sized {
    /// Try to parse the given values into an instance of this type.
    ///
    /// If parsing is successful, an instance of the type and a slice for the remaining data should
    /// be returned. Otherwise, an error is returned.
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError>;
}

/// A type implementing this trait can be parsed from some raw bytes and a list of fds.
pub trait TryParseFd: Sized {
    /// Try to parse the given values into an instance of this type.
    ///
    /// File descriptors are consumed by removing them from the beginning of the given `fds` `Vec`.
    /// If a file descriptor is expected, but missing, a `ParseError` should be returned. If more file
    /// descriptors are provided than expected, this is not an error and the remaining descriptors
    /// should be left in the `Vec`.
    ///
    /// If parsing is successful, an instance of the type and a slice for the remaining data should
    /// be returned. Otherwise, an error is returned.
    fn try_parse_fd<'a>(
        value: &'a [u8],
        fds: &mut Vec<RawFdContainer>,
    ) -> Result<(Self, &'a [u8]), ParseError>;
}

impl<T: TryParse> TryParseFd for T {
    fn try_parse_fd<'a>(
        value: &'a [u8],
        _: &mut Vec<RawFdContainer>,
    ) -> Result<(Self, &'a [u8]), ParseError> {
        T::try_parse(value)
    }
}

/// A representation of the header of a request.
#[derive(Debug, Clone, Copy)]
pub struct RequestHeader {
    /// The major opcode of the request.
    pub major_opcode: u8,
    /// The minor opcode of the request (which, for some requests, may not be an
    /// opcode at all).
    pub minor_opcode: u8,
    /// The remaining length of the request, measured in 4 bytes units. Unlike the wire format,
    /// this does *not* include the header itself, which is 1 unit (or 2 if BigRequests is
    /// enabled and the length in the first unit is zero). If the BigRequests extension is
    /// enabled this can be greater than u16::max_value - 1.
    pub remaining_length: u32,
}

/// A type implementing this trait is an X11 request.
pub trait Request {
    /// The protocol name of the extension that this request belongs to, or None for core requests
    const EXTENSION_NAME: Option<&'static str>;

    /// Serialize this request into its X11 protocol wire representation.
    ///
    /// The argument is the major opcode of the extension that this request belongs to. For core
    /// requests, the argument may not have any influence
    fn serialize(self, extension_opcode: u8) -> BufWithFds<Vec<u8>>;
}

/// A type alias for reply parsers (matches the signature of TryParseFd).
pub type ReplyParsingFunction =
    for<'a> fn(
        &'a [u8],
        &mut Vec<RawFdContainer>,
    ) -> Result<(crate::protocol::Reply, &'a [u8]), ParseError>;

/// A X11 request that does not have a reply
pub trait VoidRequest: Request {}

/// A X11 request that has a reply without FDs
pub trait ReplyRequest: Request {
    /// The kind of reply that this request generates.
    type Reply: Into<crate::protocol::Reply> + TryParse;
}

/// A X11 request that has a reply with FDs
pub trait ReplyFDsRequest: Request {
    /// The kind of reply that this request generates.
    type Reply: Into<crate::protocol::Reply> + TryParseFd;
}

/// A type implementing this trait can be serialized into X11 raw bytes.
pub trait Serialize {
    /// The value returned by `serialize`.
    ///
    /// This should be `Vec<u8>` in most cases. However, arrays like `[u8; 4]` should also be
    /// allowed and thus this is an associated type.
    ///
    /// If generic associated types were available, implementing `AsRef<[u8]>` would be required.
    type Bytes;

    /// Serialize this value into X11 raw bytes.
    fn serialize(&self) -> Self::Bytes;

    /// Serialize this value into X11 raw bytes, appending the result into `bytes`.
    ///
    /// When calling this method, the given vector must satisfy `assert_eq!(bytes.len() % 4, 0);`.
    /// In words: Its length must be a multiple of four.
    fn serialize_into(&self, bytes: &mut Vec<u8>);
}

// Now implement TryParse and Serialize for some primitive data types that we need.

macro_rules! implement_try_parse {
    ($t:ty) => {
        impl TryParse for $t {
            fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
                let len = core::mem::size_of::<$t>();
                let bytes = value
                    .get(..len)
                    .ok_or(ParseError::InsufficientData)?
                    .try_into() // TryInto<[u8; len]>
                    .unwrap();
                Ok((<$t>::from_ne_bytes(bytes), &value[len..]))
            }
        }
    };
}

macro_rules! implement_serialize {
    ($t:ty: $size:expr) => {
        impl Serialize for $t {
            type Bytes = [u8; $size];
            fn serialize(&self) -> Self::Bytes {
                self.to_ne_bytes()
            }
            fn serialize_into(&self, bytes: &mut Vec<u8>) {
                bytes.extend_from_slice(&self.to_ne_bytes());
            }
        }
    };
}

macro_rules! forward_float {
    ($from:ty: $to:ty) => {
        impl TryParse for $from {
            fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
                let (data, remaining) = <$to>::try_parse(value)?;
                Ok((<$from>::from_bits(data), remaining))
            }
        }
        impl Serialize for $from {
            type Bytes = <$to as Serialize>::Bytes;
            fn serialize(&self) -> Self::Bytes {
                self.to_bits().serialize()
            }
            fn serialize_into(&self, bytes: &mut Vec<u8>) {
                self.to_bits().serialize_into(bytes);
            }
        }
    };
}

implement_try_parse!(u8);
implement_try_parse!(i8);
implement_try_parse!(u16);
implement_try_parse!(i16);
implement_try_parse!(u32);
implement_try_parse!(i32);
implement_try_parse!(u64);
implement_try_parse!(i64);

implement_serialize!(u8: 1);
implement_serialize!(i8: 1);
implement_serialize!(u16: 2);
implement_serialize!(i16: 2);
implement_serialize!(u32: 4);
implement_serialize!(i32: 4);
implement_serialize!(u64: 8);
implement_serialize!(i64: 8);

forward_float!(f32: u32);
forward_float!(f64: u64);

#[cfg(test)]
mod float_tests {
    use super::{Serialize, TryParse};

    fn test_round_trip<F>(value: F)
    where
        F: TryParse + Serialize + PartialEq + core::fmt::Debug + Copy,
        <F as Serialize>::Bytes: AsRef<[u8]>,
    {
        let empty = &[][..];

        // Test using serialize()
        assert_eq!(Ok((value, empty)), F::try_parse(value.serialize().as_ref()));

        // Test using serialize_into()
        let mut output = alloc::vec::Vec::new();
        value.serialize_into(&mut output);
        assert_eq!(Ok((value, empty)), F::try_parse(&output));
    }

    #[test]
    fn test_f32_round_trips() {
        for &f in &[0f32, 1., std::f32::consts::PI, 42., 1337., 1e7] {
            test_round_trip(f);
            test_round_trip(-f);
        }
    }

    #[test]
    fn test_f64_round_trips() {
        for &f in &[0f64, 1., std::f64::consts::PI, 42., 1337., 1e7] {
            test_round_trip(f);
            test_round_trip(-f);
        }
    }

    #[test]
    fn test_parse_known_value() {
        let bytes = 0x42280000u32.to_ne_bytes();
        let value = f32::try_parse(&bytes);
        let empty = &[][..];
        assert_eq!(Ok((42., empty)), value);
    }

    #[test]
    fn test_serialize_known_value() {
        assert_eq!(0x42280000u32.to_ne_bytes(), 42f32.serialize());
    }
}

impl TryParse for bool {
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (data, remaining) = u8::try_parse(value)?;
        Ok((data != 0, remaining))
    }
}

impl Serialize for bool {
    type Bytes = [u8; 1];
    fn serialize(&self) -> Self::Bytes {
        [u8::from(*self)]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.push(u8::from(*self));
    }
}

// Tuple handling

macro_rules! tuple_try_parse {
    ($($name:ident)*) => {
        impl<$($name,)*> TryParse for ($($name,)*)
        where $($name: TryParse,)*
        {
            #[allow(non_snake_case)]
            fn try_parse(remaining: &[u8]) -> Result<(($($name,)*), &[u8]), ParseError> {
                $(let ($name, remaining) = $name::try_parse(remaining)?;)*
                Ok((($($name,)*), remaining))
            }
        }
    }
}

macro_rules! tuple_serialize {
    ($($name:ident:$idx:tt)*) => {
        impl<$($name,)*> Serialize for ($($name,)*)
        where $($name: Serialize,)*
        {
            type Bytes = Vec<u8>;
            fn serialize(&self) -> Self::Bytes {
                let mut result = Vec::new();
                self.serialize_into(&mut result);
                result
            }
            fn serialize_into(&self, bytes: &mut Vec<u8>) {
                $(self.$idx.serialize_into(bytes);)*
            }
        }
    }
}

macro_rules! tuple_impls {
    ($($name:ident:$idx:tt)*) => {
        tuple_try_parse!($($name)*);
        tuple_serialize!($($name:$idx)*);
    }
}

// We can optimise serialisation of empty tuples or one-element-tuples with different Bytes type
impl Serialize for () {
    type Bytes = [u8; 0];
    fn serialize(&self) -> Self::Bytes {
        []
    }
    fn serialize_into(&self, _bytes: &mut Vec<u8>) {}
}

impl<T: Serialize> Serialize for (T,) {
    type Bytes = T::Bytes;
    fn serialize(&self) -> Self::Bytes {
        self.0.serialize()
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        self.0.serialize_into(bytes)
    }
}

tuple_try_parse!();
tuple_try_parse!(A);
tuple_impls!(A:0 B:1);
tuple_impls!(A:0 B:1 C:2);
tuple_impls!(A:0 B:1 C:2 D:3);
tuple_impls!(A:0 B:1 C:2 D:3 E:4);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8 J:9);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8 J:9 K:10);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8 J:9 K:10 L:11);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8 J:9 K:10 L:11 M:12);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8 J:9 K:10 L:11 M:12 N:13);
tuple_impls!(A:0 B:1 C:2 D:3 E:4 F:5 G:6 H:7 I:8 J:9 K:10 L:11 M:12 N:13 O:14);

/// Parse a list of objects from the given data.
///
/// This function parses a list of objects where the length of the list was specified externally.
/// The wire format for `list_length` instances of `T` will be read from the given data.
pub(crate) fn parse_list<T>(data: &[u8], list_length: usize) -> Result<(Vec<T>, &[u8]), ParseError>
where
    T: TryParse,
{
    let mut remaining = data;
    let mut result = Vec::with_capacity(list_length);
    for _ in 0..list_length {
        let (entry, new_remaining) = T::try_parse(remaining)?;
        result.push(entry);
        remaining = new_remaining;
    }
    Ok((result, remaining))
}

/// Parse a list of `u8` from the given data.
#[inline]
pub(crate) fn parse_u8_list(data: &[u8], list_length: usize) -> Result<(&[u8], &[u8]), ParseError> {
    if data.len() < list_length {
        Err(ParseError::InsufficientData)
    } else {
        Ok(data.split_at(list_length))
    }
}

/// Parse an array of `u8` from the given data.
#[inline]
pub(crate) fn parse_u8_array_ref<const N: usize>(
    data: &[u8],
) -> Result<(&[u8; N], &[u8]), ParseError> {
    let (slice, remaining) = parse_u8_list(data, N)?;
    let slice = slice
        .try_into()
        .expect("Cannot fail since slice has expected length");
    Ok((slice, remaining))
}

/// Parse an array of `u8` from the given data.
#[inline]
pub(crate) fn parse_u8_array<const N: usize>(data: &[u8]) -> Result<([u8; N], &[u8]), ParseError> {
    let (array, remaining) = parse_u8_array_ref(data)?;
    Ok((*array, remaining))
}

impl<T: Serialize> Serialize for [T] {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Self::Bytes {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        for item in self {
            item.serialize_into(bytes);
        }
    }
}

// This macro is used by the generated code to implement e.g. `std::ops::BitOr` and
// `std::ops::BitOrAssign`.
macro_rules! bitmask_binop {
    ($t:ty, $u:ty) => {
        impl core::ops::BitOr for $t {
            type Output = $t;
            fn bitor(self, other: Self) -> Self::Output {
                Self::from(<$u>::from(self) | <$u>::from(other))
            }
        }
        impl core::ops::BitOr<$u> for $t {
            type Output = $t;
            fn bitor(self, other: $u) -> Self::Output {
                self | Self::from(other)
            }
        }
        impl core::ops::BitOr<$t> for $u {
            type Output = $t;
            fn bitor(self, other: $t) -> Self::Output {
                <$t>::from(self) | other
            }
        }
        impl core::ops::BitOrAssign for $t {
            fn bitor_assign(&mut self, other: $t) {
                *self = *self | Self::from(other)
            }
        }
        impl core::ops::BitOrAssign<$t> for $u {
            fn bitor_assign(&mut self, other: $t) {
                *self |= Self::from(other)
            }
        }
        impl core::ops::BitOrAssign<$u> for $t {
            fn bitor_assign(&mut self, other: $u) {
                self.0 |= other
            }
        }
        impl core::ops::BitAnd for $t {
            type Output = $t;
            fn bitand(self, other: Self) -> Self::Output {
                Self::from(<$u>::from(self) & <$u>::from(other))
            }
        }
        impl core::ops::BitAnd<$u> for $t {
            type Output = $t;
            fn bitand(self, other: $u) -> Self::Output {
                self & Self::from(other)
            }
        }
        impl core::ops::BitAnd<$t> for $u {
            type Output = $t;
            fn bitand(self, other: $t) -> Self::Output {
                <$t>::from(self) & other
            }
        }
        impl core::ops::BitAndAssign for $t {
            fn bitand_assign(&mut self, other: $t) {
                self.0 &= other
            }
        }
        impl core::ops::BitAndAssign<$t> for $u {
            fn bitand_assign(&mut self, other: $t) {
                *self &= Self::from(other)
            }
        }
        impl core::ops::BitAndAssign<$u> for $t {
            fn bitand_assign(&mut self, other: $u) {
                self.0 &= other
            }
        }
        impl $t {
            /// Check if this object has all bits set that are also set in `flag`.
            ///
            /// `flag` can be a single enum variant or a whole other mask.
            pub fn contains(self, flag: impl Into<$u>) -> bool {
                let flag = flag.into();
                (<$u>::from(self) & flag) == flag
            }

            /// Check if this object has some bits set that are also set in `flag`.
            ///
            /// `flag` can be a single enum variant or a whole other mask.
            pub fn intersects(self, flag: impl Into<$u>) -> bool {
                let flag = flag.into();
                (<$u>::from(self) & flag) != 0
            }

            /// Remove some flags.
            ///
            /// All bits that are set in the given flags are removed from the `self` instance, if
            /// they are present.
            pub fn remove(self, flags: impl Into<$u>) -> Self {
                Self::from(self.bits() & !flags.into())
            }

            /// Returns the internal value of the object.
            pub fn bits(self) -> $u {
                self.0
            }
        }
    };
}

macro_rules! impl_debug_if_no_extra_traits {
    ($type:ty, $name:literal) => {
        #[cfg(not(feature = "extra-traits"))]
        impl core::fmt::Debug for $type {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct($name).finish_non_exhaustive()
            }
        }
    };
}

/// Wrapper around TryInto that produces a ParseError.
///
/// This trait shortens `x.try_into().or(Err(ParseError::ConversionFailed))` to `x.try_to_usize()`.
pub(crate) trait TryIntoUSize: TryInto<usize> {
    /// Attempt the conversion
    fn try_to_usize(self) -> Result<usize, ParseError> {
        self.try_into().or(Err(ParseError::ConversionFailed))
    }
}

impl TryIntoUSize for u8 {}
impl TryIntoUSize for u16 {}
impl TryIntoUSize for u32 {}
impl TryIntoUSize for u64 {}
impl TryIntoUSize for i8 {}
impl TryIntoUSize for i16 {}
impl TryIntoUSize for i32 {}
impl TryIntoUSize for i64 {}

/// Has the BigRequests extension been enabled?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BigRequests {
    /// The BigRequests extension has been enabled.
    Enabled,
    /// The BigRequests extension has not been enabled.
    NotEnabled,
}

/// Parse the given input for a RequestHeader and the remaining input.
pub fn parse_request_header(
    input: &[u8],
    big_requests_enabled: BigRequests,
) -> Result<(RequestHeader, &[u8]), ParseError> {
    let (major_opcode, remaining) = u8::try_parse(input)?;
    let (minor_opcode, remaining) = u8::try_parse(remaining)?;
    let (length, remaining) = u16::try_parse(remaining)?;
    let (remaining_length, finally_remaining) = if length == 0 {
        if big_requests_enabled == BigRequests::NotEnabled {
            return Err(ParseError::InvalidValue);
        }

        let (length, remaining) = u32::try_parse(remaining)?;
        if length < 2 {
            return Err(ParseError::InvalidValue);
        }
        // Adjust length for the size of this header (two 4 byte units).
        (length - 2, remaining)
    } else {
        // Adjust length for the size of this header (one 4 byte unit).
        (u32::from(length) - 1, remaining)
    };
    Ok((
        RequestHeader {
            major_opcode,
            minor_opcode,
            remaining_length,
        },
        finally_remaining,
    ))
}
