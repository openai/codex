use futures::executor::block_on;
use futures::future::{join_all, ready, Future, JoinAll};
use futures::pin_mut;
use std::fmt::Debug;

#[track_caller]
fn assert_done<T>(actual_fut: impl Future<Output = T>, expected: T)
where
    T: PartialEq + Debug,
{
    pin_mut!(actual_fut);
    let output = block_on(actual_fut);
    assert_eq!(output, expected);
}

#[test]
fn collect_collects() {
    assert_done(join_all(vec![ready(1), ready(2)]), vec![1, 2]);
    assert_done(join_all(vec![ready(1)]), vec![1]);
    // REVIEW: should this be implemented?
    // assert_done(join_all(Vec::<i32>::new()), vec![]);

    // TODO: needs more tests
}

#[test]
fn join_all_iter_lifetime() {
    // In futures-rs version 0.1, this function would fail to typecheck due to an overly
    // conservative type parameterization of `JoinAll`.
    fn sizes(bufs: Vec<&[u8]>) -> impl Future<Output = Vec<usize>> {
        let iter = bufs.into_iter().map(|b| ready::<usize>(b.len()));
        join_all(iter)
    }

    assert_done(sizes(vec![&[1, 2, 3], &[], &[0]]), vec![3_usize, 0, 1]);
}

#[test]
fn join_all_from_iter() {
    assert_done(vec![ready(1), ready(2)].into_iter().collect::<JoinAll<_>>(), vec![1, 2])
}
