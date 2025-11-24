use clap::{arg, command, ArgAction};

fn main() {
    let matches = command!() // requires `cargo` feature
        .next_line_help(true)
        .arg(arg!(--two <VALUE>).required(true).action(ArgAction::Set))
        .arg(arg!(--one <VALUE>).required(true).action(ArgAction::Set))
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
