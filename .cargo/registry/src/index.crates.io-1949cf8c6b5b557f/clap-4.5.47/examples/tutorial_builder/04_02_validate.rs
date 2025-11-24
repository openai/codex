use std::ops::RangeInclusive;

use clap::{arg, command};

fn main() {
    let matches = command!() // requires `cargo` feature
        .arg(
            arg!(<PORT>)
                .help("Network port to use")
                .value_parser(port_in_range),
        )
        .get_matches();

    // Note, it's safe to call unwrap() because the arg is required
    let port: u16 = *matches
        .get_one::<u16>("PORT")
        .expect("'PORT' is required and parsing will fail if its missing");
    println!("PORT = {port}");
}

const PORT_RANGE: RangeInclusive<usize> = 1..=65535;

fn port_in_range(s: &str) -> Result<u16, String> {
    let port: usize = s
        .parse()
        .map_err(|_| format!("`{s}` isn't a port number"))?;
    if PORT_RANGE.contains(&port) {
        Ok(port as u16)
    } else {
        Err(format!(
            "port not in range {}-{}",
            PORT_RANGE.start(),
            PORT_RANGE.end()
        ))
    }
}
