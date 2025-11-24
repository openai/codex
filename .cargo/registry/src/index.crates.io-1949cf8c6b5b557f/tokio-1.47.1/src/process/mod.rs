//! An implementation of asynchronous process management for Tokio.
//!
//! This module provides a [`Command`] struct that imitates the interface of the
//! [`std::process::Command`] type in the standard library, but provides asynchronous versions of
//! functions that create processes. These functions (`spawn`, `status`, `output` and their
//! variants) return "future aware" types that interoperate with Tokio. The asynchronous process
//! support is provided through signal handling on Unix and system APIs on Windows.
//!
//! [`std::process::Command`]: std::process::Command
//!
//! # Examples
//!
//! Here's an example program which will spawn `echo hello world` and then wait
//! for it complete.
//!
//! ```no_run
//! use tokio::process::Command;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // The usage is similar as with the standard library's `Command` type
//!     let mut child = Command::new("echo")
//!         .arg("hello")
//!         .arg("world")
//!         .spawn()
//!         .expect("failed to spawn");
//!
//!     // Await until the command completes
//!     let status = child.wait().await?;
//!     println!("the command exited with: {}", status);
//!     Ok(())
//! }
//! ```
//!
//! Next, let's take a look at an example where we not only spawn `echo hello
//! world` but we also capture its output.
//!
//! ```no_run
//! use tokio::process::Command;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Like above, but use `output` which returns a future instead of
//!     // immediately returning the `Child`.
//!     let output = Command::new("echo").arg("hello").arg("world")
//!                         .output();
//!
//!     let output = output.await?;
//!
//!     assert!(output.status.success());
//!     assert_eq!(output.stdout, b"hello world\n");
//!     Ok(())
//! }
//! ```
//!
//! We can also read input line by line.
//!
//! ```no_run
//! use tokio::io::{BufReader, AsyncBufReadExt};
//! use tokio::process::Command;
//!
//! use std::process::Stdio;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut cmd = Command::new("cat");
//!
//!     // Specify that we want the command's standard output piped back to us.
//!     // By default, standard input/output/error will be inherited from the
//!     // current process (for example, this means that standard input will
//!     // come from the keyboard and standard output/error will go directly to
//!     // the terminal if this process is invoked from the command line).
//!     cmd.stdout(Stdio::piped());
//!
//!     let mut child = cmd.spawn()
//!         .expect("failed to spawn command");
//!
//!     let stdout = child.stdout.take()
//!         .expect("child did not have a handle to stdout");
//!
//!     let mut reader = BufReader::new(stdout).lines();
//!
//!     // Ensure the child process is spawned in the runtime so it can
//!     // make progress on its own while we await for any output.
//!     tokio::spawn(async move {
//!         let status = child.wait().await
//!             .expect("child process encountered an error");
//!
//!         println!("child status was: {}", status);
//!     });
//!
//!     while let Some(line) = reader.next_line().await? {
//!         println!("Line: {}", line);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! Here is another example using `sort` writing into the child process
//! standard input, capturing the output of the sorted text.
//!
//! ```no_run
//! use tokio::io::AsyncWriteExt;
//! use tokio::process::Command;
//!
//! use std::process::Stdio;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut cmd = Command::new("sort");
//!
//!     // Specifying that we want pipe both the output and the input.
//!     // Similarly to capturing the output, by configuring the pipe
//!     // to stdin it can now be used as an asynchronous writer.
//!     cmd.stdout(Stdio::piped());
//!     cmd.stdin(Stdio::piped());
//!
//!     let mut child = cmd.spawn().expect("failed to spawn command");
//!
//!     // These are the animals we want to sort
//!     let animals: &[&str] = &["dog", "bird", "frog", "cat", "fish"];
//!
//!     let mut stdin = child
//!         .stdin
//!         .take()
//!         .expect("child did not have a handle to stdin");
//!
//!     // Write our animals to the child process
//!     // Note that the behavior of `sort` is to buffer _all input_ before writing any output.
//!     // In the general sense, it is recommended to write to the child in a separate task as
//!     // awaiting its exit (or output) to avoid deadlocks (for example, the child tries to write
//!     // some output but gets stuck waiting on the parent to read from it, meanwhile the parent
//!     // is stuck waiting to write its input completely before reading the output).
//!     stdin
//!         .write(animals.join("\n").as_bytes())
//!         .await
//!         .expect("could not write to stdin");
//!
//!     // We drop the handle here which signals EOF to the child process.
//!     // This tells the child process that it there is no more data on the pipe.
//!     drop(stdin);
//!
//!     let op = child.wait_with_output().await?;
//!
//!     // Results should come back in sorted order
//!     assert_eq!(op.stdout, "bird\ncat\ndog\nfish\nfrog\n".as_bytes());
//!
//!     Ok(())
//! }
//! ```
//!
//! With some coordination, we can also pipe the output of one command into
//! another.
//!
//! ```no_run
//! use tokio::join;
//! use tokio::process::Command;
//! use std::process::Stdio;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut echo = Command::new("echo")
//!         .arg("hello world!")
//!         .stdout(Stdio::piped())
//!         .spawn()
//!         .expect("failed to spawn echo");
//!
//!     let tr_stdin: Stdio = echo
//!         .stdout
//!         .take()
//!         .unwrap()
//!         .try_into()
//!         .expect("failed to convert to Stdio");
//!
//!     let tr = Command::new("tr")
//!         .arg("a-z")
//!         .arg("A-Z")
//!         .stdin(tr_stdin)
//!         .stdout(Stdio::piped())
//!         .spawn()
//!         .expect("failed to spawn tr");
//!
//!     let (echo_result, tr_output) = join!(echo.wait(), tr.wait_with_output());
//!
//!     assert!(echo_result.unwrap().success());
//!
//!     let tr_output = tr_output.expect("failed to await tr");
//!     assert!(tr_output.status.success());
//!
//!     assert_eq!(tr_output.stdout, b"HELLO WORLD!\n");
//!
//!     Ok(())
//! }
//! ```
//!
//! # Caveats
//!
//! ## Dropping/Cancellation
//!
//! Similar to the behavior to the standard library, and unlike the futures
//! paradigm of dropping-implies-cancellation, a spawned process will, by
//! default, continue to execute even after the `Child` handle has been dropped.
//!
//! The [`Command::kill_on_drop`] method can be used to modify this behavior
//! and kill the child process if the `Child` wrapper is dropped before it
//! has exited.
//!
//! ## Unix Processes
//!
//! On Unix platforms processes must be "reaped" by their parent process after
//! they have exited in order to release all OS resources. A child process which
//! has exited, but has not yet been reaped by its parent is considered a "zombie"
//! process. Such processes continue to count against limits imposed by the system,
//! and having too many zombie processes present can prevent additional processes
//! from being spawned.
//!
//! The tokio runtime will, on a best-effort basis, attempt to reap and clean up
//! any process which it has spawned. No additional guarantees are made with regard to
//! how quickly or how often this procedure will take place.
//!
//! It is recommended to avoid dropping a [`Child`] process handle before it has been
//! fully `await`ed if stricter cleanup guarantees are required.
//!
//! [`Command`]: crate::process::Command
//! [`Command::kill_on_drop`]: crate::process::Command::kill_on_drop
//! [`Child`]: crate::process::Child

