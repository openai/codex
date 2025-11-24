use clap::Arg;
use std::os::unix::process::CommandExt;
use std::process;

macro_rules! die {
    ($fmt:expr) => ({
        eprintln!($fmt);
        process::exit(1);
    });
    ($fmt:expr, $($arg:tt)*) => ({
        eprintln!($fmt, $($arg)*);
        process::exit(1);
    });
}

fn make_command(name: &str, args: Vec<&str>) -> process::Command {
    let mut command = process::Command::new(name);

    for arg in args {
        command.arg(arg);
    }

    return command;
}

fn main() {
    let matches = clap::Command::new("dotenvy")
        .about("Run a command using the environment in a .env file")
        .override_usage("dotenvy <COMMAND> [ARGS]...")
        .allow_external_subcommands(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("FILE")
                .short('f')
                .long("file")
                .takes_value(true)
                .help("Use a specific .env file (defaults to .env)"),
        )
        .get_matches();

    match matches.value_of("FILE") {
        None => dotenvy::dotenv(),
        Some(file) => dotenvy::from_filename(file),
    }
    .unwrap_or_else(|e| die!("error: failed to load environment: {}", e));

    let mut command = match matches.subcommand() {
        Some((name, matches)) => {
            let args = matches
                .values_of("")
                .map(|v| v.collect())
                .unwrap_or(Vec::new());

            make_command(name, args)
        }
        None => die!("error: missing required argument <COMMAND>"),
    };

    if cfg!(target_os = "windows") {
        match command.spawn().and_then(|mut child| child.wait()) {
            Ok(status) => process::exit(status.code().unwrap_or(1)),
            Err(error) => die!("fatal: {}", error),
        };
    } else {
        let error = command.exec();
        die!("fatal: {}", error);
    };
}
