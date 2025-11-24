use clap::{arg, command, value_parser};

fn main() {
    let matches = command!() // requires `cargo` feature
        .arg(
            arg!(<PORT>)
                .help("Network port to use")
                .value_parser(value_parser!(u16).range(1..)),
        )
        .get_matches();

    // Note, it's safe to call unwrap() because the arg is required
    let port: u16 = *matches
        .get_one::<u16>("PORT")
        .expect("'PORT' is required and parsing will fail if its missing");
    println!("PORT = {port}");
}
