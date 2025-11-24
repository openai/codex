use crate::sync::mpsc::list;

use loom::thread;
use std::sync::Arc;

#[test]
fn smoke() {
    use crate::sync::mpsc::block::Read;

    const NUM_TX: usize = 2;
    const NUM_MSG: usize = 2;

    loom::model(|| {
        let (tx, mut rx) = list::channel();
        let tx = Arc::new(tx);

        for th in 0..NUM_TX {
            let tx = tx.clone();

            thread::spawn(move || {
                for i in 0..NUM_MSG {
                    tx.push((th, i));
                }
            });
        }

        let mut next = vec![0; NUM_TX];

        loop {
            match rx.pop(&tx) {
                Some(Read::Value((th, v))) => {
                    assert_eq!(v, next[th]);
                    next[th] += 1;

                    if next.iter().all(|&i| i == NUM_MSG) {
                        break;
                    }
                }
                Some(Read::Closed) => {
                    panic!();
                }
                None => {
                    thread::yield_now();
                }
            }
        }
    });
}
