extern crate lalrpop;
extern crate pico_args;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

use pico_args::Arguments;

use lalrpop::Configuration;

static VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "
Usage: lalrpop [options] <inputs>...
       lalrpop --help
       lalrpop (-V | --version)

Options:
    -h, --help           Print help.
    -V, --version        Print version.
    -l, --level LEVEL    Set the debug level. (Default: info)
                         Valid values: quiet, info, verbose, debug.
    -o, --out-dir DIR    Sets the directory in which to output the .rs file(s).
    --features FEATURES  Comma separated list of features for conditional compilation.
    -f, --force          Force execution, even if the .lalrpop file is older than the .rs file.
    -c, --color          Force colorful output, even if this is not a TTY.
    --no-whitespace      Removes redundant whitespace from the generated file. (Default: false)
    --comments           Enable comments in the generated code.
    --report             Generate report files.
";

#[derive(Debug)]
struct Args {
    arg_inputs: Vec<OsString>,
    flag_out_dir: Option<PathBuf>,
    flag_features: Option<String>,
    flag_level: Option<LevelFlag>,
    flag_help: bool,
    flag_force: bool,
    flag_color: bool,
    flag_comments: bool,
    flag_no_whitespace: bool,
    flag_report: bool,
    flag_version: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum LevelFlag {
    Quiet,
    Info,
    Verbose,
    Debug,
}

impl FromStr for LevelFlag {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::LevelFlag::*;
        match s {
            "quiet" => Ok(Quiet),
            "info" => Ok(Info),
            "verbose" => Ok(Verbose),
            "debug" => Ok(Debug),
            x => Err(format!("Unknown level {}", x)),
        }
    }
}

fn parse_args(mut args: Arguments) -> Result<Args, Box<dyn std::error::Error>> {
    Ok(Args {
        flag_out_dir: args.opt_value_from_fn(["-o", "--out-dir"], PathBuf::from_str)?,
        flag_features: args.opt_value_from_str("--features")?,
        flag_level: args.opt_value_from_fn(["-l", "--level"], LevelFlag::from_str)?,
        flag_help: args.contains(["-h", "--help"]),
        flag_force: args.contains(["-f", "--force"]),
        flag_color: args.contains(["-c", "--color"]),
        flag_comments: args.contains("--comments"),
        flag_no_whitespace: args.contains("--no-whitespace"),
        flag_report: args.contains("--report"),
        flag_version: args.contains(["-V", "--version"]),
        arg_inputs: args.finish(),
    })
}

fn main() {
    main1().unwrap();
}

fn main1() -> Result<(), Box<dyn std::error::Error>> {
    let mut stderr = std::io::stderr();
    let mut stdout = std::io::stdout();

    let args = parse_args(Arguments::from_env())?;

    if args.flag_help {
        writeln!(stdout, "{}", USAGE)?;
        process::exit(0);
    }

    if args.flag_version {
        writeln!(stdout, "{}", VERSION)?;
        process::exit(0);
    }

    let mut config = Configuration::new();

    match args.flag_level.unwrap_or(LevelFlag::Info) {
        LevelFlag::Quiet => config.log_quiet(),
        LevelFlag::Info => config.log_info(),
        LevelFlag::Verbose => config.log_verbose(),
        LevelFlag::Debug => config.log_debug(),
    };

    if args.flag_force {
        config.force_build(true);
    }

    if args.flag_color {
        config.always_use_colors();
    }

    if args.flag_comments {
        config.emit_comments(true);
    }

    if args.flag_no_whitespace {
        config.emit_whitespace(false);
    }

    if args.flag_report {
        config.emit_report(true);
    }

    if args.arg_inputs.is_empty() {
        writeln!(
            stderr,
            "Error: no input files specified! Try --help for help."
        )?;
        process::exit(1);
    }

    if let Some(ref out_dir) = args.flag_out_dir {
        config.set_out_dir(out_dir);
    }

    if let Some(ref flag_features) = args.flag_features {
        config.set_features(flag_features.split(',').map(String::from));
    }

    for arg in args.arg_inputs {
        let arg = Path::new(&arg);
        match config.process_file(arg) {
            Ok(()) => {}
            Err(err) => {
                writeln!(
                    stderr,
                    "Error encountered processing `{}`: {}",
                    arg.display(),
                    err
                )?;
                process::exit(1);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::ffi::OsString;

    fn os_vec(vals: &[&str]) -> Vec<OsString> {
        vals.iter().map(|v| v.into()).collect()
    }

    fn parse_args_vec(args: &Vec<&str>) -> Args {
        parse_args(Arguments::from_vec(os_vec(args))).unwrap()
    }

    #[test]
    fn test_usage_help() {
        assert!(parse_args_vec(&vec!["--help"]).flag_help);
    }

    #[test]
    fn test_usage_version() {
        assert!(parse_args_vec(&vec!["--version"]).flag_version);
    }

    #[test]
    fn test_usage_single_input() {
        assert_eq!(
            parse_args_vec(&vec!["file.lalrpop"]).arg_inputs,
            ["file.lalrpop"]
        );
    }

    #[test]
    fn test_usage_multiple_inputs() {
        let files = vec!["file.lalrpop", "../file2.lalrpop"];
        assert_eq!(parse_args_vec(&files).arg_inputs, files);
    }

    #[test]
    fn test_usage_out_dir() {
        let args = parse_args_vec(&vec!["--out-dir", "abc", "file.lalrpop"]);
        assert_eq!(args.flag_out_dir, Some(PathBuf::from_str("abc").unwrap()));
        assert_eq!(args.arg_inputs, ["file.lalrpop"]);
    }

    #[test]
    fn test_usage_features() {
        let args = parse_args_vec(&vec!["--features", "test,abc", "file.lalrpop"]);
        assert_eq!(args.flag_features, Some("test,abc".into()));
        assert_eq!(args.arg_inputs, ["file.lalrpop"]);
    }

    #[test]
    fn test_usage_emit_whitespace() {
        let args = parse_args_vec(&vec!["--no-whitespace", "file.lalrpop"]);
        assert!(args.flag_no_whitespace);
        assert_eq!(args.arg_inputs, ["file.lalrpop"]);
    }

    #[test]
    fn test_usage_level() {
        assert_eq!(
            parse_args_vec(&vec!["-l", "info"]).flag_level,
            Some(LevelFlag::Info)
        );
    }
}
