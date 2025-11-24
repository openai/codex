//! Async version of connection.
//!
//! This module requires the `futures` feature to be enabled.
//!
//! Current status:
//!  * Basic client functionality is up and running, i e, you can make method calls and
//!    receive incoming messages (e g signals).
//!  * As for server side code, you can use the `tree` module with this connection, but it does not
//!    support async method handlers.
//!
//! You're probably going to need a companion crate - dbus-tokio - for this connection to make sense.
//! (Although you can also just call read_write and process_all at regular intervals, and possibly
//! set a timeout handler.)


use crate::{Error, Message};
use crate::channel::{MatchingReceiver, Channel, Sender, Token};
use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::message::{MatchRule, MessageType};

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::{task, pin, mem};
use std::cell::RefCell;
use std::time::Duration;
use crate::filters::Filters;
use std::future::Future;
use std::time::Instant;
use std::collections::HashMap;


#[allow(missing_docs)]
mod generated_org_freedesktop_standard_interfaces;
#[allow(dead_code)]
mod generated_org_freedesktop_dbus;


/// This module contains some standard interfaces and an easy way to call them.
///
/// See the [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces) for more information about these standard interfaces.
///
/// The code was created by dbus-codegen.
pub mod stdintf {
    #[allow(missing_docs)]
    pub mod org_freedesktop_dbus {
        pub use super::super::generated_org_freedesktop_standard_interfaces::*;
        #[allow(unused_imports)]
        pub(crate) use super::super::generated_org_freedesktop_dbus::*;

        #[derive(Debug, PartialEq, Eq, Copy, Clone)]
        pub enum RequestNameReply {
            PrimaryOwner = 1,
            InQueue = 2,
            Exists = 3,
            AlreadyOwner = 4,
        }

        #[derive(Debug, PartialEq, Eq, Copy, Clone)]
        pub enum ReleaseNameReply {
            Released = 1,
            NonExistent = 2,
            NotOwner = 3,
        }

    }
}


type Replies<F> = HashMap<Token, F>;

/// A connection to D-Bus, thread local + async version
pub struct LocalConnection {
    channel: Channel,
    filters: RefCell<Filters<LocalFilterCb>>,
    replies: RefCell<Replies<LocalRepliesCb>>,
    timeout_maker: Option<TimeoutMakerCb>,
    waker: Option<WakerCb>,
    all_signal_matches: AtomicBool,
}

/// A connection to D-Bus, async version, which is Send but not Sync.
pub struct Connection {
    channel: Channel,
    filters: RefCell<Filters<FilterCb>>,
    replies: RefCell<Replies<RepliesCb>>,
    timeout_maker: Option<TimeoutMakerCb>,
    waker: Option<WakerCb>,
    all_signal_matches: AtomicBool,
}

/// A connection to D-Bus, Send + Sync + async version
pub struct SyncConnection {
    channel: Channel,
    filters: Mutex<Filters<SyncFilterCb>>,
    replies: Mutex<Replies<SyncRepliesCb>>,
    timeout_maker: Option<TimeoutMakerCb>,
    waker: Option<WakerCb>,
    all_signal_matches: AtomicBool,
}

use stdintf::org_freedesktop_dbus::DBus;

