//! Contains some helper structs and traits common to all Connection types.-

use crate::{Error, Message, to_c_str};
use std::{str, time::Duration, collections::HashMap};
use std::sync::{Mutex, atomic::AtomicU8, atomic::Ordering};
use std::ffi::CStr;
use std::os::raw::{c_void, c_int};
use super::{BusType, Watch, WatchFd};

#[derive(Debug)]
struct ConnHandle(*mut ffi::DBusConnection, bool);

unsafe impl Send for ConnHandle {}
unsafe impl Sync for ConnHandle {}

impl Drop for ConnHandle {
    fn drop(&mut self) {
        if self.1 { unsafe {
            ffi::dbus_connection_close(self.0);
            ffi::dbus_connection_unref(self.0);
        }}
    }
}

#[derive(Debug, Eq, PartialEq, Hash)]
struct WatchHandle(*mut ffi::DBusWatch);

unsafe impl Send for WatchHandle {}
unsafe impl Sync for WatchHandle {}

/// This struct must be boxed as it is called from D-Bus callbacks!
#[derive(Debug)]
struct WatchMap {
    conn: ConnHandle,
    list: Mutex<HashMap<WatchHandle, (Watch, bool)>>,
    current_rw: AtomicU8,
    current_fd: Option<WatchFd>,
}

fn calc_rw(list: &HashMap<WatchHandle, (Watch, bool)>) -> u8 {
    let mut r = 0;
    for (w, b) in list.values() {
        if *b && w.read { r |= 1; }
        if *b && w.write { r |= 2; }
    }
    r
}

impl WatchMap {
    fn new(conn: ConnHandle) -> Box<WatchMap> {
        extern "C" fn add_watch_cb(watch: *mut ffi::DBusWatch, data: *mut c_void) -> u32 { unsafe {
            let wm: &WatchMap = &*(data as *mut _);
            wm.list.lock().unwrap().insert(WatchHandle(watch), Watch::from_raw_enabled(watch));
            1
        }}
        extern "C" fn remove_watch_cb(watch: *mut ffi::DBusWatch, data: *mut c_void) { unsafe {
            let wm: &WatchMap = &*(data as *mut _);
            wm.list.lock().unwrap().remove(&WatchHandle(watch));
        }}
        extern "C" fn toggled_watch_cb(watch: *mut ffi::DBusWatch, data: *mut c_void) { unsafe {
            let wm: &WatchMap = &*(data as *mut _);
            let mut list = wm.list.lock().unwrap();
            let (_, ref mut b) = list.get_mut(&WatchHandle(watch)).unwrap();
            *b = ffi::dbus_watch_get_enabled(watch) != 0;
            wm.current_rw.store(calc_rw(&list), Ordering::Release);
        }}

        let mut wm = Box::new(WatchMap {
            conn, list: Default::default(), current_rw: Default::default(), current_fd: None
        });
        let wptr: &WatchMap = &wm;
        if unsafe { ffi::dbus_connection_set_watch_functions(wm.conn.0,
            Some(add_watch_cb), Some(remove_watch_cb), Some(toggled_watch_cb), wptr as *const _ as *mut _, None) } == 0 {
                panic!("Cannot enable watch tracking (OOM?)")
        }

        {
            let list = wm.list.lock().unwrap();
            wm.current_rw.store(calc_rw(&list), Ordering::Release);

            // This will never panic in practice, see https://lists.freedesktop.org/archives/dbus/2019-July/017786.html
            for (w, _) in list.values() {
                if let Some(ref fd) = &wm.current_fd {
                    assert_eq!(*fd, w.fd);
                } else {
                    wm.current_fd = Some(w.fd);
                }
            }
        }

        wm
    }
}

impl Drop for WatchMap {
    fn drop(&mut self) {
        let wptr: &WatchMap = &self;
        if unsafe { ffi::dbus_connection_set_watch_functions(self.conn.0,
            None, None, None, wptr as *const _ as *mut _, None) } == 0 {
                panic!("Cannot disable watch tracking (OOM?)")
        }
    }
}

/// Low-level connection - handles read/write to the socket
///
/// You probably do not need to worry about this as you would typically
/// use the various blocking and non-blocking "Connection" structs instead.
///
/// This version avoids dbus_connection_dispatch, and thus avoids
/// callbacks from that function. Instead the same functionality
/// is implemented in the various blocking and non-blocking "Connection" components.
///
/// Blocking operations are clearly marked as such, although if you
/// try to access the connection from several threads at the same time,
/// blocking might occur due to an internal mutex inside the dbus library.
#[derive(Debug)]
pub struct Channel {
    handle: ConnHandle,
    watchmap: Option<Box<WatchMap>>,
}