#[path = "unix/mod.rs"]
#[cfg(unix)]
mod imp;

#[cfg(unix)]
pub(crate) mod unix {
    pub(crate) use super::imp::*;
}

#[path = "windows.rs"]
#[cfg(windows)]
mod imp;

mod kill;

use crate::io::{AsyncRead, AsyncWrite, ReadBuf};
use crate::process::kill::Kill;

use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::process::{Child as StdChild, Command as StdCommand, ExitStatus, Output, Stdio};
use std::task::{ready, Context, Poll};

#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

cfg_windows! {
    use crate::os::windows::io::{AsRawHandle, RawHandle};
}

/// This structure mimics the API of [`std::process::Command`] found in the standard library, but
/// replaces functions that create a process with an asynchronous variant. The main provided
/// asynchronous functions are [spawn](Command::spawn), [status](Command::status), and
/// [output](Command::output).
///
/// `Command` uses asynchronous versions of some `std` types (for example [`Child`]).
///
/// [`std::process::Command`]: std::process::Command
/// [`Child`]: struct@Child
#[derive(Debug)]
pub struct Command {
    std: StdCommand,
    kill_on_drop: bool,
}

pub(crate) struct SpawnedChild {
    child: imp::Child,
    stdin: Option<imp::ChildStdio>,
    stdout: Option<imp::ChildStdio>,
    stderr: Option<imp::ChildStdio>,
}

impl Command {
    /// Constructs a new `Command` for launching the program at
    /// path `program`, with the following default configuration:
    ///
    /// * No arguments to the program
    /// * Inherit the current process's environment
    /// * Inherit the current process's working directory
    /// * Inherit stdin/stdout/stderr for `spawn` or `status`, but create pipes for `output`
    ///
    /// Builder methods are provided to change these defaults and
    /// otherwise configure the process.
    ///
    /// If `program` is not an absolute path, the `PATH` will be searched in
    /// an OS-defined way.
    ///
    /// The search path to be used may be controlled by setting the
    /// `PATH` environment variable on the Command,
    /// but this has some implementation limitations on Windows
    /// (see issue [rust-lang/rust#37519]).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use tokio::process::Command;
    /// let mut command = Command::new("sh");
    /// # let _ = command.output(); // assert borrow checker
    /// ```
    ///
    /// [rust-lang/rust#37519]: https://github.com/rust-lang/rust/issues/37519
    pub fn new<S: AsRef<OsStr>>(program: S) -> Command {
        Self::from(StdCommand::new(program))
    }

    /// Cheaply convert to a `&std::process::Command` for places where the type from the standard
    /// library is expected.
    pub fn as_std(&self) -> &StdCommand {
        &self.std
    }

    /// Cheaply convert to a `&mut std::process::Command` for places where the type from the
    /// standard library is expected.
    pub fn as_std_mut(&mut self) -> &mut StdCommand {
        &mut self.std
    }

    /// Cheaply convert into a `std::process::Command`.
    ///
    /// Note that Tokio specific options will be lost. Currently, this only applies to [`kill_on_drop`].
    ///
    /// [`kill_on_drop`]: Command::kill_on_drop
    pub fn into_std(self) -> StdCommand {
        self.std
    }

