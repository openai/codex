//! Contains data structures and utilities for handling gRPC custom metadata.

mod encoding;
mod key;
mod map;
mod value;

pub use self::encoding::Ascii;
pub use self::encoding::Binary;
pub use self::key::AsciiMetadataKey;
pub use self::key::BinaryMetadataKey;
pub use self::key::MetadataKey;
pub use self::map::Entry;
pub use self::map::GetAll;
pub use self::map::Iter;
pub use self::map::IterMut;
pub use self::map::KeyAndMutValueRef;
pub use self::map::KeyAndValueRef;
pub use self::map::KeyRef;
pub use self::map::Keys;
pub use self::map::MetadataMap;
pub use self::map::OccupiedEntry;
pub use self::map::VacantEntry;
pub use self::map::ValueDrain;
pub use self::map::ValueIter;
pub use self::map::ValueRef;
pub use self::map::ValueRefMut;
pub use self::map::Values;
pub use self::map::ValuesMut;
pub use self::value::AsciiMetadataValue;
pub use self::value::BinaryMetadataValue;
pub use self::value::MetadataValue;
use http::HeaderValue;

pub(crate) use self::map::GRPC_TIMEOUT_HEADER;

/// HTTP Header `content-type` value for gRPC calls.
pub const GRPC_CONTENT_TYPE: HeaderValue = HeaderValue::from_static("application/grpc");

/// The metadata::errors module contains types for errors that can occur
/// while handling gRPC custom metadata.
pub mod errors {
    pub use super::encoding::InvalidMetadataValue;
    pub use super::encoding::InvalidMetadataValueBytes;
    pub use super::key::InvalidMetadataKey;
    pub use super::value::ToStrError;
}
