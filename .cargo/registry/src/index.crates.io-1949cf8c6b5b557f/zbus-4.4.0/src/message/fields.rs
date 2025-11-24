use serde::{Deserialize, Serialize};
use static_assertions::assert_impl_all;
use std::num::NonZeroU32;
use zbus_names::{BusName, ErrorName, InterfaceName, MemberName, UniqueName};
use zvariant::{ObjectPath, Signature, Type};

use crate::{
    message::{Field, FieldCode, Header, Message},
    Result,
};

// It's actually 10 (and even not that) but let's round it to next 8-byte alignment
const MAX_FIELDS_IN_MESSAGE: usize = 16;

/// A collection of [`Field`] instances.
///
/// [`Field`]: enum.Field.html
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub(crate) struct Fields<'m>(#[serde(borrow)] Vec<Field<'m>>);

assert_impl_all!(Fields<'_>: Send, Sync, Unpin);

impl<'m> Fields<'m> {
    /// Creates an empty collection of fields.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a [`Field`] to the collection of fields in the message.
    ///
    /// [`Field`]: enum.Field.html
    pub fn add<'f: 'm>(&mut self, field: Field<'f>) {
        self.0.push(field);
    }

    /// Replaces a [`Field`] from the collection of fields with one with the same code,
    /// returning the old value if present.
    ///
    /// [`Field`]: enum.Field.html
    pub fn replace<'f: 'm>(&mut self, field: Field<'f>) -> Option<Field<'m>> {
        let code = field.code();
        if let Some(found) = self.0.iter_mut().find(|f| f.code() == code) {
            return Some(std::mem::replace(found, field));
        }
        self.add(field);
        None
    }

    /// Returns a slice with all the [`Field`] in the message.
    ///
    /// [`Field`]: enum.Field.html
    pub fn get(&self) -> &[Field<'m>] {
        &self.0
    }

    /// Gets a reference to a specific [`Field`] by its code.
    ///
    /// Returns `None` if the message has no such field.
    ///
    /// [`Field`]: enum.Field.html
    pub fn get_field(&self, code: FieldCode) -> Option<&Field<'m>> {
        self.0.iter().find(|f| f.code() == code)
    }

    /// Remove the field matching the `code`.
    ///
    /// Returns `true` if a field was found and removed, `false` otherwise.
    pub(crate) fn remove(&mut self, code: FieldCode) -> bool {
        match self.0.iter().enumerate().find(|(_, f)| f.code() == code) {
            Some((i, _)) => {
                self.0.remove(i);

                true
            }
            None => false,
        }
    }
}

/// A byte range of a field in a Message, used in [`QuickFields`].
///
/// Some invalid encodings (end = 0) are used to indicate "not cached" and "not present".
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct FieldPos {
    start: u32,
    end: u32,
}

impl FieldPos {
    pub fn new_not_present() -> Self {
        Self { start: 1, end: 0 }
    }

    pub fn build(msg_buf: &[u8], field_buf: &str) -> Option<Self> {
        let buf_start = msg_buf.as_ptr() as usize;
        let field_start = field_buf.as_ptr() as usize;
        let offset = field_start.checked_sub(buf_start)?;
        if offset <= msg_buf.len() && offset + field_buf.len() <= msg_buf.len() {
            Some(Self {
                start: offset.try_into().ok()?,
                end: (offset + field_buf.len()).try_into().ok()?,
            })
        } else {
            None
        }
    }

    pub fn new<T>(msg_buf: &[u8], field: Option<&T>) -> Self
    where
        T: std::ops::Deref<Target = str>,
    {
        field
            .and_then(|f| Self::build(msg_buf, f.deref()))
            .unwrap_or_else(Self::new_not_present)
    }

    /// Reassemble a previously cached field.
    ///
    /// **NOTE**: The caller must ensure that the `msg_buff` is the same one `build` was called for.
    /// Otherwise, you'll get a panic.
    pub fn read<'m, T>(&self, msg_buf: &'m [u8]) -> Option<T>
    where
        T: TryFrom<&'m str>,
        T::Error: std::fmt::Debug,
    {
        match self {
            Self {
                start: 0..=1,
                end: 0,
            } => None,
            Self { start, end } => {
                let s = std::str::from_utf8(&msg_buf[(*start as usize)..(*end as usize)])
                    .expect("Invalid utf8 when reconstructing string");
                // We already check the fields during the construction of `Self`.
                T::try_from(s)
                    .map(Some)
                    .expect("Invalid field reconstruction")
            }
        }
    }
}

