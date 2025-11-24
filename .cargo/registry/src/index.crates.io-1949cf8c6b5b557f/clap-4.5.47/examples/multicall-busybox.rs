use std::path::PathBuf;
use std::process::exit;

use clap::{value_parser, Arg, ArgAction, Command};

fn applet_commands() -> [Command; 2] {
    [
        Command::new("true").about("does nothing successfully"),
        Command::new("false").about("does nothing unsuccessfully"),
    ]
}

fn main() {
    let cmd = Command::new(env!("CARGO_CRATE_NAME"))
        .multicall(true)
        .subcommand(
            Command::new("busybox")
                .arg_required_else_help(true)
                .subcommand_value_name("APPLET")
                .subcommand_help_heading("APPLETS")
                .arg(
                    Arg::new("install")
                        .long("install")
                        .help("Install hardlinks for all subcommands in path")
                        .exclusive(true)
                        .action(ArgAction::Set)
                        .default_missing_value("/usr/local/bin")
                        .value_parser(value_parser!(PathBuf)),
                )
                .subcommands(applet_commands()),
        )
        .subcommands(applet_commands());

    let matches = cmd.get_matches();
    let mut subcommand = matches.subcommand();
    if let Some(("busybox", cmd)) = subcommand {
        if cmd.contains_id("install") {
            unimplemented!("Make hardlinks to the executable here");
        }
        subcommand = cmd.subcommand();
    }
    match subcommand {
        Some(("false", _)) => exit(1),
        Some(("true", _)) => exit(0),
        _ => unreachable!("parser should ensure only valid subcommand names are used"),
    }
}
