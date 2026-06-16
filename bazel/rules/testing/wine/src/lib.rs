#[cfg(not(target_os = "linux"))]
compile_error!("wine_test_support can only run on Linux");

use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::future::Future;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use tempfile::TempDir;
use tokio::process::Child;
use tokio::process::ChildStdout;
use tokio::process::Command as TokioCommand;

const WINE_TEST_PREFIX_ENV_VAR: &str = "CODEX_WINE_TEST_PREFIX";

/// Builds a command that runs a Windows executable in an isolated Wine prefix.
pub struct WineTestCommand {
    executable: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

/// Owns a Wine process and its isolated wineserver.
///
/// Call [`Self::scope`] or [`Self::shutdown`] on every successful path. A
/// normal unguarded drop panics, while a drop during unwinding performs
/// blocking cleanup without introducing a second panic.
pub struct WineTestProcess {
    processes: Option<WineProcesses>,
}

/// Ambient context for launching another Windows process in an active test prefix.
///
/// Applying this context to a host command does not transfer ownership of the
/// prefix. The command must finish before the [`WineTestProcess`] that created
/// this context is shut down.
#[derive(Clone, Debug)]
pub struct WineTestCommandContext {
    prefix: PathBuf,
}

struct WineProcesses {
    child: Child,
    cleanup_complete: bool,
    prefix: TempDir,
    runtime: WineRuntimePaths,
}

struct WineRuntimePaths {
    dll_path: PathBuf,
    powershell_runtime: PathBuf,
    wine: PathBuf,
    wineserver: PathBuf,
}

impl WineTestCommand {
    /// Creates a Wine command for `executable`.
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    /// Adds an argument passed to the Windows executable.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds or overrides an environment variable for the Wine process.
    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Starts the Windows executable with a fresh `WINEPREFIX`.
    pub fn spawn(self) -> Result<WineTestProcess> {
        let runtime = WineRuntimePaths::from_runfiles()?;
        let prefix = TempDir::new().context("create isolated Wine prefix")?;
        install_powershell_runtime(prefix.path(), &runtime.powershell_runtime)?;
        let mut command = StdCommand::new(&runtime.wine);
        configure_wine_environment(&mut command, &runtime, prefix.path());
        command
            .arg(self.executable)
            .args(self.args)
            .envs(self.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut command = TokioCommand::from(command);
        command.kill_on_drop(true);
        let child = command
            .spawn()
            .context("start Windows process under Wine")?;

        Ok(WineTestProcess {
            processes: Some(WineProcesses {
                child,
                cleanup_complete: false,
                prefix,
                runtime,
            }),
        })
    }
}

impl WineTestProcess {
    /// Takes the piped standard output of the Wine process.
    ///
    /// This may only be called once for a process created by
    /// [`WineTestCommand::spawn`].
    pub fn take_stdout(&mut self) -> ChildStdout {
        let Some(processes) = self.processes.as_mut() else {
            panic!("Wine process guard is missing");
        };
        let Some(stdout) = processes.child.stdout.take() else {
            panic!("Wine process stdout has already been taken");
        };
        stdout
    }

    /// Returns a context that a host subprocess can use to join this Wine prefix.
    pub fn command_context(&self) -> WineTestCommandContext {
        let Some(processes) = self.processes.as_ref() else {
            panic!("Wine process guard is missing");
        };
        WineTestCommandContext {
            prefix: processes.prefix.path().to_path_buf(),
        }
    }

    /// Runs `future`, then asynchronously tears down Wine before returning.
    ///
    /// If both the scoped operation and teardown fail, the operation error is
    /// returned with the teardown error attached as context. A panic in the
    /// scoped operation triggers the blocking unwind-time fallback instead.
    pub async fn scope<T>(self, future: impl Future<Output = Result<T>>) -> Result<T> {
        let scope_result = future.await;
        let shutdown_result = self.shutdown().await;
        match (scope_result, shutdown_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(shutdown_error)) => {
                Err(error.context(format!("Wine teardown also failed: {shutdown_error:#}")))
            }
        }
    }

