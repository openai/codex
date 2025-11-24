//! Miscellaneous helpers for running commands

use std::{
    borrow::Cow,
    collections::hash_map,
    ffi::OsString,
    fmt::Display,
    fs,
    hash::Hasher,
    io::{self, Read, Write},
    path::Path,
    process::{Child, ChildStderr, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::{Error, ErrorKind, Object};

#[derive(Clone, Debug)]
pub(crate) struct CargoOutput {
    pub(crate) metadata: bool,
    pub(crate) warnings: bool,
    pub(crate) debug: bool,
    pub(crate) output: OutputKind,
    checked_dbg_var: Arc<AtomicBool>,
}

/// Different strategies for handling compiler output (to stdout)
#[derive(Clone, Debug)]
pub(crate) enum OutputKind {
    /// Forward the output to this process' stdout ([`Stdio::inherit()`])
    Forward,
    /// Discard the output ([`Stdio::null()`])
    Discard,
    /// Capture the result (`[Stdio::piped()`])
    Capture,
}

impl CargoOutput {
    pub(crate) fn new() -> Self {
        #[allow(clippy::disallowed_methods)]
        Self {
            metadata: true,
            warnings: true,
            output: OutputKind::Forward,
            debug: match std::env::var_os("CC_ENABLE_DEBUG_OUTPUT") {
                Some(v) => v != "0" && v != "false" && !v.is_empty(),
                None => false,
            },
            checked_dbg_var: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn print_metadata(&self, s: &dyn Display) {
        if self.metadata {
            println!("{s}");
        }
    }

    pub(crate) fn print_warning(&self, arg: &dyn Display) {
        if self.warnings {
            println!("cargo:warning={arg}");
        }
    }

    pub(crate) fn print_debug(&self, arg: &dyn Display) {
        if self.metadata && !self.checked_dbg_var.load(Ordering::Relaxed) {
            self.checked_dbg_var.store(true, Ordering::Relaxed);
            println!("cargo:rerun-if-env-changed=CC_ENABLE_DEBUG_OUTPUT");
        }
        if self.debug {
            println!("{arg}");
        }
    }

    fn stdio_for_warnings(&self) -> Stdio {
        if self.warnings {
            Stdio::piped()
        } else {
            Stdio::null()
        }
    }

    fn stdio_for_output(&self) -> Stdio {
        match self.output {
            OutputKind::Capture => Stdio::piped(),
            OutputKind::Forward => Stdio::inherit(),
            OutputKind::Discard => Stdio::null(),
        }
    }
}

pub(crate) struct StderrForwarder {
    inner: Option<(ChildStderr, Vec<u8>)>,
    #[cfg(feature = "parallel")]
    is_non_blocking: bool,
    #[cfg(feature = "parallel")]
    bytes_available_failed: bool,
    /// number of bytes buffered in inner
    bytes_buffered: usize,
}

const MIN_BUFFER_CAPACITY: usize = 100;

impl StderrForwarder {
    pub(crate) fn new(child: &mut Child) -> Self {
        Self {
            inner: child
                .stderr
                .take()
                .map(|stderr| (stderr, Vec::with_capacity(MIN_BUFFER_CAPACITY))),
            bytes_buffered: 0,
            #[cfg(feature = "parallel")]
            is_non_blocking: false,
            #[cfg(feature = "parallel")]
            bytes_available_failed: false,
        }
    }

    fn forward_available(&mut self) -> bool {
        if let Some((stderr, buffer)) = self.inner.as_mut() {
            loop {
                // For non-blocking we check to see if there is data available, so we should try to
                // read at least that much. For blocking, always read at least the minimum amount.
                #[cfg(not(feature = "parallel"))]
                let to_reserve = MIN_BUFFER_CAPACITY;
                #[cfg(feature = "parallel")]
                let to_reserve = if self.is_non_blocking && !self.bytes_available_failed {
                    match crate::parallel::stderr::bytes_available(stderr) {
                        #[cfg(windows)]
                        Ok(0) => break false,
                        #[cfg(unix)]
                        Ok(0) => {
                            // On Unix, depending on the implementation, we may sometimes get 0 in a
                            // loop (either there is data available or the pipe is broken), so
                            // continue with the non-blocking read anyway.
                            MIN_BUFFER_CAPACITY
                        }
                        #[cfg(windows)]
                        Err(_) => {
                            // On Windows, if we get an error then the pipe is broken, so flush
                            // the buffer and bail.
                            if !buffer.is_empty() {
                                write_warning(&buffer[..]);
                            }
                            self.inner = None;
                            break true;
                        }
                        #[cfg(unix)]
                        Err(_) => {
                            // On Unix, depending on the implementation, we may get spurious
                            // errors so make a note not to use bytes_available again and try
                            // the non-blocking read anyway.
                            self.bytes_available_failed = true;
                            MIN_BUFFER_CAPACITY
                        }
                        #[cfg(target_family = "wasm")]
                        Err(_) => panic!("bytes_available should always succeed on wasm"),
                        Ok(bytes_available) => MIN_BUFFER_CAPACITY.max(bytes_available),
                    }
                } else {
                    MIN_BUFFER_CAPACITY
                };
                if self.bytes_buffered + to_reserve > buffer.len() {
                    buffer.resize(self.bytes_buffered + to_reserve, 0);
                }

                match stderr.read(&mut buffer[self.bytes_buffered..]) {
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        // No data currently, yield back.
                        break false;
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {
                        // Interrupted, try again.
                        continue;
                    }
                    Ok(bytes_read) if bytes_read != 0 => {
                        self.bytes_buffered += bytes_read;
                        let mut consumed = 0;
                        for line in buffer[..self.bytes_buffered].split_inclusive(|&b| b == b'\n') {
                            // Only forward complete lines, leave the rest in the buffer.
                            if let Some((b'\n', line)) = line.split_last() {
                                consumed += line.len() + 1;
                                write_warning(line);
                            }
                        }
                        if consumed > 0 && consumed < self.bytes_buffered {
                            // Remove the consumed bytes from buffer
                            buffer.copy_within(consumed.., 0);
                        }
                        self.bytes_buffered -= consumed;
                    }
                    res => {
                        // End of stream: flush remaining data and bail.
                        if self.bytes_buffered > 0 {
                            write_warning(&buffer[..self.bytes_buffered]);
                        }
                        if let Err(err) = res {
                            write_warning(
                                format!("Failed to read from child stderr: {err}").as_bytes(),
                            );
                        }
                        self.inner.take();
                        break true;
                    }
                }
            }
        } else {
            true
        }
    }

    #[cfg(feature = "parallel")]
    pub(crate) fn set_non_blocking(&mut self) -> Result<(), Error> {
        assert!(!self.is_non_blocking);

        #[cfg(unix)]
        if let Some((stderr, _)) = self.inner.as_ref() {
            crate::parallel::stderr::set_non_blocking(stderr)?;
        }

        self.is_non_blocking = true;
        Ok(())
    }

    #[cfg(feature = "parallel")]
    fn forward_all(&mut self) {
        while !self.forward_available() {}
    }

    #[cfg(not(feature = "parallel"))]
    fn forward_all(&mut self) {
        let forward_result = self.forward_available();
        assert!(forward_result, "Should have consumed all data");
    }
}

fn write_warning(line: &[u8]) {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(b"cargo:warning=").unwrap();
    stdout.write_all(line).unwrap();
    stdout.write_all(b"\n").unwrap();
}

fn wait_on_child(
    cmd: &Command,
    child: &mut Child,
    cargo_output: &CargoOutput,
) -> Result<(), Error> {
    StderrForwarder::new(child).forward_all();

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            return Err(Error::new(
                ErrorKind::ToolExecError,
                format!("failed to wait on spawned child process `{cmd:?}`: {e}"),
            ));
        }
    };

    cargo_output.print_debug(&status);

    if status.success() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::ToolExecError,
            format!("command did not execute successfully (status code {status}): {cmd:?}"),
        ))
    }
}

