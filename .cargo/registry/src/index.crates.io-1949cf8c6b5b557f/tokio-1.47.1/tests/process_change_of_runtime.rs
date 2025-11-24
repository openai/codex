#![cfg(feature = "process")]
#![warn(rust_2018_idioms)]
// This tests test the behavior of `process::Command::spawn` when it is used
// outside runtime, and when `process::Child::wait ` is used in a different
// runtime from which `process::Command::spawn` is used.
#![cfg(all(unix, not(target_os = "freebsd"), not(miri)))]

use std::process::Stdio;
use tokio::{process::Command, runtime::Runtime};

#[test]
fn process_spawned_and_wait_in_different_runtime() {
    let mut child = Runtime::new().unwrap().block_on(async {
        Command::new("true")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap()
    });
    Runtime::new().unwrap().block_on(async {
        let _ = child.wait().await.unwrap();
    });
}

#[test]
#[should_panic(
    expected = "there is no reactor running, must be called from the context of a Tokio 1.x runtime"
)]
fn process_spawned_outside_runtime() {
    let _ = Command::new("true")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn();
}