macro_rules! connimpl {
     ($c: ident, $cb: ident, $rcb: ident $(, $ss:tt)*) =>  {

type
    $cb = Box<dyn FnMut(Message, &$c) -> bool $(+ $ss)* + 'static>;
type
    $rcb = Box<dyn FnOnce(Message, &$c) $(+ $ss)* + 'static>;

impl From<Channel> for $c {
    fn from(x: Channel) -> Self {
        $c {
            channel: x,
            replies: Default::default(),
            filters: Default::default(),
            timeout_maker: None,
            waker: None,
            all_signal_matches: AtomicBool::new(false),
        }
    }
}

impl AsRef<Channel> for $c {
    fn as_ref(&self) -> &Channel { &self.channel }
}

impl Sender for $c {
    fn send(&self, msg: Message) -> Result<u32, ()> {
        let token = self.channel.send(msg);
        if self.channel.has_messages_to_send() {
            // Non-blocking send failed
            // Wake up task that will send the message
            if self.waker.as_ref().map(|wake| wake().is_err() ).unwrap_or(false) {
                return Err(());
            }
        }
        token
    }
}

impl MatchingReceiver for $c {
    type F = $cb;
    fn start_receive(&self, m: MatchRule<'static>, f: Self::F) -> Token {
        self.filters_mut().add(m, f)
    }
    fn stop_receive(&self, id: Token) -> Option<(MatchRule<'static>, Self::F)> {
        self.filters_mut().remove(id)
    }
}

impl NonblockReply for $c {
    type F = $rcb;
    fn send_with_reply(&self, msg: Message, f: Self::F) -> Result<Token, ()> {
        let token = {
            // We must hold the mutex from moment we send the message
            // To moment we set a handler for the reply
            // So reply can't arrive before we set handler
            let mut replies = self.replies_mut();
            self.channel.send(msg).map(|x| {
                let t = Token(x as usize);
                replies.insert(t, f);
                t
            })
        };
        if self.channel.has_messages_to_send() {
            // Non-blocking send failed
            // Wake up task that will send the message
            if self.waker.as_ref().map(|wake| wake().is_err() ).unwrap_or(false) {
                return Err(());
            }
        }
        token
    }
    fn cancel_reply(&self, id: Token) -> Option<Self::F> { self.replies_mut().remove(&id) }
    fn make_f<G: FnOnce(Message, &Self) + Send + 'static>(g: G) -> Self::F { Box::new(g) }
    fn timeout_maker(&self) -> Option<TimeoutMakerCb> { self.timeout_maker }
    fn set_timeout_maker(&mut self, f: Option<TimeoutMakerCb>) -> Option<TimeoutMakerCb> {
        mem::replace(&mut self.timeout_maker, f)
    }
    fn set_waker(&mut self, f: Option<WakerCb>) -> Option<WakerCb> {
        mem::replace(&mut self.waker, f)
    }
}


impl Process for $c {
    fn process_one(&self, msg: Message) {
        if let Some(serial) = msg.get_reply_serial() {
            if let Some(f) = self.replies_mut().remove(&Token(serial as usize)) {
                f(msg, self);
                return;
            }
        }
        if self.all_signal_matches.load(Ordering::Acquire) && msg.msg_type() == MessageType::Signal {
            // If it's a signal and the mode is enabled, send a copy of the message to all
            // matching filters.
            let matching_filters = self.filters_mut().remove_all_matching(&msg);
            // `matching_filters` needs to be a separate variable and not inlined here, because if
            // it's inline then the `MutexGuard` will live too long and we'll get a deadlock on the
            // next call to `filters_mut()` below.
            for mut ff in matching_filters {
                if let Ok(copy) = msg.duplicate() {
                    if ff.2(copy, self) {
                        self.filters_mut().insert(ff);
                    }
                } else {
                    // Silently drop the message, but add the filter back.
                    self.filters_mut().insert(ff);
                }
            }
        } else {
            // Otherwise, send the original message to only the first matching filter.
            let ff = self.filters_mut().remove_first_matching(&msg);
            if let Some(mut ff) = ff {
                if ff.2(msg, self) {
                    self.filters_mut().insert(ff);
                }
            } else if let Some(reply) = crate::channel::default_reply(&msg) {
                let _ = self.channel.send(reply);
            }
        }
    }
}

impl $c {
    fn dbus_proxy(&self) -> Proxy<&Self> {
        Proxy::new("org.freedesktop.DBus", "/org/freedesktop/DBus", Duration::from_secs(10), self)
    }

    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> BusName { self.channel.unique_name().unwrap().into() }

    /// Request a name on the D-Bus.
    ///
    /// For detailed information on the flags and return values, see the libdbus documentation.
    pub async fn request_name<'a, N: Into<BusName<'a>>>(&self, name: N, allow_replacement: bool, replace_existing: bool, do_not_queue: bool)
    -> Result<stdintf::org_freedesktop_dbus::RequestNameReply, Error> {
        let flags: u32 =
            if allow_replacement { 1 } else { 0 } +
            if replace_existing { 2 } else { 0 } +
            if do_not_queue { 4 } else { 0 };
        let r = self.dbus_proxy().request_name(&name.into(), flags).await?;
        use stdintf::org_freedesktop_dbus::RequestNameReply::*;
        let all = [PrimaryOwner, InQueue, Exists, AlreadyOwner];
        all.iter().find(|x| **x as u32 == r).copied().ok_or_else(||
            crate::Error::new_failed("Invalid reply from DBus server")
        )
    }

    /// Release a previously requested name on the D-Bus.
    pub async fn release_name<'a, N: Into<BusName<'a>>>(&self, name: N) -> Result<stdintf::org_freedesktop_dbus::ReleaseNameReply, Error> {
        let r = self.dbus_proxy().release_name(&name.into()).await?;
        use stdintf::org_freedesktop_dbus::ReleaseNameReply::*;
        let all = [Released, NonExistent, NotOwner];
        all.iter().find(|x| **x as u32 == r).copied().ok_or_else(||
            crate::Error::new_failed("Invalid reply from DBus server")
        )
    }

    /// Adds a new match to the connection, and sets up a callback when this message arrives.
    ///
    /// If multiple [`MatchRule`]s match the same message, then by default only the first will get
    /// the callback. This behaviour can be changed for signal messages by calling
    /// [`set_signal_match_mode`](Self::set_signal_match_mode).
    ///
    /// The returned value can be used to remove the match.
    pub async fn add_match(&self, match_rule: MatchRule<'static>) -> Result<MsgMatch, Error> {
        let m = match_rule.match_str();
        self.add_match_no_cb(&m).await?;
        let mi = Arc::new(MatchInner {
            token: Default::default(),
            cb: Default::default(),
        });
        let mi_weak = Arc::downgrade(&mi);
        let token = self.start_receive(match_rule, Box::new(move |msg, _| {
            mi_weak.upgrade().map(|mi| mi.incoming(msg)).unwrap_or(false)
        }));
        mi.token.store(token.0, Ordering::SeqCst);
        Ok(MsgMatch(mi))
    }


    /// Adds a new match to the connection, without setting up a callback when this message arrives.
    pub async fn add_match_no_cb(&self, match_str: &str) -> Result<(), Error> {
        self.dbus_proxy().add_match(match_str).await
    }

    /// Removes a match from the connection, without removing any callbacks.
    pub async fn remove_match_no_cb(&self, match_str: &str) -> Result<(), Error> {
        self.dbus_proxy().remove_match(match_str).await
    }

    /// Removes a previously added match and callback from the connection.
    pub async fn remove_match(&self, id: Token) -> Result<(), Error> {
        let (mr, _) = self.stop_receive(id).ok_or_else(|| Error::new_failed("No match with that id found"))?;
        self.remove_match_no_cb(&mr.match_str()).await
    }

    /// If true, configures the connection to send signal messages to all matching [`MatchRule`]
    /// filters added with [`add_match`](Self::add_match) rather than just the first one. This comes
    /// with the following gotchas:
    ///
    ///  * The messages might be duplicated, so the message serial might be lost (this is
    ///    generally not a problem for signals).
    ///  * Panicking inside a match callback might mess with other callbacks, causing them
    ///    to be permanently dropped.
    ///  * Removing other matches from inside a match callback is not supported.
    ///
    /// This is false by default, for a newly-created connection.
    pub fn set_signal_match_mode(&self, match_all: bool) {
        self.all_signal_matches.store(match_all, Ordering::Release);
    }
}


    }
}

