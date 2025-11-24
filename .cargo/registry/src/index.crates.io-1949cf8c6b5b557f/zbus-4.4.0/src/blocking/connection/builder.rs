use static_assertions::assert_impl_all;
#[cfg(not(feature = "tokio"))]
use std::net::TcpStream;
#[cfg(all(unix, not(feature = "tokio")))]
use std::os::unix::net::UnixStream;
#[cfg(feature = "tokio")]
use tokio::net::TcpStream;
#[cfg(all(unix, feature = "tokio"))]
use tokio::net::UnixStream;
#[cfg(all(windows, not(feature = "tokio")))]
use uds_windows::UnixStream;

use zvariant::{ObjectPath, Str};

#[cfg(feature = "p2p")]
use crate::Guid;
use crate::{
    address::Address, blocking::Connection, connection::socket::BoxedSplit, names::WellKnownName,
    object_server::Interface, utils::block_on, AuthMechanism, Error, Result,
};

/// A builder for [`zbus::blocking::Connection`].
#[derive(Debug)]
#[must_use]
pub struct Builder<'a>(crate::connection::Builder<'a>);

assert_impl_all!(Builder<'_>: Send, Sync, Unpin);

impl<'a> Builder<'a> {
    /// Create a builder for the session/user message bus connection.
    pub fn session() -> Result<Self> {
        crate::connection::Builder::session().map(Self)
    }

    /// Create a builder for the system-wide message bus connection.
    pub fn system() -> Result<Self> {
        crate::connection::Builder::system().map(Self)
    }

    /// Create a builder for connection that will use the given [D-Bus bus address].
    ///
    /// [D-Bus bus address]: https://dbus.freedesktop.org/doc/dbus-specification.html#addresses
    pub fn address<A>(address: A) -> Result<Self>
    where
        A: TryInto<Address>,
        A::Error: Into<Error>,
    {
        crate::connection::Builder::address(address).map(Self)
    }

    /// Create a builder for connection that will use the given unix stream.
    ///
    /// If the default `async-io` feature is disabled, this method will expect
    /// [`tokio::net::UnixStream`](https://docs.rs/tokio/latest/tokio/net/struct.UnixStream.html)
    /// argument.
    ///
    /// Since tokio currently [does not support Unix domain sockets][tuds] on Windows, this method
    /// is not available when the `tokio` feature is enabled and building for Windows target.
    ///
    /// [tuds]: https://github.com/tokio-rs/tokio/issues/2201
    #[cfg(any(unix, not(feature = "tokio")))]
    pub fn unix_stream(stream: UnixStream) -> Self {
        Self(crate::connection::Builder::unix_stream(stream))
    }

    /// Create a builder for connection that will use the given TCP stream.
    ///
    /// If the default `async-io` feature is disabled, this method will expect
    /// [`tokio::net::TcpStream`](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html)
    /// argument.
    pub fn tcp_stream(stream: TcpStream) -> Self {
        Self(crate::connection::Builder::tcp_stream(stream))
    }

    /// Create a builder for a connection that will use the given pre-authenticated socket.
    ///
    /// This is similar to [`Builder::socket`], except that the socket is either already
    /// authenticated or does not require authentication.
    pub fn authenticated_socket<S, G>(socket: S, guid: G) -> Result<Self>
    where
        S: Into<BoxedSplit>,
        G: TryInto<crate::Guid<'a>>,
        G::Error: Into<Error>,
    {
        crate::connection::Builder::authenticated_socket(socket, guid).map(Self)
    }

    /// Create a builder for connection that will use the given socket.
    pub fn socket<S: Into<BoxedSplit>>(socket: S) -> Self {
        Self(crate::connection::Builder::socket(socket))
    }

    /// Specify the mechanism to use during authentication.
    pub fn auth_mechanism(self, auth_mechanism: AuthMechanism) -> Self {
        Self(self.0.auth_mechanism(auth_mechanism))
    }

    /// Specify the mechanisms to use during authentication.
    #[deprecated(since = "4.1.3", note = "Use `auth_mechanism` instead.")]
    pub fn auth_mechanisms(self, auth_mechanisms: &[AuthMechanism]) -> Self {
        #[allow(deprecated)]
        Self(self.0.auth_mechanisms(auth_mechanisms))
    }

    /// The cookie context to use during authentication.
    ///
    /// This is only used when the `cookie` authentication mechanism is enabled and only valid for
    /// server connection.
    ///
    /// If not specified, the default cookie context of `org_freedesktop_general` will be used.
    ///
    /// # Errors
    ///
    /// If the given string is not a valid cookie context.
    pub fn cookie_context<C>(self, context: C) -> Result<Self>
    where
        C: Into<Str<'a>>,
    {
        self.0.cookie_context(context).map(Self)
    }

    /// The ID of the cookie to use during authentication.
    ///
    /// This is only used when the `cookie` authentication mechanism is enabled and only valid for
    /// server connection.
    ///
    /// If not specified, the first cookie found in the cookie context file will be used.
    pub fn cookie_id(self, id: usize) -> Self {
        Self(self.0.cookie_id(id))
    }

    /// The to-be-created connection will be a peer-to-peer connection.
    ///
    /// This method is only available when the `p2p` feature is enabled.
    #[cfg(feature = "p2p")]
    pub fn p2p(self) -> Self {
        Self(self.0.p2p())
    }

    /// The to-be-created connection will be a server using the given GUID.
    ///
    /// The to-be-created connection will wait for incoming client authentication handshake and
    /// negotiation messages, for peer-to-peer communications after successful creation.
    ///
    /// This method is only available when the `p2p` feature is enabled.
    ///
    /// **NOTE:** This method is redundant when using [`Builder::authenticated_socket`] since the
    /// latter already sets the GUID for the connection and zbus doesn't differentiate between a
    /// server and a client connection, except for authentication.
    #[cfg(feature = "p2p")]
    pub fn server<G>(self, guid: G) -> Result<Self>
    where
        G: TryInto<Guid<'a>>,
        G::Error: Into<Error>,
    {
        self.0.server(guid).map(Self)
    }

    /// Set the capacity of the main (unfiltered) queue.
    ///
    /// Since typically you'd want to set this at instantiation time, you can set it through the
    /// builder.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::error::Error;
    /// # use zbus::blocking::connection;
    /// #
    /// let conn = connection::Builder::session()?
    ///     .max_queued(30)
    ///     .build()?;
    /// assert_eq!(conn.max_queued(), 30);
    ///
    /// // Do something useful with `conn`..
    /// # Ok::<_, Box<dyn Error + Send + Sync>>(())
    /// ```
    pub fn max_queued(self, max: usize) -> Self {
        Self(self.0.max_queued(max))
    }

    /// Register a D-Bus [`Interface`] to be served at a given path.
    ///
    /// This is similar to [`zbus::blocking::ObjectServer::at`], except that it allows you to have
    /// your interfaces available immediately after the connection is established. Typically, this
    /// is exactly what you'd want. Also in contrast to [`zbus::blocking::ObjectServer::at`], this
    /// method will replace any previously added interface with the same name at the same path.
    pub fn serve_at<P, I>(self, path: P, iface: I) -> Result<Self>
    where
        I: Interface,
        P: TryInto<ObjectPath<'a>>,
        P::Error: Into<Error>,
    {
        self.0.serve_at(path, iface).map(Self)
    }

    /// Register a well-known name for this connection on the bus.
    ///
    /// This is similar to [`zbus::blocking::Connection::request_name`], except the name is
    /// requested as part of the connection setup ([`Builder::build`]), immediately after
    /// interfaces registered (through [`Builder::serve_at`]) are advertised. Typically
    /// this is exactly what you want.
    pub fn name<W>(self, well_known_name: W) -> Result<Self>
    where
        W: TryInto<WellKnownName<'a>>,
        W::Error: Into<Error>,
    {
        self.0.name(well_known_name).map(Self)
    }

    /// Sets the unique name of the connection.
    ///
    /// This method is only available when the `bus-impl` feature is enabled.
    ///
    /// # Panics
    ///
    /// This method panics if the to-be-created connection is not a peer-to-peer connection.
    /// It will always panic if the connection is to a message bus as it's the bus that assigns
    /// peers their unique names. This is mainly provided for bus implementations. All other users
    /// should not need to use this method.
    #[cfg(feature = "bus-impl")]
    pub fn unique_name<U>(self, unique_name: U) -> Result<Self>
    where
        U: TryInto<crate::names::UniqueName<'a>>,
        U::Error: Into<Error>,
    {
        self.0.unique_name(unique_name).map(Self)
    }

    /// Build the connection, consuming the builder.
    ///
    /// # Errors
    ///
    /// Until server-side bus connection is supported, attempting to build such a connection will
    /// result in [`Error::Unsupported`] error.
    pub fn build(self) -> Result<Connection> {
        block_on(self.0.build()).map(Into::into)
    }
}
