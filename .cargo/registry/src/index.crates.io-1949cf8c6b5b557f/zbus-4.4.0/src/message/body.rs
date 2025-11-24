use zvariant::{
    serialized::{self, Data},
    Signature, Type,
};

use crate::{Error, Message, Result};

/// The body of a message.
///
/// This contains the bytes and the signature of the body.
#[derive(Clone, Debug)]
pub struct Body {
    data: Data<'static, 'static>,
    msg: Message,
}

impl Body {
    pub(super) fn new(data: Data<'static, 'static>, msg: Message) -> Self {
        Self { data, msg }
    }

    /// Deserialize the body using the contained signature.
    pub fn deserialize<'s, B>(&'s self) -> Result<B>
    where
        B: zvariant::DynamicDeserialize<'s>,
    {
        let body_sig = self
            .signature()
            .unwrap_or_else(|| Signature::from_static_str_unchecked(""));

        self.data
            .deserialize_for_dynamic_signature(body_sig)
            .map_err(Error::from)
            .map(|b| b.0)
    }

    /// Deserialize the body (without checking signature matching).
    pub fn deserialize_unchecked<'d, 'm: 'd, B>(&'m self) -> Result<B>
    where
        B: serde::de::Deserialize<'d> + Type,
    {
        self.data.deserialize().map_err(Error::from).map(|b| b.0)
    }

    /// The signature of the body.
    ///
    /// **Note:** While zbus treats multiple arguments as a struct (to allow you to use the tuple
    /// syntax), D-Bus does not. Since this method gives you the signature expected on the wire by
    /// D-Bus, the trailing and leading STRUCT signature parenthesis will not be present in case of
    /// multiple arguments.
    pub fn signature(&self) -> Option<Signature<'_>> {
        self.msg.inner.quick_fields.signature(&self.msg)
    }

    /// The length of the body in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the body is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get a reference to the underlying byte encoding of the message.
    pub fn data(&self) -> &serialized::Data<'static, 'static> {
        &self.data
    }

    /// Reference to the message this body belongs to.
    pub fn message(&self) -> &Message {
        &self.msg
    }
}
