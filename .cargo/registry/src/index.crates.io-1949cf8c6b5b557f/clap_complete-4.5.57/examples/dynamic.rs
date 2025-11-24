fn command() -> clap::Command {
    clap::Command::new("dynamic")
        .arg(
            clap::Arg::new("input")
                .long("input")
                .short('i')
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            clap::Arg::new("format")
                .long("format")
                .short('F')
                .value_parser(["json", "yaml", "toml"]),
        )
        .args_conflicts_with_subcommands(true)
}

fn main() {
    clap_complete::CompleteEnv::with_factory(command).complete();

    let cmd = command();
    let matches = cmd.get_matches();
    println!("{matches:#?}");
}

#[test]
fn verify_cli() {
    command().debug_assert();
}
