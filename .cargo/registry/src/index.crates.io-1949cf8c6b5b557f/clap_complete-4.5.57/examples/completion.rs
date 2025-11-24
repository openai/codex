//! Example to test arguments with different `ValueHint` values.
//!
//! Usage with zsh:
//! ```console
//! $ cargo run --example completion -- --generate=zsh > /usr/local/share/zsh/site-functions/_completion$
//! $ compinit
//! $ ./target/debug/examples/completion --<TAB>
//! ```
//! fish:
//! ```console
//! $ cargo run --example completion -- --generate=fish > completion.fish
//! $ . ./completion.fish
//! $ ./target/debug/examples/completion --<TAB>
//! ```
use clap::{value_parser, Arg, Command, ValueHint};
use clap_complete::{generate, Generator, Shell};
use std::io;

fn build_cli() -> Command {
    let value_hint_command = Command::new("value-hint")
        .visible_alias("hint")
        .arg(
            Arg::new("unknown")
                .long("unknown")
                .value_hint(ValueHint::Unknown),
        )
        .arg(Arg::new("other").long("other").value_hint(ValueHint::Other))
        .arg(
            Arg::new("path")
                .long("path")
                .short('p')
                .value_hint(ValueHint::AnyPath),
        )
        .arg(
            Arg::new("file")
                .long("file")
                .short('f')
                .value_hint(ValueHint::FilePath),
        )
        .arg(
            Arg::new("dir")
                .long("dir")
                .short('d')
                .value_hint(ValueHint::DirPath),
        )
        .arg(
            Arg::new("exe")
                .long("exe")
                .short('e')
                .value_hint(ValueHint::ExecutablePath),
        )
        .arg(
            Arg::new("cmd_name")
                .long("cmd-name")
                .value_hint(ValueHint::CommandName),
        )
        .arg(
            Arg::new("cmd")
                .long("cmd")
                .short('c')
                .value_hint(ValueHint::CommandString),
        )
        .arg(
            Arg::new("command_with_args")
                .num_args(1..)
                // AppSettings::TrailingVarArg is required to use ValueHint::CommandWithArguments
                .trailing_var_arg(true)
                .value_hint(ValueHint::CommandWithArguments),
        )
        .arg(
            Arg::new("user")
                .short('u')
                .long("user")
                .value_hint(ValueHint::Username),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .value_hint(ValueHint::Hostname),
        )
        .arg(Arg::new("url").long("url").value_hint(ValueHint::Url))
        .arg(
            Arg::new("email")
                .long("email")
                .value_hint(ValueHint::EmailAddress),
        );

    Command::new("completion")
        .arg(
            Arg::new("generator")
                .long("generate")
                .value_parser(value_parser!(Shell)),
        )
        .subcommand(value_hint_command)
}

fn print_completions<G: Generator>(generator: G, cmd: &mut Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut io::stdout(),
    );
}

fn main() {
    let matches = build_cli().get_matches();

    if let Some(generator) = matches.get_one::<Shell>("generator") {
        let mut cmd = build_cli();
        eprintln!("Generating completion file for {generator}...");
        print_completions(*generator, &mut cmd);
    }
}
