//! WSL text copies with Windows interop use the Windows clipboard without contacting the display.
//! Use the lightweight Windows clip writer for single-line ASCII; PowerShell preserves other text.
//! Bound discovery, input, and exit; give fallback native setup its own short deadline.
//! Never release the worker while a writer can still run.

#[cfg(any(target_os = "linux", all(test, unix)))]
use std::path::PathBuf;
#[cfg(any(target_os = "linux", all(test, unix)))]
use std::sync::Mutex;
#[cfg(any(target_os = "linux", all(test, unix)))]
use std::sync::mpsc;
#[cfg(any(target_os = "linux", all(test, unix)))]
use std::time::Instant;

#[cfg(any(target_os = "linux", all(test, unix)))]
type Pending<T> = Mutex<Option<mpsc::Receiver<Result<T, String>>>>;

#[cfg(target_os = "linux")]
pub(super) fn copy(text: &str, deadline: Instant) -> Result<(), String> {
    static SETUP: Pending<(std::ffi::OsString, Option<PathBuf>)> = Mutex::new(None);
    static CLIP: Pending<Option<PathBuf>> = Mutex::new(None);
    let (path, adapter) = read_only_setup(&SETUP, deadline, || {
        let path = codex_utils_path::system_path()
            .map_err(|error| format!("failed to resolve system PATH: {error}"))?;
        // An installed Linux adapter takes precedence over both Windows writers. Avoid
        // looking up the built-in PowerShell until clip.exe actually needs a fallback.
        let adapter = codex_utils_path::linux_system_executable("powershell.exe");
        Ok((path, adapter))
    })?;
    let clip = (adapter.is_none() && clip_input(text).is_some())
        .then(|| {
            read_only_setup(
                &CLIP,
                deadline.min(Instant::now() + std::time::Duration::from_millis(/*millis*/ 200)),
                || Ok(codex_utils_path::system_executable("clip.exe")),
            )
            .ok()
            .flatten()
        })
        .flatten();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("could not start Windows clipboard writer: {error}"))?;
    runtime.block_on(copy_with_writers(
        text,
        deadline,
        &path,
        adapter.as_deref(),
        clip.as_deref(),
    ))
}

#[cfg(any(target_os = "linux", all(test, unix)))]
async fn copy_with_writers(
    text: &str,
    deadline: Instant,
    path: &std::ffi::OsStr,
    adapter: Option<&std::path::Path>,
    clip: Option<&std::path::Path>,
) -> Result<(), String> {
    static POWERSHELL: Pending<Option<PathBuf>> = Mutex::new(None);
    if let Some(adapter) = adapter {
        return write_writer(command(adapter, path), text, deadline, "powershell.exe").await;
    }
    let clip_error = if let Some(clip) = clip
        && let Some(input) = clip_input(text)
    {
        match write_writer(
            shell_command(clip, path),
            &input,
            deadline.min(Instant::now() + std::time::Duration::from_secs(/*secs*/ 1)),
            "clip.exe",
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(error) => Some(error),
        }
    } else {
        None
    };
    let fallback = async {
        let powershell = read_only_setup(&POWERSHELL, deadline, || {
            Ok(codex_utils_path::system_executable("powershell.exe"))
        })?
        .ok_or("PowerShell is unavailable in the system PATH")?;
        write_writer(command(&powershell, path), text, deadline, "powershell.exe").await
    }
    .await;
    fallback.map_err(|error| match clip_error {
        Some(clip_error) => format!("{clip_error}; {error}"),
        None => error,
    })
}

