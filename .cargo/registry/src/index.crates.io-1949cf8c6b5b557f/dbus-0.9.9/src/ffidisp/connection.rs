//! Contains structs and traits relevant to the connection itself, and dispatching incoming messages.

use crate::{Error, Message, MessageType, c_str_to_slice, channel::WatchFd, ffi, to_c_str};
use crate::ffidisp::ConnPath;
use std::{fmt, mem, ptr, thread, panic, ops};
use std::{collections::VecDeque, time::Duration};
use std::cell::{Cell, RefCell};
use std::os::raw::{c_void, c_char, c_int, c_uint};
use crate::strings::{BusName, Path};
use super::{Watch, WatchList, MessageCallback, ConnectionItem, MsgHandler, MsgHandlerList, MessageReply, BusType};


/* Since we register callbacks with userdata pointers,
   we need to make sure the connection pointer does not move around.
   Hence this extra indirection. */
struct IConnection {
    conn: Cell<*mut ffi::DBusConnection>,
    pending_items: RefCell<VecDeque<Message>>,
    watches: Option<Box<WatchList>>,
    handlers: RefCell<super::MsgHandlerList>,

    filter_cb: RefCell<Option<MessageCallback>>,
    filter_cb_panic: RefCell<thread::Result<()>>,
}

/// A D-Bus connection. Start here if you want to get on the D-Bus!
pub struct Connection {
    i: Box<IConnection>,
}

pub (crate) fn conn_handle(c: &Connection) -> *mut ffi::DBusConnection {
    c.i.conn.get()
}

extern "C" fn filter_message_cb(conn: *mut ffi::DBusConnection, msg: *mut ffi::DBusMessage,
    user_data: *mut c_void) -> ffi::DBusHandlerResult {

    let i: &IConnection = unsafe { mem::transmute(user_data) };
    let connref: panic::AssertUnwindSafe<&Connection> = unsafe { mem::transmute(&i) };
    if i.conn.get() != conn || i.filter_cb_panic.try_borrow().is_err() {
        // This should never happen, but let's be extra sure
        // process::abort(); ??
        return ffi::DBusHandlerResult::Handled;
    }
    if i.filter_cb_panic.borrow().is_err() {
        // We're in panic mode. Let's quit this ASAP
        return ffi::DBusHandlerResult::Handled;
    }

    let fcb = panic::AssertUnwindSafe(&i.filter_cb);
    let r = panic::catch_unwind(|| {
        let m = Message::from_ptr(msg, true);
        let mut cb = fcb.borrow_mut().take().unwrap(); // Take the callback out while we call it.
        let r = cb(connref.0, m);
        let mut cb2 = fcb.borrow_mut(); // If the filter callback has not been replaced, put it back in.
        if cb2.is_none() { *cb2 = Some(cb) };
        r
    });

    match r {
        Ok(false) => ffi::DBusHandlerResult::NotYetHandled,
        Ok(true) => ffi::DBusHandlerResult::Handled,
        Err(e) => {
            *i.filter_cb_panic.borrow_mut() = Err(e);
            ffi::DBusHandlerResult::Handled
        }
    }
}

fn default_filter_callback(c: &Connection, m: Message) -> bool {
    let b = m.msg_type() == MessageType::Signal;
    c.i.pending_items.borrow_mut().push_back(m);
    b
}

extern "C" fn object_path_message_cb(_conn: *mut ffi::DBusConnection, _msg: *mut ffi::DBusMessage,
    _user_data: *mut c_void) -> ffi::DBusHandlerResult {
    /* Already pushed in filter_message_cb, so we just set the handled flag here to disable the
       "default" handler. */
    ffi::DBusHandlerResult::Handled
}

impl Connection {
    #[inline(always)]
    fn conn(&self) -> *mut ffi::DBusConnection {
        self.i.conn.get()
    }

    fn conn_from_ptr(conn: *mut ffi::DBusConnection) -> Result<Connection, Error> {
        let mut c = Connection { i: Box::new(IConnection {
            conn: Cell::new(conn),
            pending_items: RefCell::new(VecDeque::new()),
            watches: None,
            handlers: RefCell::new(vec!()),
            filter_cb: RefCell::new(Some(Box::new(default_filter_callback))),
            filter_cb_panic: RefCell::new(Ok(())),
        })};

        /* No, we don't want our app to suddenly quit if dbus goes down */
        unsafe { ffi::dbus_connection_set_exit_on_disconnect(conn, 0) };
        assert!(unsafe {
            ffi::dbus_connection_add_filter(c.conn(), Some(filter_message_cb), mem::transmute(&*c.i), None)
        } != 0);

        c.i.watches = Some(WatchList::new(&c, Box::new(|_| {})));
        Ok(c)
    }