/// A cache of the Message header fields.
#[derive(Debug, Default, Copy, Clone)]
pub(crate) struct QuickFields {
    path: FieldPos,
    interface: FieldPos,
    member: FieldPos,
    error_name: FieldPos,
    reply_serial: Option<NonZeroU32>,
    destination: FieldPos,
    sender: FieldPos,
    signature: FieldPos,
    unix_fds: Option<u32>,
}

impl QuickFields {
    pub fn new(buf: &[u8], header: &Header<'_>) -> Result<Self> {
        Ok(Self {
            path: FieldPos::new(buf, header.path()),
            interface: FieldPos::new(buf, header.interface()),
            member: FieldPos::new(buf, header.member()),
            error_name: FieldPos::new(buf, header.error_name()),
            reply_serial: header.reply_serial(),
            destination: FieldPos::new(buf, header.destination()),
            sender: FieldPos::new(buf, header.sender()),
            signature: FieldPos::new(buf, header.signature()),
            unix_fds: header.unix_fds(),
        })
    }

    pub fn path<'m>(&self, msg: &'m Message) -> Option<ObjectPath<'m>> {
        self.path.read(msg.data())
    }

    pub fn interface<'m>(&self, msg: &'m Message) -> Option<InterfaceName<'m>> {
        self.interface.read(msg.data())
    }

    pub fn member<'m>(&self, msg: &'m Message) -> Option<MemberName<'m>> {
        self.member.read(msg.data())
    }

    pub fn error_name<'m>(&self, msg: &'m Message) -> Option<ErrorName<'m>> {
        self.error_name.read(msg.data())
    }

    pub fn reply_serial(&self) -> Option<NonZeroU32> {
        self.reply_serial
    }

    pub fn destination<'m>(&self, msg: &'m Message) -> Option<BusName<'m>> {
        self.destination.read(msg.data())
    }

    pub fn sender<'m>(&self, msg: &'m Message) -> Option<UniqueName<'m>> {
        self.sender.read(msg.data())
    }

    pub fn signature<'m>(&self, msg: &'m Message) -> Option<Signature<'m>> {
        self.signature.read(msg.data())
    }

    pub fn unix_fds(&self) -> Option<u32> {
        self.unix_fds
    }
}

impl<'m> Default for Fields<'m> {
    fn default() -> Self {
        Self(Vec::with_capacity(MAX_FIELDS_IN_MESSAGE))
    }
}

impl<'m> std::ops::Deref for Fields<'m> {
    type Target = [Field<'m>];

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

#[cfg(test)]
mod tests {
    use super::{Field, Fields};

    #[test]
    fn test() {
        let mut mf = Fields::new();
        assert_eq!(mf.len(), 0);
        mf.add(Field::ReplySerial(42.try_into().unwrap()));
        assert_eq!(mf.len(), 1);
        mf.add(Field::ReplySerial(43.try_into().unwrap()));
        assert_eq!(mf.len(), 2);

        let mut mf = Fields::new();
        assert_eq!(mf.len(), 0);
        mf.replace(Field::ReplySerial(42.try_into().unwrap()));
        assert_eq!(mf.len(), 1);
        mf.replace(Field::ReplySerial(43.try_into().unwrap()));
        assert_eq!(mf.len(), 1);
    }
}
