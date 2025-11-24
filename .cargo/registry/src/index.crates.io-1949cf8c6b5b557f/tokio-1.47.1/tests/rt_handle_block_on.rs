#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(miri)))]

// All io tests that deal with shutdown is currently ignored because there are known bugs in with
// shutting down the io driver while concurrently registering new resources. See
// https://github.com/tokio-rs/tokio/pull/3569#pullrequestreview-612703467 for more details.
//
// When this has been fixed we want to re-enable these tests.

use std::time::Duration;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc;
#[cfg(not(target_os = "wasi"))]
use tokio::{net, time};

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support threads
macro_rules! multi_threaded_rt_test {
    ($($t:tt)*) => {
        mod threaded_scheduler_4_threads_only {
            use super::*;

            $($t)*

            fn rt() -> Runtime {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .unwrap()
            }
        }

        mod threaded_scheduler_1_thread_only {
            use super::*;

            $($t)*

            fn rt() -> Runtime {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .unwrap()
            }
        }
    }
}

#[cfg(not(target_os = "wasi"))]
macro_rules! rt_test {
    ($($t:tt)*) => {
        mod current_thread_scheduler {
            use super::*;

            $($t)*

            fn rt() -> Runtime {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
            }
        }

        mod threaded_scheduler_4_threads {
            use super::*;

            $($t)*

            fn rt() -> Runtime {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .unwrap()
            }
        }

        mod threaded_scheduler_1_thread {
            use super::*;

            $($t)*

            fn rt() -> Runtime {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .unwrap()
            }
        }
    }
}

// ==== runtime independent futures ======

#[test]
fn basic() {
    test_with_runtimes(|| {
        let one = Handle::current().block_on(async { 1 });
        assert_eq!(1, one);
    });
}

#[test]
fn bounded_mpsc_channel() {
    test_with_runtimes(|| {
        let (tx, mut rx) = mpsc::channel(1024);

        Handle::current().block_on(tx.send(42)).unwrap();

        let value = Handle::current().block_on(rx.recv()).unwrap();
        assert_eq!(value, 42);
    });
}

#[test]
fn unbounded_mpsc_channel() {
    test_with_runtimes(|| {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let _ = tx.send(42);

        let value = Handle::current().block_on(rx.recv()).unwrap();
        assert_eq!(value, 42);
    })
}

