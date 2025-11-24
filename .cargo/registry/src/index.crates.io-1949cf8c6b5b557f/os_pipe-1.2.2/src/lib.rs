//! A cross-platform library for opening OS pipes, like those from
//! [`pipe`](https://man7.org/linux/man-pages/man2/pipe.2.html) on Linux or
//! [`CreatePipe`](https://docs.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createpipe)
//! on Windows. The Rust standard library provides
//! [`Stdio::piped`](https://doc.rust-lang.org/std/process/struct.Stdio.html#method.piped) for
//! simple use cases involving child processes, ~~but it doesn't support creating pipes directly.
//! This crate fills that gap.~~ **Update:** Rust 1.87 added
//! [`std::io::pipe`](https://doc.rust-lang.org/std/io/fn.pipe.html), so this crate is no longer
//! needed except to support older compiler versions.
//!
//! - [Docs](https://docs.rs/os_pipe)
//! - [Crate](https://crates.io/crates/os_pipe)
//! - [Repo](https://github.com/oconnor663/os_pipe.rs)
//!
//! # Common deadlocks related to pipes
//!
//! When you work with pipes, you often end up debugging a deadlock at
//! some point. These can be confusing if you don't know why they
//! happen. Here are two things you need to know:
//!
//! 1. Pipe reads block until some bytes are written or all writers are
//!    closed. **If you forget to close a writer, reads can block
//!    forever.** This includes writers inside a
//!    [`std::process::Command`](https://doc.rust-lang.org/std/process/struct.Command.html)
//!    object or writers given to child processes.
//! 2. Pipes have an internal buffer of some fixed size. On Linux for
//!    example, pipe buffers are 64 KiB by default. Pipe writes block
//!    until buffer space is available or all readers are closed. **If
//!    you have readers open but not reading, writes can block
//!    forever.**
//!
//! Deadlocked reads caused by a forgotten writer usually show up
//! immediately, which makes them relatively easy to fix once you know
//! what to look for. (See "Avoid a deadlock!" in the example code
//! below.) However, deadlocked writes caused by full pipe buffers are
//! trickier. These might only show up for larger inputs, and they might
//! be timing-dependent or platform-dependent. If you find that writing
//! to a pipe deadlocks sometimes, think about who's supposed to be
//! reading from that pipe and whether that thread or process might be
//! blocked on something else. For more on this, see the [Gotchas
//! Doc](https://github.com/oconnor663/duct.py/blob/master/gotchas.md#using-io-threads-to-avoid-blocking-children)
//! from the [`duct`](https://github.com/oconnor663/duct.rs) crate. (And
//! consider whether [`duct`](https://github.com/oconnor663/duct.rs)
//! might be a good fit for your use case.)
//!
//! # Examples
//!
//! Here we write a single byte into a pipe and read it back out:
//!
//! ```rust
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use std::io::prelude::*;
//!
//! let (mut reader, mut writer) = os_pipe::pipe()?;
//! // XXX: If this write blocks, we'll never get to the read.
//! writer.write_all(b"x")?;
//! let mut output = [0];
//! reader.read_exact(&mut output)?;
//! assert_eq!(b"x", &output);
//! # Ok(())
//! # }
//! ```
//!
//! This is a minimal working example, but as discussed in the section
//! above, reading and writing on the same thread like this is
//! deadlock-prone. If we wrote 100 KB instead of just one byte, this
//! example would block on `write_all`, it would never make it to
//! `read_exact`, and that would be a deadlock. Doing the read and write
//! from different threads or different processes would fix the
//! deadlock.
//!
//! For a more complex example, here we join the stdout and stderr of a
//! child process into a single pipe. To do that we open a pipe, clone
//! its writer, and set that pair of writers as the child's stdout and
//! stderr. (This is possible because `PipeWriter` implements
//! `Into<Stdio>`.) Then we can read interleaved output from the pipe
//! reader. This example is deadlock-free, but note the comment about
//! closing the writers.
//!
//! ```rust
//! # use std::io::prelude::*;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // We're going to spawn a child process that prints "foo" to stdout
//! // and "bar" to stderr, and we'll combine these into a single pipe.
//! let mut command = std::process::Command::new("python");
//! command.args(&["-c", r#"
//! import sys
//! sys.stdout.write("foo")
//! sys.stdout.flush()
//! sys.stderr.write("bar")
//! sys.stderr.flush()
//! "#]);
//!
//! // Here's the interesting part. Open a pipe, clone its writer, and
//! // set that pair of writers as the child's stdout and stderr.
//! let (mut reader, writer) = os_pipe::pipe()?;
//! let writer_clone = writer.try_clone()?;
//! command.stdout(writer);
//! command.stderr(writer_clone);
//!
//! // Now start the child process running.
//! let mut handle = command.spawn()?;
//!
//! // Avoid a deadlock! This parent process is still holding open pipe
//! // writers inside the Command object, and we have to close those
//! // before we read. Here we do this by dropping the Command object.
//! drop(command);
//!
//! // Finally we can read all the output and clean up the child.
//! let mut output = String::new();
//! reader.read_to_string(&mut output)?;
//! handle.wait()?;
//! assert_eq!(output, "foobar");
//! # Ok(())
//! # }
//! ```
//!
//! Note that the [`duct`](https://github.com/oconnor663/duct.rs) crate
//! can reproduce the example above in a single line of code, with no
//! risk of deadlocks and no risk of leaking [zombie
//! children](https://en.wikipedia.org/wiki/Zombie_process).

