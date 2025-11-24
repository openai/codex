//! Connection API.
use async_broadcast::{broadcast, InactiveReceiver, Receiver, Sender as Broadcaster};
use enumflags2::BitFlags;
use event_listener::{Event, EventListener};
use ordered_stream::{OrderedFuture, OrderedStream, PollResult};
use static_assertions::assert_impl_all;
use std::{
    collections::HashMap,
    io::{self, ErrorKind},
    num::NonZeroU32,
    ops::Deref,
    pin::Pin,
    sync::{Arc, OnceLock, Weak},
    task::{Context, Poll},
};
use tracing::{debug, info_span, instrument, trace, trace_span, warn, Instrument};
use zbus_names::{BusName, ErrorName, InterfaceName, MemberName, OwnedUniqueName, WellKnownName};
use zvariant::ObjectPath;

use futures_core::Future;
use futures_util::StreamExt;

use crate::{
    async_lock::{Mutex, Semaphore, SemaphorePermit},
    blocking,
    fdo::{self, ConnectionCredentials, RequestNameFlags, RequestNameReply},
    is_flatpak,
    message::{Flags, Message, Type},
    proxy::CacheProperties,
    DBusError, Error, Executor, MatchRule, MessageStream, ObjectServer, OwnedGuid, OwnedMatchRule,
    Result, Task,
};

mod builder;
pub use builder::Builder;

pub mod socket;
pub use socket::Socket;

mod socket_reader;
use socket_reader::SocketReader;

pub(crate) mod handshake;
use handshake::Authenticated;

const DEFAULT_MAX_QUEUED: usize = 64;
const DEFAULT_MAX_METHOD_RETURN_QUEUED: usize = 8;

/// Inner state shared by Connection and WeakConnection
#[derive(Debug)]
pub(crate) struct ConnectionInner {
    server_guid: OwnedGuid,
    #[cfg(unix)]
    cap_unix_fd: bool,
    #[cfg(feature = "p2p")]
    bus_conn: bool,
    unique_name: OnceLock<OwnedUniqueName>,
    registered_names: Mutex<HashMap<WellKnownName<'static>, NameStatus>>,

    activity_event: Arc<Event>,
    socket_write: Mutex<Box<dyn socket::WriteHalf>>,

    // Our executor
    executor: Executor<'static>,

    // Socket reader task
    #[allow(unused)]
    socket_reader_task: OnceLock<Task<()>>,

    pub(crate) msg_receiver: InactiveReceiver<Result<Message>>,
    pub(crate) method_return_receiver: InactiveReceiver<Result<Message>>,
    msg_senders: Arc<Mutex<HashMap<Option<OwnedMatchRule>, MsgBroadcaster>>>,

    subscriptions: Mutex<Subscriptions>,

    object_server: OnceLock<blocking::ObjectServer>,
    object_server_dispatch_task: OnceLock<Task<()>>,
}

type Subscriptions = HashMap<OwnedMatchRule, (u64, InactiveReceiver<Result<Message>>)>;

pub(crate) type MsgBroadcaster = Broadcaster<Result<Message>>;

/// A D-Bus connection.
///
/// A connection to a D-Bus bus, or a direct peer.
///
/// Once created, the connection is authenticated and negotiated and messages can be sent or
/// received, such as [method calls] or [signals].
///
/// For higher-level message handling (typed functions, introspection, documentation reasons etc),
/// it is recommended to wrap the low-level D-Bus messages into Rust functions with the
/// [`proxy`] and [`interface`] macros instead of doing it directly on a `Connection`.
///
/// Typically, a connection is made to the session bus with [`Connection::session`], or to the
/// system bus with [`Connection::system`]. Then the connection is used with [`crate::Proxy`]
/// instances or the on-demand [`ObjectServer`] instance that can be accessed through
/// [`Connection::object_server`].
///
/// `Connection` implements [`Clone`] and cloning it is a very cheap operation, as the underlying
/// data is not cloned. This makes it very convenient to share the connection between different
/// parts of your code. `Connection` also implements [`std::marker::Sync`] and [`std::marker::Send`]
/// so you can send and share a connection instance across threads as well.
///
/// `Connection` keeps internal queues of incoming message. The default capacity of each of these is
/// 64. The capacity of the main (unfiltered) queue is configurable through the [`set_max_queued`]
/// method. When the queue is full, no more messages can be received until room is created for more.
/// This is why it's important to ensure that all [`crate::MessageStream`] and
/// [`crate::blocking::MessageIterator`] instances are continuously polled and iterated on,
/// respectively.
///
/// For sending messages you can either use [`Connection::send`] method.
///
/// [method calls]: struct.Connection.html#method.call_method
/// [signals]: struct.Connection.html#method.emit_signal
/// [`proxy`]: attr.proxy.html
/// [`interface`]: attr.interface.html
/// [`Clone`]: https://doc.rust-lang.org/std/clone/trait.Clone.html
/// [`set_max_queued`]: struct.Connection.html#method.set_max_queued
///
/// ### Examples
///
/// #### Get the session bus ID
///
/// ```
/// # zbus::block_on(async {
/// use zbus::Connection;
///
/// let connection = Connection::session().await?;
///
/// let reply_body = connection
///     .call_method(
///         Some("org.freedesktop.DBus"),
///         "/org/freedesktop/DBus",
///         Some("org.freedesktop.DBus"),
///         "GetId",
///         &(),
///     )
///     .await?
///     .body();
///
/// let id: &str = reply_body.deserialize()?;
/// println!("Unique ID of the bus: {}", id);
/// # Ok::<(), zbus::Error>(())
/// # }).unwrap();
/// ```
///
/// #### Monitoring all messages
///
/// Let's eavesdrop on the session bus 😈 using the [Monitor] interface:
///
/// ```rust,no_run
/// # zbus::block_on(async {
/// use futures_util::stream::TryStreamExt;
/// use zbus::{Connection, MessageStream};
///
/// let connection = Connection::session().await?;
///
/// connection
///     .call_method(
///         Some("org.freedesktop.DBus"),
///         "/org/freedesktop/DBus",
///         Some("org.freedesktop.DBus.Monitoring"),
///         "BecomeMonitor",
///         &(&[] as &[&str], 0u32),
///     )
///     .await?;
///
/// let mut stream = MessageStream::from(connection);
/// while let Some(msg) = stream.try_next().await? {
///     println!("Got message: {}", msg);
/// }
///
/// # Ok::<(), zbus::Error>(())
/// # }).unwrap();
/// ```
///
/// This should print something like:
///
/// ```console
/// Got message: Signal NameAcquired from org.freedesktop.DBus
/// Got message: Signal NameLost from org.freedesktop.DBus
/// Got message: Method call GetConnectionUnixProcessID from :1.1324
/// Got message: Error org.freedesktop.DBus.Error.NameHasNoOwner:
///              Could not get PID of name ':1.1332': no such name from org.freedesktop.DBus
/// Got message: Method call AddMatch from :1.918
/// Got message: Method return from org.freedesktop.DBus
/// ```
///
/// [Monitor]: https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-become-monitor
#[derive(Clone, Debug)]
#[must_use = "Dropping a `Connection` will close the underlying socket."]
pub struct Connection {
    pub(crate) inner: Arc<ConnectionInner>,
}

