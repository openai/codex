use static_assertions::assert_impl_all;

/// The encoding format.
#[derive(Debug, Default, PartialEq, Eq, Copy, Clone)]
pub enum Format {
    /// [D-Bus](https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-marshaling)
    /// format.
    #[default]
    DBus,
    /// [GVariant](https://developer.gnome.org/glib/stable/glib-GVariant.html) format.
    #[cfg(feature = "gvariant")]
    GVariant,
}

assert_impl_all!(Format: Send, Sync, Unpin);

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::DBus => write!(f, "D-Bus"),
            #[cfg(feature = "gvariant")]
            Format::GVariant => write!(f, "GVariant"),
        }
    }
}
