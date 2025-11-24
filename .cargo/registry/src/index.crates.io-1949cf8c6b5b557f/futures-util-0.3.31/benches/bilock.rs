#![feature(test)]
#![cfg(feature = "bilock")]

extern crate test;

use futures::task::Poll;
use futures_test::task::noop_context;
use futures_util::lock::BiLock;

use crate::test::Bencher;

#[bench]
fn contended(b: &mut Bencher) {
    let mut context = noop_context();

    b.iter(|| {
        let (x, y) = BiLock::new(1);

        for _ in 0..1000 {
            let x_guard = match x.poll_lock(&mut context) {
                Poll::Ready(guard) => guard,
                _ => panic!(),
            };

            // Try poll second lock while first lock still holds the lock
            match y.poll_lock(&mut context) {
                Poll::Pending => (),
                _ => panic!(),
            };

            drop(x_guard);

            let y_guard = match y.poll_lock(&mut context) {
                Poll::Ready(guard) => guard,
                _ => panic!(),
            };

            drop(y_guard);
        }
        (x, y)
    });
}

#[bench]
fn lock_unlock(b: &mut Bencher) {
    let mut context = noop_context();

    b.iter(|| {
        let (x, y) = BiLock::new(1);

        for _ in 0..1000 {
            let x_guard = match x.poll_lock(&mut context) {
                Poll::Ready(guard) => guard,
                _ => panic!(),
            };

            drop(x_guard);

            let y_guard = match y.poll_lock(&mut context) {
                Poll::Ready(guard) => guard,
                _ => panic!(),
            };

            drop(y_guard);
        }
        (x, y)
    })
}