connimpl!(Connection, FilterCb, RepliesCb, Send);
connimpl!(LocalConnection, LocalFilterCb, LocalRepliesCb);
connimpl!(SyncConnection, SyncFilterCb, SyncRepliesCb, Send);

impl Connection {
    fn filters_mut(&self) -> std::cell::RefMut<Filters<FilterCb>> { self.filters.borrow_mut() }
    fn replies_mut(&self) -> std::cell::RefMut<Replies<RepliesCb>> { self.replies.borrow_mut() }
}

impl LocalConnection {
    fn filters_mut(&self) -> std::cell::RefMut<Filters<LocalFilterCb>> { self.filters.borrow_mut() }
    fn replies_mut(&self) -> std::cell::RefMut<Replies<LocalRepliesCb>> { self.replies.borrow_mut() }
}

impl SyncConnection {
    fn filters_mut(&self) -> std::sync::MutexGuard<Filters<SyncFilterCb>> { self.filters.lock().unwrap() }
    fn replies_mut(&self) -> std::sync::MutexGuard<Replies<SyncRepliesCb>> { self.replies.lock().unwrap() }
}

/// Internal callback for the executor when a timeout needs to be made.
pub type TimeoutMakerCb = fn(timeout: Instant) -> pin::Pin<Box<dyn Future<Output=()> + Send + Sync + 'static>>;

