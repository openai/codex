use core::pin::Pin;
use std::convert::Infallible;

use futures::{
    stream::{self, repeat, Repeat, StreamExt, TryStreamExt},
    task::Poll,
    Stream,
};
use futures_executor::block_on;
use futures_task::Context;
use futures_test::task::noop_context;

#[test]
fn try_filter_map_after_err() {
    let cx = &mut noop_context();
    let mut s = stream::iter(1..=3)
        .map(Ok)
        .try_filter_map(|v| async move { Err::<Option<()>, _>(v) })
        .filter_map(|r| async move { r.ok() })
        .boxed();
    assert_eq!(Poll::Ready(None), s.poll_next_unpin(cx));
}

#[test]
fn try_skip_while_after_err() {
    let cx = &mut noop_context();
    let mut s = stream::iter(1..=3)
        .map(Ok)
        .try_skip_while(|_| async move { Err::<_, ()>(()) })
        .filter_map(|r| async move { r.ok() })
        .boxed();
    assert_eq!(Poll::Ready(None), s.poll_next_unpin(cx));
}

#[test]
fn try_take_while_after_err() {
    let cx = &mut noop_context();
    let mut s = stream::iter(1..=3)
        .map(Ok)
        .try_take_while(|_| async move { Err::<_, ()>(()) })
        .filter_map(|r| async move { r.ok() })
        .boxed();
    assert_eq!(Poll::Ready(None), s.poll_next_unpin(cx));
}

#[test]
fn try_flatten_unordered() {
    let test_st = stream::iter(1..7)
        .map(|val: u32| {
            if val % 2 == 0 {
                Ok(stream::unfold((val, 1), |(val, pow)| async move {
                    Some((val.pow(pow), (val, pow + 1)))
                })
                .take(3)
                .map(move |val| if val % 16 != 0 { Ok(val) } else { Err(val) }))
            } else {
                Err(val)
            }
        })
        .map_ok(Box::pin)
        .try_flatten_unordered(None);

    block_on(async move {
        assert_eq!(
            // All numbers can be divided by 16 and odds must be `Err`
            // For all basic evens we must have powers from 1 to 3
            vec![
                Err(1),
                Err(3),
                Err(5),
                Ok(2),
                Ok(4),
                Ok(6),
                Ok(4),
                Err(16),
                Ok(36),
                Ok(8),
                Err(64),
                Ok(216)
            ],
            test_st.collect::<Vec<_>>().await
        )
    });

    #[derive(Clone, Debug)]
    struct ErrorStream {
        error_after: usize,
        polled: usize,
    }

    impl Stream for ErrorStream {
        type Item = Result<Repeat<Result<(), ()>>, ()>;

        fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            if self.polled > self.error_after {
                panic!("Polled after error");
            } else {
                let out =
                    if self.polled == self.error_after { Err(()) } else { Ok(repeat(Ok(()))) };
                self.polled += 1;
                Poll::Ready(Some(out))
            }
        }
    }

    block_on(async move {
        let mut st = ErrorStream { error_after: 3, polled: 0 }.try_flatten_unordered(None);
        let mut ctr = 0;
        while (st.try_next().await).is_ok() {
            ctr += 1;
        }
        assert_eq!(ctr, 0);

        assert_eq!(
            ErrorStream { error_after: 10, polled: 0 }
                .try_flatten_unordered(None)
                .inspect_ok(|_| panic!("Unexpected `Ok`"))
                .try_collect::<Vec<_>>()
                .await,
            Err(())
        );

        let mut taken = 0;
        assert_eq!(
            ErrorStream { error_after: 10, polled: 0 }
                .map_ok(|st| st.take(3))
                .try_flatten_unordered(1)
                .inspect(|_| taken += 1)
                .try_fold((), |(), res| async move { Ok(res) })
                .await,
            Err(())
        );
        assert_eq!(taken, 31);
    })
}

async fn is_even(number: u8) -> bool {
    number % 2 == 0
}

#[test]
fn try_all() {
    block_on(async {
        let empty: [Result<u8, Infallible>; 0] = [];
        let st = stream::iter(empty);
        let all = st.try_all(is_even).await;
        assert_eq!(Ok(true), all);

        let st = stream::iter([Ok::<_, Infallible>(2), Ok(4), Ok(6), Ok(8)]);
        let all = st.try_all(is_even).await;
        assert_eq!(Ok(true), all);

        let st = stream::iter([Ok::<_, Infallible>(2), Ok(3), Ok(4)]);
        let all = st.try_all(is_even).await;
        assert_eq!(Ok(false), all);

        let st = stream::iter([Ok(2), Ok(4), Err("err"), Ok(8)]);
        let all = st.try_all(is_even).await;
        assert_eq!(Err("err"), all);
    });
}

#[test]
fn try_any() {
    block_on(async {
        let empty: [Result<u8, Infallible>; 0] = [];
        let st = stream::iter(empty);
        let any = st.try_any(is_even).await;
        assert_eq!(Ok(false), any);

        let st = stream::iter([Ok::<_, Infallible>(1), Ok(2), Ok(3)]);
        let any = st.try_any(is_even).await;
        assert_eq!(Ok(true), any);

        let st = stream::iter([Ok::<_, Infallible>(1), Ok(3), Ok(5)]);
        let any = st.try_any(is_even).await;
        assert_eq!(Ok(false), any);

        let st = stream::iter([Ok(1), Ok(3), Err("err"), Ok(8)]);
        let any = st.try_any(is_even).await;
        assert_eq!(Err("err"), any);
    });
}