#[test]
fn yield_in_block_in_place() {
    test_with_runtimes(|| {
        Handle::current().block_on(async {
            tokio::task::block_in_place(|| {
                Handle::current().block_on(tokio::task::yield_now());
            });
        });
    })
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support file operations or bind
rt_test! {
    use tokio::fs;
    // ==== spawn blocking futures ======

    #[test]
    fn basic_fs() {
        let rt = rt();
        let _enter = rt.enter();

        let contents = Handle::current()
            .block_on(fs::read_to_string("Cargo.toml"))
            .unwrap();
        assert!(contents.contains("https://tokio.rs"));
    }

    #[test]
    fn fs_shutdown_before_started() {
        let rt = rt();
        let _enter = rt.enter();
        rt.shutdown_timeout(Duration::from_secs(1000));

        let err: std::io::Error = Handle::current()
            .block_on(fs::read_to_string("Cargo.toml"))
            .unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);

        let inner_err = err.get_ref().expect("no inner error");
        assert_eq!(inner_err.to_string(), "background task failed");
    }

    #[test]
    fn basic_spawn_blocking() {
        use tokio::task::spawn_blocking;
        let rt = rt();
        let _enter = rt.enter();

        let answer = Handle::current()
            .block_on(spawn_blocking(|| {
                std::thread::sleep(Duration::from_millis(100));
                42
            }))
            .unwrap();

        assert_eq!(answer, 42);
    }

    #[test]
    fn spawn_blocking_after_shutdown_fails() {
        use tokio::task::spawn_blocking;
        let rt = rt();
        let _enter = rt.enter();
        rt.shutdown_timeout(Duration::from_secs(1000));

        let join_err = Handle::current()
            .block_on(spawn_blocking(|| {
                std::thread::sleep(Duration::from_millis(100));
                42
            }))
            .unwrap_err();

        assert!(join_err.is_cancelled());
    }

    #[test]
    fn spawn_blocking_started_before_shutdown_continues() {
        use tokio::task::spawn_blocking;
        let rt = rt();
        let _enter = rt.enter();

        let handle = spawn_blocking(|| {
            std::thread::sleep(Duration::from_secs(1));
            42
        });

        rt.shutdown_timeout(Duration::from_secs(1000));

        let answer = Handle::current().block_on(handle).unwrap();

        assert_eq!(answer, 42);
    }

    #[test]
    fn thread_park_ok() {
        use core::task::Poll;
        let rt = rt();
        let mut exit = false;
        rt.handle().block_on(core::future::poll_fn(move |cx| {
            if exit {
                return Poll::Ready(());
            }
            cx.waker().wake_by_ref();
            // Verify that consuming the park token does not result in a hang.
            std::thread::current().unpark();
            std::thread::park();
            exit = true;
            Poll::Pending
        }));
    }

    // ==== net ======

    #[test]
    #[cfg_attr(miri, ignore)] // No `socket` in miri.
    fn tcp_listener_bind() {
        let rt = rt();
        let _enter = rt.enter();

        Handle::current()
            .block_on(net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
    }

    // All io tests are ignored for now. See above why that is.
    #[ignore]
    #[test]
    fn tcp_listener_connect_after_shutdown() {
        let rt = rt();
        let _enter = rt.enter();

        rt.shutdown_timeout(Duration::from_secs(1000));

        let err = Handle::current()
            .block_on(net::TcpListener::bind("127.0.0.1:0"))
            .unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            err.get_ref().unwrap().to_string(),
            "A Tokio 1.x context was found, but it is being shutdown.",
        );
    }

    // All io tests are ignored for now. See above why that is.
    #[ignore]
    #[test]
    fn tcp_listener_connect_before_shutdown() {
        let rt = rt();
        let _enter = rt.enter();

        let bind_future = net::TcpListener::bind("127.0.0.1:0");

        rt.shutdown_timeout(Duration::from_secs(1000));

        let err = Handle::current().block_on(bind_future).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            err.get_ref().unwrap().to_string(),
            "A Tokio 1.x context was found, but it is being shutdown.",
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)] // No `socket` in miri.
    fn udp_socket_bind() {
        let rt = rt();
        let _enter = rt.enter();

        Handle::current()
            .block_on(net::UdpSocket::bind("127.0.0.1:0"))
            .unwrap();
    }

    // All io tests are ignored for now. See above why that is.
    #[ignore]
    #[test]
    fn udp_stream_bind_after_shutdown() {
        let rt = rt();
        let _enter = rt.enter();

        rt.shutdown_timeout(Duration::from_secs(1000));

        let err = Handle::current()
            .block_on(net::UdpSocket::bind("127.0.0.1:0"))
            .unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            err.get_ref().unwrap().to_string(),
            "A Tokio 1.x context was found, but it is being shutdown.",
        );
    }

    // All io tests are ignored for now. See above why that is.
    #[ignore]
    #[test]
    fn udp_stream_bind_before_shutdown() {
        let rt = rt();
        let _enter = rt.enter();

        let bind_future = net::UdpSocket::bind("127.0.0.1:0");

        rt.shutdown_timeout(Duration::from_secs(1000));

        let err = Handle::current().block_on(bind_future).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            err.get_ref().unwrap().to_string(),
            "A Tokio 1.x context was found, but it is being shutdown.",
        );
    }

    // All io tests are ignored for now. See above why that is.
    #[ignore]
    #[cfg(unix)]
    #[test]
    fn unix_listener_bind_after_shutdown() {
        let rt = rt();
        let _enter = rt.enter();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("socket");

        rt.shutdown_timeout(Duration::from_secs(1000));

        let err = net::UnixListener::bind(path).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            err.get_ref().unwrap().to_string(),
            "A Tokio 1.x context was found, but it is being shutdown.",
        );
    }

    // All io tests are ignored for now. See above why that is.
    #[ignore]
    #[cfg(unix)]
    #[test]
    fn unix_listener_shutdown_after_bind() {
        let rt = rt();
        let _enter = rt.enter();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("socket");

        let listener = net::UnixListener::bind(path).unwrap();

        rt.shutdown_timeout(Duration::from_secs(1000));

        // this should not timeout but fail immediately since the runtime has been shutdown
        let err = Handle::current().block_on(listener.accept()).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(err.get_ref().unwrap().to_string(), "reactor gone");
    }

    // All io tests are ignored for now. See above why that is.
    #[ignore]
    #[cfg(unix)]
    #[test]
    fn unix_listener_shutdown_after_accept() {
        let rt = rt();
        let _enter = rt.enter();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("socket");

        let listener = net::UnixListener::bind(path).unwrap();

        let accept_future = listener.accept();

        rt.shutdown_timeout(Duration::from_secs(1000));

        // this should not timeout but fail immediately since the runtime has been shutdown
        let err = Handle::current().block_on(accept_future).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(err.get_ref().unwrap().to_string(), "reactor gone");
    }

    // ==== nesting ======

    #[test]
    #[should_panic(
        expected = "Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the thread is being used to drive asynchronous tasks."
    )]
    fn nesting() {
        fn some_non_async_function() -> i32 {
            Handle::current().block_on(time::sleep(Duration::from_millis(10)));
            1
        }

        let rt = rt();

        rt.block_on(async { some_non_async_function() });
    }

    #[test]
    fn spawn_after_runtime_dropped() {
        use futures::future::FutureExt;

        let rt = rt();

        let handle = rt.block_on(async move {
            Handle::current()
        });

        let jh1 = handle.spawn(futures::future::pending::<()>());

        drop(rt);

        let jh2 = handle.spawn(futures::future::pending::<()>());

        let err1 = jh1.now_or_never().unwrap().unwrap_err();
        let err2 = jh2.now_or_never().unwrap().unwrap_err();
        assert!(err1.is_cancelled());
        assert!(err2.is_cancelled());
    }
}

