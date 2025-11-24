use clap::builder::PossibleValue;
use clap_complete::{generate, Generator, Shell};

fn main() {
    #[cfg(feature = "unstable-dynamic")]
    clap_complete::CompleteEnv::with_factory(cli)
        // Avoid tests snapshotting a path into `target/`
        .completer("exhaustive")
        .complete();

    let matches = cli().get_matches();
    if let Some(generator) = matches.get_one::<Shell>("generate") {
        let mut cmd = cli();
        eprintln!("Generating completion file for {generator}...");
        print_completions(*generator, &mut cmd);
        return;
    }

    println!("{matches:?}");
}

fn print_completions<G: Generator>(generator: G, cmd: &mut clap::Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut std::io::stdout(),
    );
}

const EMPTY: [&str; 0] = [];

#[allow(clippy::let_and_return)]
fn cli() -> clap::Command {
    clap::Command::new("exhaustive")
        .args([
            clap::Arg::new("generate")
                .long("generate")
                .value_name("SHELL")
                .value_parser(clap::value_parser!(Shell))
                .help("generate"),
            clap::Arg::new("empty-choice")
                .long("empty-choice")
                .value_parser(EMPTY),
        ])
        .subcommands([
            clap::Command::new("empty")
                .disable_help_subcommand(true)
                .disable_help_flag(true),
            clap::Command::new("global")
                .version("3.0")
                .propagate_version(true)
                .args([clap::Arg::new("global")
                    .long("global")
                    .global(true)
                    .action(clap::ArgAction::SetTrue)
                    .help("everywhere")])
                .subcommands([
                    clap::Command::new("one").subcommand(clap::Command::new("one-one")),
                    clap::Command::new("two"),
                ]),
            clap::Command::new("action").args([
                clap::Arg::new("set-true")
                    .long("set-true")
                    .action(clap::ArgAction::SetTrue)
                    .help("bool"),
                clap::Arg::new("set")
                    .long("set")
                    .action(clap::ArgAction::Set)
                    .help("value"),
                clap::Arg::new("count")
                    .long("count")
                    .action(clap::ArgAction::Count)
                    .help("number"),
                clap::Arg::new("choice")
                    .long("choice")
                    .value_parser(["first", "second"])
                    .help("enum"),
            ]),
            clap::Command::new("quote")
                .args([
                    clap::Arg::new("single-quotes")
                        .long("single-quotes")
                        .action(clap::ArgAction::SetTrue)
                        .help("Can be 'always', 'auto', or 'never'"),
                    clap::Arg::new("double-quotes")
                        .long("double-quotes")
                        .action(clap::ArgAction::SetTrue)
                        .help("Can be \"always\", \"auto\", or \"never\""),
                    clap::Arg::new("backticks")
                        .long("backticks")
                        .action(clap::ArgAction::SetTrue)
                        .help("For more information see `echo test`"),
                    clap::Arg::new("backslash")
                        .long("backslash")
                        .action(clap::ArgAction::SetTrue)
                        .help("Avoid '\\n'"),
                    clap::Arg::new("brackets")
                        .long("brackets")
                        .action(clap::ArgAction::SetTrue)
                        .help("List packages [filter]"),
                    clap::Arg::new("expansions")
                        .long("expansions")
                        .action(clap::ArgAction::SetTrue)
                        .help("Execute the shell command with $SHELL"),
                    clap::Arg::new("choice")
                        .long("choice")
                        .action(clap::ArgAction::Set)
                        .value_parser(clap::builder::PossibleValuesParser::new([
                            PossibleValue::new("another shell").help("something with a space"),
                            PossibleValue::new("bash").help("bash (shell)"),
                            PossibleValue::new("fish").help("fish shell"),
                            PossibleValue::new("zsh").help("zsh shell"),
                        ])),
                ])
                .subcommands([
                    clap::Command::new("cmd-single-quotes")
                        .about("Can be 'always', 'auto', or 'never'"),
                    clap::Command::new("cmd-double-quotes")
                        .about("Can be \"always\", \"auto\", or \"never\""),
                    clap::Command::new("cmd-backticks")
                        .about("For more information see `echo test`"),
                    clap::Command::new("cmd-backslash").about("Avoid '\\n'"),
                    clap::Command::new("cmd-brackets").about("List packages [filter]"),
                    clap::Command::new("cmd-expansions")
                        .about("Execute the shell command with $SHELL"),
                    clap::Command::new("escape-help").about("\\tab\t\"'\nNew Line"),
                ]),
            clap::Command::new("value").args([
                clap::Arg::new("delim").long("delim").value_delimiter(','),
                clap::Arg::new("tuple").long("tuple").num_args(2),
                clap::Arg::new("require-eq")
                    .long("require-eq")
                    .require_equals(true),
                clap::Arg::new("term").num_args(1..).value_terminator(";"),
            ]),
            clap::Command::new("pacman").subcommands([
                clap::Command::new("one").long_flag("one").short_flag('o'),
                clap::Command::new("two").long_flag("two").short_flag('t'),
            ]),
            clap::Command::new("last")
                .args([clap::Arg::new("first"), clap::Arg::new("free").last(true)]),
            clap::Command::new("alias").args([
                clap::Arg::new("flag")
                    .short('f')
                    .visible_short_alias('F')
                    .long("flag")
                    .action(clap::ArgAction::SetTrue)
                    .visible_alias("flg")
                    .help("cmd flag"),
                clap::Arg::new("option")
                    .short('o')
                    .visible_short_alias('O')
                    .long("option")
                    .visible_alias("opt")
                    .help("cmd option")
                    .action(clap::ArgAction::Set),
                clap::Arg::new("positional"),
            ]),
            clap::Command::new("hint").args([
                clap::Arg::new("choice")
                    .long("choice")
                    .action(clap::ArgAction::Set)
                    .value_parser(["bash", "fish", "zsh"]),
                clap::Arg::new("unknown")
                    .long("unknown")
                    .value_hint(clap::ValueHint::Unknown),
                clap::Arg::new("other")
                    .long("other")
                    .value_hint(clap::ValueHint::Other),
                clap::Arg::new("path")
                    .long("path")
                    .short('p')
                    .value_hint(clap::ValueHint::AnyPath),
                clap::Arg::new("file")
                    .long("file")
                    .short('f')
                    .value_hint(clap::ValueHint::FilePath),
                clap::Arg::new("dir")
                    .long("dir")
                    .short('d')
                    .value_hint(clap::ValueHint::DirPath),
                clap::Arg::new("exe")
                    .long("exe")
                    .short('e')
                    .value_hint(clap::ValueHint::ExecutablePath),
                clap::Arg::new("cmd_name")
                    .long("cmd-name")
                    .value_hint(clap::ValueHint::CommandName),
                clap::Arg::new("cmd")
                    .long("cmd")
                    .short('c')
                    .value_hint(clap::ValueHint::CommandString),
                clap::Arg::new("command_with_args")
                    .action(clap::ArgAction::Set)
                    .num_args(1..)
                    .trailing_var_arg(true)
                    .value_hint(clap::ValueHint::CommandWithArguments),
                clap::Arg::new("user")
                    .short('u')
                    .long("user")
                    .value_hint(clap::ValueHint::Username),
                clap::Arg::new("host")
                    .short('H')
                    .long("host")
                    .value_hint(clap::ValueHint::Hostname),
                clap::Arg::new("url")
                    .long("url")
                    .value_hint(clap::ValueHint::Url),
                clap::Arg::new("email")
                    .long("email")
                    .value_hint(clap::ValueHint::EmailAddress),
            ]),
        ])
}
