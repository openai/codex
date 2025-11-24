//! [`std::process::Command`] customized for testing.

use std::ffi;
use std::io;
use std::io::{Read, Write};
use std::ops::Deref;
use std::path;
use std::process;

use crate::assert::Assert;
use crate::assert::OutputAssertExt;
use crate::output::DebugBuffer;
use crate::output::DebugBytes;
use crate::output::OutputError;
use crate::output::OutputOkExt;
use crate::output::OutputResult;

/// [`std::process::Command`] customized for testing.
#[derive(Debug)]
pub struct Command {
    cmd: process::Command,
    stdin: Option<bstr::BString>,
    timeout: Option<std::time::Duration>,
}

impl Command {
    /// Constructs a new `Command` from a `std` `Command`.
    pub fn from_std(cmd: process::Command) -> Self {
        Self {
            cmd,
            stdin: None,
            timeout: None,
        }
    }

    /// Create a `Command` to run a specific binary of the current crate.
    ///
    /// See the [`cargo` module documentation][crate::cargo] for caveats and workarounds.
    ///
    /// **NOTE:** Prefer [`cargo_bin!`][crate::cargo::cargo_bin!] as this makes assumptions about cargo
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::Command;
    ///
    /// let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME"))
    ///     .unwrap();
    /// let output = cmd.unwrap();
    /// println!("{:?}", output);
    /// ```
    ///
    /// ```rust,no_run
    /// use assert_cmd::Command;
    ///
    /// let mut cmd = Command::cargo_bin("bin_fixture")
    ///     .unwrap();
    /// let output = cmd.unwrap();
    /// println!("{:?}", output);
    /// ```
    ///
    pub fn cargo_bin<S: AsRef<str>>(name: S) -> Result<Self, crate::cargo::CargoError> {
        let cmd = crate::cargo::cargo_bin_cmd(name)?;
        Ok(Self::from_std(cmd))
    }

    /// Write `buffer` to `stdin` when the `Command` is run.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use assert_cmd::Command;
    ///
    /// let mut cmd = Command::new("cat")
    ///     .arg("-et")
    ///     .write_stdin("42")
    ///     .assert()
    ///     .stdout("42");
    /// ```
    pub fn write_stdin<S>(&mut self, buffer: S) -> &mut Self
    where
        S: Into<Vec<u8>>,
    {
        self.stdin = Some(bstr::BString::from(buffer.into()));
        self
    }

    /// Error out if a timeout is reached
    ///
    /// ```rust,no_run
    /// use assert_cmd::Command;
    ///
    /// let assert = Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .timeout(std::time::Duration::from_secs(1))
    ///     .env("sleep", "100")
    ///     .assert();
    /// assert.failure();
    /// ```
    pub fn timeout(&mut self, timeout: std::time::Duration) -> &mut Self {
        self.timeout = Some(timeout);
        self
    }

    /// Write `path`s content to `stdin` when the `Command` is run.
    ///
    /// Paths are relative to the [`env::current_dir`][env_current_dir] and not
    /// [`Command::current_dir`][Command_current_dir].
    ///
    /// [env_current_dir]: std::env::current_dir()
    /// [Command_current_dir]: std::process::Command::current_dir()
    pub fn pipe_stdin<P>(&mut self, file: P) -> io::Result<&mut Self>
    where
        P: AsRef<path::Path>,
    {
        let buffer = std::fs::read(file)?;
        Ok(self.write_stdin(buffer))
    }

    /// Run a `Command`, returning an [`OutputResult`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use assert_cmd::Command;
    ///
    /// let result = Command::new("echo")
    ///     .args(&["42"])
    ///     .ok();
    /// assert!(result.is_ok());
    /// ```
    ///
    pub fn ok(&mut self) -> OutputResult {
        OutputOkExt::ok(self)
    }

    /// Run a `Command`, unwrapping the [`OutputResult`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use assert_cmd::Command;
    ///
    /// let output = Command::new("echo")
    ///     .args(&["42"])
    ///     .unwrap();
    /// ```
    ///
    pub fn unwrap(&mut self) -> process::Output {
        OutputOkExt::unwrap(self)
    }

