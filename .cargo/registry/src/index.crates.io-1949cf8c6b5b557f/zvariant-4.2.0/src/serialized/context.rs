use static_assertions::assert_impl_all;

use crate::{serialized::Format, Endian};

/// The encoding context to use with the [serialization and deserialization] API.
///
/// The encoding is dependent on the position of the encoding in the entire message and hence the
/// need to [specify] the byte position of the data being serialized or deserialized. Simply pass
/// `0` if serializing or deserializing to or from the beginning of message, or the preceding bytes
/// end on an 8-byte boundary.
///
/// # Examples
///
/// ```
/// use zvariant::Endian;
/// use zvariant::serialized::Context;
/// use zvariant::to_bytes;
///
/// let str_vec = vec!["Hello", "World"];
/// let ctxt = Context::new_dbus(Endian::Little, 0);
/// let encoded = to_bytes(ctxt, &str_vec).unwrap();
///
/// // Let's decode the 2nd element of the array only
/// let slice = encoded.slice(14..);
/// let decoded: &str = slice.deserialize().unwrap().0;
/// assert_eq!(decoded, "World");
/// ```
///
/// [serialization and deserialization]: index.html#functions
/// [specify]: #method.new
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Context {
    format: Format,
    position: usize,
    endian: Endian,
}

assert_impl_all!(Context: Send, Sync, Unpin);

impl Context {
    /// Create a new encoding context.
    pub fn new(format: Format, endian: Endian, position: usize) -> Self {
        Self {
            format,
            position,
            endian,
        }
    }

    /// Convenient wrapper for [`new`] to create a context for D-Bus format.
    ///
    /// [`new`]: #method.new
    pub fn new_dbus(endian: Endian, position: usize) -> Self {
        Self::new(Format::DBus, endian, position)
    }

    /// Convenient wrapper for [`new`] to create a context for GVariant format.
    ///
    /// [`new`]: #method.new
    #[cfg(feature = "gvariant")]
    pub fn new_gvariant(endian: Endian, position: usize) -> Self {
        Self::new(Format::GVariant, endian, position)
    }

    /// The [`Format`] of this context.
    pub fn format(self) -> Format {
        self.format
    }

    /// The [`Endian`] of this context.
    pub fn endian(self) -> Endian {
        self.endian
    }

    /// The byte position of the value to be encoded or decoded, in the entire message.
    pub fn position(self) -> usize {
        self.position
    }
}
