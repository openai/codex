use zbus_names::BusName;

use crate::{zvariant::ObjectPath, Connection, Error, Result};

/// A signal emission context.
///
/// For signal emission using the high-level API, you'll need instances of this type.
///
/// See [`crate::InterfaceRef::signal_context`] and [`crate::interface`]
/// documentation for details and examples of this type in use.
#[derive(Clone, Debug)]
pub struct SignalContext<'s> {
    conn: Connection,
    path: ObjectPath<'s>,
    destination: Option<BusName<'s>>,
}

impl<'s> SignalContext<'s> {
    /// Create a new signal context for the given connection and object path.
    pub fn new<P>(conn: &Connection, path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'s>>,
        P::Error: Into<Error>,
    {
        path.try_into()
            .map(|p| Self {
                conn: conn.clone(),
                path: p,
                destination: None,
            })
            .map_err(Into::into)
    }

    /// Create a new signal context for the given connection and object path.
    pub fn from_parts(conn: Connection, path: ObjectPath<'s>) -> Self {
        Self {
            conn,
            path,
            destination: None,
        }
    }

    /// Set the destination for the signal emission.
    ///
    /// Signals are typically broadcasted and thus don't have a destination. However, there are
    /// cases where you need to unicast signals to specific peers. This method allows you to set the
    /// destination for the signals emitted with this context.
    pub fn set_destination(mut self, destination: BusName<'s>) -> Self {
        self.destination = Some(destination);

        self
    }

    /// Get a reference to the associated connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get a reference to the associated object path.
    pub fn path(&self) -> &ObjectPath<'s> {
        &self.path
    }

    /// Get a reference to the associated destination (if any).
    pub fn destination(&self) -> Option<&BusName<'s>> {
        self.destination.as_ref()
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> SignalContext<'static> {
        SignalContext {
            conn: self.conn.clone(),
            path: self.path.to_owned(),
            destination: self.destination.as_ref().map(|d| d.to_owned()),
        }
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> SignalContext<'static> {
        SignalContext {
            conn: self.conn,
            path: self.path.into_owned(),
            destination: self.destination.map(|d| d.into_owned()),
        }
    }
}
