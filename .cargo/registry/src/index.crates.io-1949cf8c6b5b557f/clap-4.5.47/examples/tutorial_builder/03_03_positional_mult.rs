use clap::{command, Arg, ArgAction};

fn main() {
    let matches = command!() // requires `cargo` feature
        .arg(Arg::new("name").action(ArgAction::Append))
        .get_matches();

    let args = matches
        .get_many::<String>("name")
        .unwrap_or_default()
        .map(|v| v.as_str())
        .collect::<Vec<_>>();

    println!("names: {:?}", &args);
}
