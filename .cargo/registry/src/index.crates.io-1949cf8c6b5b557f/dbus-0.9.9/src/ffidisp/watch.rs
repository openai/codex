use crate::{channel::WatchFd, ffi};
use libc;
use crate::ffidisp::Connection;

use std::mem;
use std::sync::{Mutex, RwLock};
#[cfg(unix)]
use std::os::unix::io::{RawFd, AsRawFd};
#[cfg(windows)]
use std::os::windows::io::{RawSocket, AsRawSocket};
#[cfg(unix)]
use libc::{POLLIN, POLLOUT, POLLERR, POLLHUP};
#[cfg(windows)]
use windows_sys::Win32::Networking::WinSock::{WSAPOLLFD, POLLIN, POLLOUT, POLLERR, POLLHUP, SOCKET};
use std::os::raw::{c_void, c_uint};

/// A file descriptor to watch for incoming events (for async I/O).
///
/// # Example
/// ```
/// extern crate libc;
/// extern crate dbus;
/// fn main() {
///     use dbus::ffidisp::{Connection, BusType, WatchEvent};
///     let c = Connection::get_private(BusType::Session).unwrap();
///
///     // Get a list of fds to poll for
///     let mut fds: Vec<_> = c.watch_fds().iter().map(|w| w.to_pollfd()).collect();
///
///     // Poll them with a 1 s timeout
///     let r = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::c_ulong, 1000) };
///     assert!(r >= 0);
///
///     // And handle incoming events
///     for pfd in fds.iter().filter(|pfd| pfd.revents != 0) {
///         for item in c.watch_handle(pfd.fd, WatchEvent::from_revents(pfd.revents)) {
///             // Handle item
///             println!("Received ConnectionItem: {:?}", item);
///         }
///     }
/// }
/// ```

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
/// The enum is here for backwards compatibility mostly.
///
/// It should really be bitflags instead.
pub enum WatchEvent {
    /// The fd is readable
    Readable = ffi::DBUS_WATCH_READABLE as isize,
    /// The fd is writable
    Writable = ffi::DBUS_WATCH_WRITABLE as isize,
    /// An error occurred on the fd
    Error = ffi::DBUS_WATCH_ERROR as isize,
    /// The fd received a hangup.
    Hangup = ffi::DBUS_WATCH_HANGUP as isize,
}

