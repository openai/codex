//! Blocking connection API.

use enumflags2::BitFlags;
use event_listener::EventListener;
use static_assertions::assert_impl_all;
use std::{io, ops::Deref};
use zbus_names::{BusName, ErrorName, InterfaceName, MemberName, OwnedUniqueName, WellKnownName};
use zvariant::ObjectPath;

use crate::{
    blocking::ObjectServer,
    fdo::{ConnectionCredentials, RequestNameFlags, RequestNameReply},
    message::Message,
    utils::block_on,
    DBusError, Error, Result,
};

mod builder;
pub use builder::Builder;

/// A blocking wrapper of [`zbus::Connection`].
///
/// Most of the API is very similar to [`zbus::Connection`], except it's blocking.
#[derive(Debug, Clone)]
#[must_use = "Dropping a `Connection` will close the underlying socket."]
pub struct Connection {
    inner: crate::Connection,
}

assert_impl_all!(Connection: Send, Sync, Unpin);

impl Connection {
    /// Create a `Connection` to the session/user message bus.
    pub fn session() -> Result<Self> {
        block_on(crate::Connection::session()).map(Self::from)
    }

    /// Create a `Connection` to the system-wide message bus.
    pub fn system() -> Result<Self> {
        block_on(crate::Connection::system()).map(Self::from)
    }

    /// The capacity of the main (unfiltered) queue.
    pub fn max_queued(&self) -> usize {
        self.inner.max_queued()
    }

    /// Set the capacity of the main (unfiltered) queue.
    pub fn set_max_queued(mut self, max: usize) {
        self.inner.set_max_queued(max)
    }

    /// The server's GUID.
    pub fn server_guid(&self) -> &str {
        self.inner.server_guid()
    }

    /// The unique name as assigned by the message bus or `None` if not a message bus connection.
    pub fn unique_name(&self) -> Option<&OwnedUniqueName> {
        self.inner.unique_name()
    }

    /// Send `msg` to the peer.
    pub fn send(&self, msg: &Message) -> Result<()> {
        block_on(self.inner.send(msg))
    }

    /// Send a method call.
    ///
    /// Create a method-call message, send it over the connection, then wait for the reply.
    ///
    /// On successful reply, an `Ok(Message)` is returned. On error, an `Err` is returned. D-Bus
    /// error replies are returned as [`Error::MethodError`].
    pub fn call_method<'d, 'p, 'i, 'm, D, P, I, M, B>(
        &self,
        destination: Option<D>,
        path: P,
        iface: Option<I>,
        method_name: M,
        body: &B,
    ) -> Result<Message>
    where
        D: TryInto<BusName<'d>>,
        P: TryInto<ObjectPath<'p>>,
        I: TryInto<InterfaceName<'i>>,
        M: TryInto<MemberName<'m>>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        block_on(
            self.inner
                .call_method(destination, path, iface, method_name, body),
        )
    }

    /// Emit a signal.
    ///
    /// Create a signal message, and send it over the connection.
    pub fn emit_signal<'d, 'p, 'i, 'm, D, P, I, M, B>(
        &self,
        destination: Option<D>,
        path: P,
        iface: I,
        signal_name: M,
        body: &B,
    ) -> Result<()>
    where
        D: TryInto<BusName<'d>>,
        P: TryInto<ObjectPath<'p>>,
        I: TryInto<InterfaceName<'i>>,
        M: TryInto<MemberName<'m>>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        block_on(
            self.inner
                .emit_signal(destination, path, iface, signal_name, body),
        )
    }

    /// Reply to a message.
    ///
    /// Given an existing message (likely a method call), send a reply back to the caller with the
    /// given `body`.
    pub fn reply<B>(&self, call: &Message, body: &B) -> Result<()>
    where
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        block_on(self.inner.reply(call, body))
    }

    /// Reply an error to a message.
    ///
    /// Given an existing message (likely a method call), send an error reply back to the caller
    /// with the given `error_name` and `body`.
    ///
    /// Returns the message serial number.
    pub fn reply_error<'e, E, B>(&self, call: &Message, error_name: E, body: &B) -> Result<()>
    where
        B: serde::ser::Serialize + zvariant::DynamicType,
        E: TryInto<ErrorName<'e>>,
        E::Error: Into<Error>,
    {
        block_on(self.inner.reply_error(call, error_name, body))
    }

    /// Reply to a method call with an error.
    ///
    /// Given an existing method call message header, send an error reply back to the caller
    /// using one of the standard interface reply types.
    ///
    /// Returns the message serial number.
    pub fn reply_dbus_error(
        &self,
        call: &zbus::message::Header<'_>,
        err: impl DBusError,
    ) -> Result<()> {
        block_on(self.inner.reply_dbus_error(call, err))
    }

    /// Register a well-known name for this service on the bus.
    ///
    /// Blocking version of [`crate::Connection::request_name`]. See docs there for more details
    /// and caveats.
    pub fn request_name<'w, W>(&self, well_known_name: W) -> Result<()>
    where
        W: TryInto<WellKnownName<'w>>,
        W::Error: Into<Error>,
    {
        block_on(self.inner.request_name(well_known_name))
    }

    /// Register a well-known name for this service on the bus.
    ///
    /// Blocking version of [`crate::Connection::request_name_with_flags`]. See docs there for more
    /// details and caveats.
    pub fn request_name_with_flags<'w, W>(
        &self,
        well_known_name: W,
        flags: BitFlags<RequestNameFlags>,
    ) -> Result<RequestNameReply>
    where
        W: TryInto<WellKnownName<'w>>,
        W::Error: Into<Error>,
    {
        block_on(self.inner.request_name_with_flags(well_known_name, flags))
    }

    /// Deregister a previously registered well-known name for this service on the bus.
    ///
    /// Use this method to deregister a well-known name, registered through
    /// [`Connection::request_name`].
    ///
    /// Unless an error is encountered, returns `Ok(true)` if name was previously registered with
    /// the bus through `self` and it has now been successfully deregistered, `Ok(false)` if name
    /// was not previously registered or already deregistered.
    pub fn release_name<'w, W>(&self, well_known_name: W) -> Result<bool>
    where
        W: TryInto<WellKnownName<'w>>,
        W::Error: Into<Error>,
    {
        block_on(self.inner.release_name(well_known_name))
    }

    /// Checks if `self` is a connection to a message bus.
    ///
    /// This will return `false` for p2p connections.
    pub fn is_bus(&self) -> bool {
        self.inner.is_bus()
    }

    /// Get a reference to the associated [`ObjectServer`].
    ///
    /// The `ObjectServer` is created on-demand.
    pub fn object_server(&self) -> impl Deref<Target = ObjectServer> + '_ {
        self.inner.sync_object_server(true, None)
    }

    /// Get a reference to the underlying async Connection.
    pub fn inner(&self) -> &crate::Connection {
        &self.inner
    }

    /// Get the underlying async Connection, consuming `self`.
    pub fn into_inner(self) -> crate::Connection {
        self.inner
    }

    /// Returns a listener, notified on various connection activity.
    ///
    /// This function is meant for the caller to implement idle or timeout on inactivity.
    pub fn monitor_activity(&self) -> EventListener {
        self.inner.monitor_activity()
    }

    /// Returns the peer credentials.
    ///
    /// The fields are populated on the best effort basis. Some or all fields may not even make
    /// sense for certain sockets or on certain platforms and hence will be set to `None`.
    ///
    /// # Caveats
    ///
    /// Currently `unix_group_ids` and `linux_security_label` fields are not populated.
    pub fn peer_credentials(&self) -> io::Result<ConnectionCredentials> {
        block_on(self.inner.peer_credentials())
    }

    /// Close the connection.
    ///
    /// After this call, all reading and writing operations will fail.
    pub fn close(self) -> Result<()> {
        block_on(self.inner.close())
    }
}