impl Drop for Channel {
    fn drop(&mut self) {
        self.set_watch_enabled(false); // Make sure "watchmap" is destroyed before "handle" is
    }
}

impl Channel {
    #[inline(always)]
    pub (crate) fn conn(&self) -> *mut ffi::DBusConnection {
        self.handle.0
    }

    fn conn_from_ptr(ptr: *mut ffi::DBusConnection) -> Result<Channel, Error> {
        let handle = ConnHandle(ptr, true);

        /* No, we don't want our app to suddenly quit if dbus goes down */
        unsafe { ffi::dbus_connection_set_exit_on_disconnect(ptr, 0) };

        let c = Channel { handle, watchmap: None };

        Ok(c)
    }


    /// Creates a new D-Bus connection.
    ///
    /// Blocking: until the connection is up and running.
    pub fn get_private(bus: BusType) -> Result<Channel, Error> {
        let mut e = Error::empty();
        let b = match bus {
            BusType::Session => ffi::DBusBusType::Session,
            BusType::System => ffi::DBusBusType::System,
            BusType::Starter => ffi::DBusBusType::Starter,
        };
        let conn = unsafe { ffi::dbus_bus_get_private(b, e.get_mut()) };
        if conn.is_null() {
            return Err(e)
        }
        Self::conn_from_ptr(conn)
    }

