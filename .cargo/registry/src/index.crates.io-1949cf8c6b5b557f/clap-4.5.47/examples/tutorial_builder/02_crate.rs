use clap::{arg, command};

fn main() {
    // requires `cargo` feature, reading name, version, author, and description from `Cargo.toml`
    let matches = command!()
        .arg(arg!(--two <VALUE>).required(true))
        .arg(arg!(--one <VALUE>).required(true))
        .get_matches();

    println!(
        "two: {:?}",
        matches.get_one::<String>("two").expect("required")
    );
    println!(
        "one: {:?}",
        matches.get_one::<String>("one").expect("required")
    );
}
