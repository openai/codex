#![expect(clippy::expect_used)]

//! Optional smoke tests that hit the real OpenAI /v1/responses endpoint. They are `#[ignore]` by
//! default so CI stays deterministic and free. Developers can run them locally with
//! `cargo test --test live_cli -- --ignored` provided they set a valid `OPENAI_API_KEY`.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use std::process::Stdio;
use tempfile::TempDir;

fn require_api_key() -> String {
    std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY env var not set — skip running live tests")
}

/// Helper that spawns the binary inside a TempDir with minimal flags. Returns (Assert, TempDir).
fn run_live(prompt: &str) -> (assert_cmd::assert::Assert, TempDir) {
    #![expect(clippy::unwrap_used)]
    use std::io::Read;
    use std::io::Write;
    use std::thread;

    let dir = TempDir::new().unwrap();

    // Build a plain `std::process::Command` so we have full control over the underlying stdio
    // handles. `assert_cmd`’s own `Command` wrapper always forces stdout/stderr to be piped
    // internally which prevents us from streaming them live to the terminal (see its `spawn`
    // implementation). Instead we configure the std `Command` ourselves, then later hand the
    // resulting `Output` to `assert_cmd` for the familiar assertions.

    let mut cmd = Command::new(codex_utils_cargo_bin::cargo_bin("codex-rs").unwrap());
    cmd.current_dir(dir.path());
    cmd.env("OPENAI_API_KEY", require_api_key());

    // We want three things at once:
    //   1. live streaming of the child’s stdout/stderr while the test is running
    //   2. captured output so we can keep using assert_cmd’s `Assert` helpers
    //   3. cross‑platform behavior (best effort)
    //
    // To get that we:
    //   • set both stdout and stderr to `piped()` so we can read them programmatically
    //   • spawn a thread for each stream that copies bytes into two sinks:
    //       – the parent process’ stdout/stderr for live visibility
    //       – an in‑memory buffer so we can pass it to `assert_cmd` later

    // Pass the prompt through the `--` separator so the CLI knows when user input ends.
    cmd.arg("--allow-no-git-exec")
        .arg("-v")
        .arg("--")
        .arg(prompt);

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn codex-rs");

    // Send the terminating newline so Session::run exits after the first turn.
    child
        .stdin
        .as_mut()
        .expect("child stdin unavailable")
        .write_all(b"\n")
        .expect("failed to write to child stdin");

    // Helper that tees a ChildStdout/ChildStderr into both the parent’s stdio and a Vec<u8>.
    fn tee<R: Read + Send + 'static>(
        mut reader: R,
        mut writer: impl Write + Send + 'static,
    ) -> thread::JoinHandle<Vec<u8>> {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        writer.write_all(&chunk[..n]).ok();
                        writer.flush().ok();
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    Err(_) => break,
                }
            }
            buf
        })
    }

    let stdout_handle = tee(
        child.stdout.take().expect("child stdout"),
        std::io::stdout(),
    );
    let stderr_handle = tee(
        child.stderr.take().expect("child stderr"),
        std::io::stderr(),
    );

    let status = child.wait().expect("failed to wait on child");
    let stdout = stdout_handle.join().expect("stdout thread panicked");
    let stderr = stderr_handle.join().expect("stderr thread panicked");

    let output = std::process::Output {
        status,
        stdout,
        stderr,
    };

    (output.assert(), dir)
}

#[ignore]
#[test]
fn live_create_file_hello_txt() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping live_create_file_hello_txt – OPENAI_API_KEY not set");
        return;
    }

    let (assert, dir) = run_live(
        "Use the shell tool with the apply_patch command to create a file named hello.txt containing the text 'hello'.",
    );

    assert.success();

    let path = dir.path().join("hello.txt");
    assert!(path.exists(), "hello.txt was not created by the model");

    let contents = std::fs::read_to_string(path).unwrap();

    assert_eq!(contents.trim(), "hello");
}

#[ignore]
#[test]
fn live_print_working_directory() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("skipping live_print_working_directory – OPENAI_API_KEY not set");
        return;
    }

    let (assert, dir) = run_live("Print the current working directory using the shell function.");

    assert
        .success()
        .stdout(predicate::str::contains(dir.path().to_string_lossy()));
}
