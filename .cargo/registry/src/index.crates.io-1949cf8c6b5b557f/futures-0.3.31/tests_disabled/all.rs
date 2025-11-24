use futures::channel::oneshot::{self, Canceled};
use futures::executor::block_on;
use futures::future;
use std::sync::mpsc::{channel, TryRecvError};

// mod support;
// use support::*;

fn unselect<T, E, A, B>(r: Result<Either<(T, B), (T, A)>, Either<(E, B), (E, A)>>) -> Result<T, E> {
    match r {
        Ok(Either::Left((t, _))) | Ok(Either::Right((t, _))) => Ok(t),
        Err(Either::Left((e, _))) | Err(Either::Right((e, _))) => Err(e),
    }
}

#[test]
fn result_smoke() {
    fn is_future_v<A, B, C>(_: C)
    where
        A: Send + 'static,
        B: Send + 'static,
        C: Future<Item = A, Error = B>,
    {
    }

    is_future_v::<i32, u32, _>(f_ok(1).map(|a| a + 1));
    is_future_v::<i32, u32, _>(f_ok(1).map_err(|a| a + 1));
    is_future_v::<i32, u32, _>(f_ok(1).and_then(Ok));
    is_future_v::<i32, u32, _>(f_ok(1).or_else(Err));
    is_future_v::<(i32, i32), u32, _>(f_ok(1).join(Err(3)));
    is_future_v::<i32, u32, _>(f_ok(1).map(f_ok).flatten());

    assert_done(|| f_ok(1), r_ok(1));
    assert_done(|| f_err(1), r_err(1));
    assert_done(|| result(Ok(1)), r_ok(1));
    assert_done(|| result(Err(1)), r_err(1));
    assert_done(|| ok(1), r_ok(1));
    assert_done(|| err(1), r_err(1));
    assert_done(|| f_ok(1).map(|a| a + 2), r_ok(3));
    assert_done(|| f_err(1).map(|a| a + 2), r_err(1));
    assert_done(|| f_ok(1).map_err(|a| a + 2), r_ok(1));
    assert_done(|| f_err(1).map_err(|a| a + 2), r_err(3));
    assert_done(|| f_ok(1).and_then(|a| Ok(a + 2)), r_ok(3));
    assert_done(|| f_err(1).and_then(|a| Ok(a + 2)), r_err(1));
    assert_done(|| f_ok(1).and_then(|a| Err(a as u32 + 3)), r_err(4));
    assert_done(|| f_err(1).and_then(|a| Err(a as u32 + 4)), r_err(1));
    assert_done(|| f_ok(1).or_else(|a| Ok(a as i32 + 2)), r_ok(1));
    assert_done(|| f_err(1).or_else(|a| Ok(a as i32 + 2)), r_ok(3));
    assert_done(|| f_ok(1).or_else(|a| Err(a + 3)), r_ok(1));
    assert_done(|| f_err(1).or_else(|a| Err(a + 4)), r_err(5));
    assert_done(|| f_ok(1).select(f_err(2)).then(unselect), r_ok(1));
    assert_done(|| f_ok(1).select(Ok(2)).then(unselect), r_ok(1));
    assert_done(|| f_err(1).select(f_ok(1)).then(unselect), r_err(1));
    assert_done(|| f_ok(1).select(empty()).then(unselect), Ok(1));
    assert_done(|| empty().select(f_ok(1)).then(unselect), Ok(1));
    assert_done(|| f_ok(1).join(f_err(1)), Err(1));
    assert_done(|| f_ok(1).join(Ok(2)), Ok((1, 2)));
    assert_done(|| f_err(1).join(f_ok(1)), Err(1));
    assert_done(|| f_ok(1).then(|_| Ok(2)), r_ok(2));
    assert_done(|| f_ok(1).then(|_| Err(2)), r_err(2));
    assert_done(|| f_err(1).then(|_| Ok(2)), r_ok(2));
    assert_done(|| f_err(1).then(|_| Err(2)), r_err(2));
}

#[test]
fn test_empty() {
    fn empty() -> Empty<i32, u32> {
        future::empty()
    }

    assert_empty(|| empty());
    assert_empty(|| empty().select(empty()));
    assert_empty(|| empty().join(empty()));
    assert_empty(|| empty().join(f_ok(1)));
    assert_empty(|| f_ok(1).join(empty()));
    assert_empty(|| empty().or_else(move |_| empty()));
    assert_empty(|| empty().and_then(move |_| empty()));
    assert_empty(|| f_err(1).or_else(move |_| empty()));
    assert_empty(|| f_ok(1).and_then(move |_| empty()));
    assert_empty(|| empty().map(|a| a + 1));
    assert_empty(|| empty().map_err(|a| a + 1));
    assert_empty(|| empty().then(|a| a));
}

