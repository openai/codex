#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(unix)]
#![cfg(not(miri))]

use futures::future::join_all;
use std::process::Stdio;
use tokio::process::Command;
use tokio::task;

#[tokio::test]
async fn issue_42() {
    // We spawn a many batches of processes which should exit at roughly the
    // same time (modulo OS scheduling delays), to make sure that consuming
    // a readiness event for one process doesn't inadvertently starve another.
    // We then do this many times (in parallel) in an effort to stress test the
    // implementation to ensure there are no race conditions.
    // See alexcrichton/tokio-process#42 for background
    let join_handles = (0..10usize).map(|_| {
        task::spawn(async {
            let processes = (0..10usize).map(|i| {
                let mut child = Command::new("echo")
                    .arg(format!("I am spawned process #{i}"))
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .unwrap();

                async move { child.wait().await }
            });

            join_all(processes).await;
        })
    });

    join_all(join_handles).await;
}