use std::fs::File;
use std::io;
use std::process::Stdio;

#[cfg(not(windows))]
#[path = "unix.rs"]
mod sys;
#[cfg(windows)]
#[path = "windows.rs"]
mod sys;

/// The reading end of a pipe, returned by [`pipe`](fn.pipe.html).
///
/// `PipeReader` implements `Into<Stdio>`, so you can pass it as an argument to
/// `Command::stdin` to spawn a child process that reads from the pipe.
#[derive(Debug)]
pub struct PipeReader(
    // We use std::fs::File here for two reasons: OwnedFd and OwnedHandle are platform-specific,
    // and this gives us read/write/flush for free.
    File,
);

impl PipeReader {
    pub fn try_clone(&self) -> io::Result<PipeReader> {
        self.0.try_clone().map(PipeReader)
    }
}

impl io::Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl<'a> io::Read for &'a PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.0).read(buf)
    }
}

impl From<PipeReader> for Stdio {
    fn from(p: PipeReader) -> Stdio {
        p.0.into()
    }
}

/// The writing end of a pipe, returned by [`pipe`](fn.pipe.html).
///
/// `PipeWriter` implements `Into<Stdio>`, so you can pass it as an argument to
/// `Command::stdout` or `Command::stderr` to spawn a child process that writes
/// to the pipe.
#[derive(Debug)]
pub struct PipeWriter(File);

impl PipeWriter {
    pub fn try_clone(&self) -> io::Result<PipeWriter> {
        self.0.try_clone().map(PipeWriter)
    }
}

impl io::Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a> io::Write for &'a PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.0).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.0).flush()
    }
}

impl From<PipeWriter> for Stdio {
    fn from(p: PipeWriter) -> Stdio {
        p.0.into()
    }
}

/// Open a new pipe and return a [`PipeReader`] and [`PipeWriter`] pair.
///
/// This corresponds to the `pipe2` library call on Posix and the
/// `CreatePipe` library call on Windows (though these implementation
/// details might change). These pipes are non-inheritable, so new child
/// processes won't receive a copy of them unless they're explicitly
/// passed as stdin/stdout/stderr.
///
/// [`PipeReader`]: struct.PipeReader.html
/// [`PipeWriter`]: struct.PipeWriter.html
pub fn pipe() -> io::Result<(PipeReader, PipeWriter)> {
    sys::pipe()
}

/// Get a duplicated copy of the current process's standard input, as a
/// [`PipeReader`].
///
/// Reading directly from this pipe isn't recommended, because it's not
/// synchronized with [`std::io::stdin`]. [`PipeReader`] implements
/// [`Into<Stdio>`], so it can be passed directly to [`Command::stdin`]. This is
/// equivalent to [`Stdio::inherit`], though, so it's usually not necessary
/// unless you need a collection of different pipes.
///
/// [`std::io::stdin`]: https://doc.rust-lang.org/std/io/fn.stdin.html
/// [`PipeReader`]: struct.PipeReader.html
/// [`Into<Stdio>`]: https://doc.rust-lang.org/std/process/struct.Stdio.html
/// [`Command::stdin`]: https://doc.rust-lang.org/std/process/struct.Command.html#method.stdin
/// [`Stdio::inherit`]: https://doc.rust-lang.org/std/process/struct.Stdio.html#method.inherit
pub fn dup_stdin() -> io::Result<PipeReader> {
    sys::dup(io::stdin()).map(PipeReader::from)
}

