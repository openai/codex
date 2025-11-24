//! Connection base / building block.
//!
//! Contains some helper structs and traits common to all Connection types.-

use crate::{Message, to_c_str, c_str_to_slice, MessageType};
use crate::message::MatchRule;

#[cfg(not(feature = "native-channel"))]
mod ffichannel;
#[cfg(not(feature = "native-channel"))]
pub use ffichannel::Channel;

#[cfg(feature = "native-channel")]
mod nativechannel;
#[cfg(feature = "native-channel")]
pub use nativechannel::Channel;


/// Which bus to connect to
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum BusType {
    /// The Session bus - local to every logged in session
    Session = ffi::DBusBusType::Session as isize,
    /// The system wide bus
    System = ffi::DBusBusType::System as isize,
    /// The bus that started us, if any
    Starter = ffi::DBusBusType::Starter as isize,
}

/// Platform-specific file descriptor type
#[cfg(unix)]
pub type WatchFd = std::os::unix::io::RawFd;

/// Platform-specific file descriptor type
#[cfg(windows)]
pub type WatchFd = std::os::windows::io::RawSocket;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// A file descriptor, and an indication whether it should be read from, written to, or both.
pub struct Watch {
    /// File descriptor
    pub fd: WatchFd,
    /// True if wakeup should happen when the file descriptor is ready for reading
    pub read: bool,
    /// True if wakeup should happen when the file descriptor is ready for writing
    pub write: bool,
}

/// Abstraction over different connections that send data
pub trait Sender {
    /// Schedules a message for sending.
    ///
    /// Returns a serial number than can be used to match against a reply.
    fn send(&self, msg: Message) -> Result<u32, ()>;
}

/// Use in case you don't want the send the message, but just collect it instead.
impl Sender for std::cell::RefCell<Vec<Message>> {
    fn send(&self, msg: Message) -> Result<u32, ()> {
        self.borrow_mut().push(msg);
        Ok(0)
    }
}

/// Use in case you don't want the send the message, but just collect it instead.
impl Sender for std::sync::Mutex<Vec<Message>> {
    fn send(&self, msg: Message) -> Result<u32, ()> {
        self.lock().unwrap().push(msg);
        Ok(0)
    }
}

/// Token used to identify a callback in the MatchingReceiver trait
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Token(pub usize);

/// Abstraction over different connections that receive data
pub trait MatchingReceiver {
    /// Type of callback
    type F;
    /// Add a callback to be called in case a message matches.
    ///
    /// Returns an id that can be used to remove the callback.
    fn start_receive(&self, m: MatchRule<'static>, f: Self::F) -> Token;
    /// Remove a previously added callback.
    fn stop_receive(&self, id: Token) -> Option<(MatchRule<'static>, Self::F)>;
}

impl Sender for Channel {
    fn send(&self, msg: Message) -> Result<u32, ()> { Channel::send(self, msg) }
}

/// Handles what we need to be a good D-Bus citizen.
///
/// Call this if you have not handled the message yourself:
/// * It handles calls to org.freedesktop.DBus.Peer.
/// * For other method calls, it sends an error reply back that the method was unknown.
pub fn default_reply(m: &Message) -> Option<Message> {
    peer(&m).or_else(|| unknown_method(&m))
}

/// Replies if this is a call to org.freedesktop.DBus.Peer, otherwise returns None.
fn peer(m: &Message) -> Option<Message> {
    if let Some(intf) = m.interface() {
        if &*intf != "org.freedesktop.DBus.Peer" { return None; }
        if let Some(method) = m.member() {
            if &*method == "Ping" { return Some(m.method_return()) }
            if &*method == "GetMachineId" {
                let mut r = m.method_return();
                unsafe {
                    let id = ffi::dbus_get_local_machine_id();
                    if !id.is_null() {
                        r = r.append1(c_str_to_slice(&(id as *const _)).unwrap());
                        ffi::dbus_free(id as *mut _);
                        return Some(r)
                    }
                }
                return Some(m.error(&"org.freedesktop.DBus.Error.Failed".into(), &to_c_str("Failed to retrieve UUID")))
            }
        }
        Some(m.error(&"org.freedesktop.DBus.Error.UnknownMethod".into(), &to_c_str("Method does not exist")))
    } else { None }
}

/// For method calls, it replies that the method was unknown, otherwise returns None.
fn unknown_method(m: &Message) -> Option<Message> {
    if m.msg_type() != MessageType::MethodCall { return None; }
    // if m.get_no_reply() { return None; } // The reference implementation does not do this?
    Some(m.error(&"org.freedesktop.DBus.Error.UnknownMethod".into(), &to_c_str("Path, Interface, or Method does not exist")))
}

#[test]
fn test_channel_send_sync() {
    fn is_send<T: Send>(_: &T) {}
    fn is_sync<T: Sync>(_: &T) {}
    let c = Channel::get_private(BusType::Session).unwrap();
    is_send(&c);
    is_sync(&c);
}

#[test]
fn channel_simple_test() {
    let mut c = Channel::get_private(BusType::Session).unwrap();
    assert!(c.is_connected());
    c.set_watch_enabled(true);
    let fd = c.watch();
    println!("{:?}", fd);
    let m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "ListNames").unwrap();
    let reply = c.send(m).unwrap();
    let my_name = c.unique_name().unwrap();
    loop {
        while let Some(mut msg) = c.pop_message() {
            println!("{:?}", msg);
            if msg.get_reply_serial() == Some(reply) {
                let r = msg.as_result().unwrap();
                let z: crate::arg::Array<&str, _>  = r.get1().unwrap();
                for n in z {
                    println!("{}", n);
                    if n == my_name { return; } // Hooray, we found ourselves!
                }
                assert!(false);
            } else if let Some(r) = default_reply(&msg) {
                c.send(r).unwrap();
            }
        }
        c.read_write(Some(std::time::Duration::from_millis(100))).unwrap();
    }
}

#[test]
fn test_bus_type_is_compatible_with_set() {
    use std::collections::HashSet;

    let mut set: HashSet<BusType> = HashSet::new();
    set.insert(BusType::Starter);
    set.insert(BusType::Starter);

    assert_eq!(set.len(), 1);
    assert!(!set.contains(&BusType::Session));
    assert!(!set.contains(&BusType::System));
    assert!(set.contains(&BusType::Starter));
}


#[test]
fn watchmap() {
    let mut c = Channel::get_private(BusType::Session).unwrap();
    c.set_watch_enabled(true);
    let w = c.watch();
    assert_eq!(w.write, false);
    assert_eq!(w.read, true);
    c.set_watch_enabled(false);
    println!("{:?}", w);
    c.set_watch_enabled(true);
}
