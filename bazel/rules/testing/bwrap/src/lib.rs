#[cfg(not(target_os = "linux"))]
compile_error!("bwrap_test_support can only run on Linux");

use std::ffi::OsString;
use std::fs;
use std::future::Future;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use tempfile::Builder;
use tempfile::TempDir;
use tokio::process::Child;
use tokio::process::ChildStdout;
use tokio::process::Command as TokioCommand;

use self::sandbox_init::BWRAP_CLEANUP_TIMEOUT;
use self::sandbox_init::SandboxInit;

const BWRAP_SETUP_TIMEOUT: Duration = Duration::from_secs(10);

mod sandbox_init;

/// Builds a command that runs inside the Linux integration-test namespace.
pub struct BwrapTestCommand {
    executable: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

/// Owns a process tree running inside a dedicated PID namespace.
///
/// Call [`Self::scope`] or [`Self::shutdown`] on every successful path. A
/// normal unguarded drop panics, while a drop during unwinding performs
/// blocking cleanup without introducing a second panic. Teardown signals the
/// namespace init and waits for it, so detached descendants cannot escape.
pub struct BwrapTestProcess {
    processes: Option<BwrapProcesses>,
}

struct BwrapProcesses {
    child: Child,
    sandbox_init: SandboxInit,
    cleanup_complete: bool,
    environment: TempDir,
}

struct BwrapRuntimePaths {
    bwrap: PathBuf,
}

impl BwrapTestCommand {
    /// Creates an isolated bwrap command for `executable`.
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    /// Adds an argument passed to the isolated executable.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds or overrides an environment variable for the isolated process.
    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Starts the executable with an isolated filesystem, IPC, and UTS view.
    pub fn spawn(self) -> Result<BwrapTestProcess> {
        let runtime = BwrapRuntimePaths::from_runfiles()?;
        let workspace = std::env::current_dir().context("resolve Bazel test workspace")?;
        anyhow::ensure!(
            workspace != Path::new("/"),
            "refusing to make the filesystem root writable"
        );
        let environment = Builder::new()
            .prefix("codex-bwrap-test-")
            .tempdir_in("/tmp")
            .context("create isolated bwrap test environment")?;
        let private_tmp = environment.path().join("tmp");
        fs::create_dir(&private_tmp)
            .with_context(|| format!("create private tmp directory {}", private_tmp.display()))?;
        fs::set_permissions(&private_tmp, fs::Permissions::from_mode(0o1777)).with_context(
            || {
                format!(
                    "set permissions on private tmp directory {}",
                    private_tmp.display()
                )
            },
        )?;
        let home = private_tmp.join("home");
        let codex_home = private_tmp.join("codex-home");
        let xdg_runtime_dir = private_tmp.join("xdg-runtime");
        for path in [&home, &codex_home, &xdg_runtime_dir] {
            fs::create_dir(path)
                .with_context(|| format!("create bwrap test directory {}", path.display()))?;
        }
        fs::set_permissions(&xdg_runtime_dir, fs::Permissions::from_mode(0o700)).with_context(
            || {
                format!(
                    "set permissions on bwrap runtime directory {}",
                    xdg_runtime_dir.display()
                )
            },
        )?;

        // Firecracker supplies the host PID boundary. The outer wrapper adds a
        // child PID namespace so terminating its init kills every descendant,
        // but deliberately rebinds the VM's existing procfs read-write. Nested
        // unprivileged bwrap writes UID/GID maps through the caller's /proc
        // before mounting a fresh procfs for its own PID namespace. Giving the
        // outer wrapper a fresh procfs on this executor leaves locked child
        // mounts that make the nested proc mount fail Linux's
        // mount_too_revealing check; making the inherited procfs read-only
        // instead makes UID/GID map setup fail.
        //
        // A private directory backs /tmp so remote fixtures cannot alias files
        // owned by the test runner. The Bazel workspace is the other deliberate
        // writable carveout: production bwrap setup needs to materialize
        // missing mount targets below a writable command cwd.
        //
        // Map the caller to a non-root ID and leave the final process with no
        // capabilities. Bubblewrap rejects a non-root caller that already has
        // capabilities, while an unprivileged nested bwrap acquires the setup
        // capabilities it needs inside the user namespace it creates.
        let (mut info_reader, info_writer) =
            UnixStream::pair().context("create bwrap info pipe")?;
        let (block_reader, block_writer) =
            UnixStream::pair().context("create bwrap startup gate")?;
        info_reader
            .set_read_timeout(Some(BWRAP_SETUP_TIMEOUT))
            .context("set bwrap info timeout")?;
        let info_writer_fd = info_writer.as_raw_fd();
        let block_reader_fd = block_reader.as_raw_fd();

        // Hold the sandbox at --block-fd before it forks the payload. This
        // keeps the reported PID-namespace init alive until its pidfd is open;
        // dropping block_writer below releases the sandbox with EOF.
        let mut command = StdCommand::new(&runtime.bwrap);
        command
            .arg("--info-fd")
            .arg(info_writer_fd.to_string())
            .arg("--block-fd")
            .arg(block_reader_fd.to_string())
            .args([
                "--new-session",
                "--die-with-parent",
                "--ro-bind",
                "/",
                "/",
                "--bind",
                "/proc",
                "/proc",
                "--bind",
            ])
            .arg(&private_tmp)
            .arg("/tmp")
            .args(["--dev", "/dev", "--bind"])
            .arg(&workspace)
            .arg(&workspace)
            .args([
                "--unshare-user",
                "--unshare-pid",
                "--unshare-ipc",
                "--unshare-uts",
                "--uid",
                "1000",
                "--gid",
                "1000",
                "--cap-drop",
                "ALL",
                "--",
            ])
            .arg(self.executable)
            .args(self.args)
            .env("HOME", "/tmp/home")
            .env("CODEX_HOME", "/tmp/codex-home")
            .env("XDG_RUNTIME_DIR", "/tmp/xdg-runtime")
            .env("TMPDIR", "/tmp")
            .envs(self.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        // SAFETY: only async-signal-safe fcntl calls run between fork and exec.
        // The parent retains both descriptors until spawn completes.
        unsafe {
            command.pre_exec(move || {
                for fd in [info_writer_fd, block_reader_fd] {
                    let flags = libc::fcntl(fd, libc::F_GETFD);
                    if flags == -1
                        || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) == -1
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }

        let mut command = TokioCommand::from(command);
        command.kill_on_drop(true);
        let mut child = command
            .spawn()
            .context("start process inside bwrap test environment")?;
        // This binding is intentionally declared after child. On an early
        // return it drops first, releasing the startup gate before
        // kill_on_drop terminates the launcher. Failures before a pidfd is
        // acquired ultimately remain contained by the disposable action VM.
        let startup_gate = block_writer;
        let launcher_pid = child.id().context("bwrap launcher omitted its PID")?;
        drop(info_writer);
        drop(block_reader);
        let mut bwrap_info = String::new();
        let startup_result = info_reader
            .read_to_string(&mut bwrap_info)
            .context("read bwrap child information")
            .and_then(|_| SandboxInit::from_bwrap_info(&bwrap_info, launcher_pid));
        drop(startup_gate);
        let sandbox_init = match startup_result {
            Ok(sandbox_init) => sandbox_init,
            Err(error) => {
                if let Err(kill_error) = child.start_kill() {
                    return Err(error.context(format!(
                        "bwrap startup cleanup could not kill its launcher: {kill_error}"
                    )));
                }
                return Err(error);
            }
        };

        Ok(BwrapTestProcess {
            processes: Some(BwrapProcesses {
                child,
                sandbox_init,
                cleanup_complete: false,
                environment,
            }),
        })
    }
}

impl BwrapTestProcess {
    /// Returns the host path containing this process's isolated environment.
    pub fn environment_path(&self) -> &Path {
        let Some(processes) = self.processes.as_ref() else {
            panic!("bwrap process guard is missing");
        };
        processes.environment.path()
    }

    /// Takes the piped standard output of the isolated process.
    ///
    /// This may only be called once for a process created by
    /// [`BwrapTestCommand::spawn`].
    pub fn take_stdout(&mut self) -> ChildStdout {
        let Some(processes) = self.processes.as_mut() else {
            panic!("bwrap process guard is missing");
        };
        let Some(stdout) = processes.child.stdout.take() else {
            panic!("bwrap process stdout has already been taken");
        };
        stdout
    }

    /// Runs `future`, then asynchronously tears down bwrap before returning.
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
                Err(error.context(format!("bwrap teardown also failed: {shutdown_error:#}")))
            }
        }
    }