/// Find the destination object path for each file in the input source files,
/// and store them in the output Object.
pub(crate) fn objects_from_files(files: &[Arc<Path>], dst: &Path) -> Result<Vec<Object>, Error> {
    let mut objects = Vec::with_capacity(files.len());
    for file in files {
        let basename = file
            .file_name()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidArgument,
                    "No file_name for object file path!",
                )
            })?
            .to_string_lossy();
        let dirname = file
            .parent()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidArgument,
                    "No parent for object file path!",
                )
            })?
            .to_string_lossy();

        // Hash the dirname. This should prevent conflicts if we have multiple
        // object files with the same filename in different subfolders.
        let mut hasher = hash_map::DefaultHasher::new();

        // Make the dirname relative (if possible) to avoid full system paths influencing the sha
        // and making the output system-dependent
        //
        // NOTE: Here we allow using std::env::var (instead of Build::getenv) because
        // CARGO_* variables always trigger a rebuild when changed
        #[allow(clippy::disallowed_methods)]
        let dirname = if let Some(root) = std::env::var_os("CARGO_MANIFEST_DIR") {
            let root = root.to_string_lossy();
            Cow::Borrowed(dirname.strip_prefix(&*root).unwrap_or(&dirname))
        } else {
            dirname
        };

        hasher.write(dirname.as_bytes());
        if let Some(extension) = file.extension() {
            hasher.write(extension.to_string_lossy().as_bytes());
        }
        let obj = dst
            .join(format!("{:016x}-{}", hasher.finish(), basename))
            .with_extension("o");

        match obj.parent() {
            Some(s) => fs::create_dir_all(s)?,
            None => {
                return Err(Error::new(
                    ErrorKind::InvalidArgument,
                    "dst is an invalid path with no parent",
                ));
            }
        };

        objects.push(Object::new(file.to_path_buf(), obj));
    }

    Ok(objects)
}