    /// Creates a new D-Bus connection to a remote address.
    ///
    /// Note: for all common cases (System / Session bus) you probably want "get_private" instead.
    ///
    /// Blocking: until the connection is established.
    pub fn open_private(address: &str) -> Result<Channel, Error> {
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
    ///
    /// Blocking: until a "Hello" response is received from the server.
    pub fn register(&mut self) -> Result<(), Error> {
        // This function needs to take &mut self, because it changes unique_name and unique_name takes a &self
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

    /// Get the connection's unique name.
    ///
    /// It's usually something like ":1.54"
    pub fn unique_name(&self) -> Option<&str> {
        let c = unsafe { ffi::dbus_bus_get_unique_name(self.conn()) };
        if c.is_null() { return None; }
        let s = unsafe { CStr::from_ptr(c) };
        str::from_utf8(s.to_bytes()).ok()
    }


    /// Puts a message into libdbus out queue, and tries to send it.
    ///
    /// Returns a serial number than can be used to match against a reply.
    ///
    /// Note: usually the message is sent when this call happens, but in
    /// case internal D-Bus buffers are full, it will be left in the out queue.
    /// Call "flush" or "read_write" to retry flushing the out queue.
    pub fn send(&self, msg: Message) -> Result<u32, ()> {
        let mut serial = 0u32;
        let r = unsafe { ffi::dbus_connection_send(self.conn(), msg.ptr(), &mut serial) };
        if r == 0 { return Err(()); }
        Ok(serial)
    }

    /// Sends a message over the D-Bus and waits for a reply. This is used for method calls.
    ///
    /// Blocking: until a reply is received or the timeout expires.
    ///
    /// Note: In case of an error reply, this is returned as an Err(), not as a Ok(Message) with the error type.
    ///
    /// Note: In case pop_message and send_with_reply_and_block is called in parallel from different threads,
    /// they might race to retrieve the reply message from the internal queue.
    pub fn send_with_reply_and_block(&self, msg: Message, timeout: Duration) -> Result<Message, Error> {
        let mut e = Error::empty();
        let response = unsafe {
            ffi::dbus_connection_send_with_reply_and_block(self.conn(), msg.ptr(),
                timeout.as_millis() as c_int, e.get_mut())
        };
        if response.is_null() {
            return Err(e);
        }
        Ok(Message::from_ptr(response, false))
    }

    /// Flush the queue of outgoing messages.
    ///
    /// Blocking: until the outgoing queue is empty.
    pub fn flush(&self) { unsafe { ffi::dbus_connection_flush(self.conn()) } }

    /// Read and write to the connection.
    ///
    /// Incoming messages are put in the internal queue, outgoing messages are written.
    ///
    /// Blocking: If there are no messages, for up to timeout, or forever if timeout is None.
    /// For non-blocking behaviour, set timeout to Some(0).
    pub fn read_write(&self, timeout: Option<Duration>) -> Result<(), ()> {
        let t = timeout.map_or(-1, |t| t.as_millis() as c_int);
        if unsafe { ffi::dbus_connection_read_write(self.conn(), t) == 0 } {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Gets whether the output message buffer is non-empty
    pub fn has_messages_to_send(&self) -> bool {
        unsafe { ffi::dbus_connection_has_messages_to_send(self.conn()) == 1 }
    }

    /// Removes a message from the incoming queue, or returns None if the queue is empty.
    ///
    /// Use "read_write" first, so that messages are put into the incoming queue.
    /// For unhandled messages, please call MessageDispatcher::default_dispatch to return
    /// default replies for method calls.
    pub fn pop_message(&self) -> Option<Message> {
        let mptr = unsafe { ffi::dbus_connection_pop_message(self.conn()) };
        if mptr.is_null() {
            None
        } else {
            let msg = Message::from_ptr(mptr, false);
            // println!("Incoming: {:?}", msg);
            Some(msg)
        }
    }

    /// Removes a message from the incoming queue, or waits until timeout if the queue is empty.
    ///
    pub fn blocking_pop_message(&self, timeout: Duration) -> Result<Option<Message>, Error> {
        if let Some(msg) = self.pop_message() { return Ok(Some(msg)) }
        self.read_write(Some(timeout)).map_err(|_|
            Error::new_failed("Failed to read/write data, disconnected from D-Bus?")
        )?;
        Ok(self.pop_message())
    }

    /// Enables watch tracking, a prerequisite for calling watch.
    ///
    /// (In theory, this could panic in case libdbus ever changes to listen to
    /// something else than one file descriptor,
    /// but this should be extremely unlikely to ever happen.)
    pub fn set_watch_enabled(&mut self, enable: bool) {
        if enable == self.watchmap.is_some() { return }
        if enable {
            self.watchmap = Some(WatchMap::new(ConnHandle(self.conn(), false)));
        } else {
            self.watchmap = None;
        }
    }

    /// Gets the file descriptor to listen for read/write.
    ///
    /// Panics: if set_watch_enabled is false.
    ///
    /// (In theory, this could panic in case libdbus ever changes to listen to
    /// something else than one file descriptor,
    /// but this should be extremely unlikely to ever happen.)
    pub fn watch(&self) -> Watch {
        let wm = self.watchmap.as_ref().unwrap();
        let rw = wm.current_rw.load(Ordering::Acquire);
        Watch {
            fd: wm.current_fd.unwrap(),
            read: (rw & 1) != 0,
            write: (rw & 2) != 0,
        }
    }

    /// Get an up-to-date list of file descriptors to watch.
    ///
    /// Obsolete - in practice, you can use watch and set_watch_enabled instead.
    #[deprecated]
    pub fn watch_fds(&mut self) -> Result<Vec<Watch>, ()> {
        let en = self.watchmap.is_some();
        self.set_watch_enabled(true);
        let mut wlist: Vec<Watch> = self.watchmap.as_ref().unwrap().list.lock().unwrap().values()
            .map(|&(w, b)| Watch { fd: w.fd, read: b && w.read, write: b && w.write })
            .collect();
        self.set_watch_enabled(en);

        if wlist.len() == 2 && wlist[0].fd == wlist[1].fd {
            // This is always true in practice, see https://lists.freedesktop.org/archives/dbus/2019-July/017786.html
            wlist = vec!(Watch {
                fd: wlist[0].fd,
                read: wlist[0].read || wlist[1].read,
                write: wlist[0].write || wlist[1].write
            });
        }

        Ok(wlist)
    }
}


impl Watch {
    unsafe fn from_raw_enabled(watch: *mut ffi::DBusWatch) -> (Self, bool) {
        #[cfg(unix)]
        let mut w = Watch {fd: ffi::dbus_watch_get_unix_fd(watch), read: false, write: false};
        #[cfg(windows)]
        let mut w = Watch {fd: ffi::dbus_watch_get_socket(watch) as WatchFd, read: false, write: false};
        let enabled = ffi::dbus_watch_get_enabled(watch) != 0;
        let flags = ffi::dbus_watch_get_flags(watch);
        use std::os::raw::c_uint;
        w.read = (flags & ffi::DBUS_WATCH_READABLE as c_uint) != 0;
        w.write = (flags & ffi::DBUS_WATCH_WRITABLE as c_uint) != 0;
        (w, enabled)
    }
}