    /// Kills the isolated PID namespace and waits for bwrap to exit.
    pub async fn shutdown(mut self) -> Result<()> {
        let Some(processes) = self.processes.as_mut() else {
            anyhow::bail!("bwrap process guard is missing");
        };
        let result = processes.shutdown().await;
        self.processes.take();
        result
    }
}

impl Drop for BwrapTestProcess {
    fn drop(&mut self) {
        // Panicking here starts unwinding, after which BwrapProcesses performs
        // the blocking fallback while its field is dropped.
        if self.processes.is_some() && !std::thread::panicking() {
            panic!("BwrapTestProcess dropped without async teardown");
        }
    }
}

impl BwrapRuntimePaths {
    fn from_runfiles() -> Result<Self> {
        Ok(Self {
            bwrap: codex_utils_cargo_bin::cargo_bin("bwrap")?,
        })
    }
}

impl BwrapProcesses {
    async fn shutdown(&mut self) -> Result<()> {
        let child_status = self.child.try_wait();
        let sandbox_kill_result = self
            .sandbox_init
            .start_kill()
            .context("kill bwrap PID namespace init");
        let (mut check_exit_status, status_check_result) = match child_status {
            Ok(Some(_)) => (true, Ok(())),
            Ok(None) => (false, Ok(())),
            Err(error) => (false, Err(error).context("check bwrap process status")),
        };
        let mut fallback_started = false;
        let early_fallback_result = if sandbox_kill_result.is_err() {
            check_exit_status = false;
            fallback_started = true;
            self.start_kill_launcher()
        } else {
            Ok(())
        };
        let sandbox_wait_result = self
            .sandbox_init
            .wait()
            .await
            .context("wait for bwrap PID namespace init");
        let late_fallback_result = if sandbox_wait_result.is_err() && !fallback_started {
            check_exit_status = false;
            self.start_kill_launcher()
        } else {
            Ok(())
        };
        let mut timeout_kill_result = Ok(());
        let launcher_timeout_result;
        let launcher_wait_result =
            match tokio::time::timeout(BWRAP_CLEANUP_TIMEOUT, self.child.wait()).await {
                Ok(result) => {
                    launcher_timeout_result = Ok(());
                    result.context("wait for bwrap launcher")
                }
                Err(_) => {
                    launcher_timeout_result = Err(anyhow::anyhow!(
                        "timed out waiting for bwrap launcher after {BWRAP_CLEANUP_TIMEOUT:?}"
                    ));
                    check_exit_status = false;
                    timeout_kill_result = self.start_kill_launcher();
                    match tokio::time::timeout(BWRAP_CLEANUP_TIMEOUT, self.child.wait()).await {
                        Ok(result) => result.context("wait for killed bwrap launcher"),
                        Err(_) => Err(anyhow::anyhow!(
                            "timed out waiting for killed bwrap launcher after {BWRAP_CLEANUP_TIMEOUT:?}"
                        )),
                    }
                }
            }
            .and_then(|status| {
                anyhow::ensure!(
                    !check_exit_status || status.success(),
                    "bwrap process exited with {status}"
                );
                Ok(())
            });

        // Every cleanup action has been attempted, so an individual error
        // should not cause the blocking fallback to repeat them.
        self.cleanup_complete = true;
        status_check_result?;
        sandbox_kill_result?;
        early_fallback_result?;
        sandbox_wait_result?;
        late_fallback_result?;
        launcher_timeout_result?;
        timeout_kill_result?;
        launcher_wait_result
    }