    /// Kills the Windows process, waits for it, and stops its wineserver.
    pub async fn shutdown(mut self) -> Result<()> {
        let Some(processes) = self.processes.as_mut() else {
            anyhow::bail!("Wine process guard is missing");
        };
        let result = processes.shutdown().await;
        self.processes.take();
        result
    }
}

impl WineTestCommandContext {
    /// Exports this context to a host command and its descendants.
    pub fn apply_to_command(&self, command: &mut TokioCommand) {
        command.env(WINE_TEST_PREFIX_ENV_VAR, &self.prefix);
    }
}

/// Builds a Wine command that joins the prefix exported by [`WineTestCommandContext`].
pub fn ambient_wine_command(executable: impl AsRef<OsStr>) -> Result<StdCommand> {
    let prefix = PathBuf::from(
        std::env::var_os(WINE_TEST_PREFIX_ENV_VAR)
            .with_context(|| format!("{WINE_TEST_PREFIX_ENV_VAR} must be set"))?,
    );
    let runtime = WineRuntimePaths::from_runfiles()?;
    let mut command = StdCommand::new(&runtime.wine);
    configure_wine_environment(&mut command, &runtime, &prefix);
    command.arg(executable);
    Ok(command)
}

impl Drop for WineTestProcess {
    fn drop(&mut self) {
        // Panicking here starts unwinding, after which WineProcesses performs
        // the blocking fallback while its field is dropped.
        if self.processes.is_some() && !std::thread::panicking() {
            panic!("WineTestProcess dropped without async teardown");
        }
    }
}

impl WineRuntimePaths {
    fn from_runfiles() -> Result<Self> {
        let wine = codex_utils_cargo_bin::cargo_bin("wine")?;
        let runtime_marker = codex_utils_cargo_bin::cargo_bin("wine-runtime-marker")?;
        let dll_path = runtime_marker
            .parent()
            .context("locate Wine runtime directory")?
            .to_path_buf();
        let wineserver = codex_utils_cargo_bin::cargo_bin("wineserver")?;
        let powershell_runtime = codex_utils_cargo_bin::cargo_bin("pwsh-runtime-marker")?
            .parent()
            .context("locate PowerShell runtime directory")?
            .to_path_buf();
        Ok(Self {
            dll_path,
            powershell_runtime,
            wine,
            wineserver,
        })
    }
}

impl WineProcesses {
    async fn shutdown(&mut self) -> Result<()> {
        let (kill_result, check_exit_status) = match self.child.try_wait() {
            Ok(Some(_)) => (Ok(()), true),
            Ok(None) => (
                self.child
                    .start_kill()
                    .context("kill Windows process running under Wine"),
                false,
            ),
            Err(error) => (Err(error).context("check Windows process status"), false),
        };
        let wait_result = self
            .child
            .wait()
            .await
            .context("wait for Windows process running under Wine")
            .and_then(|status| {
                anyhow::ensure!(
                    !check_exit_status || status.success(),
                    "Windows process exited with {status}"
                );
                Ok(())
            });
        let wineserver_result = async {
            let mut command = TokioCommand::from(self.stop_wineserver_command());
            let status = command.status().await.context("stop isolated wineserver")?;
            anyhow::ensure!(status.success(), "wineserver exited with {status}");
            Ok(())
        }
        .await;

        // Every cleanup action has been attempted, so an individual error
        // should not cause the blocking fallback to repeat them.
        self.cleanup_complete = true;
        kill_result?;
        wait_result?;
        wineserver_result
    }

