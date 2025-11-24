use clap::{command, Arg};

fn main() {
    let matches = command!() // requires `cargo` feature
        .arg(Arg::new("name").short('n').long("name"))
        .get_matches();

    println!("name: {:?}", matches.get_one::<String>("name"));
}
