use clap::error::Error;
use clap::{Arg, ArgAction, ArgMatches, Args, Command, FromArgMatches, Parser};

#[derive(Debug)]
struct CliArgs {
    foo: bool,
    bar: bool,
    quuz: Option<String>,
}

impl FromArgMatches for CliArgs {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        let mut matches = matches.clone();
        Self::from_arg_matches_mut(&mut matches)
    }
    fn from_arg_matches_mut(matches: &mut ArgMatches) -> Result<Self, Error> {
        Ok(Self {
            foo: matches.get_flag("foo"),
            bar: matches.get_flag("bar"),
            quuz: matches.remove_one::<String>("quuz"),
        })
    }
    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        let mut matches = matches.clone();
        self.update_from_arg_matches_mut(&mut matches)
    }
    fn update_from_arg_matches_mut(&mut self, matches: &mut ArgMatches) -> Result<(), Error> {
        self.foo |= matches.get_flag("foo");
        self.bar |= matches.get_flag("bar");
        if let Some(quuz) = matches.remove_one::<String>("quuz") {
            self.quuz = Some(quuz);
        }
        Ok(())
    }
}

impl Args for CliArgs {
    fn augment_args(cmd: Command) -> Command {
        cmd.arg(
            Arg::new("foo")
                .short('f')
                .long("foo")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("bar")
                .short('b')
                .long("bar")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quuz")
                .short('q')
                .long("quuz")
                .action(ArgAction::Set),
        )
    }
    fn augment_args_for_update(cmd: Command) -> Command {
        cmd.arg(
            Arg::new("foo")
                .short('f')
                .long("foo")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("bar")
                .short('b')
                .long("bar")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quuz")
                .short('q')
                .long("quuz")
                .action(ArgAction::Set),
        )
    }
}

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short, long)]
    top_level: bool,
    #[command(flatten)]
    more_args: CliArgs,
}

fn main() {
    let args = Cli::parse();
    println!("{args:#?}");
}
