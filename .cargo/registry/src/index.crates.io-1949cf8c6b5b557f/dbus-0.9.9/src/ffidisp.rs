//! A connection that uses FFI callbacks to dispatch messages.
//!
//! This is the legacy design used up to 0.6.x. It is not recommended for new development.


use super::{Error, ffi, Message, MessageType};
use crate::strings::{BusName, Path, Member, Interface};
use crate::arg::{AppendAll, ReadAll, IterAppend};
use crate::message::SignalArgs;

pub mod stdintf;

mod connection;

pub use connection::{Connection, ConnMsgs};

/// A convenience struct that wraps connection, destination and path.
///
/// Useful if you want to make many method calls to the same destination path.
#[derive(Clone, Debug)]
pub struct ConnPath<'a, C> {
    /// Some way to access the connection, e g a &Connection or Rc<Connection>
    pub conn: C,
    /// Destination, i e what D-Bus service you're communicating with
    pub dest: BusName<'a>,
    /// Object path on the destination
    pub path: Path<'a>,
    /// Timeout in milliseconds for blocking method calls
    pub timeout: i32,
}

impl<'a, C: ::std::ops::Deref<Target=Connection>> ConnPath<'a, C> {
    /// Make a D-Bus method call, where you can append arguments inside the closure.
    pub fn method_call_with_args<F: FnOnce(&mut Message)>(&self, i: &Interface, m: &Member, f: F) -> Result<Message, Error> {
        let mut msg = Message::method_call(&self.dest, &self.path, i, m);
        f(&mut msg);
        self.conn.send_with_reply_and_block(msg, self.timeout)
    }

    /// Emit a D-Bus signal, where you can append arguments inside the closure.
    pub fn signal_with_args<F: FnOnce(&mut Message)>(&self, i: &Interface, m: &Member, f: F) -> Result<u32, Error> {
        let mut msg = Message::signal(&self.path, i, m);
        f(&mut msg);
        self.conn.send(msg).map_err(|_| Error::new_failed("Sending signal failed"))
    }

    /// Emit a D-Bus signal, where the arguments are in a struct.
    pub fn emit<S: SignalArgs + AppendAll>(&self, signal: &S) -> Result<u32, Error> {
        let msg = signal.to_emit_message(&self.path);
        self.conn.send(msg).map_err(|_| Error::new_failed("Sending signal failed"))
    }

    /// Make a method call using typed input and output arguments.
    ///
    /// # Example
    ///
    /// ```
    /// use dbus::ffidisp::{Connection, BusType};
    ///
    /// let conn = Connection::get_private(BusType::Session)?;
    /// let dest = conn.with_path("org.freedesktop.DBus", "/", 5000);
    /// let (has_owner,): (bool,) = dest.method_call("org.freedesktop.DBus", "NameHasOwner", ("dummy.name.without.owner",))?;
    /// assert_eq!(has_owner, false);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn method_call<'i, 'm, R: ReadAll, A: AppendAll, I: Into<Interface<'i>>, M: Into<Member<'m>>>(&self, i: I, m: M, args: A) -> Result<R, Error> {
        let mut r = self.method_call_with_args(&i.into(), &m.into(), |mut msg| {
            args.append(&mut IterAppend::new(&mut msg));
        })?;
        r.as_result()?;
        Ok(R::read(&mut r.iter_init())?)
    }
}

/// The type of function to use for replacing the message callback.
///
/// See the documentation for Connection::replace_message_callback for more information.
pub type MessageCallback = Box<dyn FnMut(&Connection, Message) -> bool + 'static>;

pub use crate::ffi::DBusRequestNameReply as RequestNameReply;
pub use crate::ffi::DBusReleaseNameReply as ReleaseNameReply;
pub use crate::ffi::DBusBusType as BusType;

mod watch;

pub use self::watch::{Watch, WatchEvent};
use watch::WatchList;

#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
/// Flags to use for Connection::register_name.
///
/// More than one flag can be specified, if so just add their values.
pub enum NameFlag {
    /// Allow another service to become the primary owner if requested
    AllowReplacement = ffi::DBUS_NAME_FLAG_ALLOW_REPLACEMENT as isize,
    /// Request to replace the current primary owner
    ReplaceExisting = ffi::DBUS_NAME_FLAG_REPLACE_EXISTING as isize,
    /// If we can not become the primary owner do not place us in the queue
    DoNotQueue = ffi::DBUS_NAME_FLAG_DO_NOT_QUEUE as isize,
}

impl NameFlag {
    /// u32 value of flag.
    pub fn value(self) -> u32 { self as u32 }
}

/// When listening for incoming events on the D-Bus, this enum will tell you what type
/// of incoming event has happened.
#[derive(Debug)]
pub enum ConnectionItem {
    /// No event between now and timeout
    Nothing,
    /// Incoming method call
    MethodCall(Message),
    /// Incoming signal
    Signal(Message),
    /// Incoming method return, including method return errors (mostly used for Async I/O)
    MethodReturn(Message),
}

impl From<Message> for ConnectionItem {
    fn from(m: Message) -> Self {
        let mtype = m.msg_type();
        match mtype {
            MessageType::Signal => ConnectionItem::Signal(m),
            MessageType::MethodReturn => ConnectionItem::MethodReturn(m),
            MessageType::Error => ConnectionItem::MethodReturn(m),
            MessageType::MethodCall => ConnectionItem::MethodCall(m),
        }
    }
}




#[derive(Clone, Debug)]
/// Type of messages to be handled by a MsgHandler.
///
/// Note: More variants can be added in the future; but unless you're writing your own D-Bus engine
/// you should not have to match on these anyway.
pub enum MsgHandlerType {
    /// Handle all messages
    All,
    /// Handle only messages of a specific type
    MsgType(MessageType),
    /// Handle only method replies with this serial number
    Reply(u32),
}