    /// Adds an argument to pass to the program.
    ///
    /// Only one argument can be passed per use. So instead of:
    ///
    /// ```no_run
    /// let mut command = tokio::process::Command::new("sh");
    /// command.arg("-C /path/to/repo");
    ///
    /// # let _ = command.output(); // assert borrow checker
    /// ```
    ///
    /// usage would be:
    ///
    /// ```no_run
    /// let mut command = tokio::process::Command::new("sh");
    /// command.arg("-C");
    /// command.arg("/path/to/repo");
    ///
    /// # let _ = command.output(); // assert borrow checker
    /// ```
    ///
    /// To pass multiple arguments see [`args`].
    ///
    /// [`args`]: method@Self::args
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    ///
    /// let output = Command::new("ls")
    ///         .arg("-l")
    ///         .arg("-a")
    ///         .output().await.unwrap();
    /// # }
    ///
    /// ```
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.std.arg(arg);
        self
    }

    /// Adds multiple arguments to pass to the program.
    ///
    /// To pass a single argument see [`arg`].
    ///
    /// [`arg`]: method@Self::arg
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    ///
    /// let output = Command::new("ls")
    ///         .args(&["-l", "-a"])
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn args<I, S>(&mut self, args: I) -> &mut Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.std.args(args);
        self
    }

    cfg_windows! {
        /// Append literal text to the command line without any quoting or escaping.
        ///
        /// This is useful for passing arguments to `cmd.exe /c`, which doesn't follow
        /// `CommandLineToArgvW` escaping rules.
        pub fn raw_arg<S: AsRef<OsStr>>(&mut self, text_to_append_as_is: S) -> &mut Command {
            self.std.raw_arg(text_to_append_as_is);
            self
        }
    }

    /// Inserts or updates an environment variable mapping.
    ///
    /// Note that environment variable names are case-insensitive (but case-preserving) on Windows,
    /// and case-sensitive on all other platforms.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    ///
    /// let output = Command::new("ls")
    ///         .env("PATH", "/bin")
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Command
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.std.env(key, val);
        self
    }

    /// Adds or updates multiple environment variable mappings.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    /// use std::process::{Stdio};
    /// use std::env;
    /// use std::collections::HashMap;
    ///
    /// let filtered_env : HashMap<String, String> =
    ///     env::vars().filter(|&(ref k, _)|
    ///         k == "TERM" || k == "TZ" || k == "LANG" || k == "PATH"
    ///     ).collect();
    ///
    /// let output = Command::new("printenv")
    ///         .stdin(Stdio::null())
    ///         .stdout(Stdio::inherit())
    ///         .env_clear()
    ///         .envs(&filtered_env)
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Command
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.std.envs(vars);
        self
    }

    /// Removes an environment variable mapping.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    ///
    /// let output = Command::new("ls")
    ///         .env_remove("PATH")
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Command {
        self.std.env_remove(key);
        self
    }

    /// Clears the entire environment map for the child process.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    ///
    /// let output = Command::new("ls")
    ///         .env_clear()
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn env_clear(&mut self) -> &mut Command {
        self.std.env_clear();
        self
    }

    /// Sets the working directory for the child process.
    ///
    /// # Platform-specific behavior
    ///
    /// If the program path is relative (e.g., `"./script.sh"`), it's ambiguous
    /// whether it should be interpreted relative to the parent's working
    /// directory or relative to `current_dir`. The behavior in this case is
    /// platform specific and unstable, and it's recommended to use
    /// [`canonicalize`] to get an absolute program path instead.
    ///
    /// [`canonicalize`]: crate::fs::canonicalize()
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    ///
    /// let output = Command::new("ls")
    ///         .current_dir("/bin")
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Command {
        self.std.current_dir(dir);
        self
    }

    /// Sets configuration for the child process's standard input (stdin) handle.
    ///
    /// Defaults to [`inherit`].
    ///
    /// [`inherit`]: std::process::Stdio::inherit
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use std::process::{Stdio};
    /// use tokio::process::Command;
    ///
    /// let output = Command::new("ls")
    ///         .stdin(Stdio::null())
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.std.stdin(cfg);
        self
    }

    /// Sets configuration for the child process's standard output (stdout) handle.
    ///
    /// Defaults to [`inherit`] when used with `spawn` or `status`, and
    /// defaults to [`piped`] when used with `output`.
    ///
    /// [`inherit`]: std::process::Stdio::inherit
    /// [`piped`]: std::process::Stdio::piped
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    /// use std::process::Stdio;
    ///
    /// let output = Command::new("ls")
    ///         .stdout(Stdio::null())
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.std.stdout(cfg);
        self
    }

    /// Sets configuration for the child process's standard error (stderr) handle.
    ///
    /// Defaults to [`inherit`] when used with `spawn` or `status`, and
    /// defaults to [`piped`] when used with `output`.
    ///
    /// [`inherit`]: std::process::Stdio::inherit
    /// [`piped`]: std::process::Stdio::piped
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    /// use std::process::{Stdio};
    ///
    /// let output = Command::new("ls")
    ///         .stderr(Stdio::null())
    ///         .output().await.unwrap();
    /// # }
    /// ```
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.std.stderr(cfg);
        self
    }

    /// Controls whether a `kill` operation should be invoked on a spawned child
    /// process when its corresponding `Child` handle is dropped.
    ///
    /// By default, this value is assumed to be `false`, meaning the next spawned
    /// process will not be killed on drop, similar to the behavior of the standard
    /// library.
    ///
    /// # Caveats
    ///
    /// On Unix platforms processes must be "reaped" by their parent process after
    /// they have exited in order to release all OS resources. A child process which
    /// has exited, but has not yet been reaped by its parent is considered a "zombie"
    /// process. Such processes continue to count against limits imposed by the system,
    /// and having too many zombie processes present can prevent additional processes
    /// from being spawned.
    ///
    /// Although issuing a `kill` signal to the child process is a synchronous
    /// operation, the resulting zombie process cannot be `.await`ed inside of the
    /// destructor to avoid blocking other tasks. The tokio runtime will, on a
    /// best-effort basis, attempt to reap and clean up such processes in the
    /// background, but no additional guarantees are made with regard to
    /// how quickly or how often this procedure will take place.
    ///
    /// If stronger guarantees are required, it is recommended to avoid dropping
    /// a [`Child`] handle where possible, and instead utilize `child.wait().await`
    /// or `child.kill().await` where possible.
    pub fn kill_on_drop(&mut self, kill_on_drop: bool) -> &mut Command {
        self.kill_on_drop = kill_on_drop;
        self
    }

    cfg_windows! {
        /// Sets the [process creation flags][1] to be passed to `CreateProcess`.
        ///
        /// These will always be ORed with `CREATE_UNICODE_ENVIRONMENT`.
        ///
        /// [1]: https://msdn.microsoft.com/en-us/library/windows/desktop/ms684863(v=vs.85).aspx
        pub fn creation_flags(&mut self, flags: u32) -> &mut Command {
            self.std.creation_flags(flags);
            self
        }
    }

    /// Sets the child process's user ID. This translates to a
    /// `setuid` call in the child process. Failure in the `setuid`
    /// call will cause the spawn to fail.
    #[cfg(unix)]
    #[cfg_attr(docsrs, doc(cfg(unix)))]
    pub fn uid(&mut self, id: u32) -> &mut Command {
        #[cfg(target_os = "nto")]
        let id = id as i32;
        self.std.uid(id);
        self
    }

    /// Similar to `uid` but sets the group ID of the child process. This has
    /// the same semantics as the `uid` field.
    #[cfg(unix)]
    #[cfg_attr(docsrs, doc(cfg(unix)))]
    pub fn gid(&mut self, id: u32) -> &mut Command {
        #[cfg(target_os = "nto")]
        let id = id as i32;
        self.std.gid(id);
        self
    }

    /// Sets executable argument.
    ///
    /// Set the first process argument, `argv[0]`, to something other than the
    /// default executable path.
    #[cfg(unix)]
    #[cfg_attr(docsrs, doc(cfg(unix)))]
    pub fn arg0<S>(&mut self, arg: S) -> &mut Command
    where
        S: AsRef<OsStr>,
    {
        self.std.arg0(arg);
        self
    }

    /// Schedules a closure to be run just before the `exec` function is
    /// invoked.
    ///
    /// The closure is allowed to return an I/O error whose OS error code will
    /// be communicated back to the parent and returned as an error from when
    /// the spawn was requested.
    ///
    /// Multiple closures can be registered and they will be called in order of
    /// their registration. If a closure returns `Err` then no further closures
    /// will be called and the spawn operation will immediately return with a
    /// failure.
    ///
    /// # Safety
    ///
    /// This closure will be run in the context of the child process after a
    /// `fork`. This primarily means that any modifications made to memory on
    /// behalf of this closure will **not** be visible to the parent process.
    /// This is often a very constrained environment where normal operations
    /// like `malloc` or acquiring a mutex are not guaranteed to work (due to
    /// other threads perhaps still running when the `fork` was run).
    ///
    /// This also means that all resources such as file descriptors and
    /// memory-mapped regions got duplicated. It is your responsibility to make
    /// sure that the closure does not violate library invariants by making
    /// invalid use of these duplicates.
    ///
    /// When this closure is run, aspects such as the stdio file descriptors and
    /// working directory have successfully been changed, so output to these
    /// locations may not appear where intended.
    #[cfg(unix)]
    #[cfg_attr(docsrs, doc(cfg(unix)))]
    pub unsafe fn pre_exec<F>(&mut self, f: F) -> &mut Command
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static,
    {
        self.std.pre_exec(f);
        self
    }

    /// Sets the process group ID (PGID) of the child process. Equivalent to a
    /// `setpgid` call in the child process, but may be more efficient.
    ///
    /// Process groups determine which processes receive signals.
    ///
    /// # Examples
    ///
    /// Pressing Ctrl-C in a terminal will send `SIGINT` to all processes
    /// in the current foreground process group. By spawning the `sleep`
    /// subprocess in a new process group, it will not receive `SIGINT`
    /// from the terminal.
    ///
    /// The parent process could install a [signal handler] and manage the
    /// process on its own terms.
    ///
    /// A process group ID of 0 will use the process ID as the PGID.
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use tokio::process::Command;
    ///
    /// let output = Command::new("sleep")
    ///     .arg("10")
    ///     .process_group(0)
    ///     .output()
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    ///
    /// [signal handler]: crate::signal
    #[cfg(unix)]
    #[cfg_attr(docsrs, doc(cfg(unix)))]
    pub fn process_group(&mut self, pgroup: i32) -> &mut Command {
        self.std.process_group(pgroup);
        self
    }

    /// Executes the command as a child process, returning a handle to it.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    ///
    /// This method will spawn the child process synchronously and return a
    /// handle to a future-aware child process. The `Child` returned implements
    /// `Future` itself to acquire the `ExitStatus` of the child, and otherwise
    /// the `Child` has methods to acquire handles to the stdin, stdout, and
    /// stderr streams.
    ///
    /// All I/O this child does will be associated with the current default
    /// event loop.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use tokio::process::Command;
    ///
    /// async fn run_ls() -> std::process::ExitStatus {
    ///     Command::new("ls")
    ///         .spawn()
    ///         .expect("ls command failed to start")
    ///         .wait()
    ///         .await
    ///         .expect("ls command failed to run")
    /// }
    /// ```
    ///
    /// # Caveats
    ///
    /// ## Dropping/Cancellation
    ///
    /// Similar to the behavior to the standard library, and unlike the futures
    /// paradigm of dropping-implies-cancellation, a spawned process will, by
    /// default, continue to execute even after the `Child` handle has been dropped.
    ///
    /// The [`Command::kill_on_drop`] method can be used to modify this behavior
    /// and kill the child process if the `Child` wrapper is dropped before it
    /// has exited.
    ///
    /// ## Unix Processes
    ///
    /// On Unix platforms processes must be "reaped" by their parent process after
    /// they have exited in order to release all OS resources. A child process which
    /// has exited, but has not yet been reaped by its parent is considered a "zombie"
    /// process. Such processes continue to count against limits imposed by the system,
    /// and having too many zombie processes present can prevent additional processes
    /// from being spawned.
    ///
    /// The tokio runtime will, on a best-effort basis, attempt to reap and clean up
    /// any process which it has spawned. No additional guarantees are made with regard to
    /// how quickly or how often this procedure will take place.
    ///
    /// It is recommended to avoid dropping a [`Child`] process handle before it has been
    /// fully `await`ed if stricter cleanup guarantees are required.
    ///
    /// [`Command`]: crate::process::Command
    /// [`Command::kill_on_drop`]: crate::process::Command::kill_on_drop
    /// [`Child`]: crate::process::Child
    ///
    /// # Errors
    ///
    /// On Unix platforms this method will fail with `std::io::ErrorKind::WouldBlock`
    /// if the system process limit is reached (which includes other applications
    /// running on the system).
    #[inline]
    pub fn spawn(&mut self) -> io::Result<Child> {
        // On two lines to circumvent a mutable borrow check failure.
        let child = self.std.spawn()?;
        self.build_child(child)
    }

    /// Executes the command as a child process with a custom spawning function,
    /// returning a handle to it.
    ///
    /// This is identical to [`Self::spawn`] in every aspect except the spawn:
    /// here, it is customizable through the `with` parameter instead of
    /// defaulting to the usual spawn. In fact, [`Self::spawn`] is just
    /// [`Self::spawn_with`] with [`StdCommand::spawn`].
    ///
    /// This is useful mostly under Windows for now, since the platform exposes
    /// special APIs to configure child processes when spawning them with various
    /// attributes that customize the exact behavior of the spawn operation.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # async fn test() { // allow using await
    /// use std::process::Stdio;
    ///
    /// let output = tokio::process::Command::new("ls")
    ///     .stdin(Stdio::null())
    ///     .stdout(Stdio::piped())
    ///     .stderr(Stdio::piped())
    ///     .spawn_with(std::process::Command::spawn)
    ///     .unwrap()
    ///     .wait_with_output()
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    ///
    /// Actually customizing the spawn under Windows:
    ///
    /// ```ignore
    /// #![feature(windows_process_extensions_raw_attribute)]
    /// # #[cfg(windows)]   // Windows-only nightly APIs are used here.
    /// # async fn test() { // Allow using await.
    /// use std::os::windows::process::{CommandExt, ProcThreadAttributeList};
    /// use std::process::Stdio;
    /// use tokio::process::Command;
    ///
    /// let parent = Command::new("cmd").spawn().unwrap();
    /// let parent_process_handle = parent.raw_handle();
    ///
    /// const PROC_THREAD_ATTRIBUTE_PARENT_PROCESS: usize = 0x00020000;
    /// let attribute_list = ProcThreadAttributeList::build()
    ///     .attribute(PROC_THREAD_ATTRIBUTE_PARENT_PROCESS, &parent_process_handle)
    ///     .finish()
    ///     .unwrap();
    ///
    /// let _output = Command::new("ls")
    ///     .stdin(Stdio::null())
    ///     .stdout(Stdio::piped())
    ///     .stderr(Stdio::piped())
    ///     .spawn_with(|cmd| cmd.spawn_with_attributes(&attribute_list))
    ///     .unwrap()
    ///     .wait_with_output()
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    #[cfg(tokio_unstable)]
    #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
    #[inline]
    pub fn spawn_with(
        &mut self,
        with: impl Fn(&mut StdCommand) -> io::Result<StdChild>,
    ) -> io::Result<Child> {
        // On two lines to circumvent a mutable borrow check failure.
        let child = with(&mut self.std)?;
        self.build_child(child)
    }

    /// Small indirection for the spawn implementations.
    ///
    /// This is introduced for [`Self::spawn`] and [`Self::spawn_with`] to use:
    /// [`Self::spawn`] cannot depend directly on [`Self::spawn_with`] since
    /// it is behind `tokio_unstable`. It also serves as a way to reduce
    /// monomorphization bloat by taking in an already-spawned child process
    /// instead of a command and custom spawn function.
    fn build_child(&self, child: StdChild) -> io::Result<Child> {
        let spawned_child = imp::build_child(child)?;

        Ok(Child {
            child: FusedChild::Child(ChildDropGuard {
                inner: spawned_child.child,
                kill_on_drop: self.kill_on_drop,
            }),
            stdin: spawned_child.stdin.map(|inner| ChildStdin { inner }),
            stdout: spawned_child.stdout.map(|inner| ChildStdout { inner }),
            stderr: spawned_child.stderr.map(|inner| ChildStderr { inner }),
        })
    }

    /// Executes the command as a child process, waiting for it to finish and
    /// collecting its exit status.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent.
    /// If any input/output handles are set to a pipe then they will be immediately
    /// closed after the child is spawned.
    ///
    /// All I/O this child does will be associated with the current default
    /// event loop.
    ///
    /// The destructor of the future returned by this function will kill
    /// the child if [`kill_on_drop`] is set to true.
    ///
    /// [`kill_on_drop`]: fn@Self::kill_on_drop
    ///
    /// # Errors
    ///
    /// This future will return an error if the child process cannot be spawned
    /// or if there is an error while awaiting its status.
    ///
    /// On Unix platforms this method will fail with `std::io::ErrorKind::WouldBlock`
    /// if the system process limit is reached (which includes other applications
    /// running on the system).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use tokio::process::Command;
    ///
    /// async fn run_ls() -> std::process::ExitStatus {
    ///     Command::new("ls")
    ///         .status()
    ///         .await
    ///         .expect("ls command failed to run")
    /// }
    /// ```
    pub fn status(&mut self) -> impl Future<Output = io::Result<ExitStatus>> {
        let child = self.spawn();

        async {
            let mut child = child?;

            // Ensure we close any stdio handles so we can't deadlock
            // waiting on the child which may be waiting to read/write
            // to a pipe we're holding.
            child.stdin.take();
            child.stdout.take();
            child.stderr.take();

            child.wait().await
        }
    }

    /// Executes the command as a child process, waiting for it to finish and
    /// collecting all of its output.
    ///
    /// > **Note**: this method, unlike the standard library, will
    /// > unconditionally configure the stdout/stderr handles to be pipes, even
    /// > if they have been previously configured. If this is not desired then
    /// > the `spawn` method should be used in combination with the
    /// > `wait_with_output` method on child.
    ///
    /// This method will return a future representing the collection of the
    /// child process's stdout/stderr. It will resolve to
    /// the `Output` type in the standard library, containing `stdout` and
    /// `stderr` as `Vec<u8>` along with an `ExitStatus` representing how the
    /// process exited.
    ///
    /// All I/O this child does will be associated with the current default
    /// event loop.
    ///
    /// The destructor of the future returned by this function will kill
    /// the child if [`kill_on_drop`] is set to true.
    ///
    /// [`kill_on_drop`]: fn@Self::kill_on_drop
    ///
    /// # Errors
    ///
    /// This future will return an error if the child process cannot be spawned
    /// or if there is an error while awaiting its status.
    ///
    /// On Unix platforms this method will fail with `std::io::ErrorKind::WouldBlock`
    /// if the system process limit is reached (which includes other applications
    /// running on the system).
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use tokio::process::Command;
    ///
    /// async fn run_ls() {
    ///     let output: std::process::Output = Command::new("ls")
    ///         .output()
    ///         .await
    ///         .expect("ls command failed to run");
    ///     println!("stderr of ls: {:?}", output.stderr);
    /// }
    /// ```
    pub fn output(&mut self) -> impl Future<Output = io::Result<Output>> {
        self.std.stdout(Stdio::piped());
        self.std.stderr(Stdio::piped());

        let child = self.spawn();

        async { child?.wait_with_output().await }
    }

    /// Returns the boolean value that was previously set by [`Command::kill_on_drop`].
    ///
    /// Note that if you have not previously called [`Command::kill_on_drop`], the
    /// default value of `false` will be returned here.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::process::Command;
    ///
    /// let mut cmd = Command::new("echo");
    /// assert!(!cmd.get_kill_on_drop());
    ///
    /// cmd.kill_on_drop(true);
    /// assert!(cmd.get_kill_on_drop());
    /// ```
    pub fn get_kill_on_drop(&self) -> bool {
        self.kill_on_drop
    }
}

