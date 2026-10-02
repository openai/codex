//! Cover shared probing, session targeting, and conservative behavior for unknown mouse settings.

use super::*;
use pretty_assertions::assert_eq;

#[cfg(unix)]
#[test]
fn completed_probe_returns_stdout() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 'csi-u\\t0\\n'"]);

    assert_eq!(
        command_stdout_before_deadline(
            &mut command,
            Instant::now() + Duration::from_secs(/*secs*/ 5)
        ),
        Some(b"csi-u\t0\n".to_vec())
    );
}

#[cfg(unix)]
#[test]
fn hanging_probe_times_out_and_reaps_child() {
    let pid_file = tempfile::NamedTempFile::new().unwrap();
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "echo $$ > \"$1\"; exec /bin/sleep 30", "sh"])
        .arg(pid_file.path());
    let started = Instant::now();

    assert_eq!(
        command_stdout_before_deadline(
            &mut command,
            Instant::now() + Duration::from_secs(/*secs*/ 1)
        ),
        None
    );
    assert!(started.elapsed() < Duration::from_secs(/*secs*/ 3));
    let pid = std::fs::read_to_string(pid_file.path())
        .unwrap()
        .trim()
        .parse::<libc::pid_t>()
        .unwrap();
    assert_eq!(
        // SAFETY: signal 0 only checks whether this process still exists.
        unsafe {
            libc::kill(pid, /*sig*/ 0)
        },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}

#[cfg(unix)]
#[test]
fn completed_probe_does_not_wait_for_descendant_holding_stdout() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 30 & printf '%s\\tcsi-u\\t0\\n' \"$!\""]);
    let started = Instant::now();

    let output = command_stdout_before_deadline(
        &mut command,
        Instant::now() + Duration::from_secs(/*secs*/ 5),
    )
    .unwrap();
    let output = String::from_utf8(output).unwrap();
    let (pid, settings) = output.split_once('\t').unwrap();
    let pid = pid.parse::<libc::pid_t>().unwrap();
    // SAFETY: the child PID came directly from the test shell.
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }

    assert_eq!(settings, "csi-u\t0\n");
    assert!(started.elapsed() < Duration::from_secs(/*secs*/ 2));
}

#[test]
fn input_probe_targets_the_containing_pane_and_only_disables_confirmed_mouse_off() {
    for (mouse, mouse_capture) in [
        ("0", MouseCapture::DisabledByTmux),
        ("off", MouseCapture::DisabledByTmux),
        ("1", MouseCapture::Enabled),
        ("on", MouseCapture::Enabled),
        ("", MouseCapture::Enabled),
        ("unknown", MouseCapture::Enabled),
    ] {
        let mut calls = Vec::new();
        let options = read_options(Some("%42"), |args| {
            calls.push(args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>());
            Some(format!("csi-u\t{mouse}\n").into_bytes())
        });
        assert_eq!(
            options,
            Options {
                extended_keys_format: Some("csi-u".to_owned()),
                mouse_capture,
            }
        );
        assert_eq!(
            calls,
            vec![vec![
                "display-message",
                "-p",
                "-t",
                "%42",
                "#{extended-keys-format}\t#{mouse}",
            ]]
        );
    }
}

#[test]
fn keyboard_fallback_preserves_confirmed_mouse_policy_without_guessing_on_failure() {
    for (output, mouse_capture) in [
        (Some(b"\t0\n".to_vec()), MouseCapture::DisabledByTmux),
        (Some(b"\t\n".to_vec()), MouseCapture::Enabled),
        (Some(vec![0xff]), MouseCapture::Enabled),
        (None, MouseCapture::Enabled),
    ] {
        let mut calls = Vec::new();
        let options = read_options(/*pane*/ None, |args| {
            calls.push(args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>());
            if args[0] == "display-message" {
                output.clone()
            } else {
                Some(b"xterm\n".to_vec())
            }
        });
        assert_eq!(
            options,
            Options {
                extended_keys_format: Some("xterm".to_owned()),
                mouse_capture,
            }
        );
        assert_eq!(
            calls,
            vec![
                vec!["display-message", "-p", "#{extended-keys-format}\t#{mouse}"],
                vec!["show-options", "-gqv", "extended-keys-format"],
            ]
        );
    }
    assert_eq!(read_options(Some("%42"), |_| None), Options::default());
}
