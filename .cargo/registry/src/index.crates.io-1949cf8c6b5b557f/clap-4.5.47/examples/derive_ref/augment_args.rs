use clap::{arg, Args, Command, FromArgMatches as _};

#[derive(Args, Debug)]
struct DerivedArgs {
    #[arg(short, long)]
    derived: bool,
}

fn main() {
    let cli = Command::new("CLI").arg(arg!(-b - -built).action(clap::ArgAction::SetTrue));
    // Augment built args with derived args
    let cli = DerivedArgs::augment_args(cli);

    let matches = cli.get_matches();
    println!("Value of built: {:?}", matches.get_flag("built"));
    println!(
        "Value of derived via ArgMatches: {:?}",
        matches.get_flag("derived")
    );

    // Since DerivedArgs implements FromArgMatches, we can extract it from the unstructured ArgMatches.
    // This is the main benefit of using derived arguments.
    let derived_matches = DerivedArgs::from_arg_matches(&matches)
        .map_err(|err| err.exit())
        .unwrap();
    println!("Value of derived: {derived_matches:#?}");
}
