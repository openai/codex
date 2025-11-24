use futures::channel::oneshot;
use futures::executor::ThreadPool;
use futures::future::{self, ok, Future, FutureExt, TryFutureExt};
use futures::task::SpawnExt;
use std::sync::mpsc;
use std::thread;

fn run<F: Future + Send + 'static>(future: F) {
    let tp = ThreadPool::new().unwrap();
    tp.spawn(future.map(drop)).unwrap();
}

#[test]
fn join1() {
    let (tx, rx) = mpsc::channel();
    run(future::try_join(ok::<i32, i32>(1), ok(2)).map_ok(move |v| tx.send(v).unwrap()));
    assert_eq!(rx.recv(), Ok((1, 2)));
    assert!(rx.recv().is_err());

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}

#[test]
fn join2() {
    let (c1, p1) = oneshot::channel::<i32>();
    let (c2, p2) = oneshot::channel::<i32>();
    let (tx, rx) = mpsc::channel();
    run(future::try_join(p1, p2).map_ok(move |v| tx.send(v).unwrap()));
    assert!(rx.try_recv().is_err());
    c1.send(1).unwrap();
    assert!(rx.try_recv().is_err());
    c2.send(2).unwrap();
    assert_eq!(rx.recv(), Ok((1, 2)));
    assert!(rx.recv().is_err());

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}

#[test]
fn join3() {
    let (c1, p1) = oneshot::channel::<i32>();
    let (c2, p2) = oneshot::channel::<i32>();
    let (tx, rx) = mpsc::channel();
    run(future::try_join(p1, p2).map_err(move |_v| tx.send(1).unwrap()));
    assert!(rx.try_recv().is_err());
    drop(c1);
    assert_eq!(rx.recv(), Ok(1));
    assert!(rx.recv().is_err());
    drop(c2);

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}

#[test]
fn join4() {
    let (c1, p1) = oneshot::channel::<i32>();
    let (c2, p2) = oneshot::channel::<i32>();
    let (tx, rx) = mpsc::channel();
    run(future::try_join(p1, p2).map_err(move |v| tx.send(v).unwrap()));
    assert!(rx.try_recv().is_err());
    drop(c1);
    assert!(rx.recv().is_ok());
    drop(c2);
    assert!(rx.recv().is_err());

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}

#[test]
fn join5() {
    let (c1, p1) = oneshot::channel::<i32>();
    let (c2, p2) = oneshot::channel::<i32>();
    let (c3, p3) = oneshot::channel::<i32>();
    let (tx, rx) = mpsc::channel();
    run(future::try_join(future::try_join(p1, p2), p3).map_ok(move |v| tx.send(v).unwrap()));
    assert!(rx.try_recv().is_err());
    c1.send(1).unwrap();
    assert!(rx.try_recv().is_err());
    c2.send(2).unwrap();
    assert!(rx.try_recv().is_err());
    c3.send(3).unwrap();
    assert_eq!(rx.recv(), Ok(((1, 2), 3)));
    assert!(rx.recv().is_err());

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}

#[test]
fn select1() {
    let (c1, p1) = oneshot::channel::<i32>();
    let (c2, p2) = oneshot::channel::<i32>();
    let (tx, rx) = mpsc::channel();
    run(future::try_select(p1, p2).map_ok(move |v| tx.send(v).unwrap()));
    assert!(rx.try_recv().is_err());
    c1.send(1).unwrap();
    let (v, p2) = rx.recv().unwrap().into_inner();
    assert_eq!(v, 1);
    assert!(rx.recv().is_err());

    let (tx, rx) = mpsc::channel();
    run(p2.map_ok(move |v| tx.send(v).unwrap()));
    c2.send(2).unwrap();
    assert_eq!(rx.recv(), Ok(2));
    assert!(rx.recv().is_err());

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}

#[test]
fn select2() {
    let (c1, p1) = oneshot::channel::<i32>();
    let (c2, p2) = oneshot::channel::<i32>();
    let (tx, rx) = mpsc::channel();
    run(future::try_select(p1, p2).map_err(move |v| tx.send((1, v.into_inner().1)).unwrap()));
    assert!(rx.try_recv().is_err());
    drop(c1);
    let (v, p2) = rx.recv().unwrap();
    assert_eq!(v, 1);
    assert!(rx.recv().is_err());

    let (tx, rx) = mpsc::channel();
    run(p2.map_ok(move |v| tx.send(v).unwrap()));
    c2.send(2).unwrap();
    assert_eq!(rx.recv(), Ok(2));
    assert!(rx.recv().is_err());

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}

#[test]
fn select3() {
    let (c1, p1) = oneshot::channel::<i32>();
    let (c2, p2) = oneshot::channel::<i32>();
    let (tx, rx) = mpsc::channel();
    run(future::try_select(p1, p2).map_err(move |v| tx.send((1, v.into_inner().1)).unwrap()));
    assert!(rx.try_recv().is_err());
    drop(c1);
    let (v, p2) = rx.recv().unwrap();
    assert_eq!(v, 1);
    assert!(rx.recv().is_err());

    let (tx, rx) = mpsc::channel();
    run(p2.map_err(move |_v| tx.send(2).unwrap()));
    drop(c2);
    assert_eq!(rx.recv(), Ok(2));
    assert!(rx.recv().is_err());

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}

#[test]
fn select4() {
    const N: usize = if cfg!(miri) { 100 } else { 10000 };

    let (tx, rx) = mpsc::channel::<oneshot::Sender<i32>>();

    let t = thread::spawn(move || {
        for c in rx {
            c.send(1).unwrap();
        }
    });

    let (tx2, rx2) = mpsc::channel();
    for _ in 0..N {
        let (c1, p1) = oneshot::channel::<i32>();
        let (c2, p2) = oneshot::channel::<i32>();

        let tx3 = tx2.clone();
        run(future::try_select(p1, p2).map_ok(move |_| tx3.send(()).unwrap()));
        tx.send(c1).unwrap();
        rx2.recv().unwrap();
        drop(c2);
    }
    drop(tx);

    t.join().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
}
