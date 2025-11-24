use std::path::PathBuf;

use clap::{arg, command, value_parser, ArgAction, ArgGroup};

fn main() {
    // Create application like normal
    let matches = command!() // requires `cargo` feature
        // Add the version arguments
        .arg(arg!(--"set-ver" <VER> "set version manually"))
        .arg(arg!(--major         "auto inc major").action(ArgAction::SetTrue))
        .arg(arg!(--minor         "auto inc minor").action(ArgAction::SetTrue))
        .arg(arg!(--patch         "auto inc patch").action(ArgAction::SetTrue))
        // Create a group, make it required, and add the above arguments
        .group(
            ArgGroup::new("vers")
                .required(true)
                .args(["set-ver", "major", "minor", "patch"]),
        )
        // Arguments can also be added to a group individually, these two arguments
        // are part of the "input" group which is not required
        .arg(
            arg!([INPUT_FILE] "some regular input")
                .value_parser(value_parser!(PathBuf))
                .group("input"),
        )
        .arg(
            arg!(--"spec-in" <SPEC_IN> "some special input argument")
                .value_parser(value_parser!(PathBuf))
                .group("input"),
        )
        // Now let's assume we have a -c [config] argument which requires one of
        // (but **not** both) the "input" arguments
        .arg(
            arg!(config: -c <CONFIG>)
                .value_parser(value_parser!(PathBuf))
                .requires("input"),
        )
        .get_matches();

    // Let's assume the old version 1.2.3
    let mut major = 1;
    let mut minor = 2;
    let mut patch = 3;

    // See if --set-ver was used to set the version manually
    let version = if let Some(ver) = matches.get_one::<String>("set-ver") {
        ver.to_owned()
    } else {
        // Increment the one requested (in a real program, we'd reset the lower numbers)
        let (maj, min, pat) = (
            matches.get_flag("major"),
            matches.get_flag("minor"),
            matches.get_flag("patch"),
        );
        match (maj, min, pat) {
            (true, _, _) => major += 1,
            (_, true, _) => minor += 1,
            (_, _, true) => patch += 1,
            _ => unreachable!(),
        };
        format!("{major}.{minor}.{patch}")
    };

    println!("Version: {version}");

    // Check for usage of -c
    if matches.contains_id("config") {
        let input = matches
            .get_one::<PathBuf>("INPUT_FILE")
            .unwrap_or_else(|| matches.get_one::<PathBuf>("spec-in").unwrap())
            .display();
        println!(
            "Doing work using input {} and config {}",
            input,
            matches.get_one::<PathBuf>("config").unwrap().display()
        );
    }
}
