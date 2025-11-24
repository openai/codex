use clap::{arg, command};

fn main() {
    let matches = command!() // requires `cargo` feature
        .arg(
            arg!(<MODE>)
                .help("What mode to run the program in")
                .value_parser(["fast", "slow"]),
        )
        .get_matches();

    // Note, it's safe to call unwrap() because the arg is required
    match matches
        .get_one::<String>("MODE")
        .expect("'MODE' is required and parsing will fail if its missing")
        .as_str()
    {
        "fast" => {
            println!("Hare");
        }
        "slow" => {
            println!("Tortoise");
        }
        _ => unreachable!(),
    }
}
