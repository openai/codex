//! Exercise Unix shell job control with the TUI on a disposable controlling terminal.

use super::PtyCodex;
use super::STARTUP_TIMEOUT;
use super::contains_bytes;
use super::write_test_config;
use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

#[test]
fn background_continue_leaves_terminal_untouched_until_foregrounded() -> Result<()> {
    background_continue("fg")
}

// On Linux, TIOCNOTTY lets the session leader detach the job while the PTY stays open.
#[cfg(target_os = "linux")]
#[test]
fn background_continue_exits_quietly_when_the_terminal_is_detached() -> Result<()> {
    background_continue("detach")
}

fn background_continue(handoff: &str) -> Result<()> {
    // Act as the shell to control when the job gets the terminal back.
    let shell = r#"
import fcntl, os, signal, sys, termios

os.setsid()
fcntl.ioctl(0, termios.TIOCSCTTY, 0)
signal.signal(signal.SIGTTOU, signal.SIG_IGN)
job = os.fork()
if job == 0:
    os.setpgid(0, 0)
    os.tcsetpgrp(0, os.getpgrp())
    signals = {signal.SIGTTOU, signal.SIGTSTP, signal.SIGCONT, signal.SIGHUP}
    for sig in signals:
        signal.signal(sig, signal.SIG_DFL)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, signals)
    os.execv(sys.argv[2], sys.argv[2:])

try:
    signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGUSR1})
    _, status = os.waitpid(job, os.WUNTRACED)
    assert os.WIFSTOPPED(status), status
    os.tcsetpgrp(0, os.getpgrp())
    shell_modes = termios.tcgetattr(0)
    os.write(1, b"<bg>")
    os.killpg(job, signal.SIGCONT)
    signal.sigwait({signal.SIGUSR1})
    assert os.waitpid(job, os.WUNTRACED | os.WNOHANG) == (0, 0), "job stopped or exited"
    assert termios.tcgetattr(0) == shell_modes, "bg changed the shell's terminal modes"
    if sys.argv[1] == "detach":
        signal.signal(signal.SIGHUP, signal.SIG_IGN)
        # Run cleanup even if the detached job never exits.
        signal.signal(signal.SIGALRM, lambda *_: sys.exit("detached job did not exit"))
        signal.alarm(10)
        fcntl.ioctl(0, termios.TIOCNOTTY)
        status = os.waitid(os.P_PID, job, os.WEXITED | os.WNOWAIT)
        assert status.si_code == os.CLD_EXITED, status
        os.write(1, b"<detach>")
        sys.exit(0)
    # Like zsh, hand the terminal to the running job without another SIGCONT.
    os.write(1, b"<fg>")
    os.tcsetpgrp(0, job)
    # The terminal notifies us when the resumed TUI requests the cursor position.
    signal.sigwait({signal.SIGUSR1})
    modes = termios.tcgetattr(0)
    assert not modes[3] & (termios.ICANON | termios.ECHO | termios.ISIG), modes
finally:
    try:
        os.killpg(job, signal.SIGKILL)
        os.waitpid(job, 0)
    except ProcessLookupError:
        pass
"#;
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let home = tempfile::tempdir()?;
    write_test_config(home.path(), &repo_root)?;
    let codex = codex_utils_cargo_bin::cargo_bin("codex-tui")
        .or_else(|_| codex_utils_cargo_bin::cargo_bin("codex"))?;
    let mut terminal = PtyCodex::start_binary(
        Path::new("python3"),
        &repo_root,
        home,
        &[
            "-I",
            "-c",
            shell,
            handoff,
            codex.to_str().context("TUI path")?,
            "--no-alt-screen",
        ],
        /*editor*/ None,
    )?;
    terminal.wait_for_startup()?;
    terminal.write_input(b"/status")?;
    terminal.wait_for_screen("show current session configuration")?;
    let before_suspend = terminal.output.len();
    // Use the Ctrl-Z encoding of the keyboard protocol negotiated by this PTY.
    terminal.write_input(b"\x1b[122;5u")?;
    terminal.wait_for_screen("<bg>")?;
    // Observe the background interval before allowing the shell to hand off the terminal.
    terminal.read_output(Duration::from_millis(/*millis*/ 200))?;
    let shell_pid = terminal.child.id().try_into()?;
    // SAFETY: the shell is our own unreaped child.
    ensure!(unsafe { libc::kill(shell_pid, libc::SIGUSR1) } == 0);
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut notified = false;
    loop {
        terminal.read_output(Duration::from_millis(/*millis*/ 50))?;
        if let Some(status) = terminal.child.try_wait()? {
            ensure!(
                status.success(),
                "job control failed: {}",
                terminal.screen_contents()
            );
            break;
        }
        let output = String::from_utf8_lossy(&terminal.output[before_suspend..]);
        if !notified
            && let Some((_, foreground)) = output.split_once("<fg>")
            && foreground.contains("\x1b[6n")
        {
            // SAFETY: the shell is our own unreaped child.
            ensure!(unsafe { libc::kill(shell_pid, libc::SIGUSR1) } == 0);
            notified = true;
        }
        ensure!(
            Instant::now() < deadline,
            "job control stalled, output since Ctrl-Z: {output:?}"
        );
    }
    ensure!(
        contains_bytes(&terminal.output, format!("<bg><{handoff}>").as_bytes()),
        "TUI wrote to the background terminal"
    );
    Ok(())
}