assert_impl_all!(Connection: Send, Sync, Unpin);

/// A method call whose completion can be awaited or joined with other streams.
///
/// This is useful for cache population method calls, where joining the [`JoinableStream`] with
/// an update signal stream can be used to ensure that cache updates are not overwritten by a cache
/// population whose task is scheduled later.
#[derive(Debug)]
pub(crate) struct PendingMethodCall {
    stream: Option<MessageStream>,
    serial: NonZeroU32,
}

impl Future for PendingMethodCall {
    type Output = Result<Message>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_before(cx, None).map(|ret| {
            ret.map(|(_, r)| r).unwrap_or_else(|| {
                Err(crate::Error::InputOutput(
                    io::Error::new(ErrorKind::BrokenPipe, "socket closed").into(),
                ))
            })
        })
    }
}

impl OrderedFuture for PendingMethodCall {
    type Output = Result<Message>;
    type Ordering = zbus::message::Sequence;

    fn poll_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<Option<(Self::Ordering, Self::Output)>> {
        let this = self.get_mut();
        if let Some(stream) = &mut this.stream {
            loop {
                match Pin::new(&mut *stream).poll_next_before(cx, before) {
                    Poll::Ready(PollResult::Item {
                        data: Ok(msg),
                        ordering,
                    }) => {
                        if msg.header().reply_serial() != Some(this.serial) {
                            continue;
                        }
                        let res = match msg.message_type() {
                            Type::Error => Err(msg.into()),
                            Type::MethodReturn => Ok(msg),
                            _ => continue,
                        };
                        this.stream = None;
                        return Poll::Ready(Some((ordering, res)));
                    }
                    Poll::Ready(PollResult::Item {
                        data: Err(e),
                        ordering,
                    }) => {
                        return Poll::Ready(Some((ordering, Err(e))));
                    }

                    Poll::Ready(PollResult::NoneBefore) => {
                        return Poll::Ready(None);
                    }
                    Poll::Ready(PollResult::Terminated) => {
                        return Poll::Ready(None);
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
        Poll::Ready(None)
    }
}

impl Connection {
    /// Send `msg` to the peer.
    pub async fn send(&self, msg: &Message) -> Result<()> {
        #[cfg(unix)]
        if !msg.data().fds().is_empty() && !self.inner.cap_unix_fd {
            return Err(Error::Unsupported);
        }

        self.inner.activity_event.notify(usize::MAX);
        let mut write = self.inner.socket_write.lock().await;

        write.send_message(msg).await
    }

    /// Send a method call.
    ///
    /// Create a method-call message, send it over the connection, then wait for the reply.
    ///
    /// On successful reply, an `Ok(Message)` is returned. On error, an `Err` is returned. D-Bus
    /// error replies are returned as [`Error::MethodError`].
    pub async fn call_method<'d, 'p, 'i, 'm, D, P, I, M, B>(
        &self,
        destination: Option<D>,
        path: P,
        interface: Option<I>,
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
        self.call_method_raw(
            destination,
            path,
            interface,
            method_name,
            BitFlags::empty(),
            body,
        )
        .await?
        .expect("no reply")
        .await
    }

    /// Send a method call.
    ///
    /// Send the given message, which must be a method call, over the connection and return an
    /// object that allows the reply to be retrieved.  Typically you'd want to use
    /// [`Connection::call_method`] instead.
    ///
    /// If the `flags` do not contain `MethodFlags::NoReplyExpected`, the return value is
    /// guaranteed to be `Ok(Some(_))`, if there was no error encountered.
    ///
    /// INTERNAL NOTE: If this method is ever made pub, flags should become `BitFlags<MethodFlags>`.
    pub(crate) async fn call_method_raw<'d, 'p, 'i, 'm, D, P, I, M, B>(
        &self,
        destination: Option<D>,
        path: P,
        interface: Option<I>,
        method_name: M,
        flags: BitFlags<Flags>,
        body: &B,
    ) -> Result<Option<PendingMethodCall>>
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
        let _permit = acquire_serial_num_semaphore().await;

        let mut builder = Message::method(path, method_name)?;
        if let Some(sender) = self.unique_name() {
            builder = builder.sender(sender)?
        }
        if let Some(destination) = destination {
            builder = builder.destination(destination)?
        }
        if let Some(interface) = interface {
            builder = builder.interface(interface)?
        }
        for flag in flags {
            builder = builder.with_flags(flag)?;
        }
        let msg = builder.build(body)?;

        let msg_receiver = self.inner.method_return_receiver.activate_cloned();
        let stream = Some(MessageStream::for_subscription_channel(
            msg_receiver,
            // This is a lie but we only use the stream internally so it's fine.
            None,
            self,
        ));
        let serial = msg.primary_header().serial_num();
        self.send(&msg).await?;
        if flags.contains(Flags::NoReplyExpected) {
            Ok(None)
        } else {
            Ok(Some(PendingMethodCall { stream, serial }))
        }
    }

    /// Emit a signal.
    ///
    /// Create a signal message, and send it over the connection.
    pub async fn emit_signal<'d, 'p, 'i, 'm, D, P, I, M, B>(
        &self,
        destination: Option<D>,
        path: P,
        interface: I,
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
        let _permit = acquire_serial_num_semaphore().await;

        let mut b = Message::signal(path, interface, signal_name)?;
        if let Some(sender) = self.unique_name() {
            b = b.sender(sender)?;
        }
        if let Some(destination) = destination {
            b = b.destination(destination)?;
        }
        let m = b.build(body)?;

        self.send(&m).await
    }

    /// Reply to a message.
    ///
    /// Given an existing message (likely a method call), send a reply back to the caller with the
    /// given `body`.
    pub async fn reply<B>(&self, call: &Message, body: &B) -> Result<()>
    where
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        let _permit = acquire_serial_num_semaphore().await;

        let mut b = Message::method_reply(call)?;
        if let Some(sender) = self.unique_name() {
            b = b.sender(sender)?;
        }
        let m = b.build(body)?;
        self.send(&m).await
    }

    /// Reply an error to a message.
    ///
    /// Given an existing message (likely a method call), send an error reply back to the caller
    /// with the given `error_name` and `body`.
    pub async fn reply_error<'e, E, B>(&self, call: &Message, error_name: E, body: &B) -> Result<()>
    where
        B: serde::ser::Serialize + zvariant::DynamicType,
        E: TryInto<ErrorName<'e>>,
        E::Error: Into<Error>,
    {
        let _permit = acquire_serial_num_semaphore().await;

        let mut b = Message::method_error(call, error_name)?;
        if let Some(sender) = self.unique_name() {
            b = b.sender(sender)?;
        }
        let m = b.build(body)?;
        self.send(&m).await
    }

    /// Reply an error to a message.
    ///
    /// Given an existing message (likely a method call), send an error reply back to the caller
    /// using one of the standard interface reply types.
    pub async fn reply_dbus_error(
        &self,
        call: &zbus::message::Header<'_>,
        err: impl DBusError,
    ) -> Result<()> {
        let _permit = acquire_serial_num_semaphore().await;

        let m = err.create_reply(call)?;
        self.send(&m).await
    }

    /// Register a well-known name for this connection.
    ///
    /// When connecting to a bus, the name is requested from the bus. In case of p2p connection, the
    /// name (if requested) is used of self-identification.
    ///
    /// You can request multiple names for the same connection. Use [`Connection::release_name`] for
    /// deregistering names registered through this method.
    ///
    /// Note that exclusive ownership without queueing is requested (using
    /// [`RequestNameFlags::ReplaceExisting`] and [`RequestNameFlags::DoNotQueue`] flags) since that
    /// is the most typical case. If that is not what you want, you should use
    /// [`Connection::request_name_with_flags`] instead (but make sure then that name is requested
    /// **after** you've setup your service implementation with the `ObjectServer`).
    ///
    /// # Caveats
    ///
    /// The associated `ObjectServer` will only handle method calls destined for the unique name of
    /// this connection or any of the registered well-known names. If no well-known name is
    /// registered, the method calls destined to all well-known names will be handled.
    ///
    /// Since names registered through any other means than `Connection` or [`Builder`]
    /// API are not known to the connection, method calls destined to those names will only be
    /// handled by the associated `ObjectServer` if none of the names are registered through
    /// `Connection*` API. Simply put, either register all the names through `Connection*` API or
    /// none of them.
    ///
    /// # Errors
    ///
    /// Fails with `zbus::Error::NameTaken` if the name is already owned by another peer.
    pub async fn request_name<'w, W>(&self, well_known_name: W) -> Result<()>
    where
        W: TryInto<WellKnownName<'w>>,
        W::Error: Into<Error>,
    {
        self.request_name_with_flags(
            well_known_name,
            RequestNameFlags::ReplaceExisting | RequestNameFlags::DoNotQueue,
        )
        .await
        .map(|_| ())
    }

    /// Register a well-known name for this connection.
    ///
    /// This is the same as [`Connection::request_name`] but allows to specify the flags to use when
    /// requesting the name.
    ///
    /// If the [`RequestNameFlags::DoNotQueue`] flag is not specified and request ends up in the
    /// queue, you can use [`fdo::NameAcquiredStream`] to be notified when the name is acquired. A
    /// queued name request can be cancelled using [`Connection::release_name`].
    ///
    /// If the [`RequestNameFlags::AllowReplacement`] flag is specified, the requested name can be
    /// lost if another peer requests the same name. You can use [`fdo::NameLostStream`] to be
    /// notified when the name is lost
    ///
    /// # Example
    ///
    /// ```
    /// #
    /// # zbus::block_on(async {
    /// use zbus::{Connection, fdo::{DBusProxy, RequestNameFlags, RequestNameReply}};
    /// use enumflags2::BitFlags;
    /// use futures_util::stream::StreamExt;
    ///
    /// let name = "org.freedesktop.zbus.QueuedNameTest";
    /// let conn1 = Connection::session().await?;
    /// // This should just work right away.
    /// conn1.request_name(name).await?;
    ///
    /// let conn2 = Connection::session().await?;
    /// // A second request from the another connection will fail with `DoNotQueue` flag, which is
    /// // implicit with `request_name` method.
    /// assert!(conn2.request_name(name).await.is_err());
    ///
    /// // Now let's try w/o `DoNotQueue` and we should be queued.
    /// let reply = conn2
    ///     .request_name_with_flags(name, RequestNameFlags::AllowReplacement.into())
    ///     .await?;
    /// assert_eq!(reply, RequestNameReply::InQueue);
    /// // Another request should just give us the same response.
    /// let reply = conn2
    ///     // The flags on subsequent requests will however be ignored.
    ///     .request_name_with_flags(name, BitFlags::empty())
    ///     .await?;
    /// assert_eq!(reply, RequestNameReply::InQueue);
    /// let mut acquired_stream = DBusProxy::new(&conn2)
    ///     .await?
    ///     .receive_name_acquired()
    ///     .await?;
    /// assert!(conn1.release_name(name).await?);
    /// // This would have waited forever if `conn1` hadn't just release the name.
    /// let acquired = acquired_stream.next().await.unwrap();
    /// assert_eq!(acquired.args().unwrap().name, name);
    ///
    /// // conn2 made the mistake of being too nice and allowed name replacemnt, so conn1 should be
    /// // able to take it back.
    /// let mut lost_stream = DBusProxy::new(&conn2)
    ///     .await?
    ///     .receive_name_lost()
    ///     .await?;
    /// conn1.request_name(name).await?;
    /// let lost = lost_stream.next().await.unwrap();
    /// assert_eq!(lost.args().unwrap().name, name);
    ///
    /// # Ok::<(), zbus::Error>(())
    /// # }).unwrap();
    /// ```
    ///
    /// # Caveats
    ///
    /// * Same as that of [`Connection::request_name`].
    /// * If you wish to track changes to name ownership after this call, make sure that the
    /// [`fdo::NameAcquired`] and/or [`fdo::NameLostStream`] instance(s) are created **before**
    /// calling this method. Otherwise, you may loose the signal if it's emitted after this call but
    /// just before the stream instance get created.
    pub async fn request_name_with_flags<'w, W>(
        &self,
        well_known_name: W,
        flags: BitFlags<RequestNameFlags>,
    ) -> Result<RequestNameReply>
    where
        W: TryInto<WellKnownName<'w>>,
        W::Error: Into<Error>,
    {
        let well_known_name = well_known_name.try_into().map_err(Into::into)?;
        // We keep the lock until the end of this function so that the (possibly) spawned task
        // doesn't end up accessing the name entry before it's inserted.
        let mut names = self.inner.registered_names.lock().await;

        match names.get(&well_known_name) {
            Some(NameStatus::Owner(_)) => return Ok(RequestNameReply::AlreadyOwner),
            Some(NameStatus::Queued(_)) => return Ok(RequestNameReply::InQueue),
            None => (),
        }

        if !self.is_bus() {
            names.insert(well_known_name.to_owned(), NameStatus::Owner(None));

            return Ok(RequestNameReply::PrimaryOwner);
        }

        let dbus_proxy = fdo::DBusProxy::builder(self)
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        let mut acquired_stream = dbus_proxy.receive_name_acquired().await?;
        let mut lost_stream = dbus_proxy.receive_name_lost().await?;
        let reply = dbus_proxy
            .request_name(well_known_name.clone(), flags)
            .await?;
        let lost_task_name = format!("monitor name {well_known_name} lost");
        let name_lost_fut = if flags.contains(RequestNameFlags::AllowReplacement) {
            let weak_conn = WeakConnection::from(self);
            let well_known_name = well_known_name.to_owned();
            Some(
                async move {
                    loop {
                        let signal = lost_stream.next().await;
                        let inner = match weak_conn.upgrade() {
                            Some(conn) => conn.inner.clone(),
                            None => break,
                        };

                        match signal {
                            Some(signal) => match signal.args() {
                                Ok(args) if args.name == well_known_name => {
                                    tracing::info!(
                                        "Connection `{}` lost name `{}`",
                                        // SAFETY: This is bus connection so unique name can't be
                                        // None.
                                        inner.unique_name.get().unwrap(),
                                        well_known_name
                                    );
                                    inner.registered_names.lock().await.remove(&well_known_name);

                                    break;
                                }
                                Ok(_) => (),
                                Err(e) => warn!("Failed to parse `NameLost` signal: {}", e),
                            },
                            None => {
                                trace!("`NameLost` signal stream closed");
                                // This is a very strange state we end up in. Now the name is
                                // question remains in the queue
                                // forever. Maybe we can do better here but I
                                // think it's a very unlikely scenario anyway.
                                //
                                // Can happen if the connection is lost/dropped but then the whole
                                // `Connection` instance will go away soon anyway and hence this
                                // strange state along with it.
                                break;
                            }
                        }
                    }
                }
                .instrument(info_span!("{}", lost_task_name)),
            )
        } else {
            None
        };
        let status = match reply {
            RequestNameReply::InQueue => {
                let weak_conn = WeakConnection::from(self);
                let well_known_name = well_known_name.to_owned();
                let task_name = format!("monitor name {well_known_name} acquired");
                let task = self.executor().spawn(
                    async move {
                        loop {
                            let signal = acquired_stream.next().await;
                            let inner = match weak_conn.upgrade() {
                                Some(conn) => conn.inner.clone(),
                                None => break,
                            };
                            match signal {
                                Some(signal) => match signal.args() {
                                    Ok(args) if args.name == well_known_name => {
                                        let mut names = inner.registered_names.lock().await;
                                        if let Some(status) = names.get_mut(&well_known_name) {
                                            let task = name_lost_fut.map(|fut| {
                                                inner.executor.spawn(fut, &lost_task_name)
                                            });
                                            *status = NameStatus::Owner(task);

                                            break;
                                        }
                                        // else the name was released in the meantime. :shrug:
                                    }
                                    Ok(_) => (),
                                    Err(e) => warn!("Failed to parse `NameAcquired` signal: {}", e),
                                },
                                None => {
                                    trace!("`NameAcquired` signal stream closed");
                                    // See comment above for similar state in case of `NameLost`
                                    // stream.
                                    break;
                                }
                            }
                        }
                    }
                    .instrument(info_span!("{}", task_name)),
                    &task_name,
                );

                NameStatus::Queued(task)
            }
            RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {
                let task = name_lost_fut.map(|fut| self.executor().spawn(fut, &lost_task_name));

                NameStatus::Owner(task)
            }
            RequestNameReply::Exists => return Err(Error::NameTaken),
        };

        names.insert(well_known_name.to_owned(), status);

        Ok(reply)
    }

    /// Deregister a previously registered well-known name for this service on the bus.
    ///
    /// Use this method to deregister a well-known name, registered through
    /// [`Connection::request_name`].
    ///
    /// Unless an error is encountered, returns `Ok(true)` if name was previously registered with
    /// the bus through `self` and it has now been successfully deregistered, `Ok(false)` if name
    /// was not previously registered or already deregistered.
    pub async fn release_name<'w, W>(&self, well_known_name: W) -> Result<bool>
    where
        W: TryInto<WellKnownName<'w>>,
        W::Error: Into<Error>,
    {
        let well_known_name: WellKnownName<'w> = well_known_name.try_into().map_err(Into::into)?;
        let mut names = self.inner.registered_names.lock().await;
        // FIXME: Should be possible to avoid cloning/allocation here
        if names.remove(&well_known_name.to_owned()).is_none() {
            return Ok(false);
        };

        if !self.is_bus() {
            return Ok(true);
        }

        fdo::DBusProxy::builder(self)
            .cache_properties(CacheProperties::No)
            .build()
            .await?
            .release_name(well_known_name)
            .await
            .map(|_| true)
            .map_err(Into::into)
    }

    /// Checks if `self` is a connection to a message bus.
    ///
    /// This will return `false` for p2p connections. When the `p2p` feature is enabled, this will
    /// always return `true`.
    pub fn is_bus(&self) -> bool {
        #[cfg(feature = "p2p")]
        {
            self.inner.bus_conn
        }
        #[cfg(not(feature = "p2p"))]
        {
            true
        }
    }

    /// The unique name of the connection, if set/applicable.
    ///
    /// The unique name is assigned by the message bus or set manually using
    /// [`Connection::set_unique_name`].
    pub fn unique_name(&self) -> Option<&OwnedUniqueName> {
        self.inner.unique_name.get()
    }

    /// Sets the unique name of the connection (if not already set).
    ///
    /// This is mainly provided for bus implementations. All other users should not need to use this
    /// method. Hence why this method is only available when the `bus-impl` feature is enabled.
    ///
    /// # Panics
    ///
    /// This method panics if the unique name is already set. It will always panic if the connection
    /// is to a message bus as it's the bus that assigns peers their unique names.
    #[cfg(feature = "bus-impl")]
    pub fn set_unique_name<U>(&self, unique_name: U) -> Result<()>
    where
        U: TryInto<OwnedUniqueName>,
        U::Error: Into<Error>,
    {
        let name = unique_name.try_into().map_err(Into::into)?;
        self.set_unique_name_(name);

        Ok(())
    }

    /// The capacity of the main (unfiltered) queue.
    pub fn max_queued(&self) -> usize {
        self.inner.msg_receiver.capacity()
    }

    /// Set the capacity of the main (unfiltered) queue.
    pub fn set_max_queued(&mut self, max: usize) {
        self.inner.msg_receiver.clone().set_capacity(max);
    }

    /// The server's GUID.
    pub fn server_guid(&self) -> &OwnedGuid {
        &self.inner.server_guid
    }

    /// The underlying executor.
    ///
    /// When a connection is built with internal_executor set to false, zbus will not spawn a
    /// thread to run the executor. You're responsible to continuously [tick the executor][tte].
    /// Failure to do so will result in hangs.
    ///
    /// # Examples
    ///
    /// Here is how one would typically run the zbus executor through tokio's scheduler:
    ///
    /// ```
    /// # // Disable on windows because somehow it triggers a stack overflow there:
    /// # // https://gitlab.freedesktop.org/zeenix/zbus/-/jobs/34023494
    /// # #[cfg(not(target_os = "unix"))]
    /// # {
    /// use zbus::connection::Builder;
    /// use tokio::task::spawn;
    ///
    /// # struct SomeIface;
    /// #
    /// # #[zbus::interface]
    /// # impl SomeIface {
    /// # }
    /// #
    /// #[tokio::main]
    /// async fn main() {
    ///     let conn = Builder::session()
    ///         .unwrap()
    ///         .internal_executor(false)
    /// #         // This is only for testing a deadlock that used to happen with this combo.
    /// #         .serve_at("/some/iface", SomeIface)
    /// #         .unwrap()
    ///         .build()
    ///         .await
    ///         .unwrap();
    ///     {
    ///        let conn = conn.clone();
    ///        spawn(async move {
    ///            loop {
    ///                conn.executor().tick().await;
    ///            }
    ///        });
    ///     }
    ///
    ///     // All your other async code goes here.
    /// }
    /// # }
    /// ```
    ///
    /// **Note**: zbus 2.1 added support for tight integration with tokio. This means, if you use
    /// zbus with tokio, you do not need to worry about this at all. All you need to do is enable
    /// `tokio` feature. You should also disable the (default) `async-io` feature in your
    /// `Cargo.toml` to avoid unused dependencies. Also note that **prior** to zbus 3.0, disabling
    /// `async-io` was required to enable tight `tokio` integration.
    ///
    /// [tte]: https://docs.rs/async-executor/1.4.1/async_executor/struct.Executor.html#method.tick
    pub fn executor(&self) -> &Executor<'static> {
        &self.inner.executor
    }

