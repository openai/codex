extern crate wait_timeout;

use std::env;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use wait_timeout::ChildExt;

macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(e) => e,
            Err(e) => panic!("{} failed with {}", stringify!($e), e),
        }
    };
}

fn sleeper(ms: u32) -> Child {
    let mut me = env::current_exe().unwrap();
    me.pop();
    if me.ends_with("deps") {
        me.pop();
    }
    me.push("sleep");
    t!(Command::new(me).arg(ms.to_string()).spawn())
}

fn exit(code: u32) -> Child {
    let mut me = env::current_exe().unwrap();
    me.pop();
    if me.ends_with("deps") {
        me.pop();
    }
    me.push("exit");
    t!(Command::new(me).arg(code.to_string()).spawn())
}

fn reader() -> Child {
    let mut me = env::current_exe().unwrap();
    me.pop();
    if me.ends_with("deps") {
        me.pop();
    }
    me.push("reader");
    t!(Command::new(me).stdin(Stdio::piped()).spawn())
}

#[test]
fn smoke_insta_timeout() {
    let mut child = sleeper(1_000);
    assert_eq!(t!(child.wait_timeout_ms(0)), None);

    t!(child.kill());
    let status = t!(child.wait());
    assert!(!status.success());
}

#[test]
fn smoke_success() {
    let start = Instant::now();
    let mut child = sleeper(0);
    let status = t!(child.wait_timeout_ms(1_000)).expect("should have succeeded");
    assert!(status.success());

    assert!(start.elapsed() < Duration::from_millis(500));
}

#[test]
fn smoke_timeout() {
    let mut child = sleeper(1_000_000);
    let start = Instant::now();
    assert_eq!(t!(child.wait_timeout_ms(100)), None);
    assert!(start.elapsed() > Duration::from_millis(80));

    t!(child.kill());
    let status = t!(child.wait());
    assert!(!status.success());
}

#[test]
fn smoke_reader() {
    let mut child = reader();
    let dur = Duration::from_millis(100);
    let status = t!(child.wait_timeout(dur)).unwrap();
    assert!(status.success());
}

#[test]
fn exit_codes() {
    let mut child = exit(0);
    let status = t!(child.wait_timeout_ms(1_000)).unwrap();
    assert_eq!(status.code(), Some(0));

    let mut child = exit(1);
    let status = t!(child.wait_timeout_ms(1_000)).unwrap();
    assert_eq!(status.code(), Some(1));

    // check STILL_ACTIVE on windows, on unix this ends up just getting
    // truncated so don't bother with it.
    if cfg!(windows) {
        let mut child = exit(259);
        let status = t!(child.wait_timeout_ms(1_000)).unwrap();
        assert_eq!(status.code(), Some(259));
    }
}
