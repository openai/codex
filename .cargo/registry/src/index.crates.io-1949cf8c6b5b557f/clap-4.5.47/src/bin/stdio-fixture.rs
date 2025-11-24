use clap::{builder::PossibleValue, Arg, ArgAction, Command};

fn main() {
    #[allow(unused_mut)]
    let mut cmd = Command::new("stdio-fixture")
        .version("1.0")
        .long_version("1.0 - a2132c")
        .arg_required_else_help(true)
        .subcommand(Command::new("more"))
        .subcommand(
            Command::new("test")
                .visible_alias("do-stuff")
                .long_about("Subcommand with one visible alias"),
        )
        .subcommand(
            Command::new("test_2")
                .visible_aliases(["do-other-stuff", "tests"])
                .about("several visible aliases")
                .long_about("Subcommand with multiple visible aliases"),
        )
        .subcommand(
            Command::new("test_3")
                .long_flag("test")
                .about("several visible long flag aliases")
                .visible_long_flag_aliases(["testing", "testall", "test_all"]),
        )
        .subcommand(
            Command::new("test_4")
                .short_flag('t')
                .about("several visible short flag aliases")
                .visible_short_flag_aliases(['q', 'w']),
        )
        .subcommand(
            Command::new("test_5")
                .short_flag('e')
                .long_flag("test-hdr")
                .about("all kinds of visible aliases")
                .visible_aliases(["tests_4k"])
                .visible_long_flag_aliases(["thetests", "t4k"])
                .visible_short_flag_aliases(['r', 'y']),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .help("log")
                .action(ArgAction::SetTrue)
                .long_help("more log"),
        )
        .arg(
            Arg::new("config")
                .action(ArgAction::Set)
                .help("Speed configuration")
                .short('c')
                .long("config")
                .value_name("MODE")
                .value_parser([
                    PossibleValue::new("fast"),
                    PossibleValue::new("slow").help("slower than fast"),
                    PossibleValue::new("secret speed").hide(true),
                ])
                .default_value("fast"),
        )
        .arg(
            Arg::new("name")
                .action(ArgAction::Set)
                .help("App name")
                .long_help("Set the instance app name")
                .value_name("NAME")
                .visible_alias("app-name")
                .default_value("clap"),
        )
        .arg(
            Arg::new("fruits")
                .short('f')
                .visible_short_alias('b')
                .action(ArgAction::Append)
                .value_name("FRUITS")
                .help("List of fruits")
                .default_values(["apple", "banane", "orange"]),
        );
    #[cfg(feature = "env")]
    {
        cmd = cmd.arg(
            Arg::new("env_arg")
                .help("Read from env var when arg is not present.")
                .value_name("ENV")
                .env("ENV_ARG"),
        );
    }
    #[cfg(feature = "color")]
    {
        use clap::builder::styling::{AnsiColor, Styles};
        const STYLES: Styles = Styles::styled()
            .header(AnsiColor::Green.on_default().bold())
            .error(AnsiColor::Red.on_default().bold())
            .usage(AnsiColor::Green.on_default().bold().underline())
            .literal(AnsiColor::Blue.on_default().bold())
            .placeholder(AnsiColor::Cyan.on_default())
            .valid(AnsiColor::Green.on_default())
            .invalid(AnsiColor::Magenta.on_default().bold())
            .context(AnsiColor::Yellow.on_default().dimmed())
            .context_value(AnsiColor::Yellow.on_default().italic());
        cmd = cmd.styles(STYLES);
    }
    cmd.get_matches();
}