    /// Get a reference to the associated [`ObjectServer`].
    ///
    /// The `ObjectServer` is created on-demand.
    ///
    /// **Note**: Once the `ObjectServer` is created, it will be replying to all method calls
    /// received on `self`. If you want to manually reply to method calls, do not use this
    /// method (or any of the `ObjectServer` related API).
    pub fn object_server(&self) -> impl Deref<Target = ObjectServer> + '_ {
        // FIXME: Maybe it makes sense after all to implement Deref<Target= ObjectServer> for
        // crate::ObjectServer instead of this wrapper?
        struct Wrapper<'a>(&'a blocking::ObjectServer);
        impl<'a> Deref for Wrapper<'a> {
            type Target = ObjectServer;

            fn deref(&self) -> &Self::Target {
                self.0.inner()
            }
        }

        Wrapper(self.sync_object_server(true, None))
    }

    pub(crate) fn sync_object_server(
        &self,
        start: bool,
        started_event: Option<Event>,
    ) -> &blocking::ObjectServer {
        self.inner
            .object_server
            .get_or_init(move || self.setup_object_server(start, started_event))
    }

    fn setup_object_server(
        &self,
        start: bool,
        started_event: Option<Event>,
    ) -> blocking::ObjectServer {
        if start {
            self.start_object_server(started_event);
        }

        blocking::ObjectServer::new(self)
    }

    #[instrument(skip(self))]
    pub(crate) fn start_object_server(&self, started_event: Option<Event>) {
        self.inner.object_server_dispatch_task.get_or_init(|| {
            trace!("starting ObjectServer task");
            let weak_conn = WeakConnection::from(self);

            let obj_server_task_name = "ObjectServer task";
            self.inner.executor.spawn(
                async move {
                    let mut stream = match weak_conn.upgrade() {
                        Some(conn) => {
                            let mut builder = MatchRule::builder().msg_type(Type::MethodCall);
                            if let Some(unique_name) = conn.unique_name() {
                                builder = builder.destination(&**unique_name).expect("unique name");
                            }
                            let rule = builder.build();
                            match conn.add_match(rule.into(), None).await {
                                Ok(stream) => stream,
                                Err(e) => {
                                    // Very unlikely but can happen I guess if connection is closed.
                                    debug!("Failed to create message stream: {}", e);

                                    return;
                                }
                            }
                        }
                        None => {
                            trace!("Connection is gone, stopping associated object server task");

                            return;
                        }
                    };
                    if let Some(started_event) = started_event {
                        started_event.notify(1);
                    }

                    trace!("waiting for incoming method call messages..");
                    while let Some(msg) = stream.next().await.and_then(|m| {
                        if let Err(e) = &m {
                            debug!("Error while reading from object server stream: {:?}", e);
                        }
                        m.ok()
                    }) {
                        if let Some(conn) = weak_conn.upgrade() {
                            let hdr = msg.header();
                            // If we're connected to a bus, skip the destination check as the
                            // server will only send us method calls destined to us.
                            if !conn.is_bus() {
                                match hdr.destination() {
                                    // Unique name is already checked by the match rule.
                                    Some(BusName::Unique(_)) | None => (),
                                    Some(BusName::WellKnown(dest)) => {
                                        let names = conn.inner.registered_names.lock().await;
                                        // destination doesn't matter if no name has been registered
                                        // (probably means the name is registered through external
                                        // means).
                                        if !names.is_empty() && !names.contains_key(dest) {
                                            trace!(
                                                "Got a method call for a different destination: {}",
                                                dest
                                            );

                                            continue;
                                        }
                                    }
                                }
                            }
                            let server = conn.object_server();
                            if let Err(e) = server.dispatch_call(&msg, &hdr).await {
                                debug!(
                                    "Error dispatching message. Message: {:?}, error: {:?}",
                                    msg, e
                                );
                            }
                        } else {
                            // If connection is completely gone, no reason to keep running the task
                            // anymore.
                            trace!("Connection is gone, stopping associated object server task");
                            break;
                        }
                    }
                }
                .instrument(info_span!("{}", obj_server_task_name)),
                obj_server_task_name,
            )
        });
    }

    pub(crate) async fn add_match(
        &self,
        rule: OwnedMatchRule,
        max_queued: Option<usize>,
    ) -> Result<Receiver<Result<Message>>> {
        use std::collections::hash_map::Entry;

        if self.inner.msg_senders.lock().await.is_empty() {
            // This only happens if socket reader task has errored out.
            return Err(Error::InputOutput(Arc::new(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Socket reader task has errored out",
            ))));
        }

        let mut subscriptions = self.inner.subscriptions.lock().await;
        let msg_type = rule.msg_type().unwrap_or(Type::Signal);
        match subscriptions.entry(rule.clone()) {
            Entry::Vacant(e) => {
                let max_queued = max_queued.unwrap_or(DEFAULT_MAX_QUEUED);
                let (sender, mut receiver) = broadcast(max_queued);
                receiver.set_await_active(false);
                if self.is_bus() && msg_type == Type::Signal {
                    fdo::DBusProxy::builder(self)
                        .cache_properties(CacheProperties::No)
                        .build()
                        .await?
                        .add_match_rule(e.key().inner().clone())
                        .await?;
                }
                e.insert((1, receiver.clone().deactivate()));
                self.inner
                    .msg_senders
                    .lock()
                    .await
                    .insert(Some(rule), sender);

                Ok(receiver)
            }
            Entry::Occupied(mut e) => {
                let (num_subscriptions, receiver) = e.get_mut();
                *num_subscriptions += 1;
                if let Some(max_queued) = max_queued {
                    if max_queued > receiver.capacity() {
                        receiver.set_capacity(max_queued);
                    }
                }

                Ok(receiver.activate_cloned())
            }
        }
    }

    pub(crate) async fn remove_match(&self, rule: OwnedMatchRule) -> Result<bool> {
        use std::collections::hash_map::Entry;
        let mut subscriptions = self.inner.subscriptions.lock().await;
        // TODO when it becomes stable, use HashMap::raw_entry and only require expr: &str
        // (both here and in add_match)
        let msg_type = rule.msg_type().unwrap_or(Type::Signal);
        match subscriptions.entry(rule) {
            Entry::Vacant(_) => Ok(false),
            Entry::Occupied(mut e) => {
                let rule = e.key().inner().clone();
                e.get_mut().0 -= 1;
                if e.get().0 == 0 {
                    if self.is_bus() && msg_type == Type::Signal {
                        fdo::DBusProxy::builder(self)
                            .cache_properties(CacheProperties::No)
                            .build()
                            .await?
                            .remove_match_rule(rule.clone())
                            .await?;
                    }
                    e.remove();
                    self.inner
                        .msg_senders
                        .lock()
                        .await
                        .remove(&Some(rule.into()));
                }
                Ok(true)
            }
        }
    }

    pub(crate) fn queue_remove_match(&self, rule: OwnedMatchRule) {
        let conn = self.clone();
        let task_name = format!("Remove match `{}`", *rule);
        let remove_match =
            async move { conn.remove_match(rule).await }.instrument(trace_span!("{}", task_name));
        self.inner.executor.spawn(remove_match, &task_name).detach()
    }

    pub(crate) async fn new(
        auth: Authenticated,
        #[allow(unused)] bus_connection: bool,
        executor: Executor<'static>,
    ) -> Result<Self> {
        #[cfg(unix)]
        let cap_unix_fd = auth.cap_unix_fd;

        macro_rules! create_msg_broadcast_channel {
            ($size:expr) => {{
                let (msg_sender, msg_receiver) = broadcast($size);
                let mut msg_receiver = msg_receiver.deactivate();
                msg_receiver.set_await_active(false);

                (msg_sender, msg_receiver)
            }};
        }
        // The unfiltered message channel.
        let (msg_sender, msg_receiver) = create_msg_broadcast_channel!(DEFAULT_MAX_QUEUED);
        let mut msg_senders = HashMap::new();
        msg_senders.insert(None, msg_sender);

        // The special method return & error channel.
        let (method_return_sender, method_return_receiver) =
            create_msg_broadcast_channel!(DEFAULT_MAX_METHOD_RETURN_QUEUED);
        let rule = MatchRule::builder()
            .msg_type(Type::MethodReturn)
            .build()
            .into();
        msg_senders.insert(Some(rule), method_return_sender.clone());
        let rule = MatchRule::builder().msg_type(Type::Error).build().into();
        msg_senders.insert(Some(rule), method_return_sender);
        let msg_senders = Arc::new(Mutex::new(msg_senders));
        let subscriptions = Mutex::new(HashMap::new());

        let connection = Self {
            inner: Arc::new(ConnectionInner {
                activity_event: Arc::new(Event::new()),
                socket_write: Mutex::new(auth.socket_write),
                server_guid: auth.server_guid,
                #[cfg(unix)]
                cap_unix_fd,
                #[cfg(feature = "p2p")]
                bus_conn: bus_connection,
                unique_name: OnceLock::new(),
                subscriptions,
                object_server: OnceLock::new(),
                object_server_dispatch_task: OnceLock::new(),
                executor,
                socket_reader_task: OnceLock::new(),
                msg_senders,
                msg_receiver,
                method_return_receiver,
                registered_names: Mutex::new(HashMap::new()),
            }),
        };

        if let Some(unique_name) = auth.unique_name {
            connection.set_unique_name_(unique_name);
        }

        Ok(connection)
    }

    /// Create a `Connection` to the session/user message bus.
    pub async fn session() -> Result<Self> {
        Builder::session()?.build().await
    }

    /// Create a `Connection` to the system-wide message bus.
    pub async fn system() -> Result<Self> {
        Builder::system()?.build().await
    }

    /// Returns a listener, notified on various connection activity.
    ///
    /// This function is meant for the caller to implement idle or timeout on inactivity.
    pub fn monitor_activity(&self) -> EventListener {
        self.inner.activity_event.listen()
    }

    /// Returns the peer credentials.
    ///
    /// The fields are populated on the best effort basis. Some or all fields may not even make
    /// sense for certain sockets or on certain platforms and hence will be set to `None`.
    ///
    /// # Caveats
    ///
    /// Currently `unix_group_ids` and `linux_security_label` fields are not populated.
    pub async fn peer_credentials(&self) -> io::Result<ConnectionCredentials> {
        self.inner
            .socket_write
            .lock()
            .await
            .peer_credentials()
            .await
    }

    /// Close the connection.
    ///
    /// After this call, all reading and writing operations will fail.
    pub async fn close(self) -> Result<()> {
        self.inner.activity_event.notify(usize::MAX);
        self.inner
            .socket_write
            .lock()
            .await
            .close()
            .await
            .map_err(Into::into)
    }

    pub(crate) fn init_socket_reader(
        &self,
        socket_read: Box<dyn socket::ReadHalf>,
        already_read: Vec<u8>,
        #[cfg(unix)] already_received_fds: Vec<std::os::fd::OwnedFd>,
    ) {
        let inner = &self.inner;
        inner
            .socket_reader_task
            .set(
                SocketReader::new(
                    socket_read,
                    inner.msg_senders.clone(),
                    already_read,
                    #[cfg(unix)]
                    already_received_fds,
                    inner.activity_event.clone(),
                )
                .spawn(&inner.executor),
            )
            .expect("Attempted to set `socket_reader_task` twice");
    }

    fn set_unique_name_(&self, name: OwnedUniqueName) {
        self.inner
            .unique_name
            .set(name)
            // programmer (probably our) error if this fails.
            .expect("unique name already set");
    }
}