/// Internal callback for the executor when we need wakeup a task
pub type WakerCb = Box<dyn Fn() -> Result<(), ()> + Send + Sync +'static>;

/// Internal helper trait for async method replies.
pub trait NonblockReply {
    /// Callback type
    type F;
    /// Sends a message and calls the callback when a reply is received.
    fn send_with_reply(&self, msg: Message, f: Self::F) -> Result<Token, ()>;
    /// Cancels a pending reply.
    fn cancel_reply(&self, id: Token) -> Option<Self::F>;
    /// Internal helper function that creates a callback.
    fn make_f<G: FnOnce(Message, &Self) + Send + 'static>(g: G) -> Self::F where Self: Sized;
    /// Set the internal timeout maker
    fn set_timeout_maker(&mut self, f: Option<TimeoutMakerCb>) -> Option<TimeoutMakerCb>;
    /// Get the internal timeout maker
    fn timeout_maker(&self) -> Option<TimeoutMakerCb>;
    /// Set the wakeup call
    fn set_waker(&mut self, f: Option<WakerCb>) -> Option<WakerCb>;
}


/// Internal helper trait, implemented for connections that process incoming messages.
pub trait Process: Sender + AsRef<Channel> {
    /// Dispatches all pending messages, without blocking.
    ///
    /// This is usually called from the reactor only, after read_write.
    /// Despite this taking &self and not "&mut self", it is a logic error to call this
    /// recursively or from more than one thread at a time.
    fn process_all(&self) {
        let c: &Channel = self.as_ref();
        while let Some(msg) = c.pop_message() {
            self.process_one(msg);
        }
    }

    /// Dispatches a message.
    fn process_one(&self, msg: Message);
}

/// A struct used to handle incoming matches
///
/// Note: Due to the lack of async destructors, please call Connection.remove_match()
/// in order to properly stop matching (instead of just dropping this struct).
pub struct MsgMatch(Arc<MatchInner>);

struct MatchInner {
    token: AtomicUsize,
    cb: Mutex<Option<Box<dyn FnMut(Message) -> bool + Send>>>,
}

impl MatchInner {
    fn incoming(&self, msg: Message) -> bool {
        if let Some(ref mut cb) = self.cb.lock().unwrap().as_mut() {
            cb(msg)
        }
        else { true }
    }
}

