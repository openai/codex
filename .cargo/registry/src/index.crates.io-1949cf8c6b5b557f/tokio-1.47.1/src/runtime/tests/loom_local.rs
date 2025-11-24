use crate::runtime::tests::loom_oneshot as oneshot;
use crate::runtime::Builder;
use crate::task::LocalSet;

use std::task::Poll;

/// Waking a runtime will attempt to push a task into a queue of notifications
/// in the runtime, however the tasks in such a queue usually have a reference
/// to the runtime itself. This means that if they are not properly removed at
/// runtime shutdown, this will cause a memory leak.
///
/// This test verifies that waking something during shutdown of a `LocalSet` does
/// not result in tasks lingering in the queue once shutdown is complete. This
/// is verified using loom's leak finder.
#[test]
fn wake_during_shutdown() {
    loom::model(|| {
        let rt = Builder::new_current_thread().build().unwrap();
        let ls = LocalSet::new();

        let (send, recv) = oneshot::channel();

        ls.spawn_local(async move {
            let mut send = Some(send);

            let () = std::future::poll_fn(|cx| {
                if let Some(send) = send.take() {
                    send.send(cx.waker().clone());
                }

                Poll::Pending
            })
            .await;
        });

        let handle = loom::thread::spawn(move || {
            let waker = recv.recv();
            waker.wake();
        });

        ls.block_on(&rt, crate::task::yield_now());

        drop(ls);
        handle.join().unwrap();
        drop(rt);
    });
}
