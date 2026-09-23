//! Linux launch setup in a fresh image, avoiding a fork of the app-server.
//!
//! The shared launcher preserves descriptors and owns reaping. A private socket transfers
//! the target environment after exec. The helper itself starts with an empty
//! environment so parent and target loader settings cannot affect its bootstrap.
//! A prefix followed by EOF acknowledges target exec; setup failures append errno.
//! Exiting without a prefix means the target has not run, so direct spawn is safe.

use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::child_command::ChildDropPolicy;

/// Setup that needs a fresh, single-threaded image before target exec.
pub(crate) enum Setup {
    /// Detach from the terminal and terminate when the parent dies.
    Pipe,
    /// Establish a controlling terminal and reset interactive signal state.
    Pty,
}

pub(super) const HELPER_ARG: &str = "--codex-run-as-process-setup";
pub(super) const MAX_ENV_BYTES: usize = 8 * 1024 * 1024;
pub(super) const REPORT_PREFIX: u8 = 0;
static HELPER_READY: AtomicBool = AtomicBool::new(false);
const HELPER_EXE: &str = "/proc/self/exe";

/// Dispatch internal setup, or register this executable for later helper launches.
/// Pass the full argument list before starting threads or application initialization.
/// Executables that do not register retain their existing spawning behavior.
/// Before Rust initializes argv (notably musl constructors), read the kernel's
/// command line so re-execution can dispatch before registering the helper.
pub fn init_spawn_helper(args: impl IntoIterator<Item = std::ffi::OsString>) {
    use std::os::unix::ffi::OsStringExt;

    let mut args = args.into_iter().collect::<Vec<_>>();
    if args.is_empty() {
        let Ok(command_line) = std::fs::read("/proc/self/cmdline") else {
            return;
        };
        let Some(command_line) = command_line.strip_suffix(&[0]) else {
            return;
        };
        args = command_line
            .split(|byte| *byte == 0)
            .map(|arg| std::ffi::OsString::from_vec(arg.to_vec()))
            .collect();
    }
    let mut args = args.into_iter().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new(HELPER_ARG)) {
        crate::spawn_helper_main::dispatch(args);
    }
    if std::fs::metadata(HELPER_EXE).is_ok() {
        HELPER_READY.store(true, Ordering::Relaxed);
    }
}

pub(crate) fn is_available() -> bool {
    HELPER_READY.load(Ordering::Relaxed)
}

/// Start the helper and await target exec without blocking a runtime worker.
/// Return `None` when registration, helper resource setup, or helper launch fails,
/// preserving compatibility with callers and sandboxes that only permit the target.
pub(crate) async fn spawn(
    target: &crate::Command,
    setup: Setup,
) -> io::Result<Option<crate::Child>> {
    target.validate()?;
    if !is_available() {
        return Ok(None);
    }
    let settings = target.inner.as_std();
    let mut environment = Vec::new();
    for (key, value) in settings.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))) {
        let mut entry = key.as_bytes().to_vec();
        entry.push(b'=');
        entry.extend_from_slice(value.as_bytes());
        environment.extend_from_slice(CString::new(entry)?.as_bytes_with_nul());
    }
    if environment.len() > MAX_ENV_BYTES {
        return Err(io::Error::from_raw_os_error(libc::E2BIG));
    }
    // Helper-only resources may be denied or exhaust descriptors even when
    // the original command can still spawn. Drop them before trying the fallback.
    let prepared = (|| -> io::Result<_> {
        let (control, helper_control) = UnixStream::pair()?;
        let helper_control = crate::child::posix::child_fd(helper_control.into())?;
        control.set_nonblocking(true)?;
        let control = tokio::net::UnixStream::from_std(control)?;
        let fd = helper_control.as_raw_fd();
        let mut helper = crate::Command::new(HELPER_EXE);
        helper
            .arg(HELPER_ARG)
            .arg(fd.to_string())
            .arg(std::process::id().to_string())
            .arg(match setup {
                Setup::Pipe => "pipe",
                Setup::Pty => "pty",
            })
            .arg(
                target
                    .inherited_fds
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .arg(settings.get_current_dir().unwrap_or_else(|| Path::new(".")))
            .arg(settings.get_program())
            .arg(target.arg0.as_deref().unwrap_or(settings.get_program()))
            .args(settings.get_args())
            .drop_policy(ChildDropPolicy::ReapOnly)
            .stdin(match &target.stdin {
                crate::ChildStdin::Piped => crate::ChildStdin::Piped,
                crate::ChildStdin::Null => crate::ChildStdin::Null,
                crate::ChildStdin::File(fd) => crate::ChildStdin::File(fd.try_clone()?),
            });
        helper.stdout_file = target
            .stdout_file
            .as_ref()
            .map(std::os::fd::OwnedFd::try_clone)
            .transpose()?;
        helper.stderr_file = target
            .stderr_file
            .as_ref()
            .map(std::os::fd::OwnedFd::try_clone)
            .transpose()?;
        helper.inherited_fds = target.inherited_fds.clone();
        helper.inherited_fds.push(fd);
        Ok((control, helper, helper_control))
    })();
    let (mut control, helper, helper_control) = match prepared {
        Ok(prepared) => prepared,
        Err(_) => return Ok(None),
    };
    // Call the shared native backend directly: the helper needs no child callbacks.
    // Unsupported libc/platform setup falls back at the original target boundary.
    let child = match crate::child::posix::NativeChild::spawn(&helper) {
        Ok(Some(child)) => child,
        Ok(None) | Err(_) => return Ok(None),
    };
    drop(helper);
    drop(helper_control);
    // All inherited descriptors were passed to the helper synchronously, before
    // the first suspension. The caller can now close its copies safely.
    let mut starting = StartingChild(Some(child));
    let sent = async {
        control
            .write_all(&(environment.len() as u32).to_le_bytes())
            .await?;
        control.write_all(&environment).await
    }
    .await;
    let mut report = Vec::new();
    let received = control.take(/*limit*/ 6).read_to_end(&mut report).await;
    if report.is_empty() {
        // Bootstrap can fail before dispatch (for example in the dynamic loader).
        // No prefix means the target never ran. Kill and reap the incomplete
        // helper before falling back, including when its socket was reset.
        return Ok(None);
    }
    if (report.len() != 1 && report.len() != 5) || report.first() != Some(&REPORT_PREFIX) {
        sent?;
        received?;
        return Err(io::Error::other("invalid spawn helper report"));
    }
    if report.len() == 5 {
        // Prefer the setup errno when failure closed the socket during env transfer.
        let errno = i32::from_le_bytes(report[1..].try_into().map_err(io::Error::other)?);
        return Err(io::Error::from_raw_os_error(errno));
    }
    sent?;
    received?;
    Ok(starting.0.take())
}

/// Kill incomplete launches on cancellation; the shared child still owns reaping.
struct StartingChild(Option<crate::Child>);

impl Drop for StartingChild {
    fn drop(&mut self) {
        if let Some(pid) = self.0.as_ref().and_then(crate::Child::id) {
            // SAFETY: The child retains this unreaped PID. Kill covers both sides
            // of setsid and the window after target exec but before acknowledgement.
            unsafe {
                libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }
}