    /// Creates a new connection to the session bus.
    ///
    /// Just a shortcut for `get_private(BusType::Session)`.
    pub fn new_session() -> Result<Connection, Error> { Self::get_private(BusType::Session) }

    /// Creates a new connection to the system bus.
    ///
    /// Just a shortcut for `get_private(BusType::System)`.
    pub fn new_system() -> Result<Connection, Error> { Self::get_private(BusType::System) }

    /// Creates a new D-Bus connection.
    pub fn get_private(bus: BusType) -> Result<Connection, Error> {
        let mut e = Error::empty();
        let conn = unsafe { ffi::dbus_bus_get_private(bus, e.get_mut()) };
        if conn.is_null() {
            return Err(e)
        }
        Self::conn_from_ptr(conn)
    }

    /// Creates a new D-Bus connection to a remote address.
    ///
    /// Note: for all common cases (System / Session bus) you probably want "get_private" instead.
    pub fn open_private(address: &str) -> Result<Connection, Error> {
        let mut e = Error::empty();
        let conn = unsafe { ffi::dbus_connection_open_private(to_c_str(address).as_ptr(), e.get_mut()) };
        if conn.is_null() {
            return Err(e)
        }
        Self::conn_from_ptr(conn)
    }

    /// Registers a new D-Bus connection with the bus.
    ///
    /// Note: `get_private` does this automatically, useful with `open_private`
    pub fn register(&self) -> Result<(), Error> {
        let mut e = Error::empty();
        if unsafe { ffi::dbus_bus_register(self.conn(), e.get_mut()) == 0 } {
            Err(e)
        } else {
            Ok(())
        }
    }

    /// Gets whether the connection is currently open.
    pub fn is_connected(&self) -> bool {
        unsafe { ffi::dbus_connection_get_is_connected(self.conn()) != 0 }
    }

    /// Sends a message over the D-Bus and waits for a reply.
    /// This is usually used for method calls.
    pub fn send_with_reply_and_block(&self, msg: Message, timeout_ms: i32) -> Result<Message, Error> {
        let mut e = Error::empty();
        let response = unsafe {
            ffi::dbus_connection_send_with_reply_and_block(self.conn(), msg.ptr(),
                timeout_ms as c_int, e.get_mut())
        };
        if response.is_null() {
            return Err(e);
        }
        Ok(Message::from_ptr(response, false))
    }

    /// Sends a message over the D-Bus without waiting. Useful for sending signals and method call replies.
    pub fn send(&self, msg: Message) -> Result<u32,()> {
        let mut serial = 0u32;
        let r = unsafe { ffi::dbus_connection_send(self.conn(), msg.ptr(), &mut serial) };
        if r == 0 { return Err(()); }
        unsafe { ffi::dbus_connection_flush(self.conn()) };
        Ok(serial)
    }

    /// Sends a message over the D-Bus, returning a MessageReply.
    ///
    /// Call add_handler on the result to start waiting for reply. This should be done before next call to `incoming` or `iter`.
    pub fn send_with_reply<'a, F: FnOnce(Result<&Message, Error>) + 'a>(&self, msg: Message, f: F) -> Result<MessageReply<F>, ()> {
        let serial = self.send(msg)?;
        Ok(MessageReply(Some(f), serial))
    }