/// Get a duplicated copy of the current process's standard output, as a
/// [`PipeWriter`](struct.PipeWriter.html).
///
/// Writing directly to this pipe isn't recommended, because it's not
/// synchronized with [`std::io::stdout`]. [`PipeWriter`] implements
/// [`Into<Stdio>`], so it can be passed directly to [`Command::stdout`] or
/// [`Command::stderr`]. This can be useful if you want the child's stderr to go
/// to the parent's stdout.
///
/// [`std::io::stdout`]: https://doc.rust-lang.org/std/io/fn.stdout.html
/// [`PipeWriter`]: struct.PipeWriter.html
/// [`Into<Stdio>`]: https://doc.rust-lang.org/std/process/struct.Stdio.html
/// [`Command::stdout`]: https://doc.rust-lang.org/std/process/struct.Command.html#method.stdout
/// [`Command::stderr`]: https://doc.rust-lang.org/std/process/struct.Command.html#method.stderr
/// [`Stdio::inherit`]: https://doc.rust-lang.org/std/process/struct.Stdio.html#method.inherit
pub fn dup_stdout() -> io::Result<PipeWriter> {
    sys::dup(io::stdout()).map(PipeWriter::from)
}

/// Get a duplicated copy of the current process's standard error, as a
/// [`PipeWriter`](struct.PipeWriter.html).
///
/// Writing directly to this pipe isn't recommended, because it's not
/// synchronized with [`std::io::stderr`]. [`PipeWriter`] implements
/// [`Into<Stdio>`], so it can be passed directly to [`Command::stdout`] or
/// [`Command::stderr`]. This can be useful if you want the child's stdout to go
/// to the parent's stderr.
///
/// [`std::io::stderr`]: https://doc.rust-lang.org/std/io/fn.stderr.html
/// [`PipeWriter`]: struct.PipeWriter.html
/// [`Into<Stdio>`]: https://doc.rust-lang.org/std/process/struct.Stdio.html
/// [`Command::stdout`]: https://doc.rust-lang.org/std/process/struct.Command.html#method.stdout
/// [`Command::stderr`]: https://doc.rust-lang.org/std/process/struct.Command.html#method.stderr
/// [`Stdio::inherit`]: https://doc.rust-lang.org/std/process/struct.Stdio.html#method.inherit
pub fn dup_stderr() -> io::Result<PipeWriter> {
    sys::dup(io::stderr()).map(PipeWriter::from)
}

#[cfg(test)]
mod tests {
    use std::env::consts::EXE_EXTENSION;
    use std::io::prelude::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Once;
    use std::thread;

    fn path_to_exe(name: &str) -> PathBuf {
        // This project defines some associated binaries for testing, and we shell out to them in
        // these tests. `cargo test` doesn't automatically build associated binaries, so this
        // function takes care of building them explicitly, with the right debug/release flavor.
        static CARGO_BUILD_ONCE: Once = Once::new();
        CARGO_BUILD_ONCE.call_once(|| {
            let mut build_command = Command::new("cargo");
            build_command.args(&["build", "--quiet"]);
            if !cfg!(debug_assertions) {
                build_command.arg("--release");
            }
            let build_status = build_command.status().unwrap();
            assert!(
                build_status.success(),
                "Cargo failed to build associated binaries."
            );
        });
        let flavor = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        Path::new("target")
            .join(flavor)
            .join(name)
            .with_extension(EXE_EXTENSION)
    }

