//! Regression coverage for bounded Windows terminal detection and child cleanup.

use super::VscodeDetection;
use super::keyboard_enhancement_disabled_for;
use super::read_windows_vscode_detection_with_timeout;
use pretty_assertions::assert_eq;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

#[test]
fn reads_term_program_only_from_successful_probe() {
    for (script, expected) in [
        ("printf 'TERM_PROGRAM=vscode\r\n'", VscodeDetection::VsCode),
        (
            "printf 'TERM_PROGRAM=vscode\r\n'; exit 1",
            VscodeDetection::Other,
        ),
        (
            "printf 'TERM_PROGRAM=WindowsTerminal'",
            VscodeDetection::Other,
        ),
        ("exit 1", VscodeDetection::Other),
        ("exit 2", VscodeDetection::Unknown),
    ] {
        assert_eq!(
            read_windows_vscode_detection_with_timeout(
                move || {
                    Command::new("/bin/sh")
                        .args(["-c", script])
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                        .ok()
                },
                Duration::from_secs(/*secs*/ 5),
            ),
            expected,
        );
    }
}

#[test]
fn failed_launch_is_unknown() {
    assert_eq!(
        read_windows_vscode_detection_with_timeout(|| None, Duration::from_secs(/*secs*/ 5)),
        VscodeDetection::Unknown,
    );
}

#[test]
fn times_out_and_reaps_running_probe() {
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let result = read_windows_vscode_detection_with_timeout(
        move || {
            let child = Command::new("sleep").arg("30").spawn().unwrap();
            sender.send(child.id()).unwrap();
            Some(child)
        },
        Duration::from_millis(/*millis*/ 100),
    );
    assert_eq!(result, VscodeDetection::Unknown);
    assert!(keyboard_enhancement_disabled_for(
        /*disable_env*/ None, /*is_wsl*/ true, result
    ));
    assert!(started.elapsed() < Duration::from_secs(/*secs*/ 2));
    assert_child_reaped(
        receiver
            .recv_timeout(Duration::from_secs(/*secs*/ 5))
            .unwrap(),
    );
}

#[test]
fn times_out_during_launch_and_reaps_child_when_launch_finishes() {
    let (release, blocked) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let started = Instant::now();
    let result = read_windows_vscode_detection_with_timeout(
        move || {
            // Model a WSL launch that cannot return a Child until interop recovers.
            blocked
                .recv_timeout(Duration::from_secs(/*secs*/ 5))
                .unwrap();
            let child = Command::new("sleep").arg("30").spawn().unwrap();
            sender.send(child.id()).unwrap();
            Some(child)
        },
        Duration::from_millis(/*millis*/ 100),
    );
    release.send(()).unwrap();
    assert_eq!(result, VscodeDetection::Unknown);
    assert!(keyboard_enhancement_disabled_for(
        /*disable_env*/ None, /*is_wsl*/ true, result
    ));
    assert!(started.elapsed() < Duration::from_secs(/*secs*/ 2));
    assert_child_reaped(
        receiver
            .recv_timeout(Duration::from_secs(/*secs*/ 5))
            .unwrap(),
    );
}

fn assert_child_reaped(pid: u32) {
    let started = Instant::now();
    loop {
        // SAFETY: signal 0 only checks whether this process still exists.
        if unsafe {
            libc::kill(pid as libc::pid_t, /*sig*/ 0)
        } == -1
        {
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ESRCH)
            );
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(/*secs*/ 5),
            "timed-out terminal probe was not reaped"
        );
        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
    }
}
