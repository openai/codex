#[cfg(not(target_os = "linux"))]
compile_error!("bwrap_test_support can only run on Linux");

use std::ffi::OsString;
use std::fs;
use std::future::Future;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use tempfile::Builder;
use tempfile::TempDir;
use tokio::process::Child;
use tokio::process::ChildStdout;
use tokio::process::Command as TokioCommand;

/// Builds a command that runs inside the Linux integration-test namespace.
pub struct BwrapTestCommand {
    executable: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

/// Owns a process running inside the Linux integration-test namespace.
///
/// Call [`Self::scope`] or [`Self::shutdown`] on every successful path. A
/// normal unguarded drop panics, while a drop during unwinding performs
/// blocking cleanup without introducing a second panic.
pub struct BwrapTestProcess {
    processes: Option<BwrapProcesses>,
}

struct BwrapProcesses {
    child: Child,
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

        // Firecracker supplies the host PID boundary, so the outer wrapper
        // deliberately shares the disposable VM's PID namespace and rebinds
        // its procfs read-write. Nested unprivileged bwrap writes UID/GID maps
        // through the caller's /proc before mounting a fresh procfs for its
        // own PID namespace. Giving the outer wrapper a fresh procfs on this
        // executor leaves locked child mounts that make the nested proc mount
        // fail Linux's mount_too_revealing check; making the inherited procfs
        // read-only instead makes UID/GID map setup fail.
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
        let mut command = StdCommand::new(&runtime.bwrap);
        command
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

        let mut command = TokioCommand::from(command);
        command.kill_on_drop(true);
        let child = command
            .spawn()
            .context("start process inside bwrap test environment")?;

        Ok(BwrapTestProcess {
            processes: Some(BwrapProcesses {
                child,
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

    /// Kills the isolated process and waits for bwrap to exit.
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
        let (kill_result, check_exit_status) = match self.child.try_wait() {
            Ok(Some(_)) => (Ok(()), true),
            Ok(None) => (
                self.child
                    .start_kill()
                    .context("kill process running inside bwrap"),
                false,
            ),
            Err(error) => (Err(error).context("check bwrap process status"), false),
        };
        let wait_result = self
            .child
            .wait()
            .await
            .context("wait for process running inside bwrap")
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
        kill_result?;
        wait_result
    }

    fn shutdown_blocking(&mut self) {
        log_panic_cleanup(format_args!(
            "bwrap panic cleanup starting for environment {}",
            self.environment.path().display()
        ));
        if let Err(error) = self.child.start_kill() {
            log_panic_cleanup(format_args!(
                "bwrap panic cleanup could not kill its child: {error}"
            ));
        }

        log_panic_cleanup(format_args!("bwrap panic cleanup waiting for its child"));
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    log_panic_cleanup(format_args!(
                        "bwrap panic cleanup child exited with {status}"
                    ));
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    log_panic_cleanup(format_args!(
                        "bwrap panic cleanup could not wait for its child: {error}"
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
