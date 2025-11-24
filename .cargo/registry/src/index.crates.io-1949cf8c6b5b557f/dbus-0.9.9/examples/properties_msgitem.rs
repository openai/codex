use dbus::ffidisp::Connection;
use dbus::arg::messageitem::Props;

fn main() {
    let c = Connection::new_system().unwrap();
    let p = Props::new(&c, "org.freedesktop.PolicyKit1", "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority", 10000);
    println!("BackendVersion: {:?}", p.get("BackendVersion").unwrap())
}