    fn start_kill_launcher(&mut self) -> Result<()> {
        if let Err(kill_error) = self.child.start_kill() {
            match self.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) | Err(_) => return Err(kill_error).context("kill bwrap launcher"),
            }
        }
        Ok(())
    }

    fn shutdown_blocking(&mut self) {
        log_panic_cleanup(format_args!(
            "bwrap panic cleanup starting for environment {}",
            self.environment.path().display()
        ));
        if let Err(error) = self.sandbox_init.start_kill() {
            log_panic_cleanup(format_args!(
                "bwrap panic cleanup could not kill its PID namespace init: {error}"
            ));
        }
        if let Err(error) = self.start_kill_launcher() {
            log_panic_cleanup(format_args!(
                "bwrap panic cleanup could not kill its launcher: {error}"
            ));
        }

        log_panic_cleanup(format_args!(
            "bwrap panic cleanup waiting for its PID namespace init"
        ));
        if let Err(error) = self.sandbox_init.wait_blocking() {
            log_panic_cleanup(format_args!(
                "bwrap panic cleanup could not wait for its PID namespace init: {error}"
            ));
        }

        log_panic_cleanup(format_args!("bwrap panic cleanup waiting for its launcher"));
        let deadline = Instant::now() + BWRAP_CLEANUP_TIMEOUT;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    log_panic_cleanup(format_args!(
                        "bwrap panic cleanup launcher exited with {status}"
                    ));
                    break;
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        log_panic_cleanup(format_args!(
                            "bwrap panic cleanup timed out waiting for its launcher"
                        ));
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    log_panic_cleanup(format_args!(
                        "bwrap panic cleanup could not wait for its launcher: {error}"
                    ));
                    break;
                }
            }
        }

        self.cleanup_complete = true;
        log_panic_cleanup(format_args!("bwrap panic cleanup complete"));
    }
}

impl Drop for BwrapProcesses {
    fn drop(&mut self) {
        // Never introduce a second panic while unwinding. Blocking here is
        // intentional because test failures must not leak bwrap children.
        if !self.cleanup_complete && std::thread::panicking() {
            self.shutdown_blocking();
        }
    }
}

fn log_panic_cleanup(args: std::fmt::Arguments<'_>) {
    let _ = writeln!(std::io::stderr().lock(), "{args}");
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
