#![deny(warnings)]

/// This little test binary reads stdin and write what it reads to both
/// stdout and stderr. It depends on os_pipe's parent_* functions, and
/// we use it to test them.
use std::env::args_os;
use std::ffi::OsString;
use std::process::Command;

fn main() {
    let stdin = os_pipe::dup_stdin().unwrap();
    let stdout = os_pipe::dup_stdout().unwrap();
    let stderr = os_pipe::dup_stderr().unwrap();

    let args: Vec<OsString> = args_os().collect();
    let mut child = Command::new(&args[1]);
    child.args(&args[2..]);

    // Swap stdout and stderr in the child. Set stdin too, just for testing,
    // though this should be the same as the default behavior.
    child.stdin(stdin);
    child.stdout(stderr);
    child.stderr(stdout);

    // Run the child. This method is kind of confusingly named...
    child.status().unwrap();
}