impl WatchEvent {
    /// After running poll, this transforms the revents into a parameter you can send into `Connection::watch_handle`
    pub fn from_revents(revents: libc::c_short) -> c_uint {
        0 +
        if (revents & POLLIN) != 0 { WatchEvent::Readable as c_uint } else { 0 } +
        if (revents & POLLOUT) != 0 { WatchEvent::Writable as c_uint } else { 0 } +
        if (revents & POLLERR) != 0 { WatchEvent::Error as c_uint } else { 0 } +
        if (revents & POLLHUP) != 0 { WatchEvent::Hangup as c_uint } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// A file descriptor, and an indication whether it should be read from, written to, or both.
pub struct Watch {
    fd: WatchFd,
    read: bool,
    write: bool,
}

impl Watch {
    /// Get the RawFd this Watch is for
    pub fn fd(&self) -> WatchFd { self.fd }
    /// Add POLLIN to events to listen for
    pub fn readable(&self) -> bool { self.read }
    /// Add POLLOUT to events to listen for
    pub fn writable(&self) -> bool { self.write }
    /// Returns the current watch as a libc::pollfd, to use with libc::poll
    #[cfg(unix)]
    pub fn to_pollfd(&self) -> libc::pollfd {
        libc::pollfd { fd: self.fd, revents: 0, events: POLLERR + POLLHUP +
            if self.readable() { POLLIN } else { 0 } +
            if self.writable() { POLLOUT } else { 0 },
        }
    }
    /// Returns the current watch as a WSAPOLLFD, to use with WSAPoll
    #[cfg(windows)]
    pub fn to_pollfd(&self) -> WSAPOLLFD {
        WSAPOLLFD {
            fd: self.fd as SOCKET,
            revents: 0, events: 0 +
            if self.readable() { POLLIN } else { 0 } +
            if self.writable() { POLLOUT } else { 0 },
        }
    }
}

#[cfg(unix)]
impl AsRawFd for Watch {
    fn as_raw_fd(&self) -> RawFd { self.fd }
}

#[cfg(windows)]
impl AsRawSocket for Watch {
    fn as_raw_socket(&self) -> RawSocket { self.fd }
}

/// Note - internal struct, not to be used outside API. Moving it outside its box will break things.
pub struct WatchList {
    watches: RwLock<Vec<*mut ffi::DBusWatch>>,
    enabled_fds: Mutex<Vec<Watch>>,
    on_update: Mutex<Box<dyn Fn(Watch) + Send>>,
}

impl WatchList {
    pub fn new(c: &Connection, on_update: Box<dyn Fn(Watch) + Send>) -> Box<WatchList> {
        let w = Box::new(WatchList { on_update: Mutex::new(on_update), watches: RwLock::new(vec!()), enabled_fds: Mutex::new(vec!()) });
        if unsafe { ffi::dbus_connection_set_watch_functions(crate::ffidisp::connection::conn_handle(c),
            Some(add_watch_cb), Some(remove_watch_cb), Some(toggled_watch_cb), &*w as *const _ as *mut _, None) } == 0 {
            panic!("dbus_connection_set_watch_functions failed");
        }
        w
    }

    pub fn set_on_update(&self, on_update: Box<dyn Fn(Watch) + Send>) { *self.on_update.lock().unwrap() = on_update; }

    pub fn watch_handle(&self, fd: WatchFd, flags: c_uint) {
        // println!("watch_handle {} flags {}", fd, flags);
        for &q in self.watches.read().unwrap().iter() {
            let w = self.get_watch(q);
            if w.fd != fd { continue };
            if unsafe { ffi::dbus_watch_handle(q, flags) } == 0 {
                panic!("dbus_watch_handle failed");
            }
            self.update(q);
        };
    }

    pub fn get_enabled_fds(&self) -> Vec<Watch> {
        self.enabled_fds.lock().unwrap().clone()
    }

    fn get_watch(&self, watch: *mut ffi::DBusWatch) -> Watch {
        #[cfg(unix)]
        let mut w = Watch { fd: unsafe { ffi::dbus_watch_get_unix_fd(watch) }, read: false, write: false};
        #[cfg(windows)]
        let mut w = Watch { fd: unsafe { ffi::dbus_watch_get_socket(watch) as RawSocket }, read: false, write: false};
        let enabled = self.watches.read().unwrap().contains(&watch) && unsafe { ffi::dbus_watch_get_enabled(watch) != 0 };
        let flags = unsafe { ffi::dbus_watch_get_flags(watch) };
        if enabled {
            w.read = (flags & WatchEvent::Readable as c_uint) != 0;
            w.write = (flags & WatchEvent::Writable as c_uint) != 0;
        }
        // println!("Get watch fd {:?} ptr {:?} enabled {:?} flags {:?}", w, watch, enabled, flags);
        w
    }

    fn update(&self, watch: *mut ffi::DBusWatch) {
        let mut w = self.get_watch(watch);

        for &q in self.watches.read().unwrap().iter() {
            if q == watch { continue };
            let ww = self.get_watch(q);
            if ww.fd != w.fd { continue };
            w.read |= ww.read;
            w.write |= ww.write;
        }
        // println!("Updated sum: {:?}", w);

        {
            let mut fdarr = self.enabled_fds.lock().unwrap();

            if w.write || w.read {
                if fdarr.contains(&w) { return; } // Nothing changed
            }
            else if !fdarr.iter().any(|q| w.fd == q.fd) { return; } // Nothing changed

            fdarr.retain(|f| f.fd != w.fd);
            if w.write || w.read { fdarr.push(w) };
        }
        let func = self.on_update.lock().unwrap();
        (*func)(w);
    }
}

extern "C" fn add_watch_cb(watch: *mut ffi::DBusWatch, data: *mut c_void) -> u32 {
    let wlist: &WatchList = unsafe { mem::transmute(data) };
    // println!("Add watch {:?}", watch);
    wlist.watches.write().unwrap().push(watch);
    wlist.update(watch);
    1
}

extern "C" fn remove_watch_cb(watch: *mut ffi::DBusWatch, data: *mut c_void) {
    let wlist: &WatchList = unsafe { mem::transmute(data) };
    // println!("Removed watch {:?}", watch);
    wlist.watches.write().unwrap().retain(|w| *w != watch);
    wlist.update(watch);
}

extern "C" fn toggled_watch_cb(watch: *mut ffi::DBusWatch, data: *mut c_void) {
    let wlist: &WatchList = unsafe { mem::transmute(data) };
    // println!("Toggled watch {:?}", watch);
    wlist.update(watch);
}

#[cfg(test)]
mod test {
    #[cfg(unix)]
    use libc;
    #[cfg(windows)]
    use windows_sys::Win32::Networking::WinSock::WSAPoll;
    use super::super::{Connection, Message, BusType, WatchEvent, ConnectionItem, MessageType};
    use super::{POLLIN, POLLOUT};

    #[test]
    fn test_async() {
        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/test").unwrap();
        let m = Message::new_method_call(&c.unique_name(), "/test", "com.example.asynctest", "AsyncTest").unwrap();
        let serial = c.send(m).unwrap();
        println!("Async: sent serial {}", serial);

        let mut fds: Vec<_> = c.watch_fds().iter().map(|w| w.to_pollfd()).collect();
        let mut new_fds = None;
        let mut i = 0;
        let mut success = false;
        while !success {
            i += 1;
            if let Some(q) = new_fds { fds = q; new_fds = None };

            for f in fds.iter_mut() { f.revents = 0 };

            #[cfg(unix)]
            assert!(unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 1000) } > 0);

            #[cfg(windows)]
            assert!(unsafe { WSAPoll(fds.as_mut_ptr(), fds.len() as u32, 1000) } > 0);

            for f in fds.iter().filter(|pfd| pfd.revents != 0) {
                let m = WatchEvent::from_revents(f.revents);
                println!("Async: fd {}, revents {} -> {}", f.fd, f.revents, m);
                assert!(f.revents & POLLIN != 0 || f.revents & POLLOUT != 0);

                #[cfg(unix)]
                let fd = f.fd;
                #[cfg(windows)]
                let fd = f.fd as std::os::windows::io::RawSocket;

                for e in c.watch_handle(fd, m) {
                    println!("Async: got {:?}", e);
                    match e {
                        ConnectionItem::MethodCall(m) => {
                            assert_eq!(m.msg_type(), MessageType::MethodCall);
                            assert_eq!(&*m.path().unwrap(), "/test");
                            assert_eq!(&*m.interface().unwrap(), "com.example.asynctest");
                            assert_eq!(&*m.member().unwrap(), "AsyncTest");
                            let mut mr = Message::new_method_return(&m).unwrap();
                            mr.append_items(&["Goodies".into()]);
                            c.send(mr).unwrap();
                        }
                        ConnectionItem::MethodReturn(m) => {
                            assert_eq!(m.msg_type(), MessageType::MethodReturn);
                            assert_eq!(m.get_reply_serial().unwrap(), serial);
                            let i = m.get_items();
                            let s: &str = i[0].inner().unwrap();
                            assert_eq!(s, "Goodies");
                            success = true;
                        }
                        _ => (),
                    }
                }
                if i > 100 { panic!() };
            }
        }
    }
}
