use clap::{arg, command, value_parser};

fn main() {
    let matches = command!() // requires `cargo` feature
        .arg(
            arg!([PORT])
                .value_parser(value_parser!(u16))
                .default_value("2020"),
        )
        .get_matches();

    println!(
        "port: {:?}",
        matches
            .get_one::<u16>("PORT")
            .expect("default ensures there is always a value")
    );
}