impl MsgMatch {
    /// Configures the match to receive a synchronous callback with only a message parameter.
    pub fn msg_cb<F: FnMut(Message) -> bool + Send + 'static>(self, f: F) -> Self {
        {
            let mut cb = self.0.cb.lock().unwrap();
            *cb = Some(Box::new(f));
        }
        self
    }

    /// Configures the match to receive a synchronous callback with a message parameter and typed
    /// message arguments.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mr = MatchRule::new_signal("com.example.dbustest", "HelloHappened");
    /// let incoming_signal = connection.add_match(mr).await?.cb(|_, (source,): (String,)| {
    ///    println!("Hello from {} happened on the bus!", source);
    ///    true
    /// });
    /// ```
    pub fn cb<R: ReadAll, F: FnMut(Message, R) -> bool + Send + 'static>(self, mut f: F) -> Self {
        self.msg_cb(move |msg| {
            if let Ok(r) = R::read(&mut msg.iter_init()) {
                f(msg, r)
            } else { true }
        })
    }

    /// Configures the match to receive a stream of messages.
    ///
    /// Note: If the receiving end is disconnected and a message is received,
    /// the message matching will end but not in a clean fashion. Call remove_match() to
    /// stop matching cleanly.
    pub fn msg_stream(self) -> (Self, futures_channel::mpsc::UnboundedReceiver<Message>) {
        let (sender, receiver) = futures_channel::mpsc::unbounded();
        (self.msg_cb(move |msg| {
            sender.unbounded_send(msg).is_ok()
        }), receiver)
    }

    /// Configures the match to receive a stream of messages, parsed and ready.
    ///
    /// Note: If the receiving end is disconnected and a message is received,
    /// the message matching will end but not in a clean fashion. Call remove_match() to
    /// stop matching cleanly.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mr = MatchRule::new_signal("com.example.dbustest", "HelloHappened");
    /// let (incoming_signal, stream) = conn.add_match(mr).await?.stream();
    /// let stream = stream.for_each(|(_, (source,)): (_, (String,))| {
    ///    println!("Hello from {} happened on the bus!", source);
    ///    async {}
    /// });
    /// ```
    pub fn stream<R: ReadAll + Send + 'static>(self) -> (Self, futures_channel::mpsc::UnboundedReceiver<(Message, R)>) {
        let (sender, receiver) = futures_channel::mpsc::unbounded();
        (self.cb(move |msg, r| {
            sender.unbounded_send((msg, r)).is_ok()
        }), receiver)
    }

    /// The token retrieved can be used in a call to remove_match to stop matching on the data.
    pub fn token(&self) -> Token { Token(self.0.token.load(Ordering::SeqCst)) }
}

/// A struct that wraps a connection, destination and path.
///
/// A D-Bus "Proxy" is a client-side object that corresponds to a remote object on the server side.
/// Calling methods on the proxy object calls methods on the remote object.
/// Read more in the [D-Bus tutorial](https://dbus.freedesktop.org/doc/dbus-tutorial.html#proxies)
#[derive(Clone, Debug)]
pub struct Proxy<'a, C> {
    /// Destination, i e what D-Bus service you're communicating with
    pub destination: BusName<'a>,
    /// Object path on the destination
    pub path: Path<'a>,
    /// Some way to send and/or receive messages, non-blocking.
    pub connection: C,
    /// Timeout for method calls
    pub timeout: Duration,
}

impl<'a, C> Proxy<'a, C> {
    /// Creates a new proxy struct.
    pub fn new<D: Into<BusName<'a>>, P: Into<Path<'a>>>(dest: D, path: P, timeout: Duration, connection: C) -> Self {
        Proxy { destination: dest.into(), path: path.into(), timeout, connection }
    }
}

struct MRAwait {
    mrouter: MROuter,
    token: Result<Token, ()>,
    timeout: Instant,
    timeoutfn: Option<TimeoutMakerCb>
}

