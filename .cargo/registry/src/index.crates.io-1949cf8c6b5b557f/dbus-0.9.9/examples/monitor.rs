use std::time::Duration;

use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus::Message;

// This programs implements the equivalent of running the "dbus-monitor" tool
fn main() {
    // Very simple argument parsing.
    let use_system_bus = std::env::args().into_iter().any(|a| a == "--system");

    // First open up a connection to the desired bus.
    let conn = (if use_system_bus { Connection::new_system() } else { Connection::new_session() }).expect("D-Bus connection failed");

    // Second create a rule to match messages we want to receive; in this example we add no
    // further requirements, so all messages will match
    let rule = MatchRule::new();

    // Try matching using new scheme
    let proxy = conn.with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", Duration::from_millis(5000));
    let result: Result<(), dbus::Error> =
        proxy.method_call("org.freedesktop.DBus.Monitoring", "BecomeMonitor", (vec![rule.match_str()], 0u32));

    match result {
        // BecomeMonitor was successful, start listening for messages
        Ok(_) => {
            conn.start_receive(
                rule,
                Box::new(|msg, _| {
                    handle_message(&msg);
                    true
                }),
            );
        }
        // BecomeMonitor failed, fallback to using the old scheme
        Err(e) => {
            eprintln!("Failed to BecomeMonitor: '{}', falling back to eavesdrop", e);

            // First, we'll try "eavesdrop", which as the name implies lets us receive
            // *all* messages, not just ours.
            let rule_with_eavesdrop = {
                let mut rule = rule.clone();
                rule.eavesdrop = true;
                rule
            };

            let result = conn.add_match(rule_with_eavesdrop, |_: (), _, msg| {
                handle_message(&msg);
                true
            });

            match result {
                Ok(_) => {
                    // success, we're now listening
                }
                // This can sometimes fail, for example when listening to the system bus as a non-root user.
                // So, just like `dbus-monitor`, we attempt to fallback without `eavesdrop=true`:
                Err(e) => {
                    eprintln!("Failed to eavesdrop: '{}', trying without it", e);
                    conn.add_match(rule, |_: (), _, msg| {
                        handle_message(&msg);
                        true
                    })
                    .expect("add_match failed");
                }
            }
        }
    }

    // Loop and print out all messages received (using handle_message()) as they come.
    // Some can be quite large, e.g. if they contain embedded images..
    loop {
        conn.process(Duration::from_millis(1000)).unwrap();
    }
}

fn handle_message(msg: &Message) {
    println!("Got message: {:?}", msg);
}
