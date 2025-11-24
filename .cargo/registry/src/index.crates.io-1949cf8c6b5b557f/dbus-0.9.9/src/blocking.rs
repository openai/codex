//! Connections and proxies that make blocking method calls.


use crate::strings::{BusName, Path, Interface, Member};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::{channel, Error, Message};
use crate::message::{MatchRule, SignalArgs, MessageType};
use crate::channel::{Channel, BusType, Token};
use std::{cell::RefCell, time::Duration, sync::Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::filters::Filters;

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

        #[derive(Debug, PartialEq, Eq, Copy, Clone)]
        pub enum EmitsChangedSignal {
            True,
            Invalidates,
            Const,
            False,
        }

        pub (crate) fn request_name<S: crate::blocking::BlockingSender>(s: &S, name: &str, allow_replacement: bool, replace_existing: bool, do_not_queue: bool)
            -> Result<RequestNameReply, crate::Error> {
            let flags: u32 =
                if allow_replacement { 1 } else { 0 } +
                if replace_existing { 2 } else { 0 } +
                if do_not_queue { 4 } else { 0 };
            let proxy = super::proxy(s);
            use super::org_freedesktop::DBus;
            let r = proxy.request_name(name, flags)?;
            use RequestNameReply::*;
            let all = [PrimaryOwner, InQueue, Exists, AlreadyOwner];
            all.iter().find(|x| **x as u32 == r).copied().ok_or_else(||
                crate::Error::new_failed("Invalid reply from DBus server")
            )
        }

        pub (crate) fn release_name<S: crate::blocking::BlockingSender>(s: &S, name: &str)
            -> Result<ReleaseNameReply, crate::Error> {

            let proxy = super::proxy(s);
            use super::org_freedesktop::DBus;
            let r = proxy.release_name(name)?;
            use ReleaseNameReply::*;
            let all = [Released, NonExistent, NotOwner];
            all.iter().find(|x| **x as u32 == r).copied().ok_or_else(||
                crate::Error::new_failed("Invalid reply from DBus server")
            )
        }

        use crate::arg;
        impl PropertiesPropertiesChanged {
            pub fn add_prop<F: FnOnce() -> Box<dyn arg::RefArg>>(&mut self, prop_name: &str, emits: EmitsChangedSignal, f: F) -> bool {
                match emits {
                    EmitsChangedSignal::False => { false },
                    EmitsChangedSignal::Invalidates => {
                        if !self.invalidated_properties.iter().any(|x| x == prop_name) {
                            self.invalidated_properties.push(prop_name.into())
                        }
                        true
                    }
                    EmitsChangedSignal::True => {
                        let val = f();
                        self.changed_properties.insert(prop_name.into(), arg::Variant(val));
                        true
                    }
                    EmitsChangedSignal::Const => panic!("Called add_prop with EmitsChangedSignal::Const")
                }
            }
        }
    }

    // Not public yet, because of lack of named arguments
    pub (super) mod org_freedesktop {
        pub(crate) use super::super::generated_org_freedesktop_dbus::*;
    }

    pub (crate) fn proxy<C>(c: C) -> crate::blocking::Proxy<'static, C> {
        super::Proxy::new("org.freedesktop.DBus", "/org/freedesktop/DBus", std::time::Duration::from_millis(5000), c)
    }
}

/// A connection to D-Bus, thread local + non-async version
pub struct LocalConnection {
    channel: Channel,
    filters: RefCell<Filters<LocalFilterCb>>,
    all_signal_matches: AtomicBool,
}

/// A connection to D-Bus, non-async version where callbacks are Send but not Sync.
pub struct Connection {
    channel: Channel,
    filters: RefCell<Filters<FilterCb>>,
    all_signal_matches: AtomicBool,
}

/// A connection to D-Bus, Send + Sync + non-async version
pub struct SyncConnection {
    channel: Channel,
    filters: Mutex<Filters<SyncFilterCb>>,
    all_signal_matches: AtomicBool,
}

use crate::blocking::stdintf::org_freedesktop_dbus;