impl From<StdCommand> for Command {
    fn from(std: StdCommand) -> Command {
        Command {
            std,
            kill_on_drop: false,
        }
    }
}

/// A drop guard which can ensure the child process is killed on drop if specified.
#[derive(Debug)]
struct ChildDropGuard<T: Kill> {
    inner: T,
    kill_on_drop: bool,
}

impl<T: Kill> Kill for ChildDropGuard<T> {
    fn kill(&mut self) -> io::Result<()> {
        let ret = self.inner.kill();

        if ret.is_ok() {
            self.kill_on_drop = false;
        }

        ret
    }
}

impl<T: Kill> Drop for ChildDropGuard<T> {
    fn drop(&mut self) {
        if self.kill_on_drop {
            drop(self.kill());
        }
    }
}

impl<T, E, F> Future for ChildDropGuard<F>
where
    F: Future<Output = Result<T, E>> + Kill + Unpin,
{
    type Output = Result<T, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(crate::trace::trace_leaf(cx));
        // Keep track of task budget
        let coop = ready!(crate::task::coop::poll_proceed(cx));

        let ret = Pin::new(&mut self.inner).poll(cx);

        if let Poll::Ready(Ok(_)) = ret {
            // Avoid the overhead of trying to kill a reaped process
            self.kill_on_drop = false;
        }

        if ret.is_ready() {
            coop.made_progress();
        }

        ret
    }
}