    /// Adds a message handler to the connection.
    ///
    /// # Example
    ///
    /// ```
    /// use std::{cell, rc};
    /// use dbus::{ffidisp::Connection, Message};
    ///
    /// let c = Connection::new_session().unwrap();
    /// let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    ///
    /// let done: rc::Rc<cell::Cell<bool>> = Default::default();
    /// let done2 = done.clone();
    /// c.add_handler(c.send_with_reply(m, move |reply| {
    ///     let v: Vec<&str> = reply.unwrap().read1().unwrap();
    ///     println!("The names on the D-Bus are: {:?}", v);
    ///     done2.set(true);
    /// }).unwrap());
    /// while !done.get() { c.incoming(100).next(); }
    /// ```
    pub fn add_handler<H: MsgHandler + 'static>(&self, h: H) {
        let h = Box::new(h);
        self.i.handlers.borrow_mut().push(h);
    }

    /// Removes a MsgHandler from the connection.
    ///
    /// If there are many MsgHandlers, it is not specified which one will be returned.
    ///
    /// There might be more methods added later on, which give better ways to deal
    /// with the list of MsgHandler currently on the connection. If this would help you,
    /// please [file an issue](https://github.com/diwic/dbus-rs/issues).
    pub fn extract_handler(&self) -> Option<Box<dyn MsgHandler>> {
        self.i.handlers.borrow_mut().pop()
    }

    /// Get the connection's unique name.
    pub fn unique_name(&self) -> String {
        let c = unsafe { ffi::dbus_bus_get_unique_name(self.conn()) };
        c_str_to_slice(&c).unwrap_or("").to_string()
    }

    /// Check if there are new incoming events
    ///
    /// If there are no incoming events, ConnectionItems::Nothing will be returned.
    /// See ConnectionItems::new if you want to customize this behaviour.
    pub fn iter(&self, timeout_ms: i32) -> ConnectionItems {
        ConnectionItems::new(self, Some(timeout_ms), false)
    }

    /// Check if there are new incoming events
    ///
    /// Supersedes "iter".
    pub fn incoming(&self, timeout_ms: u32) -> ConnMsgs<&Self> {
        ConnMsgs { conn: &self, timeout_ms: Some(timeout_ms) }
    }

    /// Register an object path.
    pub fn register_object_path(&self, path: &str) -> Result<(), Error> {
        let mut e = Error::empty();
        let p = to_c_str(path);
        let vtable = ffi::DBusObjectPathVTable {
            unregister_function: None,
            message_function: Some(object_path_message_cb),
            dbus_internal_pad1: None,
            dbus_internal_pad2: None,
            dbus_internal_pad3: None,
            dbus_internal_pad4: None,
        };
        let r = unsafe {
            let user_data: *mut c_void = mem::transmute(&*self.i);
            ffi::dbus_connection_try_register_object_path(self.conn(), p.as_ptr(), &vtable, user_data, e.get_mut())
        };
        if r == 0 { Err(e) } else { Ok(()) }
    }

    /// Unregister an object path.
    pub fn unregister_object_path(&self, path: &str) {
        let p = to_c_str(path);
        let r = unsafe { ffi::dbus_connection_unregister_object_path(self.conn(), p.as_ptr()) };
        if r == 0 { panic!("Out of memory"); }
    }

    /// List registered object paths.
    pub fn list_registered_object_paths(&self, path: &str) -> Vec<String> {
        let p = to_c_str(path);
        let mut clist: *mut *mut c_char = ptr::null_mut();
        let r = unsafe { ffi::dbus_connection_list_registered(self.conn(), p.as_ptr(), &mut clist) };
        if r == 0 { panic!("Out of memory"); }
        let mut v = Vec::new();
        let mut i = 0;
        loop {
            let s = unsafe {
                let citer = clist.offset(i);
                if *citer == ptr::null_mut() { break };
                mem::transmute(citer)
            };
            v.push(format!("{}", c_str_to_slice(s).unwrap()));
            i += 1;
        }
        unsafe { ffi::dbus_free_string_array(clist) };
        v
    }

    /// Register a name.
    pub fn register_name(&self, name: &str, flags: u32) -> Result<super::RequestNameReply, Error> {
        let mut e = Error::empty();
        let n = to_c_str(name);
        let r = unsafe { ffi::dbus_bus_request_name(self.conn(), n.as_ptr(), flags, e.get_mut()) };
        if r == -1 { Err(e) } else { Ok(unsafe { mem::transmute(r) }) }
    }

    /// Release a name.
    pub fn release_name(&self, name: &str) -> Result<super::ReleaseNameReply, Error> {
        let mut e = Error::empty();
        let n = to_c_str(name);
        let r = unsafe { ffi::dbus_bus_release_name(self.conn(), n.as_ptr(), e.get_mut()) };
        if r == -1 { Err(e) } else { Ok(unsafe { mem::transmute(r) }) }
    }

    /// Add a match rule to match messages on the message bus.
    ///
    /// See the `unity_focused_window` example for how to use this to catch signals.
    /// You can use MatchRule to construct the "rule" string.
    /// (The syntax of the "rule" string is specified in the [D-Bus specification](https://dbus.freedesktop.org/doc/dbus-specification.html#message-bus-routing-match-rules).)
    pub fn add_match(&self, rule: &str) -> Result<(), Error> {
        let mut e = Error::empty();
        let n = to_c_str(rule);
        unsafe { ffi::dbus_bus_add_match(self.conn(), n.as_ptr(), e.get_mut()) };
        if e.name().is_some() { Err(e) } else { Ok(()) }
    }

    /// Remove a match rule to match messages on the message bus.
    pub fn remove_match(&self, rule: &str) -> Result<(), Error> {
        let mut e = Error::empty();
        let n = to_c_str(rule);
        unsafe { ffi::dbus_bus_remove_match(self.conn(), n.as_ptr(), e.get_mut()) };
        if e.name().is_some() { Err(e) } else { Ok(()) }
    }

    /// Async I/O: Get an up-to-date list of file descriptors to watch.
    ///
    /// See the `Watch` struct for an example.
    pub fn watch_fds(&self) -> Vec<Watch> {
        self.i.watches.as_ref().unwrap().get_enabled_fds()
    }

    /// Async I/O: Call this function whenever you detected an event on the Fd,
    /// Flags are a set of WatchEvent bits.
    /// The returned iterator will return pending items only, never block for new events.
    ///
    /// See the `Watch` struct for an example.
    pub fn watch_handle(&self, fd: WatchFd, flags: c_uint) -> ConnectionItems {
        self.i.watches.as_ref().unwrap().watch_handle(fd, flags);
        ConnectionItems::new(self, None, true)
    }

    /// Create a convenience struct for easier calling of many methods on the same destination and path.
    pub fn with_path<'a, D: Into<BusName<'a>>, P: Into<Path<'a>>>(&'a self, dest: D, path: P, timeout_ms: i32) ->
        ConnPath<'a, &'a Connection> {
        ConnPath { conn: self, dest: dest.into(), path: path.into(), timeout: timeout_ms }
    }

    /// Replace the default message callback. Returns the previously set callback.
    ///
    /// By default, when you call ConnectionItems::next, all relevant incoming messages
    /// are returned through the ConnectionItems iterator, and
    /// irrelevant messages are passed on to libdbus's default handler.
    /// If you need to customize this behaviour (i e, to handle all incoming messages yourself),
    /// you can set this message callback yourself. A few caveats apply:
    ///
    /// Return true from the callback to disable libdbus's internal handling of the message, or
    /// false to allow it. In other words, true and false correspond to
    /// `DBUS_HANDLER_RESULT_HANDLED` and `DBUS_HANDLER_RESULT_NOT_YET_HANDLED` respectively.
    ///
    /// Be sure to call the previously set callback from inside your callback,
    /// if you want, e.g. ConnectionItems::next to yield the message.
    ///
    /// You can unset the message callback (might be useful to satisfy the borrow checker), but
    /// you will get a panic if you call ConnectionItems::next while the message callback is unset.
    /// The message callback will be temporary unset while inside a message callback, so calling
    /// ConnectionItems::next recursively will also result in a panic.
    ///
    /// If your message callback panics, ConnectionItems::next will panic, too.
    ///
    /// # Examples
    ///
    /// Replace the default callback with our own:
    ///
    /// ```ignore
    /// use dbus::ffidisp::Connection;
    /// let c = Connection::new_session().unwrap();
    /// // Set our callback
    /// c.replace_message_callback(Some(Box::new(move |conn, msg| {
    ///     println!("Got message: {:?}", msg.get_items());
    ///     // Let libdbus handle some things by default,
    ///     // like "nonexistent object" error replies to method calls
    ///     false
    /// })));
    ///
    /// for _ in c.iter(1000) {
    ///    // Only `ConnectionItem::Nothing` would be ever yielded here.
    /// }
    /// ```
    ///
    /// Chain our callback to filter out some messages before `iter().next()`:
    ///
    /// ```
    /// use dbus::{ffidisp::Connection, MessageType};
    /// let c = Connection::new_session().unwrap();
    /// // Take the previously set callback
    /// let mut old_cb = c.replace_message_callback(None).unwrap();
    /// // Set our callback
    /// c.replace_message_callback(Some(Box::new(move |conn, msg| {
    ///     // Handle all signals on the spot
    ///     if msg.msg_type() == MessageType::Signal {
    ///         println!("Got signal: {:?}", msg.get_items());
    ///         // Stop all further processing of the message
    ///         return true;
    ///     }
    ///     // Delegate the rest of the messages to the previous callback
    ///     // in chain, e.g. to have them yielded by `iter().next()`
    ///     old_cb(conn, msg)
    /// })));
    ///
    /// # if false {
    /// for _ in c.iter(1000) {
    ///    // `ConnectionItem::Signal` would never be yielded here.
    /// }
    /// # }
    /// ```
    pub fn replace_message_callback(&self, f: Option<MessageCallback>) -> Option<MessageCallback> {
        mem::replace(&mut *self.i.filter_cb.borrow_mut(), f)
    }

    /// Sets a callback to be called if a file descriptor status changes.
    ///
    /// For async I/O. In rare cases, the number of fds to poll for read/write can change.
    /// If this ever happens, you'll get a callback. The watch changed is provided as a parameter.
    ///
    /// In rare cases this might not even happen in the thread calling anything on the connection,
    /// so the callback needs to be `Send`.
    /// A mutex is held during the callback. If you try to call set_watch_callback from a callback,
    /// you will deadlock.
    ///
    /// (Previously, this was instead put in a ConnectionItem queue, but this was not working correctly.
    /// see https://github.com/diwic/dbus-rs/issues/99 for additional info.)
    pub fn set_watch_callback(&self, f: Box<dyn Fn(Watch) + Send>) { self.i.watches.as_ref().unwrap().set_on_update(f); }

    fn check_panic(&self) {
        let p = mem::replace(&mut *self.i.filter_cb_panic.borrow_mut(), Ok(()));
        if let Err(perr) = p { panic::resume_unwind(perr); }
    }

    fn next_msg(&self) -> Option<Message> {
        while let Some(msg) = self.i.pending_items.borrow_mut().pop_front() {
            let mut v: MsgHandlerList = mem::replace(&mut *self.i.handlers.borrow_mut(), vec!());
            let b = msghandler_process(&mut v, &msg, self);
            let mut v2 = self.i.handlers.borrow_mut();
            v.append(&mut *v2);
            *v2 = v;
            if !b { return Some(msg) };
        };
        None
    }

}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            ffi::dbus_connection_close(self.conn());
            ffi::dbus_connection_unref(self.conn());
        }
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "D-Bus Connection({})", self.unique_name())
    }
}

