//! The client-side proxy API.

use enumflags2::BitFlags;
use futures_util::StreamExt;
use static_assertions::assert_impl_all;
use std::{fmt, ops::Deref};
use zbus_names::{BusName, InterfaceName, MemberName, UniqueName};
use zvariant::{ObjectPath, OwnedValue, Value};

use crate::{
    blocking::Connection,
    message::Message,
    proxy::{MethodFlags, ProxyDefault},
    utils::block_on,
    Error, Result,
};

use crate::fdo;

mod builder;
pub use builder::Builder;

/// A blocking wrapper of [`crate::Proxy`].
///
/// This API is mostly the same as [`crate::Proxy`], except that all its methods block to
/// completion.
///
/// # Example
///
/// ```
/// use std::result::Result;
/// use std::error::Error;
/// use zbus::blocking::{Connection, Proxy};
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let connection = Connection::session()?;
///     let p = Proxy::new(
///         &connection,
///         "org.freedesktop.DBus",
///         "/org/freedesktop/DBus",
///         "org.freedesktop.DBus",
///     )?;
///     // owned return value
///     let _id: String = p.call("GetId", &())?;
///     // borrowed return value
///     let body = p.call_method("GetId", &())?.body();
///     let _id: &str = body.deserialize()?;
///     Ok(())
/// }
/// ```
///
/// # Note
///
/// It is recommended to use the [`proxy`] macro, which provides a more convenient and
/// type-safe *façade* `Proxy` derived from a Rust trait.
///
/// ## Current limitations:
///
/// At the moment, `Proxy` doesn't prevent [auto-launching][al].
///
/// [`proxy`]: attr.proxy.html
/// [al]: https://github.com/dbus2/zbus/issues/54
#[derive(Clone)]
pub struct Proxy<'a> {
    conn: Connection,
    // Wrap it in an `Option` to ensure the proxy is dropped in a `block_on` call. This is needed
    // for tokio because the proxy spawns a task in its `Drop` impl and that needs a runtime
    // context in case of tokio.
    azync: Option<crate::Proxy<'a>>,
}

impl fmt::Debug for Proxy<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proxy")
            .field("azync", &self.azync)
            .finish_non_exhaustive()
    }
}

