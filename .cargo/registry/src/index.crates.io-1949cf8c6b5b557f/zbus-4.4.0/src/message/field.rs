use std::num::NonZeroU32;

use serde::{
    de::{Deserialize, Deserializer, Error},
    ser::{Serialize, Serializer},
};
use serde_repr::{Deserialize_repr, Serialize_repr};

use static_assertions::assert_impl_all;
use zbus_names::{BusName, ErrorName, InterfaceName, MemberName, UniqueName};
use zvariant::{ObjectPath, Signature, Type, Value};

/// The message field code.
///
/// Every [`Field`] has an associated code. This is mostly an internal D-Bus protocol detail
/// that you would not need to ever care about when using the high-level API. When using the
/// low-level API, this is how you can [retrieve a specific field] from [`Fields`].
///
/// [`Field`]: enum.Field.html
/// [retrieve a specific field]: struct.Fields.html#method.get_field
/// [`Fields`]: struct.Fields.html
#[repr(u8)]
#[derive(Copy, Clone, Debug, Deserialize_repr, PartialEq, Eq, Serialize_repr, Type)]
pub(crate) enum FieldCode {
    /// Code for [`Field::Path`](enum.Field.html#variant.Path)
    Path = 1,
    /// Code for [`Field::Interface`](enum.Field.html#variant.Interface)
    Interface = 2,
    /// Code for [`Field::Member`](enum.Field.html#variant.Member)
    Member = 3,
    /// Code for [`Field::ErrorName`](enum.Field.html#variant.ErrorName)
    ErrorName = 4,
    /// Code for [`Field::ReplySerial`](enum.Field.html#variant.ReplySerial)
    ReplySerial = 5,
    /// Code for [`Field::Destination`](enum.Field.html#variant.Destination)
    Destination = 6,
    /// Code for [`Field::Sender`](enum.Field.html#variant.Sender)
    Sender = 7,
    /// Code for [`Field::Signature`](enum.Field.html#variant.Signature)
    Signature = 8,
    /// Code for [`Field::UnixFDs`](enum.Field.html#variant.UnixFDs)
    UnixFDs = 9,
}

assert_impl_all!(FieldCode: Send, Sync, Unpin);

impl<'f> Field<'f> {
    /// Get the associated code for this field.
    pub fn code(&self) -> FieldCode {
        match self {
            Field::Path(_) => FieldCode::Path,
            Field::Interface(_) => FieldCode::Interface,
            Field::Member(_) => FieldCode::Member,
            Field::ErrorName(_) => FieldCode::ErrorName,
            Field::ReplySerial(_) => FieldCode::ReplySerial,
            Field::Destination(_) => FieldCode::Destination,
            Field::Sender(_) => FieldCode::Sender,
            Field::Signature(_) => FieldCode::Signature,
            Field::UnixFDs(_) => FieldCode::UnixFDs,
        }
    }
}

/// The dynamic message header.
///
/// All D-Bus messages contain a set of metadata [headers]. Some of these headers [are fixed] for
/// all types of messages, while others depend on the type of the message in question. The latter
/// are called message fields.
///
/// Please consult the [Message Format] section of the D-Bus spec for more details.
///
/// [headers]: struct.Header.html
/// [are fixed]: struct.PrimaryHeader.html
/// [Message Format]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-messages
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Field<'f> {
    /// The object to send a call to, or the object a signal is emitted from.
    Path(ObjectPath<'f>),
    /// The interface to invoke a method call on, or that a signal is emitted from.
    Interface(InterfaceName<'f>),
    /// The member, either the method name or signal name.
    Member(MemberName<'f>),
    /// The name of the error that occurred, for errors
    ErrorName(ErrorName<'f>),
    /// The serial number of the message this message is a reply to.
    ReplySerial(NonZeroU32),
    /// The name of the connection this message is intended for.
    Destination(BusName<'f>),
    /// Unique name of the sending connection.
    Sender(UniqueName<'f>),
    /// The signature of the message body.
    Signature(Signature<'f>),
    /// The number of Unix file descriptors that accompany the message.
    UnixFDs(u32),
}

assert_impl_all!(Field<'_>: Send, Sync, Unpin);

impl<'f> Type for Field<'f> {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked("(yv)")
    }
}

impl<'f> Serialize for Field<'f> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tuple: (FieldCode, Value<'_>) = match self {
            Field::Path(value) => (FieldCode::Path, value.as_ref().into()),
            Field::Interface(value) => (FieldCode::Interface, value.as_str().into()),
            Field::Member(value) => (FieldCode::Member, value.as_str().into()),
            Field::ErrorName(value) => (FieldCode::ErrorName, value.as_str().into()),
            Field::ReplySerial(value) => (FieldCode::ReplySerial, value.get().into()),
            Field::Destination(value) => (FieldCode::Destination, value.as_str().into()),
            Field::Sender(value) => (FieldCode::Sender, value.as_str().into()),
            Field::Signature(value) => (FieldCode::Signature, value.as_ref().into()),
            Field::UnixFDs(value) => (FieldCode::UnixFDs, (*value).into()),
        };

        tuple.serialize(serializer)
    }
}

impl<'de: 'f, 'f> Deserialize<'de> for Field<'f> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (code, value) = <(FieldCode, Value<'_>)>::deserialize(deserializer)?;
        Ok(match code {
            FieldCode::Path => Field::Path(ObjectPath::try_from(value).map_err(D::Error::custom)?),
            FieldCode::Interface => {
                Field::Interface(InterfaceName::try_from(value).map_err(D::Error::custom)?)
            }
            FieldCode::Member => {
                Field::Member(MemberName::try_from(value).map_err(D::Error::custom)?)
            }
            FieldCode::ErrorName => Field::ErrorName(
                ErrorName::try_from(value)
                    .map(Into::into)
                    .map_err(D::Error::custom)?,
            ),
            FieldCode::ReplySerial => {
                let value = u32::try_from(value)
                    .map_err(D::Error::custom)
                    .and_then(|v| v.try_into().map_err(D::Error::custom))?;
                Field::ReplySerial(value)
            }
            FieldCode::Destination => Field::Destination(
                BusName::try_from(value)
                    .map(Into::into)
                    .map_err(D::Error::custom)?,
            ),
            FieldCode::Sender => Field::Sender(
                UniqueName::try_from(value)
                    .map(Into::into)
                    .map_err(D::Error::custom)?,
            ),
            FieldCode::Signature => {
                Field::Signature(Signature::try_from(value).map_err(D::Error::custom)?)
            }
            FieldCode::UnixFDs => Field::UnixFDs(u32::try_from(value).map_err(D::Error::custom)?),
        })
    }
}
