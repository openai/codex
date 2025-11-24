//! Support for associating type name information with a [`Message`].

use crate::Message;

#[cfg(not(feature = "std"))]
use alloc::{format, string::String};

/// Associate a type name with a [`Message`] type.
pub trait Name: Message {
    /// Simple name for this [`Message`].
    /// This name is the same as it appears in the source .proto file, e.g. `FooBar`.
    const NAME: &'static str;

    /// Package name this message type is contained in. They are domain-like
    /// and delimited by `.`, e.g. `google.protobuf`.
    const PACKAGE: &'static str;

    /// Fully-qualified unique name for this [`Message`].
    /// It's prefixed with the package name and names of any parent messages,
    /// e.g. `google.rpc.BadRequest.FieldViolation`.
    /// By default, this is the package name followed by the message name.
    /// Fully-qualified names must be unique within a domain of Type URLs.
    fn full_name() -> String {
        format!("{}.{}", Self::PACKAGE, Self::NAME)
    }

    /// Type URL for this [`Message`], which by default is the full name with a
    /// leading slash, but may also include a leading domain name, e.g.
    /// `type.googleapis.com/google.profile.Person`.
    /// This can be used when serializing into the `google.protobuf.Any` type.
    fn type_url() -> String {
        format!("/{}", Self::full_name())
    }
}