macro_rules! connimpl {
     ($c: ident, $cb: ident $(, $ss:tt)*) =>  {

type
    $cb = Box<dyn FnMut(Message, &$c) -> bool $(+ $ss)* + 'static>;


impl $c {

    /// Create a new connection to the session bus.
    pub fn new_session() -> Result<Self, Error> {
        Channel::get_private(BusType::Session).map(From::from)
    }

    /// Create a new connection to the system-wide bus.
    pub fn new_system() -> Result<Self, Error> {
        Channel::get_private(BusType::System).map(From::from)
    }

    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> BusName { self.channel.unique_name().unwrap().into() }

    /// Create a convenience struct for easier calling of many methods on the same destination and path.
    pub fn with_proxy<'a, 'b, D: Into<BusName<'a>>, P: Into<Path<'a>>>(&'b self, dest: D, path: P, timeout: Duration) ->
    Proxy<'a, &'b Self> {
        Proxy { connection: self, destination: dest.into(), path: path.into(), timeout }
    }


    /// Request a name on the D-Bus.
    ///
    /// For detailed information on the flags and return values, see the libdbus documentation.
    pub fn request_name<'a, N: Into<BusName<'a>>>(&self, name: N, allow_replacement: bool, replace_existing: bool, do_not_queue: bool)
    -> Result<org_freedesktop_dbus::RequestNameReply, Error> {
        org_freedesktop_dbus::request_name(&self.channel, &name.into(), allow_replacement, replace_existing, do_not_queue)
    }

    /// Release a previously requested name on the D-Bus.
    pub fn release_name<'a, N: Into<BusName<'a>>>(&self, name: N) -> Result<org_freedesktop_dbus::ReleaseNameReply, Error> {
        org_freedesktop_dbus::release_name(&self.channel, &name.into())
    }

    /// Adds a new match to the connection, and sets up a callback when this message arrives.
    ///
    /// If multiple [`MatchRule`]s match the same message, then by default only the first match will
	/// get the callback. This behaviour can be changed for signal messages by calling
	/// [`set_signal_match_mode`](Self::set_signal_match_mode).
    ///
    /// The returned value can be used to remove the match. The match is also removed if the callback
    /// returns "false".
    pub fn add_match<S: ReadAll, F>(&self, match_rule: MatchRule<'static>, f: F) -> Result<Token, Error>
    where F: FnMut(S, &Self, &Message) -> bool $(+ $ss)* + 'static {
        let m = match_rule.match_str();
        self.add_match_no_cb(&m)?;
        use channel::MatchingReceiver;
        Ok(self.start_receive(match_rule, MakeSignal::make(f, m)))
    }

    /// Adds a new match to the connection, without setting up a callback when this message arrives.
    pub fn add_match_no_cb(&self, match_str: &str) -> Result<(), Error> {
        use crate::blocking::stdintf::org_freedesktop::DBus;
        let proxy = stdintf::proxy(self);
        proxy.add_match(match_str)
    }

    /// Removes a match from the connection, without removing any callbacks.
    pub fn remove_match_no_cb(&self, match_str: &str) -> Result<(), Error> {
        use crate::blocking::stdintf::org_freedesktop::DBus;
        let proxy = stdintf::proxy(self);
        proxy.remove_match(match_str)
    }

    /// Removes a previously added match and callback from the connection.
    pub fn remove_match(&self, id: Token) -> Result<(), Error> {
        use channel::MatchingReceiver;
        let (mr, _) = self.stop_receive(id).ok_or_else(|| Error::new_failed("No match with that id found"))?;
        self.remove_match_no_cb(&mr.match_str())
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

    /// Tries to handle an incoming message if there is one. If there isn't one,
    /// it will wait up to timeout
    ///
    /// This method only takes "&self" instead of "&mut self", but it is a logic error to call
    /// it recursively and might lead to panics or deadlocks.
    ///
    /// For `SyncConnection`: It is also a logic error to call this method from one thread, while
    /// calling this or other methods from other threads. This can lead to messages being lost.
    ///
    /// Returns true when there was a message to process, and false when time out reached.
    pub fn process(&self, timeout: Duration) -> Result<bool, Error> {
        if let Some(msg) = self.channel.blocking_pop_message(timeout)? {
            if self.all_signal_matches.load(Ordering::Acquire) && msg.msg_type() == MessageType::Signal {
                // If it's a signal and the mode is enabled, send a copy of the message to all
                // matching filters.
                let matching_filters = self.filters_mut().remove_all_matching(&msg);
                // `matching_filters` needs to be a separate variable and not inlined here, because
                // if it's inline then the `MutexGuard` will live too long and we'll get a deadlock
                // on the next call to `filters_mut()` below.
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
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// The channel for this connection
    pub fn channel(&self) -> &Channel {
        &self.channel
    }
}

impl BlockingSender for $c {
    fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error> {
        self.channel.send_with_reply_and_block(msg, timeout)
    }
}

impl From<Channel> for $c {
    fn from(channel: Channel) -> $c { $c {
        channel,
        filters: Default::default(),
        all_signal_matches: AtomicBool::new(false),
    } }
}

impl channel::Sender for $c {
    fn send(&self, msg: Message) -> Result<u32, ()> { self.channel.send(msg) }
}

impl<S: ReadAll, F: FnMut(S, &$c, &Message) -> bool $(+ $ss)* + 'static> MakeSignal<$cb, S, $c> for F {
    fn make(mut self, mstr: String) -> $cb {
        Box::new(move |msg: Message, conn: &$c| {
            if let Ok(s) = S::read(&mut msg.iter_init()) {
                if self(s, conn, &msg) { return true };
                let proxy = stdintf::proxy(conn);
                use crate::blocking::stdintf::org_freedesktop::DBus;
                let _ = proxy.remove_match(&mstr);
                false
            } else { true }
        })
    }
}

impl channel::MatchingReceiver for $c {
    type F = $cb;
    fn start_receive(&self, m: MatchRule<'static>, f: Self::F) -> Token {
        self.filters_mut().add(m, f)
    }
    fn stop_receive(&self, id: Token) -> Option<(MatchRule<'static>, Self::F)> {
        self.filters_mut().remove(id)
    }
}



     }
}

connimpl!(Connection, FilterCb, Send);
connimpl!(LocalConnection, LocalFilterCb);
connimpl!(SyncConnection, SyncFilterCb, Send, Sync);

impl Connection {
    fn filters_mut(&self) -> std::cell::RefMut<Filters<FilterCb>> { self.filters.borrow_mut() }
}

impl LocalConnection {
    fn filters_mut(&self) -> std::cell::RefMut<Filters<LocalFilterCb>> { self.filters.borrow_mut() }
}

impl SyncConnection {
    fn filters_mut(&self) -> std::sync::MutexGuard<Filters<SyncFilterCb>> { self.filters.lock().unwrap() }
}

/// Abstraction over different connections
pub trait BlockingSender {
    /// Sends a message over the D-Bus and blocks, waiting for a reply or a timeout. This is used for method calls.
    ///
    /// Note: In case of an error reply, this is returned as an Err(), not as a Ok(Message) with the error type.
    fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error>;
}

impl BlockingSender for Channel {
    fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error> {
        Channel::send_with_reply_and_block(self, msg, timeout)
    }
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
    /// Timeout for method calls
    pub timeout: Duration,
    /// Some way to send and/or receive messages, either blocking or non-blocking.
    pub connection: C,
}

impl<'a, C> Proxy<'a, C> {
    /// Creates a new proxy struct.
    pub fn new<D: Into<BusName<'a>>, P: Into<Path<'a>>>(dest: D, path: P, timeout: Duration, connection: C) -> Self {
        Proxy { destination: dest.into(), path: path.into(), timeout, connection }
    }
}

impl<'a, T: BlockingSender, C: std::ops::Deref<Target=T>> Proxy<'a, C> {
// impl<'a, S: std::convert::AsRef<channel::Sender>> Proxy<'a, S> {
    /// Make a method call using typed input and output arguments, then block waiting for a reply.
    ///
    /// # Example
    ///
    /// ```
    /// use dbus::blocking::{Connection, Proxy};
    ///
    /// let conn = Connection::new_session()?;
    /// let proxy = Proxy::new("org.freedesktop.DBus", "/", std::time::Duration::from_millis(5000), &conn);
    /// let (has_owner,): (bool,) = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",))?;
    /// assert_eq!(has_owner, false);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn method_call<'i, 'm, R: ReadAll, A: AppendAll, I: Into<Interface<'i>>, M: Into<Member<'m>>>(&self, i: I, m: M, args: A) -> Result<R, Error> {
        let mut msg = Message::method_call(&self.destination, &self.path, &i.into(), &m.into());
        args.append(&mut IterAppend::new(&mut msg));
        let r = self.connection.send_with_reply_and_block(msg, self.timeout)?;
        Ok(R::read(&mut r.iter_init())?)
    }

    /// Starts matching incoming messages on this destination and path.
    ///
    /// For matching signals, match_signal might be more convenient.
    ///
    /// The match rule will be modified to include this path and destination only.
    ///
    /// If call_add_match is true, will notify the D-Bus server that matching should start.
    pub fn match_start(&self, mut mr: MatchRule<'static>, call_add_match: bool, f: <T as channel::MatchingReceiver>::F)
    -> Result<Token, Error>
    where T: channel::MatchingReceiver {
        mr.path = Some(self.path.clone().into_static());
        mr.sender = Some(self.destination.clone().into_static());
        if call_add_match {
            use crate::blocking::stdintf::org_freedesktop::DBus;
            let proxy = stdintf::proxy(&*self.connection);
            proxy.add_match(&mr.match_str())?;
        }

        Ok(self.connection.start_receive(mr, f))
    }

    /// Stops matching a signal added with match_start or match_signal.
    ///
    /// If call_remove_match is true, will notify the D-Bus server that matching should stop,
    /// this should be true in case match_signal was used.
    pub fn match_stop(&self, id: Token, call_remove_match: bool) -> Result<(), Error>
    where T: channel::MatchingReceiver {
        if let Some((mr, _)) = self.connection.stop_receive(id) {
            if call_remove_match {
                use crate::blocking::stdintf::org_freedesktop::DBus;
                let proxy = stdintf::proxy(&*self.connection);
                proxy.remove_match(&mr.match_str())?;
            }
        }
        Ok(())
    }

    /// Sets up an incoming signal match, that calls the supplied callback every time the signal is received.
    ///
    /// The returned value can be used to remove the match. The match is also removed if the callback
    /// returns "false".
    pub fn match_signal<S: SignalArgs + ReadAll, F>(&self, f: F) -> Result<Token, Error>
    where T: channel::MatchingReceiver,
          F: MakeSignal<<T as channel::MatchingReceiver>::F, S, T>
    {
        let mr = S::match_rule(Some(&self.destination), Some(&self.path)).static_clone();
        let ff = f.make(mr.match_str());
        self.match_start(mr, true, ff)
    }
}

/// Internal helper trait
pub trait MakeSignal<G, S, T> {
    /// Internal helper trait
    fn make(self, mstr: String) -> G;
}

#[test]
fn test_add_match() {
    use self::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as Ppc;
    let c = Connection::new_session().unwrap();
    let x = c.add_match(Ppc::match_rule(None, None), |_: Ppc, _, _| { true }).unwrap();
    c.remove_match(x).unwrap();
}

#[test]
fn test_conn_send_sync() {
    fn is_send<T: Send>(_: &T) {}
    fn is_sync<T: Sync>(_: &T) {}

    let c = SyncConnection::new_session().unwrap();
    is_send(&c);
    is_sync(&c);

    let c = Connection::new_session().unwrap();
    is_send(&c);
}

#[test]
fn test_peer() {
    let c = Connection::new_session().unwrap();

    let c_name = c.unique_name().into_static();
    use std::sync::Arc;
    let done = Arc::new(false);
    let d2 = done.clone();
    let j = std::thread::spawn(move || {
        let c2 = Connection::new_session().unwrap();

        let proxy = c2.with_proxy(c_name, "/", Duration::from_secs(5));
        let (s2,): (String,) = proxy.method_call("org.freedesktop.DBus.Peer", "GetMachineId", ()).unwrap();
        println!("{}", s2);
        assert_eq!(Arc::strong_count(&d2), 2);
        s2
    });
    assert_eq!(Arc::strong_count(&done), 2);

    for _ in 0..30 {
        c.process(Duration::from_millis(100)).unwrap();
        if Arc::strong_count(&done) < 2 { break; }
    }

    let s2 = j.join().unwrap();

    #[cfg(unix)]
    {
        let proxy = c.with_proxy("org.a11y.Bus", "/org/a11y/bus", Duration::from_secs(5));
        let (s1,): (String,) = proxy.method_call("org.freedesktop.DBus.Peer", "GetMachineId", ()).unwrap();

        assert_eq!(s1, s2);
    }

}
