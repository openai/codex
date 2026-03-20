use crate::command::FsCommand;
use crate::command::parse_command_from_args;
use std::ffi::OsString;
use std::io::Read;
use std::io::Write;

/// Runs the fs-ops helper with the given arguments and I/O streams.
pub fn run_from_args_and_exit(
    args: impl Iterator<Item = OsString>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ! {
    let exit_code = match run_from_args(args, stdin, stdout, stderr) {
        Ok(()) => 0,
        Err(_) => {
            // Discard the specific error, since we already wrote it to stderr.
            1
        }
    };
    std::process::exit(exit_code);
}

/// Testable version of `run_from_args_and_exit` that returns a Result instead
/// of exiting the process.
fn run_from_args(
    args: impl Iterator<Item = OsString>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> std::io::Result<()> {
    try_run_from_args(args, stdin, stdout).inspect_err(|error| {
        writeln!(stderr, "error: {error}").ok();
    })
}

fn try_run_from_args(
    args: impl Iterator<Item = OsString>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
) -> std::io::Result<()> {
    let command = parse_command_from_args(args)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    match command {
        FsCommand::ReadFile { path } => {
            let mut file = std::fs::File::open(&path)?;
            if !file.metadata()?.is_file() {
                let error_message = format!(
                    "`{path}` is not a regular file",
                    path = path.to_string_lossy()
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    error_message,
                ));
            }

            std::io::copy(&mut file, stdout).map(|_| ())
        }
        FsCommand::WriteFile { path } => {
            let mut file = std::fs::File::create(path).map_err(FsError::from)?;
            std::io::copy(stdin, &mut file)
                .map(|_| ())
                .map_err(FsError::from)
        }
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