impl crate::channel::Sender for Connection {
    fn send(&self, msg: Message) -> Result<u32, ()> { Connection::send(self, msg) }
}

impl crate::blocking::BlockingSender for Connection {
    fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error> {
        Connection::send_with_reply_and_block(self, msg, timeout.as_millis() as i32)
    }
}


fn msghandler_process(v: &mut MsgHandlerList, m: &Message, c: &Connection) -> bool {
    let mut ii: isize = -1;
    loop {
        ii += 1;
        let i = ii as usize;
        if i >= v.len() { return false };

        if !v[i].handler_type().matches_msg(m) { continue; }
        if let Some(r) = v[i].handle_msg(m) {
            for msg in r.reply.into_iter() { c.send(msg).unwrap(); }
            if r.done { v.remove(i); ii -= 1; }
            if r.handled { return true; }
        }
    }
}

/// ConnectionItem iterator
pub struct ConnectionItems<'a> {
    c: &'a Connection,
    timeout_ms: Option<i32>,
    end_on_timeout: bool,
    handlers: MsgHandlerList,
}

impl<'a> ConnectionItems<'a> {
    /// Builder method that adds a new msg handler.
    ///
    /// Note: Likely to changed/refactored/removed in next release
    pub fn with<H: 'static + MsgHandler>(mut self, h: H) -> Self {
        self.handlers.push(Box::new(h)); self
    }

    // Returns true if processed, false if not
    fn process_handlers(&mut self, ci: &ConnectionItem) -> bool {
        let m = match *ci {
            ConnectionItem::MethodReturn(ref msg) => msg,
            ConnectionItem::Signal(ref msg) => msg,
            ConnectionItem::MethodCall(ref msg) => msg,
            ConnectionItem::Nothing => return false,
        };

        msghandler_process(&mut self.handlers, m, &self.c)
    }

    /// Access and modify message handlers
    ///
    /// Note: Likely to changed/refactored/removed in next release
    pub fn msg_handlers(&mut self) -> &mut Vec<Box<dyn MsgHandler>> { &mut self.handlers }

    /// Creates a new ConnectionItems iterator
    ///
    /// For io_timeout, setting None means the fds will not be read/written. I e, only pending
    /// items in libdbus's internal queue will be processed.
    ///
    /// For end_on_timeout, setting false will means that the iterator will never finish (unless
    /// the D-Bus server goes down). Instead, ConnectionItem::Nothing will be returned in case no
    /// items are in queue.
    pub fn new(conn: &'a Connection, io_timeout: Option<i32>, end_on_timeout: bool) -> Self {
        ConnectionItems {
            c: conn,
            timeout_ms: io_timeout,
            end_on_timeout: end_on_timeout,
            handlers: Vec::new(),
        }
    }
}

