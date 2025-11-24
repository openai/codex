use crate::arg;
use crate::{Message, MessageType};
use crate::message::MatchRule;
use crate::strings::{BusName, Path, Interface, Member};

/// Helper methods for structs representing a Signal
///
/// # Example
///
/// Listen to InterfacesRemoved signal from org.bluez.obex.
///
/// ```rust,no_run
/// use dbus::blocking::Connection;
/// use dbus::message::SignalArgs;
/// use dbus::blocking::stdintf::org_freedesktop_dbus::ObjectManagerInterfacesRemoved as IR;
/// use std::time::Duration;
///
/// let c = Connection::new_session().unwrap();
/// // Add a match for this signal
/// let mr = IR::match_rule(Some(&"org.bluez.obex".into()), None).static_clone();
/// c.add_match(mr, |ir: IR, _, _| {
///      println!("Interfaces {:?} have been removed from bluez on path {}.", ir.interfaces, ir.object);
///      true
/// });
///
/// // Wait for the signal to arrive.
/// loop { c.process(Duration::from_millis(1000)).unwrap(); }
/// ```

pub trait SignalArgs {
    /// D-Bus name of signal
    const NAME: &'static str;

    /// D-Bus name of interface this signal belongs to
    const INTERFACE: &'static str;

    /// Returns a message that emits the signal.
    fn to_emit_message(&self, path: &Path) -> Message where Self: arg::AppendAll {
        let mut m = Message::signal(path, &Interface::from(Self::INTERFACE), &Member::from(Self::NAME));
        arg::AppendAll::append(self, &mut arg::IterAppend::new(&mut m));
        m
    }

    /// If the message is a signal of the correct type, return its arguments, otherwise return None.
    ///
    /// This does not check sender and path of the message, which is likely relevant to you as well.
    #[allow(clippy::if_same_then_else)]
    fn from_message(m: &Message) -> Option<Self> where Self: Sized + arg::ReadAll {
        if m.msg_type() != MessageType::Signal { None }
        else if m.interface().as_ref().map(|x| &**x) != Some(Self::INTERFACE) { None }
        else if m.member().as_ref().map(|x| &**x) != Some(Self::NAME) { None }
        else {
            arg::ReadAll::read(&mut m.iter_init()).ok()
        }
    }

    /// Returns a match rule matching this signal.
    ///
    /// If sender and/or path is None, matches all senders and/or paths.
    fn match_rule<'a>(sender: Option<&'a BusName>, path: Option<&'a Path>) -> MatchRule<'a> {
        let mut m: MatchRule = Default::default();
        m.sender = sender.cloned();
        m.path = path.cloned();
        m.msg_type = Some(MessageType::Signal);
        m.interface = Some(Self::INTERFACE.into());
        m.member = Some(Self::NAME.into());
        m
    }


    /// Returns a string that can be sent to `Connection::add_match`.
    ///
    /// If sender and/or path is None, matches all senders and/or paths.
    fn match_str(sender: Option<&BusName>, path: Option<&Path>) -> String {
        Self::match_rule(sender, path).match_str()
    }
}

#[test]
fn intf_removed() {
    use crate::blocking::LocalConnection;
    use crate::blocking::stdintf::org_freedesktop_dbus::ObjectManagerInterfacesRemoved as IR;
    use std::{time::Duration, cell::Cell, rc::Rc};
    let c = LocalConnection::new_session().unwrap();

    let mr = IR::match_rule(Some(&c.unique_name().into()), Some(&"/hello".into())).static_clone();
    println!("Match: {:?}", mr);

    let ir = IR { object: "/hello".into(), interfaces: vec!("ABC.DEF".into(), "GHI.JKL".into()) };
    let ir_msg = ir.to_emit_message(&"/hello".into());
    let done = Rc::new(Cell::new(false));
    let done2 = done.clone();

    c.add_match(mr, move |ir2: IR, _, _| {
        assert_eq!(ir2.object, ir.object);
        assert_eq!(ir2.interfaces, ir.interfaces);
        done2.set(true);
        false
    }).unwrap();
    use crate::channel::Sender;
    c.send(ir_msg).expect("Failed to send message");
    while !done.get() { c.process(Duration::from_millis(1000)).unwrap(); }
}