pub(crate) fn run(cmd: &mut Command, cargo_output: &CargoOutput) -> Result<(), Error> {
    let mut child = spawn(cmd, cargo_output)?;
    wait_on_child(cmd, &mut child, cargo_output)
}

pub(crate) fn run_output(cmd: &mut Command, cargo_output: &CargoOutput) -> Result<Vec<u8>, Error> {
    // We specifically need the output to be captured, so override default
    let mut captured_cargo_output = cargo_output.clone();
    captured_cargo_output.output = OutputKind::Capture;
    let mut child = spawn(cmd, &captured_cargo_output)?;

    let mut stdout = vec![];
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();

    // Don't care about this output, use the normal settings
    wait_on_child(cmd, &mut child, cargo_output)?;

    Ok(stdout)
}

pub(crate) fn spawn(cmd: &mut Command, cargo_output: &CargoOutput) -> Result<Child, Error> {
    struct ResetStderr<'cmd>(&'cmd mut Command);

    impl Drop for ResetStderr<'_> {
        fn drop(&mut self) {
            // Reset stderr to default to release pipe_writer so that print thread will
            // not block forever.
            self.0.stderr(Stdio::inherit());
        }
    }

    cargo_output.print_debug(&format_args!("running: {cmd:?}"));

    let cmd = ResetStderr(cmd);
    let child = cmd
        .0
        .stderr(cargo_output.stdio_for_warnings())
        .stdout(cargo_output.stdio_for_output())
        .spawn();
    match child {
        Ok(child) => Ok(child),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            let extra = if cfg!(windows) {
                " (see https://docs.rs/cc/latest/cc/#compile-time-requirements for help)"
            } else {
                ""
            };
            Err(Error::new(
                ErrorKind::ToolNotFound,
                format!("failed to find tool {:?}: {e}{extra}", cmd.0.get_program()),
            ))
        }
        Err(e) => Err(Error::new(
            ErrorKind::ToolExecError,
            format!("command `{:?}` failed to start: {e}", cmd.0),
        )),
    }
}

pub(crate) struct CmdAddOutputFileArgs {
    pub(crate) cuda: bool,
    pub(crate) is_assembler_msvc: bool,
    pub(crate) msvc: bool,
    pub(crate) clang: bool,
    pub(crate) gnu: bool,
    pub(crate) is_asm: bool,
    pub(crate) is_arm: bool,
}

pub(crate) fn command_add_output_file(cmd: &mut Command, dst: &Path, args: CmdAddOutputFileArgs) {
    if args.is_assembler_msvc
        || !(!args.msvc || args.clang || args.gnu || args.cuda || (args.is_asm && args.is_arm))
    {
        let mut s = OsString::from("-Fo");
        s.push(dst);
        cmd.arg(s);
    } else {
        cmd.arg("-o").arg(dst);
    }
}

#[cfg(feature = "parallel")]
pub(crate) fn try_wait_on_child(
    cmd: &Command,
    child: &mut Child,
    stdout: &mut dyn io::Write,
    stderr_forwarder: &mut StderrForwarder,
) -> Result<Option<()>, Error> {
    stderr_forwarder.forward_available();

    match child.try_wait() {
        Ok(Some(status)) => {
            stderr_forwarder.forward_all();

            let _ = writeln!(stdout, "{}", status);

            if status.success() {
                Ok(Some(()))
            } else {
                Err(Error::new(
                    ErrorKind::ToolExecError,
                    format!("command did not execute successfully (status code {status}): {cmd:?}"),
                ))
            }
        }
        Ok(None) => Ok(None),
        Err(e) => {
            stderr_forwarder.forward_all();
            Err(Error::new(
                ErrorKind::ToolExecError,
                format!("failed to wait on spawned child process `{cmd:?}`: {e}"),
            ))
        }
    }
}