impl<'a> Iterator for ConnectionItems<'a> {
    type Item = ConnectionItem;
    fn next(&mut self) -> Option<ConnectionItem> {
        loop {
            if self.c.i.filter_cb.borrow().is_none() { panic!("ConnectionItems::next called recursively or with a MessageCallback set to None"); }
            let i: Option<ConnectionItem> = self.c.next_msg().map(|x| x.into());
            if let Some(ci) = i {
                if !self.process_handlers(&ci) { return Some(ci); }
            }

            if let Some(t) = self.timeout_ms {
                let r = unsafe { ffi::dbus_connection_read_write_dispatch(self.c.conn(), t as c_int) };
                self.c.check_panic();
                if !self.c.i.pending_items.borrow().is_empty() { continue };
                if r == 0 { return None; }
            }

            let r = unsafe { ffi::dbus_connection_dispatch(self.c.conn()) };
            self.c.check_panic();

            if !self.c.i.pending_items.borrow().is_empty() { continue };
            if r == ffi::DBusDispatchStatus::DataRemains { continue };
            if r == ffi::DBusDispatchStatus::Complete { return if self.end_on_timeout { None } else { Some(ConnectionItem::Nothing) } };
            panic!("dbus_connection_dispatch failed");
        }
    }
}

/// Iterator over incoming messages on a connection.
#[derive(Debug, Clone)]
pub struct ConnMsgs<C> {
    /// The connection or some reference to it.
    pub conn: C,
    /// How many ms dbus should block, waiting for incoming messages until timing out.
    ///
    /// If set to None, the dbus library will not read/write from file descriptors at all.
    /// Instead the iterator will end when there's nothing currently in the queue.
    pub timeout_ms: Option<u32>,
}

