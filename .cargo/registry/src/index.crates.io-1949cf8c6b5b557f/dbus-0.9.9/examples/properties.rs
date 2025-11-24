use dbus::{blocking::Connection, arg};
use std::time::Duration;

fn print_refarg(value: &dyn arg::RefArg) {
    // We don't know what type the value is. We'll try a few and fall back to
    // debug printing if the value is more complex than that.
    if let Some(s) = value.as_str() { println!("{}", s); }
    else if let Some(i) = value.as_i64() { println!("{}", i); }
    else { println!("{:?}", value); }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to server and create a proxy object. A proxy implements several interfaces,
    // in this case we'll use OrgFreedesktopDBusProperties, which allows us to call "get".
    let c = Connection::new_session()?;
    let p = c.with_proxy("org.mpris.MediaPlayer2.rhythmbox", "/org/mpris/MediaPlayer2", Duration::from_millis(5000));
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;

    // The Metadata property is a Dict<String, Variant>.

    // Option 1: we can get the dict straight into a hashmap, like this:

    let metadata: arg::PropMap = p.get("org.mpris.MediaPlayer2.Player", "Metadata")?;

    println!("Option 1:");

    // We now iterate over the hashmap.
    for (key, value) in metadata.iter() {
        print!("  {}: ", key);
        print_refarg(&value);
    }

    // As an alternative, if we just want a specific property and know the type of it, we can use
    // prop_cast:
    let title: Option<&String> = arg::prop_cast(&metadata, "xesam:title");
    if let Some(title) = title {
        println!("The title is: {}", title);
    }

    // Option 2: we can get the entire dict as a RefArg and get the values out by iterating over it.

    let metadata: Box<dyn arg::RefArg> = p.get("org.mpris.MediaPlayer2.Player", "Metadata")?;

    // When using "as_iter()" for a dict, we'll get one key, it's value, next key, it's value, etc.
    let mut iter = metadata.as_iter().unwrap();

    println!("Option 2:");
    while let Some(key) = iter.next() {
        // Printing the key is easy, since we know it's a String.
        print!("  {}: ", key.as_str().unwrap());
        let value = iter.next().unwrap();
        print_refarg(&value);
    }

    Ok(())
}
