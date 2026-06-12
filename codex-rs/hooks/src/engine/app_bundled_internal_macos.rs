//! Spawn the exact internal hook executable without a shell, stop it before its first
//! user-space instruction, authenticate the live Security.framework code object, and only
//! then resume it. A suspended child is always killed and reaped on failed verification.

use std::collections::BTreeMap;
use std::ffi::CString;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use std::sync::mpsc;
use std::time::Duration;

use codex_desktop_distribution::VerifiedDesktopDistribution;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

const COMPUTER_USE_CODE_IDENTIFIER: &str = "com.openai.sky.CUAService.cli";

pub(super) struct AuthenticatedOutput {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub(super) struct AuthenticatedInvocation {
    pub distribution: VerifiedDesktopDistribution,
    pub executable: AbsolutePathBuf,
    pub plugin_root: AbsolutePathBuf,
    pub source_path: AbsolutePathBuf,
    pub executable_relative: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

pub(super) async fn run_authenticated(
    invocation: AuthenticatedInvocation,
    input: String,
) -> Result<AuthenticatedOutput, String> {
    let (termination, supervisor) = process_supervision();
    let suspended = tokio::task::spawn_blocking(move || {
        SuspendedChild::spawn(
            SuspendedSpawn {
                distribution: &invocation.distribution,
                executable: &invocation.executable,
                plugin_root: &invocation.plugin_root,
                source_path: &invocation.source_path,
                executable_relative: &invocation.executable_relative,
                args: &invocation.args,
                cwd: invocation.cwd.as_path(),
            },
            supervisor,
        )
    })
    .await
    .map_err(|error| format!("authenticated hook spawn task failed to join: {error}"))??;
    let running = suspended.resume(termination)?;
    let RunningChild {
        mut termination,
        stdin,
        stdout,
        stderr,
    } = running;
    let wait = termination.take_wait_task().ok_or_else(|| {
        "authenticated internal hook lost its process supervision task".to_string()
    })?;
    let write_input = async move {
        let mut stdin = tokio::fs::File::from_std(stdin);
        let result = stdin.write_all(input.as_bytes()).await;
        drop(stdin);
        result.map_err(|error| format!("failed to write hook stdin: {error}"))
    };
    let read_output = |file: File| async move {
        let mut file = tokio::fs::File::from_std(file);
        let mut output = Vec::new();
        file.read_to_end(&mut output)
            .await
            .map_err(|error| format!("failed to read hook output: {error}"))?;
        Ok::<_, String>(output)
    };
    let (stdin_result, stdout_result, stderr_result, wait_result) = tokio::join!(
        write_input,
        read_output(stdout),
        read_output(stderr),
        flatten_join(wait),
    );
    let exit_code = wait_result?;
    termination.disarm();
    stdin_result?;
    Ok(AuthenticatedOutput {
        exit_code,
        stdout: stdout_result?,
        stderr: stderr_result?,
    })
}

async fn flatten_join<T>(join: tokio::task::JoinHandle<Result<T, String>>) -> Result<T, String> {
    join.await
        .map_err(|error| format!("hook process waiter failed to join: {error}"))?
}

struct SuspendedChild {
    pid: libc::pid_t,
    stdin: Option<File>,
    stdout: Option<File>,
    stderr: Option<File>,
    armed: bool,
}

struct SuspendedSpawn<'a> {
    distribution: &'a VerifiedDesktopDistribution,
    executable: &'a AbsolutePathBuf,
    plugin_root: &'a AbsolutePathBuf,
    source_path: &'a AbsolutePathBuf,
    executable_relative: &'a str,
    args: &'a [String],
    cwd: &'a Path,
}

struct SupervisorChannels {
    terminate_requests: mpsc::Receiver<()>,
    result: mpsc::SyncSender<Result<Option<i32>, String>>,
}

impl SuspendedChild {
    fn spawn(request: SuspendedSpawn<'_>, supervisor: SupervisorChannels) -> Result<Self, String> {
        match supervisor.terminate_requests.try_recv() {
            Ok(()) | Err(mpsc::TryRecvError::Disconnected) => {
                return Err("internal hook was cancelled before suspended spawn".to_string());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        let SuspendedSpawn {
            distribution,
            executable,
            plugin_root,
            source_path,
            executable_relative,
            args,
            cwd,
        } = request;
        let (child_stdin, parent_stdin) = pipe().map_err(|error| error.to_string())?;
        let (parent_stdout, child_stdout) = pipe().map_err(|error| error.to_string())?;
        let (parent_stderr, child_stderr) = pipe().map_err(|error| error.to_string())?;
        let mut file_actions = SpawnFileActions::new()?;
        file_actions.dup2(child_stdin.as_raw_fd(), libc::STDIN_FILENO)?;
        file_actions.dup2(child_stdout.as_raw_fd(), libc::STDOUT_FILENO)?;
        file_actions.dup2(child_stderr.as_raw_fd(), libc::STDERR_FILENO)?;
        file_actions.chdir(cwd)?;
        let mut attributes = SpawnAttributes::new()?;
        attributes
            .set_flags(libc::POSIX_SPAWN_START_SUSPENDED | libc::POSIX_SPAWN_CLOEXEC_DEFAULT)?;
        let executable_c = c_string(executable.as_path().as_os_str(), "hook executable")?;
        let argv = command_argv(&executable_c, args)?;
        let env = command_env()?;
        let argv_ptrs = c_string_ptrs(&argv);
        let env_ptrs = c_string_ptrs(&env);
        let mut pid = 0;
        let result = unsafe {
            libc::posix_spawn(
                &mut pid,
                executable_c.as_ptr(),
                file_actions.as_ptr(),
                attributes.as_ptr(),
                argv_ptrs.as_ptr(),
                env_ptrs.as_ptr(),
            )
        };
        if result != 0 {
            return Err(format!(
                "failed to spawn suspended internal hook: {}",
                io::Error::from_raw_os_error(result)
            ));
        }
        drop(child_stdin);
        drop(child_stdout);
        drop(child_stderr);
        let mut child = Self {
            pid,
            stdin: Some(parent_stdin),
            stdout: Some(parent_stdout),
            stderr: Some(parent_stderr),
            armed: true,
        };
        if let Err(error) = wait_for_initial_stop(pid) {
            if error.reaped {
                child.armed = false;
            }
            return Err(error.message);
        }
        child.start_supervisor(supervisor)?;
        distribution
            .authenticate_spawned_executable(
                pid,
                executable.as_path(),
                COMPUTER_USE_CODE_IDENTIFIER,
            )
            .map_err(|error| format!("spawned internal hook authentication failed: {error}"))?;
        super::command_runner::verify_current_internal_opt_in(
            distribution,
            plugin_root,
            source_path,
            executable_relative,
        )?;
        distribution.reverify().map_err(|error| {
            format!("app-bundled internal hook changed before process resume: {error}")
        })?;
        Ok(child)
    }

    fn start_supervisor(&mut self, supervisor: SupervisorChannels) -> Result<(), String> {
        let SupervisorChannels {
            terminate_requests,
            result,
        } = supervisor;
        let (ready, ready_wait) = mpsc::sync_channel(0);
        let pid = self.pid;
        std::thread::Builder::new()
            .name("codex-internal-hook-supervisor".to_string())
            .spawn(move || {
                if ready.send(()).is_err() {
                    kill_and_reap(pid);
                    return;
                }
                let outcome = supervise_running_process(pid, terminate_requests);
                let _ = result.send(outcome);
            })
            .map_err(|error| format!("failed to start internal hook supervisor: {error}"))?;
        ready_wait
            .recv()
            .map_err(|_| "internal hook supervisor stopped before becoming ready".to_string())?;
        self.armed = false;
        Ok(())
    }

    fn resume(mut self, termination: TerminateOnDrop) -> Result<RunningChild, String> {
        let stdin = self
            .stdin
            .take()
            .ok_or_else(|| "suspended internal hook lost its stdin pipe".to_string())?;
        let stdout = self
            .stdout
            .take()
            .ok_or_else(|| "suspended internal hook lost its stdout pipe".to_string())?;
        let stderr = self
            .stderr
            .take()
            .ok_or_else(|| "suspended internal hook lost its stderr pipe".to_string())?;
        let result = unsafe { libc::kill(self.pid, libc::SIGCONT) };
        if result != 0 {
            return Err(format!(
                "failed to resume authenticated internal hook: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(RunningChild {
            termination,
            stdin,
            stdout,
            stderr,
        })
    }
}

impl Drop for SuspendedChild {
    fn drop(&mut self) {
        if self.armed {
            kill_and_reap(self.pid);
        }
    }
}

struct RunningChild {
    termination: TerminateOnDrop,
    stdin: File,
    stdout: File,
    stderr: File,
}

struct TerminateOnDrop {
    terminate: Option<mpsc::SyncSender<()>>,
    result: Option<mpsc::Receiver<Result<Option<i32>, String>>>,
}

impl TerminateOnDrop {
    fn take_wait_task(&mut self) -> Option<tokio::task::JoinHandle<Result<Option<i32>, String>>> {
        let result = self.result.take()?;
        Some(tokio::task::spawn_blocking(move || {
            result
                .recv()
                .map_err(|_| "internal hook supervisor stopped without a result".to_string())?
        }))
    }

    fn disarm(&mut self) {
        self.terminate = None;
    }
}

fn process_supervision() -> (TerminateOnDrop, SupervisorChannels) {
    let (terminate, terminate_requests) = mpsc::sync_channel(1);
    let (result, result_wait) = mpsc::sync_channel(1);
    (
        TerminateOnDrop {
            terminate: Some(terminate),
            result: Some(result_wait),
        },
        SupervisorChannels {
            terminate_requests,
            result,
        },
    )
}

impl Drop for TerminateOnDrop {
    fn drop(&mut self) {
        if let Some(terminate) = self.terminate.take() {
            let _ = terminate.try_send(());
        }
    }
}

struct SpawnFileActions(libc::posix_spawn_file_actions_t);

impl SpawnFileActions {
    fn new() -> Result<Self, String> {
        let mut actions = MaybeUninit::uninit();
        check_spawn_call(
            unsafe { libc::posix_spawn_file_actions_init(actions.as_mut_ptr()) },
            "initialize spawn file actions",
        )?;
        Ok(Self(unsafe { actions.assume_init() }))
    }

    fn as_ptr(&self) -> *const libc::posix_spawn_file_actions_t {
        &self.0
    }

    fn dup2(&mut self, fd: libc::c_int, target: libc::c_int) -> Result<(), String> {
        check_spawn_call(
            unsafe { libc::posix_spawn_file_actions_adddup2(&mut self.0, fd, target) },
            "configure hook stdio",
        )
    }

    fn chdir(&mut self, cwd: &Path) -> Result<(), String> {
        let cwd = c_string(cwd.as_os_str(), "hook working directory")?;
        check_spawn_call(
            unsafe { posix_spawn_file_actions_addchdir_np(&mut self.0, cwd.as_ptr()) },
            "configure hook working directory",
        )
    }
}

impl Drop for SpawnFileActions {
    fn drop(&mut self) {
        let _ = unsafe { libc::posix_spawn_file_actions_destroy(&mut self.0) };
    }
}

struct SpawnAttributes(libc::posix_spawnattr_t);

impl SpawnAttributes {
    fn new() -> Result<Self, String> {
        let mut attributes = MaybeUninit::uninit();
        check_spawn_call(
            unsafe { libc::posix_spawnattr_init(attributes.as_mut_ptr()) },
            "initialize spawn attributes",
        )?;
        Ok(Self(unsafe { attributes.assume_init() }))
    }

    fn as_ptr(&self) -> *const libc::posix_spawnattr_t {
        &self.0
    }

    fn set_flags(&mut self, flags: libc::c_int) -> Result<(), String> {
        let flags = libc::c_short::try_from(flags)
            .map_err(|_| "invalid suspended spawn flags".to_string())?;
        check_spawn_call(
            unsafe { libc::posix_spawnattr_setflags(&mut self.0, flags) },
            "configure suspended spawn",
        )
    }
}

impl Drop for SpawnAttributes {
    fn drop(&mut self) {
        let _ = unsafe { libc::posix_spawnattr_destroy(&mut self.0) };
    }
}

fn pipe() -> io::Result<(File, File)> {
    let mut descriptors = [0; 2];
    if unsafe { libc::pipe(descriptors.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let read = unsafe { File::from_raw_fd(descriptors[0]) };
    let write = unsafe { File::from_raw_fd(descriptors[1]) };
    set_close_on_exec(&read)?;
    set_close_on_exec(&write)?;
    Ok((read, write))
}

fn set_close_on_exec(file: &File) -> io::Result<()> {
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn command_argv(executable: &CString, args: &[String]) -> Result<Vec<CString>, String> {
    std::iter::once(Ok(executable.clone()))
        .chain(args.iter().map(|arg| {
            CString::new(arg.as_bytes())
                .map_err(|_| "internal hook argument contained a NUL byte".to_string())
        }))
        .collect()
}

fn command_env() -> Result<Vec<CString>, String> {
    let mut env = super::command_runner::internal_ambient_environment()
        .into_iter()
        .collect::<BTreeMap<OsString, OsString>>();
    env.retain(|key, _| {
        let key = key.as_os_str().as_bytes();
        !is_dynamic_loader_variable(key)
    });
    env.into_iter()
        .map(|(key, value)| {
            let mut entry = key.as_os_str().as_bytes().to_vec();
            entry.push(b'=');
            entry.extend_from_slice(value.as_os_str().as_bytes());
            CString::new(entry)
                .map_err(|_| "internal hook environment contained a NUL byte".to_string())
        })
        .collect()
}

fn is_dynamic_loader_variable(key: &[u8]) -> bool {
    key.starts_with(b"DYLD_") || key.starts_with(b"LD_") || key.starts_with(b"__XPC_DYLD_")
}

fn c_string(value: &OsStr, label: &str) -> Result<CString, String> {
    CString::new(value.as_bytes()).map_err(|_| format!("{label} contained a NUL byte"))
}

fn c_string_ptrs(values: &[CString]) -> Vec<*mut libc::c_char> {
    values
        .iter()
        .map(|value| value.as_ptr().cast_mut())
        .chain(std::iter::once(ptr::null_mut()))
        .collect()
}

struct InitialStopError {
    message: String,
    reaped: bool,
}

fn wait_for_initial_stop(pid: libc::pid_t) -> Result<(), InitialStopError> {
    let status = waitpid(pid, libc::WUNTRACED)?;
    if libc::WIFSTOPPED(status) && libc::WSTOPSIG(status) == libc::SIGSTOP {
        return Ok(());
    }
    Err(InitialStopError {
        message: "internal hook did not remain suspended before authentication".to_string(),
        reaped: libc::WIFEXITED(status) || libc::WIFSIGNALED(status),
    })
}

impl From<String> for InitialStopError {
    fn from(message: String) -> Self {
        Self {
            message,
            // `waitpid` only returns an error here after EINTR retries. Treat the PID as
            // no longer ours rather than risking a signal to a subsequently reused PID.
            reaped: true,
        }
    }
}

fn supervise_running_process(
    pid: libc::pid_t,
    terminate_requests: mpsc::Receiver<()>,
) -> Result<Option<i32>, String> {
    loop {
        match terminate_requests.recv_timeout(Duration::from_millis(5)) {
            Ok(()) => {
                let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
                return wait_for_exit(pid);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return wait_for_exit(pid),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(status) = try_waitpid(pid)? {
            return exit_code(status);
        }
    }
}

fn wait_for_exit(pid: libc::pid_t) -> Result<Option<i32>, String> {
    exit_code(waitpid(pid, 0)?)
}

fn exit_code(status: libc::c_int) -> Result<Option<i32>, String> {
    if libc::WIFEXITED(status) {
        Ok(Some(libc::WEXITSTATUS(status)))
    } else if libc::WIFSIGNALED(status) {
        Ok(None)
    } else {
        Err("internal hook changed process state without exiting".to_string())
    }
}

fn try_waitpid(pid: libc::pid_t) -> Result<Option<libc::c_int>, String> {
    loop {
        let mut status = 0;
        let result = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if result == pid {
            return Ok(Some(status));
        }
        if result == 0 {
            return Ok(None);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("failed to inspect internal hook process: {error}"));
        }
    }
}

fn waitpid(pid: libc::pid_t, options: libc::c_int) -> Result<libc::c_int, String> {
    loop {
        let mut status = 0;
        let result = unsafe { libc::waitpid(pid, &mut status, options) };
        if result == pid {
            return Ok(status);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("failed to wait for internal hook process: {error}"));
        }
    }
}

fn kill_and_reap(pid: libc::pid_t) {
    let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
    let _ = waitpid(pid, 0);
}

fn check_spawn_call(result: libc::c_int, action: &str) -> Result<(), String> {
    if result == 0 {
        Ok(())
    } else {
        Err(format!(
            "failed to {action}: {}",
            io::Error::from_raw_os_error(result)
        ))
    }
}

unsafe extern "C" {
    fn posix_spawn_file_actions_addchdir_np(
        actions: *mut libc::posix_spawn_file_actions_t,
        path: *const libc::c_char,
    ) -> libc::c_int;
}

#[cfg(test)]
mod tests {
    use super::is_dynamic_loader_variable;

    #[test]
    fn strips_dynamic_loader_environment_overrides() {
        assert!(is_dynamic_loader_variable(b"DYLD_INSERT_LIBRARIES"));
        assert!(is_dynamic_loader_variable(b"LD_LIBRARY_PATH"));
        assert!(is_dynamic_loader_variable(b"__XPC_DYLD_INSERT_LIBRARIES"));
        assert!(!is_dynamic_loader_variable(b"PATH"));
    }
}