impl MsgHandlerType {
    fn matches_msg(&self, m: &Message) -> bool {
        match *self {
            MsgHandlerType::All => true,
            MsgHandlerType::MsgType(t) => m.msg_type() == t,
            MsgHandlerType::Reply(serial) => {
                let t = m.msg_type();
                ((t == MessageType::MethodReturn) || (t == MessageType::Error)) && (m.get_reply_serial() == Some(serial))
            }
        }
    }
}

/// A trait for handling incoming messages.
pub trait MsgHandler {
    /// Type of messages for which the handler will be called
    ///
    /// Note: The return value of this function might be cached, so it must return the same value all the time.
    fn handler_type(&self) -> MsgHandlerType;

    /// Function to be called if the message matches the MsgHandlerType
    fn handle_msg(&mut self, _msg: &Message) -> Option<MsgHandlerResult> { None }
}

/// The result from MsgHandler::handle.
#[derive(Debug, Default)]
pub struct MsgHandlerResult {
    /// Indicates that the message has been dealt with and should not be processed further.
    pub handled: bool,
    /// Indicates that this MsgHandler no longer wants to receive messages and should be removed.
    pub done: bool,
    /// Messages to send (e g, a reply to a method call)
    pub reply: Vec<Message>,
}


type MsgHandlerList = Vec<Box<dyn MsgHandler>>;

/// The struct returned from `Connection::send_and_reply`.
///
/// It implements the `MsgHandler` trait so you can use `Connection::add_handler`.
pub struct MessageReply<F>(Option<F>, u32);

impl<'a, F: FnOnce(Result<&Message, Error>) + 'a> MsgHandler for MessageReply<F> {
    fn handler_type(&self) -> MsgHandlerType { MsgHandlerType::Reply(self.1) }
    fn handle_msg(&mut self, msg: &Message) -> Option<MsgHandlerResult> {
        let e = match msg.msg_type() {
            MessageType::MethodReturn => Ok(msg),
            MessageType::Error => Err(msg.set_error_from_msg().unwrap_err()),
            _ => unreachable!(),
        };
        debug_assert_eq!(msg.get_reply_serial(), Some(self.1));
        self.0.take().unwrap()(e);
        return Some(MsgHandlerResult { handled: true, done: true, reply: Vec::new() })
    }
}

#[cfg(test)]
mod test {
    use super::{Connection, BusType, ConnectionItem, NameFlag,
        RequestNameReply, ReleaseNameReply};
    use crate::Message;

    #[test]
    fn connection() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let n = c.unique_name();
        assert!(n.starts_with(":1."));
        println!("Connected to DBus, unique name: {}", n);
    }

    #[test]
    fn invalid_message() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let m = Message::new_method_call("foo.bar", "/", "foo.bar", "FooBar").unwrap();
        let e = c.send_with_reply_and_block(m, 2000).err().unwrap();
        assert!(e.name().unwrap() == "org.freedesktop.DBus.Error.ServiceUnknown");
    }

    #[test]
    fn object_path() {
        use  std::sync::mpsc;
        let (tx, rx) = mpsc::channel();
        let thread = ::std::thread::spawn(move || {
            let c = Connection::get_private(BusType::Session).unwrap();
            c.register_object_path("/hello").unwrap();
            // println!("Waiting...");
            tx.send(c.unique_name()).unwrap();
            for n in c.iter(1000) {
                // println!("Found message... ({})", n);
                match n {
                    ConnectionItem::MethodCall(ref m) => {
                        let reply = Message::new_method_return(m).unwrap();
                        c.send(reply).unwrap();
                        break;
                    }
                    _ => {}
                }
            }
            c.unregister_object_path("/hello");
        });

        let c = Connection::get_private(BusType::Session).unwrap();
        let n = rx.recv().unwrap();
        let m = Message::new_method_call(&n, "/hello", "com.example.hello", "Hello").unwrap();
        println!("Sending...");
        let r = c.send_with_reply_and_block(m, 8000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
        thread.join().unwrap();

    }

    #[test]
    fn register_name() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let n = format!("com.example.hello.test.register_name");
        assert_eq!(c.register_name(&n, NameFlag::ReplaceExisting as u32).unwrap(), RequestNameReply::PrimaryOwner);
        assert_eq!(c.release_name(&n).unwrap(), ReleaseNameReply::Released);
    }

    #[test]
    fn signal() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let iface = "com.example.signaltest";
        let mstr = format!("interface='{}',member='ThisIsASignal'", iface);
        c.add_match(&mstr).unwrap();
        let m = Message::new_signal("/mysignal", iface, "ThisIsASignal").unwrap();
        let uname = c.unique_name();
        c.send(m).unwrap();
        for n in c.iter(1000) {
            match n {
                ConnectionItem::Signal(s) => {
                    let (p, i, m) = (s.path(), s.interface(), s.member());
                    match (&*p.unwrap(), &*i.unwrap(), &*m.unwrap()) {
                        ("/mysignal", "com.example.signaltest", "ThisIsASignal") => {
                            assert_eq!(&*s.sender().unwrap(), &*uname);
                            break;
                        },
                        (_, _, _) => println!("Other signal: {:?}", s),
                    }
                }
                _ => {},
            }
        }
        c.remove_match(&mstr).unwrap();
    }


    #[test]
    fn watch() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let d = c.watch_fds();
        assert!(d.len() > 0);
        println!("Fds to watch: {:?}", d);
    }
}