assert_impl_all!(Proxy<'_>: Send, Sync, Unpin);

impl<'a> Proxy<'a> {
    /// Create a new `Proxy` for the given destination/path/interface.
    pub fn new<D, P, I>(
        conn: &Connection,
        destination: D,
        path: P,
        interface: I,
    ) -> Result<Proxy<'a>>
    where
        D: TryInto<BusName<'a>>,
        P: TryInto<ObjectPath<'a>>,
        I: TryInto<InterfaceName<'a>>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
    {
        let proxy = block_on(crate::Proxy::new(
            conn.inner(),
            destination,
            path,
            interface,
        ))?;

        Ok(Self {
            conn: conn.clone(),
            azync: Some(proxy),
        })
    }

    /// Create a new `Proxy` for the given destination/path/interface, taking ownership of all
    /// passed arguments.
    pub fn new_owned<D, P, I>(
        conn: Connection,
        destination: D,
        path: P,
        interface: I,
    ) -> Result<Proxy<'a>>
    where
        D: TryInto<BusName<'static>>,
        P: TryInto<ObjectPath<'static>>,
        I: TryInto<InterfaceName<'static>>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
    {
        let proxy = block_on(crate::Proxy::new_owned(
            conn.clone().into_inner(),
            destination,
            path,
            interface,
        ))?;

        Ok(Self {
            conn,
            azync: Some(proxy),
        })
    }

    /// Get a reference to the associated connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get a reference to the destination service name.
    pub fn destination(&self) -> &BusName<'_> {
        self.inner().destination()
    }

    /// Get a reference to the object path.
    pub fn path(&self) -> &ObjectPath<'_> {
        self.inner().path()
    }

    /// Get a reference to the interface.
    pub fn interface(&self) -> &InterfaceName<'_> {
        self.inner().interface()
    }

    /// Introspect the associated object, and return the XML description.
    ///
    /// See the [xml](xml/index.html) module for parsing the result.
    pub fn introspect(&self) -> fdo::Result<String> {
        block_on(self.inner().introspect())
    }

    /// Get the cached value of the property `property_name`.
    ///
    /// This returns `None` if the property is not in the cache.  This could be because the cache
    /// was invalidated by an update, because caching was disabled for this property or proxy, or
    /// because the cache has not yet been populated.  Use `get_property` to fetch the value from
    /// the peer.
    pub fn cached_property<T>(&self, property_name: &str) -> Result<Option<T>>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<Error>,
    {
        self.inner().cached_property(property_name)
    }

    /// Get the cached value of the property `property_name`.
    ///
    /// Same as `cached_property`, but gives you access to the raw value stored in the cache. This
    /// is useful if you want to avoid allocations and cloning.
    pub fn cached_property_raw<'p>(
        &'p self,
        property_name: &'p str,
    ) -> Option<impl Deref<Target = Value<'static>> + 'p> {
        self.inner().cached_property_raw(property_name)
    }

    /// Get the property `property_name`.
    ///
    /// Get the property value from the cache or call the `Get` method of the
    /// `org.freedesktop.DBus.Properties` interface.
    pub fn get_property<T>(&self, property_name: &str) -> Result<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<Error>,
    {
        block_on(self.inner().get_property(property_name))
    }

    /// Set the property `property_name`.
    ///
    /// Effectively, call the `Set` method of the `org.freedesktop.DBus.Properties` interface.
    pub fn set_property<'t, T>(&self, property_name: &str, value: T) -> fdo::Result<()>
    where
        T: 't + Into<Value<'t>>,
    {
        block_on(self.inner().set_property(property_name, value))
    }

    /// Call a method and return the reply.
    ///
    /// Typically, you would want to use [`call`] method instead. Use this method if you need to
    /// deserialize the reply message manually (this way, you can avoid the memory
    /// allocation/copying, by deserializing the reply to an unowned type).
    ///
    /// [`call`]: struct.Proxy.html#method.call
    pub fn call_method<'m, M, B>(&self, method_name: M, body: &B) -> Result<Message>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        block_on(self.inner().call_method(method_name, body))
    }

    /// Call a method and return the reply body.
    ///
    /// Use [`call_method`] instead if you need to deserialize the reply manually/separately.
    ///
    /// [`call_method`]: struct.Proxy.html#method.call_method
    pub fn call<'m, M, B, R>(&self, method_name: M, body: &B) -> Result<R>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
        R: for<'d> zvariant::DynamicDeserialize<'d>,
    {
        block_on(self.inner().call(method_name, body))
    }

    /// Call a method and return the reply body, optionally supplying a set of
    /// method flags to control the way the method call message is sent and handled.
    ///
    /// Use [`call`] instead if you do not need any special handling via additional flags.
    /// If the `NoReplyExpected` flag is passed , this will return None immediately
    /// after sending the message, similar to [`call_noreply`]
    ///
    /// [`call`]: struct.Proxy.html#method.call
    /// [`call_noreply`]: struct.Proxy.html#method.call_noreply
    pub fn call_with_flags<'m, M, B, R>(
        &self,
        method_name: M,
        flags: BitFlags<MethodFlags>,
        body: &B,
    ) -> Result<Option<R>>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
        R: for<'d> zvariant::DynamicDeserialize<'d>,
    {
        block_on(self.inner().call_with_flags(method_name, flags, body))
    }

    /// Call a method without expecting a reply
    ///
    /// This sets the `NoReplyExpected` flag on the calling message and does not wait for a reply.
    pub fn call_noreply<'m, M, B>(&self, method_name: M, body: &B) -> Result<()>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        block_on(self.inner().call_noreply(method_name, body))
    }

    /// Create a stream for signal named `signal_name`.
    ///
    /// # Errors
    ///
    /// Apart from general I/O errors that can result from socket communications, calling this
    /// method will also result in an error if the destination service has not yet registered its
    /// well-known name with the bus (assuming you're using the well-known name as destination).
    pub fn receive_signal<'m, M>(&self, signal_name: M) -> Result<SignalIterator<'m>>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
    {
        self.receive_signal_with_args(signal_name, &[])
    }

    /// Same as [`Proxy::receive_signal`] but with a filter.
    ///
    /// The D-Bus specification allows you to filter signals by their arguments, which helps avoid
    /// a lot of unnecessary traffic and processing since the filter is run on the server side. Use
    /// this method where possible. Note that this filtering is limited to arguments of string
    /// types.
    ///
    /// The arguments are passed as a tuples of argument index and expected value.
    pub fn receive_signal_with_args<'m, M>(
        &self,
        signal_name: M,
        args: &[(u8, &str)],
    ) -> Result<SignalIterator<'m>>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
    {
        block_on(self.inner().receive_signal_with_args(signal_name, args))
            .map(Some)
            .map(SignalIterator)
    }

    /// Create a stream for all signals emitted by this service.
    ///
    /// # Errors
    ///
    /// Apart from general I/O errors that can result from socket communications, calling this
    /// method will also result in an error if the destination service has not yet registered its
    /// well-known name with the bus (assuming you're using the well-known name as destination).
    pub fn receive_all_signals(&self) -> Result<SignalIterator<'static>> {
        block_on(self.inner().receive_all_signals())
            .map(Some)
            .map(SignalIterator)
    }

    /// Get an iterator to receive owner changed events.
    ///
    /// If the proxy destination is a unique name, the stream will be notified of the peer
    /// disconnection from the bus (with a `None` value).
    ///
    /// If the proxy destination is a well-known name, the stream will be notified whenever the name
    /// owner is changed, either by a new peer being granted ownership (`Some` value) or when the
    /// name is released (with a `None` value).
    ///
    /// Note that zbus doesn't queue the updates. If the listener is slower than the receiver, it
    /// will only receive the last update.
    pub fn receive_property_changed<'name: 'a, T>(
        &self,
        name: &'name str,
    ) -> PropertyIterator<'a, T> {
        PropertyIterator(block_on(self.inner().receive_property_changed(name)))
    }

    /// Get an iterator to receive property changed events.
    ///
    /// Note that zbus doesn't queue the updates. If the listener is slower than the receiver, it
    /// will only receive the last update.
    pub fn receive_owner_changed(&self) -> Result<OwnerChangedIterator<'_>> {
        block_on(self.inner().receive_owner_changed()).map(OwnerChangedIterator)
    }

    /// Get a reference to the underlying async Proxy.
    pub fn inner(&self) -> &crate::Proxy<'a> {
        self.azync.as_ref().expect("Inner proxy is `None`")
    }

    /// Get the underlying async Proxy, consuming `self`.
    pub fn into_inner(mut self) -> crate::Proxy<'a> {
        self.azync.take().expect("Inner proxy is `None`")
    }
}

