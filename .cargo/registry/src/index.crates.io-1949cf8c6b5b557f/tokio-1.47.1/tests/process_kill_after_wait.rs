#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi"), not(miri)))] // Wasi cannot run system commands

use tokio::process::Command;

#[tokio::test]
async fn kill_after_wait() {
    let mut cmd;

    if cfg!(windows) {
        cmd = Command::new("cmd");
        cmd.arg("/c");
    } else {
        cmd = Command::new("sh");
        cmd.arg("-c");
    }

    let mut child = cmd.arg("exit 2").spawn().unwrap();

    child.start_kill().unwrap();

    child.wait().await.unwrap();

    // Kill after `wait` is fine.
    child.start_kill().unwrap();
    child.kill().await.unwrap();
}