    #[test]
    fn test_pipe_some_data() {
        let (mut reader, mut writer) = crate::pipe().unwrap();
        // A small write won't fill the pipe buffer, so it won't block this thread.
        writer.write_all(b"some stuff").unwrap();
        drop(writer);
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "some stuff");
    }

    #[test]
    fn test_pipe_some_data_with_refs() {
        // As with `File`, there's a second set of impls for shared
        // refs. Test those.
        let (reader, writer) = crate::pipe().unwrap();
        let mut reader_ref = &reader;
        {
            let mut writer_ref = &writer;
            // A small write won't fill the pipe buffer, so it won't block this thread.
            writer_ref.write_all(b"some stuff").unwrap();
        }
        drop(writer);
        let mut out = String::new();
        reader_ref.read_to_string(&mut out).unwrap();
        assert_eq!(out, "some stuff");
    }

    #[test]
    fn test_pipe_no_data() {
        let (mut reader, writer) = crate::pipe().unwrap();
        drop(writer);
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn test_pipe_a_megabyte_of_data_from_another_thread() {
        let data = vec![0xff; 1_000_000];
        let data_copy = data.clone();
        let (mut reader, mut writer) = crate::pipe().unwrap();
        let joiner = thread::spawn(move || {
            writer.write_all(&data_copy).unwrap();
            // This drop happens automatically, so writing it out here is mostly
            // just for clarity. For what it's worth, it also guards against
            // accidentally forgetting to drop if we switch to scoped threads or
            // something like that and change this to a non-moving closure. The
            // explicit drop forces `writer` to move.
            drop(writer);
        });
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        joiner.join().unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn test_pipes_are_not_inheritable() {
        // Create pipes for a child process.
        let (input_reader, mut input_writer) = crate::pipe().unwrap();
        let (mut output_reader, output_writer) = crate::pipe().unwrap();

        // Create a bunch of duplicated copies, which we'll close later. This
        // tests that duplication preserves non-inheritability.
        let ir_dup = input_reader.try_clone().unwrap();
        let iw_dup = input_writer.try_clone().unwrap();
        let or_dup = output_reader.try_clone().unwrap();
        let ow_dup = output_writer.try_clone().unwrap();

        // Spawn the child. Note that this temporary Command object takes
        // ownership of our copies of the child's stdin and stdout, and then
        // closes them immediately when it drops. That stops us from blocking
        // our own read below. We use our own simple implementation of cat for
        // compatibility with Windows.
        let mut child = Command::new(path_to_exe("cat"))
            .stdin(input_reader)
            .stdout(output_writer)
            .spawn()
            .unwrap();

        // Drop all the dups now that the child is spawned.
        drop(ir_dup);
        drop(iw_dup);
        drop(or_dup);
        drop(ow_dup);

        // Write to the child's stdin. This is a small write, so it shouldn't
        // block.
        input_writer.write_all(b"hello").unwrap();
        drop(input_writer);

        // Read from the child's stdout. If this child has accidentally
        // inherited the write end of its own stdin, then it will never exit,
        // and this read will block forever. That's what this test is all
        // about.
        let mut output = Vec::new();
        output_reader.read_to_end(&mut output).unwrap();
        child.wait().unwrap();

        // Confirm that we got the right bytes.
        assert_eq!(b"hello", &*output);
    }

    #[test]
    fn test_parent_handles() {
        // This test invokes the `swap` test program, which uses parent_stdout() and
        // parent_stderr() to swap the outputs for another child that it spawns.

        // Create pipes for a child process.
        let (reader, mut writer) = crate::pipe().unwrap();

        // Write input. This shouldn't block because it's small. Then close the write end, or else
        // the child will hang.
        writer.write_all(b"quack").unwrap();
        drop(writer);

        // Use `swap` to run `cat_both`. `cat_both will read "quack" from stdin
        // and write it to stdout and stderr with different tags. But because we
        // run it inside `swap`, the tags in the output should be backwards.
        let output = Command::new(path_to_exe("swap"))
            .arg(path_to_exe("cat_both"))
            .stdin(reader)
            .output()
            .unwrap();

        // Check for a clean exit.
        assert!(
            output.status.success(),
            "child process returned {:#?}",
            output
        );

        // Confirm that we got the right bytes.
        assert_eq!(b"stderr: quack", &*output.stdout);
        assert_eq!(b"stdout: quack", &*output.stderr);
    }

    #[test]
    fn test_parent_handles_dont_close() {
        // Open and close each parent pipe multiple times. If this closes the
        // original, subsequent opens should fail.
        let stdin = crate::dup_stdin().unwrap();
        drop(stdin);
        let stdin = crate::dup_stdin().unwrap();
        drop(stdin);

        let stdout = crate::dup_stdout().unwrap();
        drop(stdout);
        let stdout = crate::dup_stdout().unwrap();
        drop(stdout);

        let stderr = crate::dup_stderr().unwrap();
        drop(stderr);
        let stderr = crate::dup_stderr().unwrap();
        drop(stderr);
    }

    #[test]
    fn test_try_clone() {
        let (reader, writer) = crate::pipe().unwrap();
        let mut reader_clone = reader.try_clone().unwrap();
        let mut writer_clone = writer.try_clone().unwrap();
        // A small write won't fill the pipe buffer, so it won't block this thread.
        writer_clone.write_all(b"some stuff").unwrap();
        drop(writer);
        drop(writer_clone);
        let mut out = String::new();
        reader_clone.read_to_string(&mut out).unwrap();
        assert_eq!(out, "some stuff");
    }

    #[test]
    fn test_debug() {
        let (reader, writer) = crate::pipe().unwrap();
        _ = format!("{:?} {:?}", reader, writer);
    }
}