async fn method_call_await(mra: MRAwait) -> Result<Message, Error> {
    use futures_util::future;
    let MRAwait { mrouter, token, timeout, timeoutfn } = mra;
    if token.is_err() { return Err(Error::new_failed("Failed to send message")) };
    let timeout = if let Some(tfn) = timeoutfn { tfn(timeout) } else { Box::pin(future::pending()) };
    match future::select(mrouter, timeout).await {
        future::Either::Left((r, _)) => r,
        future::Either::Right(_) => Err(Error::new_custom("org.freedesktop.DBus.Error.Timeout", "Timeout waiting for reply")),
    }
}

impl<'a, T, C> Proxy<'a, C>
where
    T: NonblockReply,
    C: std::ops::Deref<Target=T>
{

    fn method_call_setup(&self, msg: Message) -> MRAwait {
        let mr = Arc::new(Mutex::new(MRInner::Neither));
        let mrouter = MROuter(mr.clone());
        let f = T::make_f(move |msg: Message, _: &T| {
            let mut inner = mr.lock().unwrap();
            let old = mem::replace(&mut *inner, MRInner::Ready(Ok(msg)));
            if let MRInner::Pending(waker) = old { waker.wake() }
        });

        let timeout = Instant::now() + self.timeout;
        let token = self.connection.send_with_reply(msg, f);
        let timeoutfn = self.connection.timeout_maker();
        MRAwait { mrouter, token, timeout, timeoutfn }
    }

    /// Make a method call using typed input argument, returns a future that resolves to the typed output arguments.
    pub fn method_call<'i, 'm, R: ReadAll + 'static, A: AppendAll, I: Into<Interface<'i>>, M: Into<Member<'m>>>(&self, i: I, m: M, args: A)
    -> MethodReply<R> {
        let mut msg = Message::method_call(&self.destination, &self.path, &i.into(), &m.into());
        args.append(&mut IterAppend::new(&mut msg));
        let mra = self.method_call_setup(msg);
        let r = method_call_await(mra);
        let r = futures_util::FutureExt::map(r, |r| -> Result<R, Error> { r.and_then(|rmsg| rmsg.read_all()) } );
        MethodReply::new(r)
    }
}

enum MRInner {
    Ready(Result<Message, Error>),
    Pending(task::Waker),
    Neither,
}

struct MROuter(Arc<Mutex<MRInner>>);

impl Future for MROuter {
    type Output = Result<Message, Error>;
    fn poll(self: pin::Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Self::Output> {
        let mut inner = self.0.lock().unwrap();
        let r = mem::replace(&mut *inner, MRInner::Neither);
        if let MRInner::Ready(r) = r { task::Poll::Ready(r) }
        else {
            *inner = MRInner::Pending(ctx.waker().clone());
            return task::Poll::Pending
        }
    }
}

/// Future method reply, used while waiting for a method call reply from the server.
pub struct MethodReply<T>(pin::Pin<Box<dyn Future<Output=Result<T, Error>> + Send + 'static>>);

impl<T> MethodReply<T> {
    /// Creates a new method reply from a future.
    fn new<Fut: Future<Output=Result<T, Error>> + Send + 'static>(fut: Fut) -> Self {
        MethodReply(Box::pin(fut))
    }
}

impl<T> Future for MethodReply<T> {
    type Output = Result<T, Error>;
    fn poll(mut self: pin::Pin<&mut Self>, ctx: &mut task::Context) -> task::Poll<Result<T, Error>> {
        self.0.as_mut().poll(ctx)
    }
}

impl<T: 'static> MethodReply<T> {
    /// Convenience combinator in case you want to post-process the result after reading it
    pub fn and_then<T2>(self, f: impl FnOnce(T) -> Result<T2, Error> + Send + Sync + 'static) -> MethodReply<T2> {
        MethodReply(Box::pin(async move {
            let x = self.0.await?;
            f(x)
        }))
    }
}

#[test]
fn test_conn_send_sync() {
    fn is_send<T: Send>() {}
    fn is_sync<T: Sync>() {}
    is_send::<Connection>();
    is_send::<SyncConnection>();
    is_sync::<SyncConnection>();
    is_send::<MsgMatch>();
}