/// Keeps track of the exit status of a child process without worrying about
/// polling the underlying futures even after they have completed.
#[derive(Debug)]
enum FusedChild {
    Child(ChildDropGuard<imp::Child>),
    Done(ExitStatus),
}

/// Representation of a child process spawned onto an event loop.
///
/// # Caveats
/// Similar to the behavior to the standard library, and unlike the futures
/// paradigm of dropping-implies-cancellation, a spawned process will, by
/// default, continue to execute even after the `Child` handle has been dropped.
///
/// The `Command::kill_on_drop` method can be used to modify this behavior
/// and kill the child process if the `Child` wrapper is dropped before it
/// has exited.
#[derive(Debug)]
pub struct Child {
    child: FusedChild,

    /// The handle for writing to the child's standard input (stdin), if it has
    /// been captured. To avoid partially moving the `child` and thus blocking
    /// yourself from calling functions on `child` while using `stdin`, you might
    /// find it helpful to do:
    ///
    /// ```no_run
    /// # let mut child = tokio::process::Command::new("echo").spawn().unwrap();
    /// let stdin = child.stdin.take().unwrap();
    /// ```
    pub stdin: Option<ChildStdin>,

    /// The handle for reading from the child's standard output (stdout), if it
    /// has been captured. You might find it helpful to do
    ///
    /// ```no_run
    /// # let mut child = tokio::process::Command::new("echo").spawn().unwrap();
    /// let stdout = child.stdout.take().unwrap();
    /// ```
    ///
    /// to avoid partially moving the `child` and thus blocking yourself from calling
    /// functions on `child` while using `stdout`.
    pub stdout: Option<ChildStdout>,

