use easy_parallel::Parallel;
use futures_lite::{future, prelude::*};
use piper::pipe;

use std::task::{Context, Poll};
use std::thread::sleep;
use std::time::Duration;

#[test]
fn smoke() {
    let (mut r, mut w) = pipe(8);
    let mut buf = [0u8; 8];

    future::block_on(w.write_all(&[1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
    future::block_on(r.read_exact(&mut buf)).unwrap();

    assert_eq!(buf, [1, 2, 3, 4, 5, 6, 7, 8]);

    future::block_on(w.write_all(&[9, 10, 11, 12, 13, 14, 15, 16])).unwrap();
    future::block_on(r.read_exact(&mut buf)).unwrap();

    assert_eq!(buf, [9, 10, 11, 12, 13, 14, 15, 16]);

    drop(w);
    assert_eq!(future::block_on(r.read(&mut buf)).ok(), Some(0));
}

#[test]
fn read() {
    let (mut r, mut w) = pipe(100);
    let ms = Duration::from_micros;

    Parallel::new()
        .add(move || {
            let mut buf = [0u8; 3];
            sleep(ms(1000));
            future::block_on(r.read_exact(&mut buf)).unwrap();
            assert_eq!(buf, [1, 2, 3]);

            sleep(ms(1000));
            future::block_on(r.read_exact(&mut buf)).unwrap();
            assert_eq!(buf, [4, 5, 6]);
        })
        .add(move || {
            sleep(ms(1500));
            future::block_on(w.write_all(&[1, 2, 3, 4, 5, 6])).unwrap();
        })
        .run();
}

#[should_panic]
#[test]
fn zero_cap_pipe() {
    let _ = pipe(0);
}

#[should_panic]
#[test]
fn large_pipe() {
    let _ = pipe(core::usize::MAX);
}

#[test]
fn dropping_reader_wakes_writer() {
    let (r, mut w) = pipe(1);

    Parallel::new()
        .add(move || {
            sleep(Duration::from_millis(100));
            drop(r);
        })
        .add(move || {
            future::block_on(w.write_all(&[0u8])).unwrap();
            sleep(Duration::from_millis(200));
            with_cx(|cx| {
                assert_eq!(w.poll_fill_bytes(cx, &[0u8]), Poll::Ready(0));
                assert!(w.is_closed());
            });
        })
        .run();
}

#[test]
fn dropping_writer_wakes_reader() {
    let (mut r, w) = pipe(1);

    Parallel::new()
        .add(move || {
            drop(w);
        })
        .add(move || {
            sleep(Duration::from_millis(100));
            with_cx(|cx| {
                assert_eq!(r.poll_drain_bytes(cx, &mut [0u8]), Poll::Ready(0));
                assert!(r.is_closed());
            });
        })
        .run();
}

#[test]
fn len() {
    let (mut r, mut w) = pipe(100);

    assert_eq!(r.len(), 0);
    assert_eq!(w.len(), 0);
    assert!(r.is_empty());
    assert!(w.is_empty());
    assert!(!r.is_full());
    assert!(!w.is_full());

    let buf = [0u8; 10];
    future::block_on(w.write_all(&buf)).unwrap();

    assert_eq!(r.len(), 10);
    assert_eq!(w.len(), 10);
    assert!(!r.is_empty());
    assert!(!w.is_empty());
    assert!(!r.is_full());
    assert!(!w.is_full());

    // Fill up the pipe
    let buf = [0u8; 90];
    future::block_on(w.write_all(&buf)).unwrap();

    assert_eq!(r.len(), 100);
    assert_eq!(w.len(), 100);
    assert!(!r.is_empty());
    assert!(!w.is_empty());
    assert!(r.is_full());
    assert!(w.is_full());

    // Read some bytes.
    let mut buf = [0u8; 15];
    future::block_on(r.read_exact(&mut buf)).unwrap();

    assert_eq!(r.len(), 85);
    assert_eq!(w.len(), 85);
    assert!(!r.is_empty());
    assert!(!w.is_empty());
    assert!(!r.is_full());
    assert!(!w.is_full());

    // Write some more to loop around the capacity.
    let buf = [0u8; 10];
    future::block_on(w.write_all(&buf)).unwrap();

    assert_eq!(r.len(), 95);
    assert_eq!(w.len(), 95);
    assert!(!r.is_empty());
    assert!(!w.is_empty());
    assert!(!r.is_full());
    assert!(!w.is_full());
}

fn with_cx<R, F: FnOnce(&mut Context<'_>) -> R>(f: F) -> R {
    let mut f = Some(f);
    future::block_on(future::poll_fn(|cx| Poll::Ready((f.take().unwrap())(cx))))
}
