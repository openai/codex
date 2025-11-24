#![allow(dead_code)]
use clap::error::{Error, ErrorKind};
use clap::{ArgMatches, Args as _, Command, FromArgMatches, Parser, Subcommand};

#[derive(Parser, Debug)]
struct AddArgs {
    name: Vec<String>,
}
#[derive(Parser, Debug)]
struct RemoveArgs {
    #[arg(short, long)]
    force: bool,
    name: Vec<String>,
}

#[derive(Debug)]
enum CliSub {
    Add(AddArgs),
    Remove(RemoveArgs),
}

impl FromArgMatches for CliSub {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, Error> {
        match matches.subcommand() {
            Some(("add", args)) => Ok(Self::Add(AddArgs::from_arg_matches(args)?)),
            Some(("remove", args)) => Ok(Self::Remove(RemoveArgs::from_arg_matches(args)?)),
            Some((_, _)) => Err(Error::raw(
                ErrorKind::InvalidSubcommand,
                "Valid subcommands are `add` and `remove`",
            )),
            None => Err(Error::raw(
                ErrorKind::MissingSubcommand,
                "Valid subcommands are `add` and `remove`",
            )),
        }
    }
    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), Error> {
        match matches.subcommand() {
            Some(("add", args)) => *self = Self::Add(AddArgs::from_arg_matches(args)?),
            Some(("remove", args)) => *self = Self::Remove(RemoveArgs::from_arg_matches(args)?),
            Some((_, _)) => {
                return Err(Error::raw(
                    ErrorKind::InvalidSubcommand,
                    "Valid subcommands are `add` and `remove`",
                ))
            }
            None => (),
        };
        Ok(())
    }
}

impl Subcommand for CliSub {
    fn augment_subcommands(cmd: Command) -> Command {
        cmd.subcommand(AddArgs::augment_args(Command::new("add")))
            .subcommand(RemoveArgs::augment_args(Command::new("remove")))
            .subcommand_required(true)
    }
    fn augment_subcommands_for_update(cmd: Command) -> Command {
        cmd.subcommand(AddArgs::augment_args(Command::new("add")))
            .subcommand(RemoveArgs::augment_args(Command::new("remove")))
            .subcommand_required(true)
    }
    fn has_subcommand(name: &str) -> bool {
        matches!(name, "add" | "remove")
    }
}

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short, long)]
    top_level: bool,
    #[command(subcommand)]
    subcommand: CliSub,
}

fn main() {
    let args = Cli::parse();
    println!("{args:#?}");
}