#[test]
fn test_ok() {
    assert_done(|| ok(1), r_ok(1));
    assert_done(|| err(1), r_err(1));
}

#[test]
fn flatten() {
    fn ok<T: Send + 'static>(a: T) -> FutureResult<T, u32> {
        future::ok(a)
    }
    fn err<E: Send + 'static>(b: E) -> FutureResult<i32, E> {
        future::err(b)
    }

    assert_done(|| ok(ok(1)).flatten(), r_ok(1));
    assert_done(|| ok(err(1)).flatten(), r_err(1));
    assert_done(|| err(1u32).map(ok).flatten(), r_err(1));
    assert_done(|| future::ok(future::ok(1)).flatten(), r_ok(1));
    assert_empty(|| ok(empty::<i32, u32>()).flatten());
    assert_empty(|| empty::<i32, u32>().map(ok).flatten());
}

#[test]
fn smoke_oneshot() {
    assert_done(
        || {
            let (c, p) = oneshot::channel();
            c.send(1).unwrap();
            p
        },
        Ok(1),
    );
    assert_done(
        || {
            let (c, p) = oneshot::channel::<i32>();
            drop(c);
            p
        },
        Err(Canceled),
    );
    let mut completes = Vec::new();
    assert_empty(|| {
        let (a, b) = oneshot::channel::<i32>();
        completes.push(a);
        b
    });

    let (c, mut p) = oneshot::channel::<i32>();
    drop(c);
    let res = panic_waker_lw(|lw| p.poll(lw));
    assert!(res.is_err());
    let (c, p) = oneshot::channel::<i32>();
    drop(c);
    let (tx, rx) = channel();
    p.then(move |_| tx.send(())).forget();
    rx.recv().unwrap();
}

#[test]
fn select_cancels() {
    let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
    let ((btx, brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| {
        btx.send(b).unwrap();
        b
    });
    let d = d.map(move |d| {
        dtx.send(d).unwrap();
        d
    });

    let mut f = b.select(d).then(unselect);
    // assert!(f.poll(&mut Task::new()).is_pending());
    assert!(brx.try_recv().is_err());
    assert!(drx.try_recv().is_err());
    a.send(1).unwrap();
    noop_waker_lw(|lw| {
        let res = f.poll(lw);
        assert!(res.ok().unwrap().is_ready());
        assert_eq!(brx.recv().unwrap(), 1);
        drop(c);
        assert!(drx.recv().is_err());

        let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let ((btx, _brx), (dtx, drx)) = (channel(), channel());
        let b = b.map(move |b| {
            btx.send(b).unwrap();
            b
        });
        let d = d.map(move |d| {
            dtx.send(d).unwrap();
            d
        });

        let mut f = b.select(d).then(unselect);
        assert!(f.poll(lw).ok().unwrap().is_pending());
        assert!(f.poll(lw).ok().unwrap().is_pending());
        a.send(1).unwrap();
        assert!(f.poll(lw).ok().unwrap().is_ready());
        drop((c, f));
        assert!(drx.recv().is_err());
    })
}

#[test]
fn join_cancels() {
    let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| {
        btx.send(b).unwrap();
        b
    });
    let d = d.map(move |d| {
        dtx.send(d).unwrap();
        d
    });

    let mut f = b.join(d);
    drop(a);
    let res = panic_waker_lw(|lw| f.poll(lw));
    assert!(res.is_err());
    drop(c);
    assert!(drx.recv().is_err());

    let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
    let ((btx, _brx), (dtx, drx)) = (channel(), channel());
    let b = b.map(move |b| {
        btx.send(b).unwrap();
        b
    });
    let d = d.map(move |d| {
        dtx.send(d).unwrap();
        d
    });

    let (tx, rx) = channel();
    let f = b.join(d);
    f.then(move |_| {
        tx.send(()).unwrap();
        let res: Result<(), ()> = Ok(());
        res
    })
    .forget();
    assert!(rx.try_recv().is_err());
    drop(a);
    rx.recv().unwrap();
    drop(c);
    assert!(drx.recv().is_err());
}