impl From<crate::Connection> for Connection {
    fn from(conn: crate::Connection) -> Self {
        Self { inner: conn }
    }
}

#[cfg(feature = "p2p")]
#[cfg(all(test, unix))]
mod tests {
    use event_listener::Listener;
    use ntest::timeout;
    #[cfg(all(unix, not(feature = "tokio")))]
    use std::os::unix::net::UnixStream;
    use std::thread;
    use test_log::test;
    #[cfg(all(unix, feature = "tokio"))]
    use tokio::net::UnixStream;
    #[cfg(all(windows, not(feature = "tokio")))]
    use uds_windows::UnixStream;

    use crate::{
        blocking::{connection::Builder, MessageIterator},
        Guid,
    };

    #[test]
    #[timeout(15000)]
    fn unix_p2p() {
        let guid = Guid::generate();

        // Tokio needs us to call the sync function from async context. :shrug:
        let (p0, p1) = crate::utils::block_on(async { UnixStream::pair().unwrap() });

        let (tx, rx) = std::sync::mpsc::channel();
        let server_thread = thread::spawn(move || {
            let c = Builder::unix_stream(p0)
                .server(guid)
                .unwrap()
                .p2p()
                .build()
                .unwrap();
            rx.recv().unwrap();
            let reply = c
                .call_method(None::<()>, "/", Some("org.zbus.p2p"), "Test", &())
                .unwrap();
            assert_eq!(reply.to_string(), "Method return");
            let val: String = reply.body().deserialize().unwrap();
            val
        });

        let c = Builder::unix_stream(p1).p2p().build().unwrap();

        let listener = c.monitor_activity();

        let mut s = MessageIterator::from(&c);
        tx.send(()).unwrap();
        let m = s.next().unwrap().unwrap();
        assert_eq!(m.to_string(), "Method call Test");
        c.reply(&m, &("yay")).unwrap();

        for _ in s {}

        let val = server_thread.join().expect("failed to join server thread");
        assert_eq!(val, "yay");

        // there was some activity
        listener.wait();
        // eventually, nothing happens and it will timeout
        loop {
            let listener = c.monitor_activity();
            if listener
                .wait_timeout(std::time::Duration::from_millis(10))
                .is_none()
            {
                break;
            }
        }
    }
}
