use futures::channel::{mpsc, oneshot};
use futures::executor::{block_on, block_on_stream};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::sync::mpsc as std_mpsc;
use std::thread;

#[test]
#[ignore] // FIXME: https://github.com/rust-lang/futures-rs/issues/1790
fn works() {
    const N: usize = 4;

    let (mut tx, rx) = mpsc::channel(1);

    let (tx2, rx2) = std_mpsc::channel();
    let (tx3, rx3) = std_mpsc::channel();
    let t1 = thread::spawn(move || {
        for _ in 0..=N {
            let (mytx, myrx) = oneshot::channel();
            block_on(tx.send(myrx)).unwrap();
            tx3.send(mytx).unwrap();
        }
        rx2.recv().unwrap();
        for _ in 0..N {
            let (mytx, myrx) = oneshot::channel();
            block_on(tx.send(myrx)).unwrap();
            tx3.send(mytx).unwrap();
        }
    });

    let (tx4, rx4) = std_mpsc::channel();
    let t2 = thread::spawn(move || {
        for item in block_on_stream(rx.buffer_unordered(N)) {
            tx4.send(item.unwrap()).unwrap();
        }
    });

    let o1 = rx3.recv().unwrap();
    let o2 = rx3.recv().unwrap();
    let o3 = rx3.recv().unwrap();
    let o4 = rx3.recv().unwrap();
    assert!(rx4.try_recv().is_err());

    o1.send(1).unwrap();
    assert_eq!(rx4.recv(), Ok(1));
    o3.send(3).unwrap();
    assert_eq!(rx4.recv(), Ok(3));
    tx2.send(()).unwrap();
    o2.send(2).unwrap();
    assert_eq!(rx4.recv(), Ok(2));
    o4.send(4).unwrap();
    assert_eq!(rx4.recv(), Ok(4));

    let o5 = rx3.recv().unwrap();
    let o6 = rx3.recv().unwrap();
    let o7 = rx3.recv().unwrap();
    let o8 = rx3.recv().unwrap();
    let o9 = rx3.recv().unwrap();

    o5.send(5).unwrap();
    assert_eq!(rx4.recv(), Ok(5));
    o8.send(8).unwrap();
    assert_eq!(rx4.recv(), Ok(8));
    o9.send(9).unwrap();
    assert_eq!(rx4.recv(), Ok(9));
    o7.send(7).unwrap();
    assert_eq!(rx4.recv(), Ok(7));
    o6.send(6).unwrap();
    assert_eq!(rx4.recv(), Ok(6));

    t1.join().unwrap();
    t2.join().unwrap();
}
