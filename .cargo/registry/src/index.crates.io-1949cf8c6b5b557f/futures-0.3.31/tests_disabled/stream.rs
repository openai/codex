use futures::channel::mpsc;
use futures::channel::oneshot;
use futures::executor::{block_on, block_on_stream};
use futures::future::{err, ok};
use futures::stream::{empty, iter_ok, poll_fn, Peekable};

// mod support;
// use support::*;

pub struct Iter<I> {
    iter: I,
}

pub fn iter<J, T, E>(i: J) -> Iter<J::IntoIter>
where
    J: IntoIterator<Item = Result<T, E>>,
{
    Iter { iter: i.into_iter() }
}

impl<I, T, E> Stream for Iter<I>
where
    I: Iterator<Item = Result<T, E>>,
{
    type Item = T;
    type Error = E;

    fn poll_next(&mut self, _: &mut Context<'_>) -> Poll<Option<T>, E> {
        match self.iter.next() {
            Some(Ok(e)) => Ok(Poll::Ready(Some(e))),
            Some(Err(e)) => Err(e),
            None => Ok(Poll::Ready(None)),
        }
    }
}

fn list() -> Box<Stream<Item = i32, Error = u32> + Send> {
    let (tx, rx) = mpsc::channel(1);
    tx.send(Ok(1)).and_then(|tx| tx.send(Ok(2))).and_then(|tx| tx.send(Ok(3))).forget();
    Box::new(rx.then(|r| r.unwrap()))
}

fn err_list() -> Box<Stream<Item = i32, Error = u32> + Send> {
    let (tx, rx) = mpsc::channel(1);
    tx.send(Ok(1)).and_then(|tx| tx.send(Ok(2))).and_then(|tx| tx.send(Err(3))).forget();
    Box::new(rx.then(|r| r.unwrap()))
}

#[test]
fn map() {
    assert_done(|| list().map(|a| a + 1).collect(), Ok(vec![2, 3, 4]));
}