impl<C: ops::Deref<Target = Connection>> Iterator for ConnMsgs<C> {
    type Item = Message;
    fn next(&mut self) -> Option<Self::Item> {

        loop {
            let iconn = &self.conn.i;
            if iconn.filter_cb.borrow().is_none() { panic!("ConnMsgs::next called recursively or with a MessageCallback set to None"); }
            let i = self.conn.next_msg();
            if let Some(ci) = i { return Some(ci); }

            if let Some(t) = self.timeout_ms {
                let r = unsafe { ffi::dbus_connection_read_write_dispatch(self.conn.conn(), t as c_int) };
                self.conn.check_panic();
                if !iconn.pending_items.borrow().is_empty() { continue };
                if r == 0 { return None; }
            }

            let r = unsafe { ffi::dbus_connection_dispatch(self.conn.conn()) };
            self.conn.check_panic();

            if !iconn.pending_items.borrow().is_empty() { continue };
            if r == ffi::DBusDispatchStatus::DataRemains { continue };
            if r == ffi::DBusDispatchStatus::Complete { return None }
            panic!("dbus_connection_dispatch failed");
        }
    }
}

#[test]
fn message_reply() {
    use std::{cell, rc};
    let c = Connection::get_private(BusType::Session).unwrap();
    assert!(c.is_connected());
    let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let quit = rc::Rc::new(cell::Cell::new(false));
    let quit2 = quit.clone();
    let reply = c.send_with_reply(m, move |result| {
        let r = result.unwrap();
        let _: crate::arg::Array<&str, _>  = r.get1().unwrap();
        quit2.set(true);
    }).unwrap();
    for _ in c.iter(1000).with(reply) { if quit.get() { return; } }
    assert!(false);
}
