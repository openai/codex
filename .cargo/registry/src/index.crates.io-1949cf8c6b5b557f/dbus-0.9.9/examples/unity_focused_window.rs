extern crate dbus;

// Tracks currently focused window under the Unity desktop by listening to the
// FocusedWindowChanged signal. The signal contains "window_id", "app_id" and "stage",
// we print only "app_id".

use dbus::{ffidisp::Connection, Message, MessageType};

fn focus_msg(msg: &Message) -> Option<&str> {
    if msg.msg_type() != MessageType::Signal { return None };
    if &*msg.interface().unwrap() != "com.canonical.Unity.WindowStack" { return None };
    if &*msg.member().unwrap() != "FocusedWindowChanged" { return None };
    let (_, app) = msg.get2::<u32, &str>();
    app
}

fn main() {
    let c = Connection::new_session().unwrap();
    c.add_match("interface='com.canonical.Unity.WindowStack',member='FocusedWindowChanged'").unwrap();

    loop {
        if let Some(msg) = c.incoming(1000).next() {
            if let Some(app) = focus_msg(&msg) {
                println!("{} has now focus.", app);
            }
        }
    }
}