impl ProxyDefault for Proxy<'_> {
    const INTERFACE: Option<&'static str> = None;
    const DESTINATION: Option<&'static str> = None;
    const PATH: Option<&'static str> = None;
}

impl<'a> std::convert::AsRef<Proxy<'a>> for Proxy<'a> {
    fn as_ref(&self) -> &Proxy<'a> {
        self
    }
}

impl<'a> From<crate::Proxy<'a>> for Proxy<'a> {
    fn from(proxy: crate::Proxy<'a>) -> Self {
        Self {
            conn: proxy.connection().clone().into(),
            azync: Some(proxy),
        }
    }
}

impl std::ops::Drop for Proxy<'_> {
    fn drop(&mut self) {
        block_on(async {
            self.azync.take();
        });
    }
}

/// An [`std::iter::Iterator`] implementation that yields signal [messages](`Message`).
///
/// Use [`Proxy::receive_signal`] to create an instance of this type.
#[derive(Debug)]
pub struct SignalIterator<'a>(Option<crate::proxy::SignalStream<'a>>);

impl<'a> SignalIterator<'a> {
    /// The signal name.
    pub fn name(&self) -> Option<&MemberName<'a>> {
        self.0.as_ref().expect("`SignalStream` is `None`").name()
    }
}

assert_impl_all!(SignalIterator<'_>: Send, Sync, Unpin);

impl std::iter::Iterator for SignalIterator<'_> {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        block_on(self.0.as_mut().expect("`SignalStream` is `None`").next())
    }
}

impl std::ops::Drop for SignalIterator<'_> {
    fn drop(&mut self) {
        block_on(async {
            if let Some(azync) = self.0.take() {
                crate::AsyncDrop::async_drop(azync).await;
            }
        });
    }
}