    /// Run a `Command`, unwrapping the error in the [`OutputResult`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::Command;
    ///
    /// let err = Command::new("a-command")
    ///     .args(&["--will-fail"])
    ///     .unwrap_err();
    /// ```
    ///
    /// [Output]: std::process::Output
    pub fn unwrap_err(&mut self) -> OutputError {
        OutputOkExt::unwrap_err(self)
    }

    /// Run a `Command` and make assertions on the [`Output`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use assert_cmd::Command;
    ///
    /// let mut cmd = Command::cargo_bin("bin_fixture")
    ///     .unwrap()
    ///     .assert()
    ///     .success();
    /// ```
    ///
    /// [`Output`]: std::process::Output
    pub fn assert(&mut self) -> Assert {
        OutputAssertExt::assert(self)
    }
}

/// Mirror [`std::process::Command`]'s API
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
    /// (see issue #37519).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use assert_cmd::Command;
    ///
    /// Command::new("sh").unwrap();
    /// ```
    pub fn new<S: AsRef<ffi::OsStr>>(program: S) -> Self {
        let cmd = process::Command::new(program);
        Self::from_std(cmd)
    }

    /// Adds an argument to pass to the program.
    ///
    /// Only one argument can be passed per use. So instead of:
    ///
    /// ```no_run
    /// # assert_cmd::Command::new("sh")
    /// .arg("-C /path/to/repo")
    /// # ;
    /// ```
    ///
    /// usage would be:
    ///
    /// ```no_run
    /// # assert_cmd::Command::new("sh")
    /// .arg("-C")
    /// .arg("/path/to/repo")
    /// # ;
    /// ```
    ///
    /// To pass multiple arguments see [`args`].
    ///
    /// [`args`]: Command::args()
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use assert_cmd::Command;
    ///
    /// Command::new("ls")
    ///         .arg("-l")
    ///         .arg("-a")
    ///         .unwrap();
    /// ```
    pub fn arg<S: AsRef<ffi::OsStr>>(&mut self, arg: S) -> &mut Self {
        self.cmd.arg(arg);
        self
    }

    /// Adds multiple arguments to pass to the program.
    ///
    /// To pass a single argument see [`arg`].
    ///
    /// [`arg`]: Command::arg()
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use assert_cmd::Command;
    ///
    /// Command::new("ls")
    ///         .args(&["-l", "-a"])
    ///         .unwrap();
    /// ```
    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<ffi::OsStr>,
    {
        self.cmd.args(args);
        self
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
    /// use assert_cmd::Command;
    ///
    /// Command::new("ls")
    ///         .env("PATH", "/bin")
    ///         .unwrap_err();
    /// ```
    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<ffi::OsStr>,
        V: AsRef<ffi::OsStr>,
    {
        self.cmd.env(key, val);
        self
    }

    /// Adds or updates multiple environment variable mappings.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use assert_cmd::Command;
    /// use std::process::Stdio;
    /// use std::env;
    /// use std::collections::HashMap;
    ///
    /// let filtered_env : HashMap<String, String> =
    ///     env::vars().filter(|&(ref k, _)|
    ///         k == "TERM" || k == "TZ" || k == "LANG" || k == "PATH"
    ///     ).collect();
    ///
    /// Command::new("printenv")
    ///         .env_clear()
    ///         .envs(&filtered_env)
    ///         .unwrap();
    /// ```
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<ffi::OsStr>,
        V: AsRef<ffi::OsStr>,
    {
        self.cmd.envs(vars);
        self
    }

    /// Removes an environment variable mapping.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use assert_cmd::Command;
    ///
    /// Command::new("ls")
    ///         .env_remove("PATH")
    ///         .unwrap_err();
    /// ```
    pub fn env_remove<K: AsRef<ffi::OsStr>>(&mut self, key: K) -> &mut Self {
        self.cmd.env_remove(key);
        self
    }

    /// Clears the entire environment map for the child process.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use assert_cmd::Command;
    ///
    /// Command::new("ls")
    ///         .env_clear()
    ///         .unwrap_err();
    /// ```
    pub fn env_clear(&mut self) -> &mut Self {
        self.cmd.env_clear();
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
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// use assert_cmd::Command;
    ///
    /// Command::new("ls")
    ///         .current_dir("/bin")
    ///         .unwrap();
    /// ```
    ///
    /// [`canonicalize`]: std::fs::canonicalize()
    pub fn current_dir<P: AsRef<path::Path>>(&mut self, dir: P) -> &mut Self {
        self.cmd.current_dir(dir);
        self
    }

    /// Executes the `Command` as a child process, waiting for it to finish and collecting all of its
    /// output.
    ///
    /// By default, stdout and stderr are captured (and used to provide the resulting output).
    /// Stdin is not inherited from the parent and any attempt by the child process to read from
    /// the stdin stream will result in the stream immediately closing.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use assert_cmd::Command;
    /// use std::io::{self, Write};
    /// let output = Command::new("/bin/cat")
    ///                      .arg("file.txt")
    ///                      .output()
    ///                      .expect("failed to execute process");
    ///
    /// println!("status: {}", output.status);
    /// io::stdout().write_all(&output.stdout).unwrap();
    /// io::stderr().write_all(&output.stderr).unwrap();
    ///
    /// assert!(output.status.success());
    /// ```
    pub fn output(&mut self) -> io::Result<process::Output> {
        let spawn = self.spawn()?;
        Self::wait_with_input_output(spawn, self.stdin.as_deref().cloned(), self.timeout)
    }

    /// If `input`, write it to `child`'s stdin while also reading `child`'s
    /// stdout and stderr, then wait on `child` and return its status and output.
    ///
    /// This was lifted from `std::process::Child::wait_with_output` and modified
    /// to also write to stdin.
    fn wait_with_input_output(
        mut child: process::Child,
        input: Option<Vec<u8>>,
        timeout: Option<std::time::Duration>,
    ) -> io::Result<process::Output> {
        #![allow(clippy::unwrap_used)] // changes behavior in some tests

        fn read<R>(mut input: R) -> std::thread::JoinHandle<io::Result<Vec<u8>>>
        where
            R: Read + Send + 'static,
        {
            std::thread::spawn(move || {
                let mut ret = Vec::new();
                input.read_to_end(&mut ret).map(|_| ret)
            })
        }

        let stdin = input.and_then(|i| {
            child
                .stdin
                .take()
                .map(|mut stdin| std::thread::spawn(move || stdin.write_all(&i)))
        });
        let stdout = child.stdout.take().map(read);
        let stderr = child.stderr.take().map(read);

        // Finish writing stdin before waiting, because waiting drops stdin.
        stdin.and_then(|t| t.join().unwrap().ok());
        let status = if let Some(timeout) = timeout {
            wait_timeout::ChildExt::wait_timeout(&mut child, timeout)
                .transpose()
                .unwrap_or_else(|| {
                    let _ = child.kill();
                    child.wait()
                })
        } else {
            child.wait()
        }?;

        let stdout = stdout
            .and_then(|t| t.join().unwrap().ok())
            .unwrap_or_default();
        let stderr = stderr
            .and_then(|t| t.join().unwrap().ok())
            .unwrap_or_default();

        Ok(process::Output {
            status,
            stdout,
            stderr,
        })
    }

    fn spawn(&mut self) -> io::Result<process::Child> {
        // stdout/stderr should only be piped for `output` according to `process::Command::new`.
        self.cmd.stdin(process::Stdio::piped());
        self.cmd.stdout(process::Stdio::piped());
        self.cmd.stderr(process::Stdio::piped());

        self.cmd.spawn()
    }

    /// Returns the path to the program that was given to [`Command::new`].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust
    /// use assert_cmd::Command;
    ///
    /// let cmd = Command::new("echo");
    /// assert_eq!(cmd.get_program(), "echo");
    /// ```
    pub fn get_program(&self) -> &ffi::OsStr {
        self.cmd.get_program()
    }

    /// Returns an iterator of the arguments that will be passed to the program.
    ///
    /// This does not include the path to the program as the first argument;
    /// it only includes the arguments specified with [`Command::arg`] and
    /// [`Command::args`].
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust
    /// use std::ffi::OsStr;
    /// use assert_cmd::Command;
    ///
    /// let mut cmd = Command::new("echo");
    /// cmd.arg("first").arg("second");
    /// let args: Vec<&OsStr> = cmd.get_args().collect();
    /// assert_eq!(args, &["first", "second"]);
    /// ```
    pub fn get_args(&self) -> process::CommandArgs<'_> {
        self.cmd.get_args()
    }

    /// Returns an iterator of the environment variables explicitly set for the child process.
    ///
    /// Environment variables explicitly set using [`Command::env`], [`Command::envs`], and
    /// [`Command::env_remove`] can be retrieved with this method.
    ///
    /// Note that this output does not include environment variables inherited from the parent
    /// process.
    ///
    /// Each element is a tuple key/value pair `(&OsStr, Option<&OsStr>)`. A [`None`] value
    /// indicates its key was explicitly removed via [`Command::env_remove`]. The associated key for
    /// the [`None`] value will no longer inherit from its parent process.
    ///
    /// An empty iterator can indicate that no explicit mappings were added or that
    /// [`Command::env_clear`] was called. After calling [`Command::env_clear`], the child process
    /// will not inherit any environment variables from its parent process.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust
    /// use std::ffi::OsStr;
    /// use assert_cmd::Command;
    ///
    /// let mut cmd = Command::new("ls");
    /// cmd.env("TERM", "dumb").env_remove("TZ");
    /// let envs: Vec<(&OsStr, Option<&OsStr>)> = cmd.get_envs().collect();
    /// assert_eq!(envs, &[
    ///     (OsStr::new("TERM"), Some(OsStr::new("dumb"))),
    ///     (OsStr::new("TZ"), None)
    /// ]);
    /// ```
    pub fn get_envs(&self) -> process::CommandEnvs<'_> {
        self.cmd.get_envs()
    }

    /// Returns the working directory for the child process.
    ///
    /// This returns [`None`] if the working directory will not be changed.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust
    /// use std::path::Path;
    /// use assert_cmd::Command;
    ///
    /// let mut cmd = Command::new("ls");
    /// assert_eq!(cmd.get_current_dir(), None);
    /// cmd.current_dir("/bin");
    /// assert_eq!(cmd.get_current_dir(), Some(Path::new("/bin")));
    /// ```
    pub fn get_current_dir(&self) -> Option<&path::Path> {
        self.cmd.get_current_dir()
    }
}