#[test]
fn join_incomplete() {
    let (a, b) = oneshot::channel::<i32>();
    let (tx, rx) = channel();
    noop_waker_lw(|lw| {
        let mut f = ok(1).join(b).map(move |r| tx.send(r).unwrap());
        assert!(f.poll(lw).ok().unwrap().is_pending());
        assert!(rx.try_recv().is_err());
        a.send(2).unwrap();
        assert!(f.poll(lw).ok().unwrap().is_ready());
        assert_eq!(rx.recv().unwrap(), (1, 2));

        let (a, b) = oneshot::channel::<i32>();
        let (tx, rx) = channel();
        let mut f = b.join(Ok(2)).map(move |r| tx.send(r).unwrap());
        assert!(f.poll(lw).ok().unwrap().is_pending());
        assert!(rx.try_recv().is_err());
        a.send(1).unwrap();
        assert!(f.poll(lw).ok().unwrap().is_ready());
        assert_eq!(rx.recv().unwrap(), (1, 2));

        let (a, b) = oneshot::channel::<i32>();
        let (tx, rx) = channel();
        let mut f = ok(1).join(b).map_err(move |_r| tx.send(2).unwrap());
        assert!(f.poll(lw).ok().unwrap().is_pending());
        assert!(rx.try_recv().is_err());
        drop(a);
        assert!(f.poll(lw).is_err());
        assert_eq!(rx.recv().unwrap(), 2);

        let (a, b) = oneshot::channel::<i32>();
        let (tx, rx) = channel();
        let mut f = b.join(Ok(2)).map_err(move |_r| tx.send(1).unwrap());
        assert!(f.poll(lw).ok().unwrap().is_pending());
        assert!(rx.try_recv().is_err());
        drop(a);
        assert!(f.poll(lw).is_err());
        assert_eq!(rx.recv().unwrap(), 1);
    })
}

#[test]
fn select2() {
    assert_done(|| f_ok(2).select(empty()).then(unselect), Ok(2));
    assert_done(|| empty().select(f_ok(2)).then(unselect), Ok(2));
    assert_done(|| f_err(2).select(empty()).then(unselect), Err(2));
    assert_done(|| empty().select(f_err(2)).then(unselect), Err(2));

    assert_done(
        || {
            f_ok(1).select(f_ok(2)).map_err(|_| 0).and_then(|either_tup| {
                let (a, b) = either_tup.into_inner();
                b.map(move |b| a + b)
            })
        },
        Ok(3),
    );

    // Finish one half of a select and then fail the second, ensuring that we
    // get the notification of the second one.
    {
        let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let f = b.select(d);
        let (tx, rx) = channel();
        f.map(move |r| tx.send(r).unwrap()).forget();
        a.send(1).unwrap();
        let (val, next) = rx.recv().unwrap().into_inner();
        assert_eq!(val, 1);
        let (tx, rx) = channel();
        next.map_err(move |_r| tx.send(2).unwrap()).forget();
        assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
        drop(c);
        assert_eq!(rx.recv().unwrap(), 2);
    }

    // Fail the second half and ensure that we see the first one finish
    {
        let ((a, b), (c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let f = b.select(d);
        let (tx, rx) = channel();
        f.map_err(move |r| tx.send((1, r.into_inner().1)).unwrap()).forget();
        drop(c);
        let (val, next) = rx.recv().unwrap();
        assert_eq!(val, 1);
        let (tx, rx) = channel();
        next.map(move |r| tx.send(r).unwrap()).forget();
        assert_eq!(rx.try_recv().err().unwrap(), TryRecvError::Empty);
        a.send(2).unwrap();
        assert_eq!(rx.recv().unwrap(), 2);
    }

    // Cancelling the first half should cancel the second
    {
        let ((_a, b), (_c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map(move |v| {
            btx.send(v).unwrap();
            v
        });
        let d = d.map(move |v| {
            dtx.send(v).unwrap();
            v
        });
        let f = b.select(d);
        drop(f);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
    }

    // Cancel after a schedule
    {
        let ((_a, b), (_c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map(move |v| {
            btx.send(v).unwrap();
            v
        });
        let d = d.map(move |v| {
            dtx.send(v).unwrap();
            v
        });
        let mut f = b.select(d);
        let _res = noop_waker_lw(|lw| f.poll(lw));
        drop(f);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
    }

    // Cancel propagates
    {
        let ((a, b), (_c, d)) = (oneshot::channel::<i32>(), oneshot::channel::<i32>());
        let ((btx, brx), (dtx, drx)) = (channel(), channel());
        let b = b.map(move |v| {
            btx.send(v).unwrap();
            v
        });
        let d = d.map(move |v| {
            dtx.send(v).unwrap();
            v
        });
        let (tx, rx) = channel();
        b.select(d).map(move |_| tx.send(()).unwrap()).forget();
        drop(a);
        assert!(drx.recv().is_err());
        assert!(brx.recv().is_err());
        assert!(rx.recv().is_err());
    }

    // Cancel on early drop
    {
        let (tx, rx) = channel();
        let f = f_ok(1).select(empty::<_, ()>().map(move |()| {
            tx.send(()).unwrap();
            1
        }));
        drop(f);
        assert!(rx.recv().is_err());
    }
}

#[test]
fn option() {
    assert_eq!(Ok(Some(())), block_on(Some(ok::<(), ()>(())).into_future()));
    assert_eq!(Ok::<_, ()>(None::<()>), block_on(None::<FutureResult<(), ()>>.into_future()));
}
