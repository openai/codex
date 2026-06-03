use super::Child;
use super::CommandExt as _;
use crate::test_support;
use crate::test_support::STDERR_TEXT;
use crate::test_support::STDOUT_TEXT;
#[cfg(not(debug_assertions))]
use crate::test_support::UNJOINED_CHILD_MESSAGE;
use either::Either;
use std::io::Read;
use std::ops::DerefMut;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

#[test]
fn wait_disarms_bomb() {
    let child = test_support::command("exit-success")
        .spawn_managed()
        .expect("spawn helper");
    assert!(child.bomb.is_armed());

    let status = child.wait().expect("wait for helper");
    assert!(status.success());
}

#[test]
fn stdio_is_available_through_deref_mut() {
    let mut child = test_support::command("output")
        .stdout(Stdio::piped())
        .spawn_managed()
        .expect("spawn helper");
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut output = String::new();
    stdout.read_to_string(&mut output).expect("read stdout");

    assert!(child.wait().expect("wait for helper").success());
    assert!(output.contains(STDOUT_TEXT));
}

#[test]
fn try_wait_keeps_bomb_armed_until_status_is_available() {
    let child = test_support::command("sleep")
        .spawn_managed()
        .expect("spawn helper");

    let mut child = match child.try_wait().expect("poll sleeping helper") {
        Either::Left(status) => panic!("sleeping helper exited unexpectedly: {status}"),
        Either::Right(child) => child,
    };
    assert!(child.bomb.is_armed());

    child.kill().expect("kill sleeping helper");
    assert!(child.bomb.is_armed());
    assert!(!child.wait().expect("wait for killed helper").success());
}

#[test]
fn try_wait_disarms_bomb_when_status_is_available() {
    let mut child = test_support::command("exit-success")
        .spawn_managed()
        .expect("spawn helper");
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        child = match child.try_wait().expect("poll helper") {
            Either::Left(status) => {
                assert!(status.success());
                return;
            }
            Either::Right(child) => child,
        };
        assert!(child.bomb.is_armed());
        assert!(Instant::now() < deadline, "helper did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn wait_with_output_collects_output() {
    let output = test_support::command("output")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_managed()
        .expect("spawn helper")
        .wait_with_output()
        .expect("collect helper output");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(STDOUT_TEXT));
    assert!(String::from_utf8_lossy(&output.stderr).contains(STDERR_TEXT));
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "managed child process dropped without being joined")]
fn dropping_unjoined_child_panics() {
    let mut child = test_support::command("sleep")
        .spawn_managed()
        .expect("spawn helper");
    clean_up_without_disarming(&mut child);

    drop(child);
}

#[cfg(not(debug_assertions))]
#[test]
#[tracing_test::traced_test]
fn dropping_unjoined_child_logs_an_error() {
    let mut child = test_support::command("sleep")
        .spawn_managed()
        .expect("spawn helper");
    clean_up_without_disarming(&mut child);

    drop(child);

    assert!(logs_contain(UNJOINED_CHILD_MESSAGE));
}

fn clean_up_without_disarming(child: &mut Child) {
    let child = DerefMut::deref_mut(child);
    child.kill().expect("kill sleeping helper");
    child.wait().expect("reap sleeping helper");
}