#[cfg(any(target_os = "linux", all(test, unix)))]
async fn write_writer(
    command: tokio::process::Command,
    text: &str,
    deadline: Instant,
    name: &str,
) -> Result<(), String> {
    let status = write_command(command, text, deadline)
        .await
        .map_err(|error| format!("{name}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} exited with status {status}"))
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn clip_input(text: &str) -> Option<std::borrow::Cow<'_, str>> {
    // clip.exe normalizes newlines and guesses UTF-16 for some even-length ASCII.
    // An odd byte count forces OEM text; a trailing NUL does not enter the clipboard.
    text.bytes()
        .all(|byte| byte.is_ascii_graphic() || matches!(byte, b' ' | b'\t'))
        .then(|| {
            if text.len().is_multiple_of(2) {
                std::borrow::Cow::Owned(format!("{text}\0"))
            } else {
                std::borrow::Cow::Borrowed(text)
            }
        })
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn command(executable: &std::path::Path, path: &std::ffi::OsStr) -> tokio::process::Command {
    let mut command = shell_command(executable, path);
    // Preserve the released PowerShell signature: trusted adapters may match it exactly.
    command.args([
        "-NoProfile",
        "-Command",
        "[Console]::InputEncoding = [System.Text.Encoding]::UTF8; $ErrorActionPreference = 'Stop'; $text = [Console]::In.ReadToEnd(); Set-Clipboard -Value $text",
    ]);
    command
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn shell_command(executable: &std::path::Path, path: &std::ffi::OsStr) -> tokio::process::Command {
    // Spawn only the native shell here: directly spawning a Windows exe can block on WSL.
    let mut command = tokio::process::Command::new("/bin/sh");
    command
        .env("PATH", path)
        .args(["-c", r#"exec "$@""#, "codex-wsl-copy"])
        .arg(executable);
    command
}

#[cfg(not(target_os = "linux"))]
pub(super) fn copy(_text: &str, _deadline: std::time::Instant) -> Result<(), String> {
    Err("Windows clipboard unavailable on this platform".into())
}

#[cfg(target_os = "linux")]
pub(super) fn native_clipboard() -> Result<arboard::Clipboard, String> {
    static NATIVE: Pending<arboard::Clipboard> = Mutex::new(None);
    // Windows discovery may have exhausted its budget. Do not starve a healthy WSLg.
    let deadline = Instant::now() + std::time::Duration::from_secs(/*secs*/ 1);
    read_only_setup(&NATIVE, deadline, || {
        arboard::Clipboard::new().map_err(|error| format!("clipboard unavailable: {error}"))
    })
}

// Discovery and WSLg setup may block forever. Keep at most one outstanding task of each
// kind and reuse its result after a timeout. Neither task may capture text or write a clipboard.
#[cfg(any(target_os = "linux", all(test, unix)))]
fn read_only_setup<T: Send + 'static>(
    pending: &Pending<T>,
    deadline: Instant,
    setup: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or("clipboard setup timed out")?;
    let mut pending = pending.lock().map_err(|error| error.to_string())?;
    if pending.is_none() {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("wsl-clipboard-setup".into())
            .spawn(move || {
                let _ = tx.send(setup());
            })
            .map_err(|error| format!("could not start clipboard setup: {error}"))?;
        *pending = Some(rx);
    }
    let result = pending
        .as_ref()
        .ok_or("clipboard setup unavailable")?
        .recv_timeout(remaining);
    if !matches!(result, Err(mpsc::RecvTimeoutError::Timeout)) {
        *pending = None;
    }
    result.map_err(|error| format!("clipboard setup: {error}"))?
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub(super) fn native_clipboard() -> Result<arboard::Clipboard, String> {
    Err("WSL native clipboard unavailable on this platform".into())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
async fn write_command(
    mut command: tokio::process::Command,
    text: &str,
    deadline: Instant,
) -> Result<std::process::ExitStatus, String> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;

    if Instant::now() >= deadline {
        return Err("Windows clipboard writer timed out".into());
    }
    let mut child = command
        .process_group(/*pgroup*/ 0)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        // Do not expose clipboard contents from PowerShell errors or let its stderr block input.
        .stderr(Stdio::null())
        .kill_on_drop(/*kill_on_drop*/ true)
        .spawn()
        .map_err(|error| format!("failed to spawn Windows clipboard writer: {error}"))?;
    // WSL process spawn itself may stall. Do not write after a late spawn.
    let result = tokio::time::timeout_at(deadline.into(), async {
        if Instant::now() >= deadline {
            return Err("Windows clipboard writer timed out".into());
        }
        child
            .stdin
            .as_mut()
            .ok_or("failed to open Windows clipboard writer stdin")?
            .write_all(text.as_bytes())
            .await
            .map_err(|error| format!("failed to write to Windows clipboard writer: {error}"))?;
        drop(child.stdin.take());
        child
            .wait()
            .await
            .map_err(|error| format!("failed to wait for Windows clipboard writer: {error}"))
    })
    .await
    .unwrap_or_else(|_| Err("Windows clipboard writer timed out".into()));
    match result {
        Ok(status) => Ok(status),
        Err(error) => {
            // Keep partially written stdin open until kill: PowerShell copies on EOF.
            // If kill fails, still wait. Releasing the worker earlier could allow an old copy
            // to overwrite a newer one. kill_on_drop alone does not guarantee reaping.
            if let Some(pid) = child.id() {
                // SAFETY: process_group(0) assigned the owned, unreaped child its own group.
                // An installed adapter may have spawned its own writer. Stop the whole group.
                unsafe { libc::killpg(pid as libc::pid_t, libc::SIGKILL) };
            }
            let _ = child.start_kill();
            // wait() closes child.stdin itself, even if kill failed. Never give the
            // writer EOF with partial input while waiting for it to stop.
            let _stdin = child.stdin.take();
            child
                .wait()
                .await
                .map_err(|error| format!("failed to reap Windows clipboard writer: {error}"))?;
            Err(error)
        }
    }
}

#[cfg(all(test, unix))]
#[path = "wsl_tests.rs"]
mod tests;