/// An [`std::iter::Iterator`] implementation that yields property change notifications.
///
/// Use [`Proxy::receive_property_changed`] to create an instance of this type.
pub struct PropertyIterator<'a, T>(crate::proxy::PropertyStream<'a, T>);

impl<'a, T> std::iter::Iterator for PropertyIterator<'a, T>
where
    T: Unpin,
{
    type Item = PropertyChanged<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        block_on(self.0.next()).map(PropertyChanged)
    }
}

/// A property changed event.
///
/// The property changed event generated by [`PropertyIterator`].
pub struct PropertyChanged<'a, T>(crate::proxy::PropertyChanged<'a, T>);

// split this out to avoid the trait bound on `name` method
impl<'a, T> PropertyChanged<'a, T> {
    /// Get the name of the property that changed.
    pub fn name(&self) -> &str {
        self.0.name()
    }

    // Get the raw value of the property that changed.
    //
    // If the notification signal contained the new value, it has been cached already and this call
    // will return that value. Otherwise (i-e invalidated property), a D-Bus call is made to fetch
    // and cache the new value.
    pub fn get_raw(&self) -> Result<impl Deref<Target = Value<'static>> + '_> {
        block_on(self.0.get_raw())
    }
}

impl<'a, T> PropertyChanged<'a, T>
where
    T: TryFrom<zvariant::OwnedValue>,
    T::Error: Into<crate::Error>,
{
    // Get the value of the property that changed.
    //
    // If the notification signal contained the new value, it has been cached already and this call
    // will return that value. Otherwise (i-e invalidated property), a D-Bus call is made to fetch
    // and cache the new value.
    pub fn get(&self) -> Result<T> {
        block_on(self.0.get())
    }
}

/// An [`std::iter::Iterator`] implementation that yields owner change notifications.
///
/// Use [`Proxy::receive_owner_changed`] to create an instance of this type.
pub struct OwnerChangedIterator<'a>(crate::proxy::OwnerChangedStream<'a>);

impl OwnerChangedIterator<'_> {
    /// The bus name being tracked.
    pub fn name(&self) -> &BusName<'_> {
        self.0.name()
    }
}

impl<'a> std::iter::Iterator for OwnerChangedIterator<'a> {
    type Item = Option<UniqueName<'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        block_on(self.0.next())
    }
}

/// This trait is implemented by all blocking proxies, which are generated with the
/// [`dbus_proxy`](zbus::dbus_proxy) macro.
pub trait ProxyImpl<'p>
where
    Self: Sized,
{
    /// Returns a customizable builder for this proxy.
    fn builder(conn: &Connection) -> Builder<'p, Self>;

    /// Consumes `self`, returning the underlying `zbus::Proxy`.
    fn into_inner(self) -> Proxy<'p>;

    /// The reference to the underlying `zbus::Proxy`.
    fn inner(&self) -> &Proxy<'p>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocking;
    use ntest::timeout;
    use test_log::test;

    #[test]
    #[timeout(15000)]
    fn signal() {
        // Register a well-known name with the session bus and ensure we get the appropriate
        // signals called for that.
        let conn = Connection::session().unwrap();
        let unique_name = conn.unique_name().unwrap().to_string();

        let proxy = blocking::fdo::DBusProxy::new(&conn).unwrap();
        let well_known = "org.freedesktop.zbus.ProxySignalTest";
        let mut owner_changed = proxy
            .receive_name_owner_changed_with_args(&[(0, well_known), (2, unique_name.as_str())])
            .unwrap();
        let mut name_acquired = proxy
            .receive_name_acquired_with_args(&[(0, well_known)])
            .unwrap();

        blocking::fdo::DBusProxy::new(&conn)
            .unwrap()
            .request_name(
                well_known.try_into().unwrap(),
                fdo::RequestNameFlags::ReplaceExisting.into(),
            )
            .unwrap();

        let signal = owner_changed.next().unwrap();
        let args = signal.args().unwrap();
        assert!(args.name() == well_known);
        assert!(*args.new_owner().as_ref().unwrap() == *unique_name);

        let signal = name_acquired.next().unwrap();
        // `NameAcquired` is emitted twice, first when the unique name is assigned on
        // connection and secondly after we ask for a specific name. Let's make sure we only get the
        // one we subscribed to.
        assert!(signal.args().unwrap().name() == well_known);
    }
}