    /// The handle for reading from the child's standard error (stderr), if it
    /// has been captured. You might find it helpful to do
    ///
    /// ```no_run
    /// # let mut child = tokio::process::Command::new("echo").spawn().unwrap();
    /// let stderr = child.stderr.take().unwrap();
    /// ```
    ///
    /// to avoid partially moving the `child` and thus blocking yourself from calling
    /// functions on `child` while using `stderr`.
    pub stderr: Option<ChildStderr>,
}

impl Child {
    /// Returns the OS-assigned process identifier associated with this child
    /// while it is still running.
    ///
    /// Once the child has been polled to completion this will return `None`.
    /// This is done to avoid confusion on platforms like Unix where the OS
    /// identifier could be reused once the process has completed.
    pub fn id(&self) -> Option<u32> {
        match &self.child {
            FusedChild::Child(child) => Some(child.inner.id()),
            FusedChild::Done(_) => None,
        }
    }

    cfg_windows! {
        /// Extracts the raw handle of the process associated with this child while
        /// it is still running. Returns `None` if the child has exited.
        pub fn raw_handle(&self) -> Option<RawHandle> {
            match &self.child {
                FusedChild::Child(c) => Some(c.inner.as_raw_handle()),
                FusedChild::Done(_) => None,
            }
        }
    }

    /// Attempts to force the child to exit, but does not wait for the request
    /// to take effect.
    ///
    /// On Unix platforms, this is the equivalent to sending a `SIGKILL`. Note
    /// that on Unix platforms it is possible for a zombie process to remain
    /// after a kill is sent; to avoid this, the caller should ensure that either
    /// `child.wait().await` or `child.try_wait()` is invoked successfully.
    pub fn start_kill(&mut self) -> io::Result<()> {
        match &mut self.child {
            FusedChild::Child(child) => child.kill(),
            FusedChild::Done(_) => Ok(()),
        }
    }

    /// Forces the child to exit.
    ///
    /// This is equivalent to sending a `SIGKILL` on unix platforms
    /// followed by [`wait`](Child::wait).
    ///
    /// Note: std version of [`Child::kill`](std::process::Child::kill) does not `wait`.
    /// For an equivalent of `Child::kill` in the standard library,
    /// use [`start_kill`](Child::start_kill).
    ///
    /// # Examples
    ///
    /// If the child has to be killed remotely, it is possible to do it using
    /// a combination of the select! macro and a `oneshot` channel. In the following
    /// example, the child will run until completion unless a message is sent on
    /// the `oneshot` channel. If that happens, the child is killed immediately
    /// using the `.kill()` method.
    ///
    /// ```no_run
    /// use tokio::process::Command;
    /// use tokio::sync::oneshot::channel;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (send, recv) = channel::<()>();
    ///     let mut child = Command::new("sleep").arg("1").spawn().unwrap();
    ///     tokio::spawn(async move { send.send(()) });
    ///     tokio::select! {
    ///         _ = child.wait() => {}
    ///         _ = recv => child.kill().await.expect("kill failed"),
    ///     }
    /// }
    /// ```
    ///
    /// You can also interact with the child's standard I/O. For example, you can
    /// read its stdout while waiting for it to exit.
    ///
    /// ```no_run
    /// # use std::process::Stdio;
    /// #
    /// # use tokio::io::AsyncReadExt;
    /// # use tokio::process::Command;
    /// # use tokio::sync::oneshot::channel;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let (_tx, rx) = channel::<()>();
    ///
    ///     let mut child = Command::new("echo")
    ///         .arg("Hello World!")
    ///         .stdout(Stdio::piped())
    ///         .spawn()
    ///         .unwrap();
    ///
    ///     let mut stdout = child.stdout.take().expect("stdout is not captured");
    ///
    ///     let read_stdout = tokio::spawn(async move {
    ///         let mut buff = Vec::new();
    ///         let _ = stdout.read_to_end(&mut buff).await;
    ///
    ///         buff
    ///     });
    ///
    ///     tokio::select! {
    ///         _ = child.wait() => {}
    ///         _ = rx => { child.kill().await.expect("kill failed") },
    ///     }
    ///
    ///     let buff = read_stdout.await.unwrap();
    ///
    ///     assert_eq!(buff, b"Hello World!\n");
    /// }
    /// ```
    pub async fn kill(&mut self) -> io::Result<()> {
        self.start_kill()?;
        self.wait().await?;
        Ok(())
    }

