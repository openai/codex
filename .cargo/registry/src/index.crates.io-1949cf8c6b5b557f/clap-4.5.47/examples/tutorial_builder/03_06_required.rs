use clap::{command, Arg};

fn main() {
    let matches = command!() // requires `cargo` feature
        .arg(Arg::new("name").required(true))
        .get_matches();

    println!(
        "name: {:?}",
        matches
            .get_one::<String>("name")
            .expect("clap `required` ensures its present")
    );
}