#[test]
fn map_err() {
    assert_done(|| err_list().map_err(|a| a + 1).collect::<Vec<_>>(), Err(4));
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct FromErrTest(u32);

impl From<u32> for FromErrTest {
    fn from(i: u32) -> Self {
        Self(i)
    }
}

#[test]
fn from_err() {
    assert_done(|| err_list().err_into().collect::<Vec<_>>(), Err(FromErrTest(3)));
}

#[test]
fn fold() {
    assert_done(|| list().fold(0, |a, b| ok::<i32, u32>(a + b)), Ok(6));
    assert_done(|| err_list().fold(0, |a, b| ok::<i32, u32>(a + b)), Err(3));
}

#[test]
fn filter() {
    assert_done(|| list().filter(|a| ok(*a % 2 == 0)).collect(), Ok(vec![2]));
}

#[test]
fn filter_map() {
    assert_done(
        || list().filter_map(|x| ok(if x % 2 == 0 { Some(x + 10) } else { None })).collect(),
        Ok(vec![12]),
    );
}

#[test]
fn and_then() {
    assert_done(|| list().and_then(|a| Ok(a + 1)).collect(), Ok(vec![2, 3, 4]));
    assert_done(|| list().and_then(|a| err::<i32, u32>(a as u32)).collect::<Vec<_>>(), Err(1));
}

#[test]
fn then() {
    assert_done(|| list().then(|a| a.map(|e| e + 1)).collect(), Ok(vec![2, 3, 4]));
}

#[test]
fn or_else() {
    assert_done(|| err_list().or_else(|a| ok::<i32, u32>(a as i32)).collect(), Ok(vec![1, 2, 3]));
}

#[test]
fn flatten() {
    assert_done(|| list().map(|_| list()).flatten().collect(), Ok(vec![1, 2, 3, 1, 2, 3, 1, 2, 3]));
}

#[test]
fn skip() {
    assert_done(|| list().skip(2).collect(), Ok(vec![3]));
}

#[test]
fn skip_passes_errors_through() {
    let mut s = block_on_stream(iter(vec![Err(1), Err(2), Ok(3), Ok(4), Ok(5)]).skip(1));
    assert_eq!(s.next(), Some(Err(1)));
    assert_eq!(s.next(), Some(Err(2)));
    assert_eq!(s.next(), Some(Ok(4)));
    assert_eq!(s.next(), Some(Ok(5)));
    assert_eq!(s.next(), None);
}

#[test]
fn skip_while() {
    assert_done(|| list().skip_while(|e| Ok(*e % 2 == 1)).collect(), Ok(vec![2, 3]));
}
#[test]
fn take() {
    assert_done(|| list().take(2).collect(), Ok(vec![1, 2]));
}

#[test]
fn take_while() {
    assert_done(|| list().take_while(|e| Ok(*e < 3)).collect(), Ok(vec![1, 2]));
}

#[test]
fn take_passes_errors_through() {
    let mut s = block_on_stream(iter(vec![Err(1), Err(2), Ok(3), Ok(4), Err(4)]).take(1));
    assert_eq!(s.next(), Some(Err(1)));
    assert_eq!(s.next(), Some(Err(2)));
    assert_eq!(s.next(), Some(Ok(3)));
    assert_eq!(s.next(), None);

    let mut s = block_on_stream(iter(vec![Ok(1), Err(2)]).take(1));
    assert_eq!(s.next(), Some(Ok(1)));
    assert_eq!(s.next(), None);
}

#[test]
fn peekable() {
    assert_done(|| list().peekable().collect(), Ok(vec![1, 2, 3]));
}

#[test]
fn fuse() {
    let mut stream = block_on_stream(list().fuse());
    assert_eq!(stream.next(), Some(Ok(1)));
    assert_eq!(stream.next(), Some(Ok(2)));
    assert_eq!(stream.next(), Some(Ok(3)));
    assert_eq!(stream.next(), None);
    assert_eq!(stream.next(), None);
    assert_eq!(stream.next(), None);
}

#[test]
fn buffered() {
    let (tx, rx) = mpsc::channel(1);
    let (a, b) = oneshot::channel::<u32>();
    let (c, d) = oneshot::channel::<u32>();

    tx.send(Box::new(b.recover(|_| panic!())) as Box<Future<Item = _, Error = _> + Send>)
        .and_then(|tx| tx.send(Box::new(d.map_err(|_| panic!()))))
        .forget();

    let mut rx = rx.buffered(2);
    sassert_empty(&mut rx);
    c.send(3).unwrap();
    sassert_empty(&mut rx);
    a.send(5).unwrap();
    let mut rx = block_on_stream(rx);
    assert_eq!(rx.next(), Some(Ok(5)));
    assert_eq!(rx.next(), Some(Ok(3)));
    assert_eq!(rx.next(), None);

    let (tx, rx) = mpsc::channel(1);
    let (a, b) = oneshot::channel::<u32>();
    let (c, d) = oneshot::channel::<u32>();

    tx.send(Box::new(b.recover(|_| panic!())) as Box<Future<Item = _, Error = _> + Send>)
        .and_then(|tx| tx.send(Box::new(d.map_err(|_| panic!()))))
        .forget();

    let mut rx = rx.buffered(1);
    sassert_empty(&mut rx);
    c.send(3).unwrap();
    sassert_empty(&mut rx);
    a.send(5).unwrap();
    let mut rx = block_on_stream(rx);
    assert_eq!(rx.next(), Some(Ok(5)));
    assert_eq!(rx.next(), Some(Ok(3)));
    assert_eq!(rx.next(), None);
}

#[test]
fn unordered() {
    let (tx, rx) = mpsc::channel(1);
    let (a, b) = oneshot::channel::<u32>();
    let (c, d) = oneshot::channel::<u32>();

    tx.send(Box::new(b.recover(|_| panic!())) as Box<Future<Item = _, Error = _> + Send>)
        .and_then(|tx| tx.send(Box::new(d.recover(|_| panic!()))))
        .forget();

    let mut rx = rx.buffer_unordered(2);
    sassert_empty(&mut rx);
    let mut rx = block_on_stream(rx);
    c.send(3).unwrap();
    assert_eq!(rx.next(), Some(Ok(3)));
    a.send(5).unwrap();
    assert_eq!(rx.next(), Some(Ok(5)));
    assert_eq!(rx.next(), None);

    let (tx, rx) = mpsc::channel(1);
    let (a, b) = oneshot::channel::<u32>();
    let (c, d) = oneshot::channel::<u32>();

    tx.send(Box::new(b.recover(|_| panic!())) as Box<Future<Item = _, Error = _> + Send>)
        .and_then(|tx| tx.send(Box::new(d.recover(|_| panic!()))))
        .forget();

    // We don't even get to see `c` until `a` completes.
    let mut rx = rx.buffer_unordered(1);
    sassert_empty(&mut rx);
    c.send(3).unwrap();
    sassert_empty(&mut rx);
    a.send(5).unwrap();
    let mut rx = block_on_stream(rx);
    assert_eq!(rx.next(), Some(Ok(5)));
    assert_eq!(rx.next(), Some(Ok(3)));
    assert_eq!(rx.next(), None);
}

#[test]
fn zip() {
    assert_done(|| list().zip(list()).collect(), Ok(vec![(1, 1), (2, 2), (3, 3)]));
    assert_done(|| list().zip(list().take(2)).collect(), Ok(vec![(1, 1), (2, 2)]));
    assert_done(|| list().take(2).zip(list()).collect(), Ok(vec![(1, 1), (2, 2)]));
    assert_done(|| err_list().zip(list()).collect::<Vec<_>>(), Err(3));
    assert_done(|| list().zip(list().map(|x| x + 1)).collect(), Ok(vec![(1, 2), (2, 3), (3, 4)]));
}

#[test]
fn peek() {
    struct Peek {
        inner: Peekable<Box<Stream<Item = i32, Error = u32> + Send>>,
    }

    impl Future for Peek {
        type Item = ();
        type Error = u32;

        fn poll(&mut self, cx: &mut Context<'_>) -> Poll<(), u32> {
            {
                let res = ready!(self.inner.peek(cx))?;
                assert_eq!(res, Some(&1));
            }
            assert_eq!(self.inner.peek(cx).unwrap(), Some(&1).into());
            assert_eq!(self.inner.poll_next(cx).unwrap(), Some(1).into());
            Ok(Poll::Ready(()))
        }
    }

    block_on(Peek { inner: list().peekable() }).unwrap()
}

#[test]
fn wait() {
    assert_eq!(block_on_stream(list()).collect::<Result<Vec<_>, _>>(), Ok(vec![1, 2, 3]));
}

#[test]
fn chunks() {
    assert_done(|| list().chunks(3).collect(), Ok(vec![vec![1, 2, 3]]));
    assert_done(|| list().chunks(1).collect(), Ok(vec![vec![1], vec![2], vec![3]]));
    assert_done(|| list().chunks(2).collect(), Ok(vec![vec![1, 2], vec![3]]));
    let mut list = block_on_stream(err_list().chunks(3));
    let i = list.next().unwrap().unwrap();
    assert_eq!(i, vec![1, 2]);
    let i = list.next().unwrap().unwrap_err();
    assert_eq!(i, 3);
}

#[test]
#[should_panic]
fn chunks_panic_on_cap_zero() {
    let _ = list().chunks(0);
}

#[test]
fn forward() {
    let v = Vec::new();
    let v = block_on(iter_ok::<_, Never>(vec![0, 1]).forward(v)).unwrap().1;
    assert_eq!(v, vec![0, 1]);

    let v = block_on(iter_ok::<_, Never>(vec![2, 3]).forward(v)).unwrap().1;
    assert_eq!(v, vec![0, 1, 2, 3]);

    assert_done(
        move || iter_ok::<_, Never>(vec![4, 5]).forward(v).map(|(_, s)| s),
        Ok(vec![0, 1, 2, 3, 4, 5]),
    );
}

#[test]
fn concat() {
    let a = iter_ok::<_, ()>(vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]]);
    assert_done(move || a.concat(), Ok(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]));

    let b = iter(vec![Ok::<_, ()>(vec![1, 2, 3]), Err(()), Ok(vec![7, 8, 9])]);
    assert_done(move || b.concat(), Err(()));
}