    /// Waits for the child to exit completely, returning the status that it
    /// exited with. This function will continue to have the same return value
    /// after it has been called at least once.
    ///
    /// The stdin handle to the child process, if any, will be closed
    /// before waiting. This helps avoid deadlock: it ensures that the
    /// child does not block waiting for input from the parent, while
    /// the parent waits for the child to exit.
    ///
    /// If the caller wishes to explicitly control when the child's stdin
    /// handle is closed, they may `.take()` it before calling `.wait()`:
    ///
    /// # Cancel safety
    ///
    /// This function is cancel safe.
    ///
    /// ```
    /// # #[cfg(not(unix))]fn main(){}
    /// # #[cfg(unix)]
    /// use tokio::io::AsyncWriteExt;
    /// # #[cfg(unix)]
    /// use tokio::process::Command;
    /// # #[cfg(unix)]
    /// use std::process::Stdio;
    ///
    /// # #[cfg(unix)]
    /// #[tokio::main]
    /// async fn main() {
    /// #   if cfg!(miri) { return; } // No `pidfd_spawnp` in miri.
    ///     let mut child = Command::new("cat")
    ///         .stdin(Stdio::piped())
    ///         .spawn()
    ///         .unwrap();
    ///
    ///     let mut stdin = child.stdin.take().unwrap();
    ///     tokio::spawn(async move {
    ///         // do something with stdin here...
    ///         stdin.write_all(b"hello world\n").await.unwrap();
    ///
    ///         // then drop when finished
    ///         drop(stdin);
    ///     });
    ///
    ///     // wait for the process to complete
    ///     let _ = child.wait().await;
    /// }
    /// ```
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        // Ensure stdin is closed so the child isn't stuck waiting on
        // input while the parent is waiting for it to exit.
        drop(self.stdin.take());

        match &mut self.child {
            FusedChild::Done(exit) => Ok(*exit),
            FusedChild::Child(child) => {
                let ret = child.await;

                if let Ok(exit) = ret {
                    self.child = FusedChild::Done(exit);
                }

                ret
            }
        }
    }

    /// Attempts to collect the exit status of the child if it has already
    /// exited.
    ///
    /// This function will not block the calling thread and will only
    /// check to see if the child process has exited or not. If the child has
    /// exited then on Unix the process ID is reaped. This function is
    /// guaranteed to repeatedly return a successful exit status so long as the
    /// child has already exited.
    ///
    /// If the child has exited, then `Ok(Some(status))` is returned. If the
    /// exit status is not available at this time then `Ok(None)` is returned.
    /// If an error occurs, then that error is returned.
    ///
    /// Note that unlike `wait`, this function will not attempt to drop stdin,
    /// nor will it wake the current task if the child exits.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match &mut self.child {
            FusedChild::Done(exit) => Ok(Some(*exit)),
            FusedChild::Child(guard) => {
                let ret = guard.inner.try_wait();

                if let Ok(Some(exit)) = ret {
                    // Avoid the overhead of trying to kill a reaped process
                    guard.kill_on_drop = false;
                    self.child = FusedChild::Done(exit);
                }

                ret
            }
        }
    }

    /// Returns a future that will resolve to an `Output`, containing the exit
    /// status, stdout, and stderr of the child process.
    ///
    /// The returned future will simultaneously waits for the child to exit and
    /// collect all remaining output on the stdout/stderr handles, returning an
    /// `Output` instance.
    ///
    /// The stdin handle to the child process, if any, will be closed before
    /// waiting. This helps avoid deadlock: it ensures that the child does not
    /// block waiting for input from the parent, while the parent waits for the
    /// child to exit.
    ///
    /// By default, stdin, stdout and stderr are inherited from the parent. In
    /// order to capture the output into this `Output` it is necessary to create
    /// new pipes between parent and child. Use `stdout(Stdio::piped())` or
    /// `stderr(Stdio::piped())`, respectively, when creating a `Command`.
    pub async fn wait_with_output(mut self) -> io::Result<Output> {
        use crate::future::try_join3;

        async fn read_to_end<A: AsyncRead + Unpin>(io: &mut Option<A>) -> io::Result<Vec<u8>> {
            let mut vec = Vec::new();
            if let Some(io) = io.as_mut() {
                crate::io::util::read_to_end(io, &mut vec).await?;
            }
            Ok(vec)
        }

        let mut stdout_pipe = self.stdout.take();
        let mut stderr_pipe = self.stderr.take();

        let stdout_fut = read_to_end(&mut stdout_pipe);
        let stderr_fut = read_to_end(&mut stderr_pipe);

        let (status, stdout, stderr) = try_join3(self.wait(), stdout_fut, stderr_fut).await?;

        // Drop happens after `try_join` due to <https://github.com/tokio-rs/tokio/issues/4309>
        drop(stdout_pipe);
        drop(stderr_pipe);

        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }
}

/// The standard input stream for spawned children.
///
/// This type implements the `AsyncWrite` trait to pass data to the stdin
/// handle of a child process asynchronously.
#[derive(Debug)]
pub struct ChildStdin {
    inner: imp::ChildStdio,
}

/// The standard output stream for spawned children.
///
/// This type implements the `AsyncRead` trait to read data from the stdout
/// handle of a child process asynchronously.
#[derive(Debug)]
pub struct ChildStdout {
    inner: imp::ChildStdio,
}

/// The standard error stream for spawned children.
///
/// This type implements the `AsyncRead` trait to read data from the stderr
/// handle of a child process asynchronously.
#[derive(Debug)]
pub struct ChildStderr {
    inner: imp::ChildStdio,
}

impl ChildStdin {
    /// Creates an asynchronous `ChildStdin` from a synchronous one.
    ///
    /// # Errors
    ///
    /// This method may fail if an error is encountered when setting the pipe to
    /// non-blocking mode, or when registering the pipe with the runtime's IO
    /// driver.
    pub fn from_std(inner: std::process::ChildStdin) -> io::Result<Self> {
        Ok(Self {
            inner: imp::stdio(inner)?,
        })
    }
}

impl ChildStdout {
    /// Creates an asynchronous `ChildStdout` from a synchronous one.
    ///
    /// # Errors
    ///
    /// This method may fail if an error is encountered when setting the pipe to
    /// non-blocking mode, or when registering the pipe with the runtime's IO
    /// driver.
    pub fn from_std(inner: std::process::ChildStdout) -> io::Result<Self> {
        Ok(Self {
            inner: imp::stdio(inner)?,
        })
    }
}

impl ChildStderr {
    /// Creates an asynchronous `ChildStderr` from a synchronous one.
    ///
    /// # Errors
    ///
    /// This method may fail if an error is encountered when setting the pipe to
    /// non-blocking mode, or when registering the pipe with the runtime's IO
    /// driver.
    pub fn from_std(inner: std::process::ChildStderr) -> io::Result<Self> {
        Ok(Self {
            inner: imp::stdio(inner)?,
        })
    }
}

