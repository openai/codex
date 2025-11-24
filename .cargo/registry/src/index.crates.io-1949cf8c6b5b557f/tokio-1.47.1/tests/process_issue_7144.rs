#![cfg(feature = "process")]
#![warn(rust_2018_idioms)]
#![cfg(target_os = "linux")]
#![cfg(not(miri))]

use tokio::process::Command;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn issue_7144() {
    let mut threads = vec![];
    for _ in 0..20 {
        threads.push(tokio::spawn(test_one()));
    }
    for thread in threads {
        thread.await.unwrap();
    }
}

async fn test_one() {
    let mut t = Command::new("strace")
        .args("-o /dev/null -D sleep 5".split(' '))
        .spawn()
        .unwrap();
    sleep(Duration::from_millis(100)).await;
    unsafe { libc::kill(t.id().unwrap() as _, libc::SIGINT) };
    t.wait().await.unwrap();
}