impl From<process::Command> for Command {
    fn from(cmd: process::Command) -> Self {
        Command::from_std(cmd)
    }
}

impl OutputOkExt for &mut Command {
    fn ok(self) -> OutputResult {
        let output = self.output().map_err(OutputError::with_cause)?;
        if output.status.success() {
            Ok(output)
        } else {
            let error = OutputError::new(output).set_cmd(format!("{:?}", self.cmd));
            let error = if let Some(stdin) = self.stdin.as_ref() {
                error.set_stdin(stdin.deref().clone())
            } else {
                error
            };
            Err(error)
        }
    }

    fn unwrap_err(self) -> OutputError {
        match self.ok() {
            Ok(output) => {
                if let Some(stdin) = self.stdin.as_ref() {
                    panic!(
                        "Completed successfully:\ncommand=`{:?}`\nstdin=```{}```\nstdout=```{}```",
                        self.cmd,
                        DebugBytes::new(stdin),
                        DebugBytes::new(&output.stdout)
                    )
                } else {
                    panic!(
                        "Completed successfully:\ncommand=`{:?}`\nstdout=```{}```",
                        self.cmd,
                        DebugBytes::new(&output.stdout)
                    )
                }
            }
            Err(err) => err,
        }
    }
}

impl OutputAssertExt for &mut Command {
    fn assert(self) -> Assert {
        let output = match self.output() {
            Ok(output) => output,
            Err(err) => {
                panic!("Failed to spawn {self:?}: {err}");
            }
        };
        let assert = Assert::new(output).append_context("command", format!("{:?}", self.cmd));
        if let Some(stdin) = self.stdin.as_ref() {
            assert.append_context("stdin", DebugBuffer::new(stdin.deref().clone()))
        } else {
            assert
        }
    }
}