impl From<crate::blocking::Connection> for Connection {
    fn from(conn: crate::blocking::Connection) -> Self {
        conn.into_inner()
    }
}

// Internal API that allows keeping a weak connection ref around.
#[derive(Debug)]
pub(crate) struct WeakConnection {
    inner: Weak<ConnectionInner>,
}

impl WeakConnection {
    /// Upgrade to a Connection.
    pub fn upgrade(&self) -> Option<Connection> {
        self.inner.upgrade().map(|inner| Connection { inner })
    }
}

impl From<&Connection> for WeakConnection {
    fn from(conn: &Connection) -> Self {
        Self {
            inner: Arc::downgrade(&conn.inner),
        }
    }
}

#[derive(Debug)]
enum NameStatus {
    // The task waits for name lost signal if owner allows replacement.
    Owner(#[allow(unused)] Option<Task<()>>),
    // The task waits for name acquisition signal.
    Queued(#[allow(unused)] Task<()>),
}

static SERIAL_NUM_SEMAPHORE: Semaphore = Semaphore::new(1);

// Make message creation and sending an atomic operation, using an async
// semaphore if flatpak portal is detected to workaround an xdg-dbus-proxy issue:
//
// https://github.com/flatpak/xdg-dbus-proxy/issues/46
async fn acquire_serial_num_semaphore() -> Option<SemaphorePermit<'static>> {
    if is_flatpak() {
        Some(SERIAL_NUM_SEMAPHORE.acquire().await)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fdo::DBusProxy;
    use ntest::timeout;
    use test_log::test;

    #[cfg(windows)]
    #[test]
    fn connect_autolaunch_session_bus() {
        let addr =
            crate::win32::autolaunch_bus_address().expect("Unable to get session bus address");

        crate::block_on(async { addr.connect().await }).expect("Unable to connect to session bus");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn connect_launchd_session_bus() {
        use crate::address::{transport::Launchd, Address, Transport};
        crate::block_on(async {
            let addr = Address::from(Transport::Launchd(Launchd::new(
                "DBUS_LAUNCHD_SESSION_BUS_SOCKET",
            )));
            addr.connect().await
        })
        .expect("Unable to connect to session bus");
    }

    #[test]
    #[timeout(15000)]
    fn disconnect_on_drop() {
        // Reproducer for https://github.com/dbus2/zbus/issues/308 where setting up the
        // objectserver would cause the connection to not disconnect on drop.
        crate::utils::block_on(test_disconnect_on_drop());
    }

    async fn test_disconnect_on_drop() {
        #[derive(Default)]
        struct MyInterface {}

        #[crate::interface(name = "dev.peelz.FooBar.Baz")]
        impl MyInterface {
            fn do_thing(&self) {}
        }
        let name = "dev.peelz.foobar";
        let connection = Builder::session()
            .unwrap()
            .name(name)
            .unwrap()
            .serve_at("/dev/peelz/FooBar", MyInterface::default())
            .unwrap()
            .build()
            .await
            .unwrap();

        let connection2 = Connection::session().await.unwrap();
        let dbus = DBusProxy::new(&connection2).await.unwrap();
        let mut stream = dbus
            .receive_name_owner_changed_with_args(&[(0, name), (2, "")])
            .await
            .unwrap();

        drop(connection);

        // If the connection is not dropped, this will hang forever.
        stream.next().await.unwrap();

        // Let's still make sure the name is gone.
        let name_has_owner = dbus.name_has_owner(name.try_into().unwrap()).await.unwrap();
        assert!(!name_has_owner);
    }
}

#[cfg(feature = "p2p")]
#[cfg(test)]
mod p2p_tests {
    use futures_util::stream::TryStreamExt;
    use ntest::timeout;
    use test_log::test;
    use zvariant::{Endian, NATIVE_ENDIAN};

    use crate::{AuthMechanism, Guid};

    use super::*;

    // Same numbered client and server are already paired up.
    async fn test_p2p(
        server1: Connection,
        client1: Connection,
        server2: Connection,
        client2: Connection,
    ) -> Result<()> {
        let forward1 = {
            let stream = MessageStream::from(server1.clone());
            let sink = client2.clone();

            stream.try_for_each(move |msg| {
                let sink = sink.clone();
                async move { sink.send(&msg).await }
            })
        };
        let forward2 = {
            let stream = MessageStream::from(client2.clone());
            let sink = server1.clone();

            stream.try_for_each(move |msg| {
                let sink = sink.clone();
                async move { sink.send(&msg).await }
            })
        };
        let _forward_task = client1.executor().spawn(
            async move { futures_util::try_join!(forward1, forward2) },
            "forward_task",
        );

        let server_ready = Event::new();
        let server_ready_listener = server_ready.listen();
        let client_done = Event::new();
        let client_done_listener = client_done.listen();

        let server_future = async move {
            let mut stream = MessageStream::from(&server2);
            server_ready.notify(1);
            let method = loop {
                let m = stream.try_next().await?.unwrap();
                if m.to_string() == "Method call Test" {
                    assert_eq!(m.body().deserialize::<u64>().unwrap(), 64);
                    break m;
                }
            };

            // Send another message first to check the queueing function on client side.
            server2
                .emit_signal(None::<()>, "/", "org.zbus.p2p", "ASignalForYou", &())
                .await?;
            server2.reply(&method, &("yay")).await?;
            client_done_listener.await;

            Ok(())
        };

        let client_future = async move {
            let mut stream = MessageStream::from(&client1);
            server_ready_listener.await;
            // We want to set non-native endian to ensure that:
            // 1. the message is actually encoded with the specified endian.
            // 2. the server side is able to decode it and replies in the same encoding.
            let endian = match NATIVE_ENDIAN {
                Endian::Little => Endian::Big,
                Endian::Big => Endian::Little,
            };
            let method = Message::method("/", "Test")?
                .interface("org.zbus.p2p")?
                .endian(endian)
                .build(&64u64)?;
            client1.send(&method).await?;
            // Check we didn't miss the signal that was sent during the call.
            let m = stream.try_next().await?.unwrap();
            client_done.notify(1);
            assert_eq!(m.to_string(), "Signal ASignalForYou");
            let reply = stream.try_next().await?.unwrap();
            assert_eq!(reply.to_string(), "Method return");
            // Check if the reply was in the non-native endian.
            assert_eq!(Endian::from(reply.primary_header().endian_sig()), endian);
            reply.body().deserialize::<String>()
        };

        let (val, _) = futures_util::try_join!(client_future, server_future,)?;
        assert_eq!(val, "yay");

        Ok(())
    }

    #[test]
    #[timeout(15000)]
    fn tcp_p2p() {
        crate::utils::block_on(test_tcp_p2p()).unwrap();
    }

    async fn test_tcp_p2p() -> Result<()> {
        let (server1, client1) = tcp_p2p_pipe().await?;
        let (server2, client2) = tcp_p2p_pipe().await?;

        test_p2p(server1, client1, server2, client2).await
    }

    async fn tcp_p2p_pipe() -> Result<(Connection, Connection)> {
        let guid = Guid::generate();

        #[cfg(not(feature = "tokio"))]
        let (server_conn_builder, client_conn_builder) = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let p1 = std::net::TcpStream::connect(addr).unwrap();
            let p0 = listener.incoming().next().unwrap().unwrap();

            (
                Builder::tcp_stream(p0)
                    .server(guid)
                    .unwrap()
                    .p2p()
                    .auth_mechanism(AuthMechanism::Anonymous),
                Builder::tcp_stream(p1).p2p(),
            )
        };

        #[cfg(feature = "tokio")]
        let (server_conn_builder, client_conn_builder) = {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let p1 = tokio::net::TcpStream::connect(addr).await.unwrap();
            let p0 = listener.accept().await.unwrap().0;

            (
                Builder::tcp_stream(p0)
                    .server(guid)
                    .unwrap()
                    .p2p()
                    .auth_mechanism(AuthMechanism::Anonymous),
                Builder::tcp_stream(p1).p2p(),
            )
        };

        futures_util::try_join!(server_conn_builder.build(), client_conn_builder.build())
    }

    #[cfg(unix)]
    #[test]
    #[timeout(15000)]
    fn unix_p2p() {
        crate::utils::block_on(test_unix_p2p()).unwrap();
    }

    #[cfg(unix)]
    async fn test_unix_p2p() -> Result<()> {
        let (server1, client1) = unix_p2p_pipe().await?;
        let (server2, client2) = unix_p2p_pipe().await?;

        test_p2p(server1, client1, server2, client2).await
    }

    #[cfg(unix)]
    async fn unix_p2p_pipe() -> Result<(Connection, Connection)> {
        #[cfg(not(feature = "tokio"))]
        use std::os::unix::net::UnixStream;
        #[cfg(feature = "tokio")]
        use tokio::net::UnixStream;
        #[cfg(all(windows, not(feature = "tokio")))]
        use uds_windows::UnixStream;

        let guid = Guid::generate();

        let (p0, p1) = UnixStream::pair().unwrap();

        futures_util::try_join!(
            Builder::unix_stream(p1).p2p().build(),
            Builder::unix_stream(p0).server(guid).unwrap().p2p().build(),
        )
    }

    // Compile-test only since we don't have a VM setup to run this with/in.
    #[cfg(any(
        all(feature = "vsock", not(feature = "tokio")),
        feature = "tokio-vsock"
    ))]
    #[test]
    #[timeout(15000)]
    #[ignore]
    fn vsock_p2p() {
        crate::utils::block_on(test_vsock_p2p()).unwrap();
    }

    #[cfg(any(
        all(feature = "vsock", not(feature = "tokio")),
        feature = "tokio-vsock"
    ))]
    async fn test_vsock_p2p() -> Result<()> {
        let (server1, client1) = vsock_p2p_pipe().await?;
        let (server2, client2) = vsock_p2p_pipe().await?;

        test_p2p(server1, client1, server2, client2).await
    }

    #[cfg(all(feature = "vsock", not(feature = "tokio")))]
    async fn vsock_p2p_pipe() -> Result<(Connection, Connection)> {
        let guid = Guid::generate();

        let listener = vsock::VsockListener::bind_with_cid_port(vsock::VMADDR_CID_ANY, 42).unwrap();
        let addr = listener.local_addr().unwrap();
        let client = vsock::VsockStream::connect(&addr).unwrap();
        let server = listener.incoming().next().unwrap().unwrap();

        futures_util::try_join!(
            Builder::vsock_stream(server)
                .server(guid)
                .unwrap()
                .p2p()
                .auth_mechanism(AuthMechanism::Anonymous)
                .build(),
            Builder::vsock_stream(client).p2p().build(),
        )
    }

    #[cfg(feature = "tokio-vsock")]
    async fn vsock_p2p_pipe() -> Result<(Connection, Connection)> {
        let guid = Guid::generate();

        let listener = tokio_vsock::VsockListener::bind(2, 42).unwrap();
        let client = tokio_vsock::VsockStream::connect(3, 42).await.unwrap();
        let server = listener.incoming().next().await.unwrap().unwrap();

        futures_util::try_join!(
            Builder::vsock_stream(server)
                .server(guid)
                .unwrap()
                .p2p()
                .auth_mechanism(AuthMechanism::Anonymous)
                .build(),
            Builder::vsock_stream(client).p2p().build(),
        )
    }
    #[cfg(any(unix, not(feature = "tokio")))]
    #[test]
    #[timeout(15000)]
    fn unix_p2p_cookie_auth() {
        use crate::utils::block_on;
        use std::{
            fs::{create_dir_all, remove_file, write},
            time::{SystemTime as Time, UNIX_EPOCH},
        };
        #[cfg(unix)]
        use std::{
            fs::{set_permissions, Permissions},
            os::unix::fs::PermissionsExt,
        };
        use xdg_home::home_dir;

        let cookie_context = "zbus-test-cookie-context";
        let cookie_id = 123456789;
        let cookie = hex::encode(b"our cookie");

        // Ensure cookie directory exists.
        let cookie_dir = home_dir().unwrap().join(".dbus-keyrings");
        create_dir_all(&cookie_dir).unwrap();
        #[cfg(unix)]
        set_permissions(&cookie_dir, Permissions::from_mode(0o700)).unwrap();

        // Create a cookie file.
        let cookie_file = cookie_dir.join(cookie_context);
        let ts = Time::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let cookie_entry = format!("{cookie_id} {ts} {cookie}");
        write(&cookie_file, cookie_entry).unwrap();

        // Explicit cookie ID.
        let res1 = block_on(test_unix_p2p_cookie_auth(cookie_context, Some(cookie_id)));
        // Implicit cookie ID (first one should be picked).
        let res2 = block_on(test_unix_p2p_cookie_auth(cookie_context, None));

        // Remove the cookie file.
        remove_file(&cookie_file).unwrap();

        res1.unwrap();
        res2.unwrap();
    }

    #[cfg(any(unix, not(feature = "tokio")))]
    async fn test_unix_p2p_cookie_auth(
        cookie_context: &'static str,
        cookie_id: Option<usize>,
    ) -> Result<()> {
        #[cfg(all(unix, not(feature = "tokio")))]
        use std::os::unix::net::UnixStream;
        #[cfg(all(unix, feature = "tokio"))]
        use tokio::net::UnixStream;
        #[cfg(all(windows, not(feature = "tokio")))]
        use uds_windows::UnixStream;

        let guid = Guid::generate();

        let (p0, p1) = UnixStream::pair().unwrap();
        let mut server_builder = Builder::unix_stream(p0)
            .server(guid)
            .unwrap()
            .p2p()
            .auth_mechanism(AuthMechanism::Cookie)
            .cookie_context(cookie_context)
            .unwrap();
        if let Some(cookie_id) = cookie_id {
            server_builder = server_builder.cookie_id(cookie_id);
        }

        futures_util::try_join!(
            Builder::unix_stream(p1).p2p().build(),
            server_builder.build(),
        )
        .map(|_| ())
    }

    #[test]
    #[timeout(15000)]
    fn channel_pair() {
        crate::utils::block_on(test_channel_pair()).unwrap();
    }

    async fn test_channel_pair() -> Result<()> {
        let (server1, client1) = create_channel_pair().await;
        let (server2, client2) = create_channel_pair().await;

        test_p2p(server1, client1, server2, client2).await
    }

    async fn create_channel_pair() -> (Connection, Connection) {
        let (a, b) = socket::Channel::pair();

        let guid = crate::Guid::generate();
        let conn1 = Builder::authenticated_socket(a, guid.clone())
            .unwrap()
            .p2p()
            .build()
            .await
            .unwrap();
        let conn2 = Builder::authenticated_socket(b, guid)
            .unwrap()
            .p2p()
            .build()
            .await
            .unwrap();

        (conn1, conn2)
    }
}
