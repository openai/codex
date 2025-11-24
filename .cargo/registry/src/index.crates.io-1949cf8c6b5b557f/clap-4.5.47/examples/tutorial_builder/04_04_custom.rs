use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::{arg, command, value_parser, ArgAction};

fn main() {
    // Create application like normal
    let mut cmd = command!() // requires `cargo` feature
        // Add the version arguments
        .arg(arg!(--"set-ver" <VER> "set version manually"))
        .arg(arg!(--major         "auto inc major").action(ArgAction::SetTrue))
        .arg(arg!(--minor         "auto inc minor").action(ArgAction::SetTrue))
        .arg(arg!(--patch         "auto inc patch").action(ArgAction::SetTrue))
        // Arguments can also be added to a group individually, these two arguments
        // are part of the "input" group which is not required
        .arg(arg!([INPUT_FILE] "some regular input").value_parser(value_parser!(PathBuf)))
        .arg(
            arg!(--"spec-in" <SPEC_IN> "some special input argument")
                .value_parser(value_parser!(PathBuf)),
        )
        // Now let's assume we have a -c [config] argument which requires one of
        // (but **not** both) the "input" arguments
        .arg(arg!(config: -c <CONFIG>).value_parser(value_parser!(PathBuf)));
    let matches = cmd.get_matches_mut();

    // Let's assume the old version 1.2.3
    let mut major = 1;
    let mut minor = 2;
    let mut patch = 3;

    // See if --set-ver was used to set the version manually
    let version = if let Some(ver) = matches.get_one::<String>("set-ver") {
        if matches.get_flag("major") || matches.get_flag("minor") || matches.get_flag("patch") {
            cmd.error(
                ErrorKind::ArgumentConflict,
                "Can't do relative and absolute version change",
            )
            .exit();
        }
        ver.to_string()
    } else {
        // Increment the one requested (in a real program, we'd reset the lower numbers)
        let (maj, min, pat) = (
            matches.get_flag("major"),
            matches.get_flag("minor"),
            matches.get_flag("patch"),
        );
        match (maj, min, pat) {
            (true, false, false) => major += 1,
            (false, true, false) => minor += 1,
            (false, false, true) => patch += 1,
            _ => {
                cmd.error(
                    ErrorKind::ArgumentConflict,
                    "Can only modify one version field",
                )
                .exit();
            }
        };
        format!("{major}.{minor}.{patch}")
    };

    println!("Version: {version}");

    // Check for usage of -c
    if matches.contains_id("config") {
        let input = matches
            .get_one::<PathBuf>("INPUT_FILE")
            .or_else(|| matches.get_one::<PathBuf>("spec-in"))
            .unwrap_or_else(|| {
                cmd.error(
                    ErrorKind::MissingRequiredArgument,
                    "INPUT_FILE or --spec-in is required when using --config",
                )
                .exit()
            })
            .display();
        println!(
            "Doing work using input {} and config {}",
            input,
            matches.get_one::<PathBuf>("config").unwrap().display()
        );
    }
}