#[cfg(not(target_os = "wasi"))]
multi_threaded_rt_test! {
    #[cfg(unix)]
    #[cfg_attr(miri, ignore)] // No `socket` in miri.
    #[test]
    fn unix_listener_bind() {
        let rt = rt();
        let _enter = rt.enter();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("socket");

        let listener = net::UnixListener::bind(path).unwrap();

        // this should timeout and not fail immediately since the runtime has not been shutdown
        let _: tokio::time::error::Elapsed = Handle::current()
            .block_on(tokio::time::timeout(
                Duration::from_millis(10),
                listener.accept(),
            ))
            .unwrap_err();
    }

    // ==== timers ======

    // `Handle::block_on` doesn't work with timer futures on a current thread runtime as there is no
    // one to drive the timers so they will just hang forever. Therefore they are not tested.

    #[test]
    fn sleep() {
        let rt = rt();
        let _enter = rt.enter();

        Handle::current().block_on(time::sleep(Duration::from_millis(100)));
    }

    #[test]
    #[should_panic(expected = "A Tokio 1.x context was found, but it is being shutdown.")]
    fn sleep_before_shutdown_panics() {
        let rt = rt();
        let _enter = rt.enter();

        let f = time::sleep(Duration::from_millis(100));

        rt.shutdown_timeout(Duration::from_secs(1000));

        Handle::current().block_on(f);
    }

    #[test]
    #[should_panic(expected = "A Tokio 1.x context was found, but it is being shutdown.")]
    fn sleep_after_shutdown_panics() {
        let rt = rt();
        let _enter = rt.enter();

        rt.shutdown_timeout(Duration::from_secs(1000));

        Handle::current().block_on(time::sleep(Duration::from_millis(100)));
    }
}

// ==== utils ======

/// Create a new multi threaded runtime
#[cfg(not(target_os = "wasi"))]
fn new_multi_thread(n: usize) -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(n)
        .enable_all()
        .build()
        .unwrap()
}

/// Create a new single threaded runtime
fn new_current_thread() -> Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Utility to test things on both kinds of runtimes both before and after shutting it down.
fn test_with_runtimes<F>(f: F)
where
    F: Fn(),
{
    {
        let rt = new_current_thread();
        let _enter = rt.enter();
        f();

        rt.shutdown_timeout(Duration::from_secs(1000));
        f();
    }

    #[cfg(not(target_os = "wasi"))]
    {
        let rt = new_multi_thread(1);
        let _enter = rt.enter();
        f();

        rt.shutdown_timeout(Duration::from_secs(1000));
        f();
    }

    #[cfg(not(target_os = "wasi"))]
    {
        let rt = new_multi_thread(4);
        let _enter = rt.enter();
        f();

        rt.shutdown_timeout(Duration::from_secs(1000));
        f();
    }
}
