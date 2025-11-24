use futures::channel::oneshot;
use futures::future::{FutureExt, TryFutureExt};
use futures_test::future::FutureTestExt;
use std::sync::mpsc;
use std::thread;

#[test]
fn oneshot_send1() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = mpsc::channel();

    let t = thread::spawn(|| tx1.send(1).unwrap());
    rx1.map_ok(move |x| tx2.send(x)).run_in_background();
    assert_eq!(1, rx2.recv().unwrap());
    t.join().unwrap();
}

#[test]
fn oneshot_send2() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = mpsc::channel();

    thread::spawn(|| tx1.send(1).unwrap()).join().unwrap();
    rx1.map_ok(move |x| tx2.send(x).unwrap()).run_in_background();
    assert_eq!(1, rx2.recv().unwrap());
}

#[test]
fn oneshot_send3() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = mpsc::channel();

    rx1.map_ok(move |x| tx2.send(x).unwrap()).run_in_background();
    thread::spawn(|| tx1.send(1).unwrap()).join().unwrap();
    assert_eq!(1, rx2.recv().unwrap());
}

#[test]
fn oneshot_drop_tx1() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = mpsc::channel();

    drop(tx1);
    rx1.map(move |result| tx2.send(result).unwrap()).run_in_background();

    assert_eq!(Err(oneshot::Canceled), rx2.recv().unwrap());
}

#[test]
fn oneshot_drop_tx2() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = mpsc::channel();

    let t = thread::spawn(|| drop(tx1));
    rx1.map(move |result| tx2.send(result).unwrap()).run_in_background();
    t.join().unwrap();

    assert_eq!(Err(oneshot::Canceled), rx2.recv().unwrap());
}

#[test]
fn oneshot_drop_rx() {
    let (tx, rx) = oneshot::channel::<i32>();
    drop(rx);
    assert_eq!(Err(2), tx.send(2));
}

#[test]
fn oneshot_debug() {
    let (tx, rx) = oneshot::channel::<i32>();
    assert_eq!(format!("{:?}", tx), "Sender { complete: false }");
    assert_eq!(format!("{:?}", rx), "Receiver { complete: false }");
    drop(rx);
    assert_eq!(format!("{:?}", tx), "Sender { complete: true }");
    let (tx, rx) = oneshot::channel::<i32>();
    drop(tx);
    assert_eq!(format!("{:?}", rx), "Receiver { complete: true }");
}
