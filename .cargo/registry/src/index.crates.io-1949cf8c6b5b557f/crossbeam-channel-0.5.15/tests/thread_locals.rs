//! Tests that make sure accessing thread-locals while exiting the thread doesn't cause panics.

#![cfg(not(miri))] // Miri detects that this test is buggy: the destructor of `FOO` uses `std::thread::current()`!

use std::thread;
use std::time::Duration;

use crossbeam_channel::{select, unbounded};
use crossbeam_utils::thread::scope;

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "TLS is destroyed too early on macOS")]
fn use_while_exiting() {
    struct Foo;

    impl Drop for Foo {
        fn drop(&mut self) {
            // A blocking operation after the thread-locals have been dropped. This will attempt to
            // use the thread-locals and must not panic.
            let (_s, r) = unbounded::<()>();
            select! {
                recv(r) -> _ => {}
                default(ms(100)) => {}
            }
        }
    }

    thread_local! {
        static FOO: Foo = const { Foo };
    }

    let (s, r) = unbounded::<()>();

    scope(|scope| {
        scope.spawn(|_| {
            // First initialize `FOO`, then the thread-locals related to crossbeam-channel.
            FOO.with(|_| ());
            r.recv().unwrap();
            // At thread exit, thread-locals related to crossbeam-channel get dropped first and
            // `FOO` is dropped last.
        });

        scope.spawn(|_| {
            thread::sleep(ms(100));
            s.send(()).unwrap();
        });
    })
    .unwrap();
}