#[test]
fn concat2() {
    let a = iter_ok::<_, ()>(vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]]);
    assert_done(move || a.concat(), Ok(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]));

    let b = iter(vec![Ok::<_, ()>(vec![1, 2, 3]), Err(()), Ok(vec![7, 8, 9])]);
    assert_done(move || b.concat(), Err(()));

    let c = empty::<Vec<()>, ()>();
    assert_done(move || c.concat(), Ok(vec![]))
}

#[test]
fn stream_poll_fn() {
    let mut counter = 5usize;

    let read_stream = poll_fn(move |_| -> Poll<Option<usize>, std::io::Error> {
        if counter == 0 {
            return Ok(Poll::Ready(None));
        }
        counter -= 1;
        Ok(Poll::Ready(Some(counter)))
    });

    assert_eq!(block_on_stream(read_stream).count(), 5);
}

#[test]
fn inspect() {
    let mut seen = vec![];
    assert_done(|| list().inspect(|&a| seen.push(a)).collect(), Ok(vec![1, 2, 3]));
    assert_eq!(seen, [1, 2, 3]);
}

#[test]
fn inspect_err() {
    let mut seen = vec![];
    assert_done(|| err_list().inspect_err(|&a| seen.push(a)).collect::<Vec<_>>(), Err(3));
    assert_eq!(seen, [3]);
}
