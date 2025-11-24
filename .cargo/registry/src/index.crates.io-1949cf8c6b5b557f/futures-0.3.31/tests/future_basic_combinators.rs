use futures::future::{self, FutureExt, TryFutureExt};
use futures_test::future::FutureTestExt;
use std::sync::mpsc;

#[test]
fn basic_future_combinators() {
    let (tx1, rx) = mpsc::channel();
    let tx2 = tx1.clone();
    let tx3 = tx1.clone();

    let fut = future::ready(1)
        .then(move |x| {
            tx1.send(x).unwrap(); // Send 1
            tx1.send(2).unwrap(); // Send 2
            future::ready(3)
        })
        .map(move |x| {
            tx2.send(x).unwrap(); // Send 3
            tx2.send(4).unwrap(); // Send 4
            5
        })
        .map(move |x| {
            tx3.send(x).unwrap(); // Send 5
        });

    assert!(rx.try_recv().is_err()); // Not started yet
    fut.run_in_background(); // Start it
    for i in 1..=5 {
        assert_eq!(rx.recv(), Ok(i));
    } // Check it
    assert!(rx.recv().is_err()); // Should be done
}

#[test]
fn basic_try_future_combinators() {
    let (tx1, rx) = mpsc::channel();
    let tx2 = tx1.clone();
    let tx3 = tx1.clone();
    let tx4 = tx1.clone();
    let tx5 = tx1.clone();
    let tx6 = tx1.clone();
    let tx7 = tx1.clone();
    let tx8 = tx1.clone();
    let tx9 = tx1.clone();
    let tx10 = tx1.clone();

    let fut = future::ready(Ok(1))
        .and_then(move |x: i32| {
            tx1.send(x).unwrap(); // Send 1
            tx1.send(2).unwrap(); // Send 2
            future::ready(Ok(3))
        })
        .or_else(move |x: i32| {
            tx2.send(x).unwrap(); // Should not run
            tx2.send(-1).unwrap();
            future::ready(Ok(-1))
        })
        .map_ok(move |x: i32| {
            tx3.send(x).unwrap(); // Send 3
            tx3.send(4).unwrap(); // Send 4
            5
        })
        .map_err(move |x: i32| {
            tx4.send(x).unwrap(); // Should not run
            tx4.send(-1).unwrap();
            -1
        })
        .map(move |x: Result<i32, i32>| {
            tx5.send(x.unwrap()).unwrap(); // Send 5
            tx5.send(6).unwrap(); // Send 6
            Err(7) // Now return errors!
        })
        .and_then(move |x: i32| {
            tx6.send(x).unwrap(); // Should not run
            tx6.send(-1).unwrap();
            future::ready(Err(-1))
        })
        .or_else(move |x: i32| {
            tx7.send(x).unwrap(); // Send 7
            tx7.send(8).unwrap(); // Send 8
            future::ready(Err(9))
        })
        .map_ok(move |x: i32| {
            tx8.send(x).unwrap(); // Should not run
            tx8.send(-1).unwrap();
            -1
        })
        .map_err(move |x: i32| {
            tx9.send(x).unwrap(); // Send 9
            tx9.send(10).unwrap(); // Send 10
            11
        })
        .map(move |x: Result<i32, i32>| {
            tx10.send(x.err().unwrap()).unwrap(); // Send 11
            tx10.send(12).unwrap(); // Send 12
        });

    assert!(rx.try_recv().is_err()); // Not started yet
    fut.run_in_background(); // Start it
    for i in 1..=12 {
        assert_eq!(rx.recv(), Ok(i));
    } // Check it
    assert!(rx.recv().is_err()); // Should be done
}
