use clap::Parser;
use std::io::Read;
use std::io::Write;

pub fn main() -> ! {
    let exit_code = run_main();
    std::process::exit(exit_code);
}

/// We would prefer to return `std::process::ExitCode`, but its `exit_process()`
/// method is still a nightly API and we want main() to return !.
pub fn run_main() -> i32 {
    let cli = Cli::parse();

    // CLI overrides env; if not provided, respect env via default inside eol module
    if let Some(val) = cli.assume_eol.as_deref() {
        match crate::eol::parse_assume_eol(val) {
            Some(sel) => crate::eol::set_assume_eol(sel),
            None => {
                eprintln!("Error: invalid --assume-eol value: {val}");
                return 2;
            }
        }
    }

    let patch_arg = match cli.patch {
        Some(s) => s,
        None => {
            // No positional provided; attempt to read the patch from stdin.
            let mut buf = String::new();
            match std::io::stdin().read_to_string(&mut buf) {
                Ok(_) => {
                    if buf.is_empty() {
                        eprintln!(
                            "Usage: apply_patch [-E|--assume-eol=lf|crlf|git|detect] 'PATCH'\n       echo 'PATCH' | apply_patch"
                        );
                        return 2;
                    }
                    buf
                }
                Err(err) => {
                    eprintln!("Error: Failed to read PATCH from stdin.\n{err}");
                    return 1;
                }
            }
        }
    };

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    match crate::apply_patch(&patch_arg, &mut stdout, &mut stderr) {
        Ok(()) => {
            // Flush to ensure output ordering when used in pipelines.
            let _ = stdout.flush();
            0
        }
        Err(_) => 1,
    }
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Apply a simple patch format to the filesystem",
    disable_help_subcommand = true
)]
struct Cli {
    /// Assume EOL policy for writes: lf|crlf|git|detect (CLI overrides env)
    #[arg(short = 'E', long = "assume-eol", value_name = "MODE")]
    assume_eol: Option<String>,

    /// The raw patch body; if omitted, reads from stdin
    patch: Option<String>,
}
