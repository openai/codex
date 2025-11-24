use std::future::Future;
use std::sync::Arc;
use std::task::Poll;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::PollSemaphore;

type SemRet = Option<OwnedSemaphorePermit>;

fn semaphore_poll(
    sem: &mut PollSemaphore,
) -> tokio_test::task::Spawn<impl Future<Output = SemRet> + '_> {
    let fut = std::future::poll_fn(move |cx| sem.poll_acquire(cx));
    tokio_test::task::spawn(fut)
}

fn semaphore_poll_many(
    sem: &mut PollSemaphore,
    permits: u32,
) -> tokio_test::task::Spawn<impl Future<Output = SemRet> + '_> {
    let fut = std::future::poll_fn(move |cx| sem.poll_acquire_many(cx, permits));
    tokio_test::task::spawn(fut)
}

#[tokio::test]
async fn it_works() {
    let sem = Arc::new(Semaphore::new(1));
    let mut poll_sem = PollSemaphore::new(sem.clone());

    let permit = sem.acquire().await.unwrap();
    let mut poll = semaphore_poll(&mut poll_sem);
    assert!(poll.poll().is_pending());
    drop(permit);

    assert!(matches!(poll.poll(), Poll::Ready(Some(_))));
    drop(poll);

    sem.close();

    assert!(semaphore_poll(&mut poll_sem).await.is_none());

    // Check that it is fused.
    assert!(semaphore_poll(&mut poll_sem).await.is_none());
    assert!(semaphore_poll(&mut poll_sem).await.is_none());
}

#[tokio::test]
async fn can_acquire_many_permits() {
    let sem = Arc::new(Semaphore::new(4));
    let mut poll_sem = PollSemaphore::new(sem.clone());

    let permit1 = semaphore_poll(&mut poll_sem).poll();
    assert!(matches!(permit1, Poll::Ready(Some(_))));

    let permit2 = semaphore_poll_many(&mut poll_sem, 2).poll();
    assert!(matches!(permit2, Poll::Ready(Some(_))));

    assert_eq!(sem.available_permits(), 1);

    drop(permit2);

    let mut permit4 = semaphore_poll_many(&mut poll_sem, 4);
    assert!(permit4.poll().is_pending());

    drop(permit1);

    let permit4 = permit4.poll();
    assert!(matches!(permit4, Poll::Ready(Some(_))));
    assert_eq!(sem.available_permits(), 0);
}

#[tokio::test]
async fn can_poll_different_amounts_of_permits() {
    let sem = Arc::new(Semaphore::new(4));
    let mut poll_sem = PollSemaphore::new(sem.clone());
    assert!(semaphore_poll_many(&mut poll_sem, 5).poll().is_pending());
    assert!(semaphore_poll_many(&mut poll_sem, 4).poll().is_ready());

    let permit = sem.acquire_many(4).await.unwrap();
    assert!(semaphore_poll_many(&mut poll_sem, 5).poll().is_pending());
    assert!(semaphore_poll_many(&mut poll_sem, 4).poll().is_pending());
    drop(permit);
    assert!(semaphore_poll_many(&mut poll_sem, 5).poll().is_pending());
    assert!(semaphore_poll_many(&mut poll_sem, 4).poll().is_ready());
}
