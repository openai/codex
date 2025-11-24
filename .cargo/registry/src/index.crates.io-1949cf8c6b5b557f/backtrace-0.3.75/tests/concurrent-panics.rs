use std::env;
use std::panic;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Arc;
use std::thread;

const PANICS: usize = 100;
const THREADS: usize = 8;
const VAR: &str = "__THE_TEST_YOU_ARE_LUKE";

mod common;

fn main() {
    // If we cannot re-exec this test, there's no point in trying to do it.
    if common::cannot_reexec_the_test() {
        println!("test result: ok");
        return;
    }

    if env::var(VAR).is_err() {
        parent();
    } else {
        child();
    }
}

fn parent() {
    let me = env::current_exe().unwrap();
    let result = Command::new(&me)
        .env("RUST_BACKTRACE", "1")
        .env(VAR, "1")
        .output()
        .unwrap();
    if result.status.success() {
        println!("test result: ok");
        return;
    }
    println!("stdout:\n{}", String::from_utf8_lossy(&result.stdout));
    println!("stderr:\n{}", String::from_utf8_lossy(&result.stderr));
    println!("code: {}", result.status);
    panic!();
}

fn child() {
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();
    let a = thread::spawn(move || loop {
        if done2.load(SeqCst) {
            break format!("{:?}", backtrace::Backtrace::new());
        }
    });

    let threads = (0..THREADS)
        .map(|_| {
            thread::spawn(|| {
                for _ in 0..PANICS {
                    assert!(panic::catch_unwind(|| {
                        panic!();
                    })
                    .is_err());
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }

    done.store(true, SeqCst);
    a.join().unwrap();
}