    fn stop_wineserver_command(&self) -> StdCommand {
        let mut command = StdCommand::new(&self.runtime.wineserver);
        configure_wine_environment(&mut command, &self.runtime, self.prefix.path());
        command
            .args(["-k", "-w"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    fn shutdown_blocking(&mut self) {
        log_panic_cleanup(format_args!(
            "Wine panic cleanup starting for prefix {}",
            self.prefix.path().display()
        ));
        if let Err(error) = self.child.start_kill() {
            log_panic_cleanup(format_args!(
                "Wine panic cleanup could not kill its child: {error}"
            ));
        }

        log_panic_cleanup(format_args!("Wine panic cleanup waiting for its child"));
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    log_panic_cleanup(format_args!(
                        "Wine panic cleanup child exited with {status}"
                    ));
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    log_panic_cleanup(format_args!(
                        "Wine panic cleanup could not wait for its child: {error}"
                    ));
                    break;
                }
            }
        }

        log_panic_cleanup(format_args!("Wine panic cleanup stopping its wineserver"));
        match self.stop_wineserver_command().status() {
            Ok(status) => log_panic_cleanup(format_args!(
                "Wine panic cleanup wineserver exited with {status}"
            )),
            Err(error) => log_panic_cleanup(format_args!(
                "Wine panic cleanup could not stop its wineserver: {error}"
            )),
        }
        self.cleanup_complete = true;
        log_panic_cleanup(format_args!("Wine panic cleanup complete"));
    }
}

impl Drop for WineProcesses {
    fn drop(&mut self) {
        // Never introduce a second panic while unwinding. Blocking here is
        // intentional because test failures must not leak Wine children.
        if !self.cleanup_complete && std::thread::panicking() {
            self.shutdown_blocking();
        }
    }
}

fn log_panic_cleanup(args: std::fmt::Arguments<'_>) {
    let _ = writeln!(std::io::stderr().lock(), "{args}");
}

fn configure_wine_environment(command: &mut StdCommand, runtime: &WineRuntimePaths, prefix: &Path) {
    command
        .env_remove("DISPLAY")
        .env("HOME", prefix)
        .env("XDG_RUNTIME_DIR", prefix)
        .env("WINEARCH", "win64")
        .env("WINEPREFIX", prefix)
        .env("WINEDLLPATH", &runtime.dll_path)
        .env("WINESERVER", &runtime.wineserver)
        .env("WINEDEBUG", "-all")
        .env("WINEDLLOVERRIDES", "mscoree,mshtml,winegstreamer=")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("LC_CTYPE", "C.UTF-8")
        .env("TEMP", r"C:\windows\temp")
        .env("TMP", r"C:\windows\temp");
}

/// Installs the complete pinned PowerShell distribution where Windows tooling
/// expects to discover PowerShell 7.
///
/// `pwsh.exe` is not a standalone executable: it loads its adjacent .NET host,
/// managed assemblies, native libraries, modules, and configuration files at
/// startup. The Bazel archive is exposed through runfiles rather than a normal
/// Windows installation, while shell detection deliberately probes the
/// conventional `C:\Program Files\PowerShell\7` fallback. We therefore have to
/// reproduce the archive's directory tree inside each isolated Wine prefix;
/// copying only the executable would fail before a command could run.
fn install_powershell_runtime(prefix: &Path, runtime: &Path) -> Result<()> {
    let powershell_parent = prefix
        .join("drive_c")
        .join("Program Files")
        .join("PowerShell");
    fs::create_dir_all(&powershell_parent).context("create PowerShell installation parent")?;
    let destination = powershell_parent.join("7");
    materialize_runtime_directory(runtime, &destination)
}

/// Recursively reproduces a runfiles directory in a writable Wine prefix.
///
/// Bazel runfiles may be immutable, represented by a symlink forest, and may
/// contain the PowerShell distribution on a different filesystem from the
/// temporary prefix. Hard links avoid repeatedly copying the roughly
/// hundred-megabyte runtime when both locations share a filesystem; the copy
/// fallback preserves correctness for sandbox or remote-execution layouts
/// where cross-device hard links are unavailable.
fn materialize_runtime_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).with_context(|| {
        format!(
            "create PowerShell runtime directory {}",
            destination.display()
        )
    })?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("read PowerShell runtime directory {}", source.display()))?
    {
        let entry = entry.context("read PowerShell runtime entry")?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        // Local Bazel runfiles trees expose external-repository files as
        // symlinks. Resolve those trusted runfiles entries before inspecting
        // or linking them so the writable prefix contains ordinary files.
        let resolved_source_path = fs::canonicalize(&source_path).with_context(|| {
            format!("resolve PowerShell runtime entry {}", source_path.display())
        })?;
        let file_type = fs::metadata(&resolved_source_path)
            .with_context(|| {
                format!(
                    "inspect PowerShell runtime entry {}",
                    resolved_source_path.display()
                )
            })?
            .file_type();
        if file_type.is_dir() {
            // PowerShell resolves assemblies and modules by their relative
            // locations, so flattening the archive is not an option.
            materialize_runtime_directory(&resolved_source_path, &destination_path)?;
        } else if file_type.is_file() {
            // A hard link gives each prefix the expected installation layout
            // without duplicating the large runtime in the common local case.
            if fs::hard_link(&resolved_source_path, &destination_path).is_err() {
                // Cross-device links are common under Bazel sandboxing and
                // remote execution, where an ordinary copy is still valid.
                fs::copy(&resolved_source_path, &destination_path).with_context(|| {
                    format!(
                        "copy PowerShell runtime file {} to {}",
                        resolved_source_path.display(),
                        destination_path.display()
                    )
                })?;
            }
        } else {
            anyhow::bail!(
                "unsupported PowerShell runtime entry type at {}",
                source_path.display()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