impl AsyncWrite for ChildStdin {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

impl AsyncRead for ChildStdout {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncRead for ChildStderr {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl TryInto<Stdio> for ChildStdin {
    type Error = io::Error;

    fn try_into(self) -> Result<Stdio, Self::Error> {
        imp::convert_to_stdio(self.inner)
    }
}

impl TryInto<Stdio> for ChildStdout {
    type Error = io::Error;

    fn try_into(self) -> Result<Stdio, Self::Error> {
        imp::convert_to_stdio(self.inner)
    }
}

impl TryInto<Stdio> for ChildStderr {
    type Error = io::Error;

    fn try_into(self) -> Result<Stdio, Self::Error> {
        imp::convert_to_stdio(self.inner)
    }
}

#[cfg(unix)]
#[cfg_attr(docsrs, doc(cfg(unix)))]
mod sys {
    use std::{
        io,
        os::unix::io::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd},
    };

    use super::{ChildStderr, ChildStdin, ChildStdout};

    macro_rules! impl_traits {
        ($type:ty) => {
            impl $type {
                /// Convert into [`OwnedFd`].
                pub fn into_owned_fd(self) -> io::Result<OwnedFd> {
                    self.inner.into_owned_fd()
                }
            }

            impl AsRawFd for $type {
                fn as_raw_fd(&self) -> RawFd {
                    self.inner.as_raw_fd()
                }
            }

            impl AsFd for $type {
                fn as_fd(&self) -> BorrowedFd<'_> {
                    unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
                }
            }
        };
    }

    impl_traits!(ChildStdin);
    impl_traits!(ChildStdout);
    impl_traits!(ChildStderr);
}

#[cfg(any(windows, docsrs))]
#[cfg_attr(docsrs, doc(cfg(windows)))]
mod windows {
    use super::*;
    use crate::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, OwnedHandle, RawHandle};

    #[cfg(not(docsrs))]
    macro_rules! impl_traits {
        ($type:ty) => {
            impl $type {
                /// Convert into [`OwnedHandle`].
                pub fn into_owned_handle(self) -> io::Result<OwnedHandle> {
                    self.inner.into_owned_handle()
                }
            }

            impl AsRawHandle for $type {
                fn as_raw_handle(&self) -> RawHandle {
                    self.inner.as_raw_handle()
                }
            }

            impl AsHandle for $type {
                fn as_handle(&self) -> BorrowedHandle<'_> {
                    unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
                }
            }
        };
    }

    #[cfg(docsrs)]
    macro_rules! impl_traits {
        ($type:ty) => {
            impl $type {
                /// Convert into [`OwnedHandle`].
                pub fn into_owned_handle(self) -> io::Result<OwnedHandle> {
                    todo!("For doc generation only")
                }
            }

            impl AsRawHandle for $type {
                fn as_raw_handle(&self) -> RawHandle {
                    todo!("For doc generation only")
                }
            }

            impl AsHandle for $type {
                fn as_handle(&self) -> BorrowedHandle<'_> {
                    todo!("For doc generation only")
                }
            }
        };
    }

    impl_traits!(ChildStdin);
    impl_traits!(ChildStdout);
    impl_traits!(ChildStderr);
}

#[cfg(all(test, not(loom)))]
mod test {
    use super::kill::Kill;
    use super::ChildDropGuard;

    use futures::future::FutureExt;
    use std::future::Future;
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct Mock {
        num_kills: usize,
        num_polls: usize,
        poll_result: Poll<Result<(), ()>>,
    }

    impl Mock {
        fn new() -> Self {
            Self::with_result(Poll::Pending)
        }

        fn with_result(result: Poll<Result<(), ()>>) -> Self {
            Self {
                num_kills: 0,
                num_polls: 0,
                poll_result: result,
            }
        }
    }

    impl Kill for Mock {
        fn kill(&mut self) -> io::Result<()> {
            self.num_kills += 1;
            Ok(())
        }
    }

    impl Future for Mock {
        type Output = Result<(), ()>;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            let inner = Pin::get_mut(self);
            inner.num_polls += 1;
            inner.poll_result
        }
    }

    #[test]
    fn kills_on_drop_if_specified() {
        let mut mock = Mock::new();

        {
            let guard = ChildDropGuard {
                inner: &mut mock,
                kill_on_drop: true,
            };
            drop(guard);
        }

        assert_eq!(1, mock.num_kills);
        assert_eq!(0, mock.num_polls);
    }

    #[test]
    fn no_kill_on_drop_by_default() {
        let mut mock = Mock::new();

        {
            let guard = ChildDropGuard {
                inner: &mut mock,
                kill_on_drop: false,
            };
            drop(guard);
        }

        assert_eq!(0, mock.num_kills);
        assert_eq!(0, mock.num_polls);
    }

    #[test]
    fn no_kill_if_already_killed() {
        let mut mock = Mock::new();

        {
            let mut guard = ChildDropGuard {
                inner: &mut mock,
                kill_on_drop: true,
            };
            let _ = guard.kill();
            drop(guard);
        }

        assert_eq!(1, mock.num_kills);
        assert_eq!(0, mock.num_polls);
    }

    #[test]
    fn no_kill_if_reaped() {
        let mut mock_pending = Mock::with_result(Poll::Pending);
        let mut mock_reaped = Mock::with_result(Poll::Ready(Ok(())));
        let mut mock_err = Mock::with_result(Poll::Ready(Err(())));

        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        {
            let mut guard = ChildDropGuard {
                inner: &mut mock_pending,
                kill_on_drop: true,
            };
            let _ = guard.poll_unpin(&mut context);

            let mut guard = ChildDropGuard {
                inner: &mut mock_reaped,
                kill_on_drop: true,
            };
            let _ = guard.poll_unpin(&mut context);

            let mut guard = ChildDropGuard {
                inner: &mut mock_err,
                kill_on_drop: true,
            };
            let _ = guard.poll_unpin(&mut context);
        }

        assert_eq!(1, mock_pending.num_kills);
        assert_eq!(1, mock_pending.num_polls);

        assert_eq!(0, mock_reaped.num_kills);
        assert_eq!(1, mock_reaped.num_polls);

        assert_eq!(1, mock_err.num_kills);
        assert_eq!(1, mock_err.num_polls);
    }
}
