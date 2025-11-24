// A simple cmdline app to change the screen brightness on laptops.
//
// NB: It only works on a GNOME (based) system.
//
// Usage is simple. Either pass a '+' as argument or no argument on commandline, and it increases
// the brightness by 5%. Pass '-' for decreasing it by 5%.

fn main() {
    tracing_subscriber::fmt::init();
    let connection = zbus::blocking::Connection::session().unwrap();

    let method = match std::env::args().nth(1) {
        Some(s) => {
            if s == "+" {
                "StepUp"
            } else if s == "-" {
                "StepDown"
            } else {
                panic!("Expected either '+' or '-' argument. Got: {}", s);
            }
        }
        None => "StepUp",
    };

    let reply = connection
        .call_method(
            Some("org.gnome.SettingsDaemon.Power"),
            "/org/gnome/SettingsDaemon/Power",
            Some("org.gnome.SettingsDaemon.Power.Screen"),
            method,
            &(),
        )
        .unwrap();

    let (percent, _) = reply.body().deserialize::<(i32, &str)>().unwrap();
    println!("New level: {percent}%");
}
