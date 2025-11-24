use async_stream::stream;

use futures_core::stream::{FusedStream, Stream};
use futures_util::pin_mut;
use futures_util::stream::StreamExt;
use tokio::sync::mpsc;
use tokio_test::assert_ok;

#[tokio::test]
async fn noop_stream() {
    let s = stream! {};
    pin_mut!(s);

    while s.next().await.is_some() {
        unreachable!();
    }
}

#[tokio::test]
async fn empty_stream() {
    let mut ran = false;

    {
        let r = &mut ran;
        let s = stream! {
            *r = true;
            println!("hello world!");
        };
        pin_mut!(s);

        while s.next().await.is_some() {
            unreachable!();
        }
    }

    assert!(ran);
}

#[tokio::test]
async fn yield_single_value() {
    let s = stream! {
        yield "hello";
    };

    let values: Vec<_> = s.collect().await;

    assert_eq!(1, values.len());
    assert_eq!("hello", values[0]);
}

#[tokio::test]
async fn fused() {
    let s = stream! {
        yield "hello";
    };
    pin_mut!(s);

    assert!(!s.is_terminated());
    assert_eq!(s.next().await, Some("hello"));
    assert_eq!(s.next().await, None);

    assert!(s.is_terminated());
    // This should return None from now on
    assert_eq!(s.next().await, None);
}

#[tokio::test]
async fn yield_multi_value() {
    let s = stream! {
        yield "hello";
        yield "world";
        yield "dizzy";
    };

    let values: Vec<_> = s.collect().await;

    assert_eq!(3, values.len());
    assert_eq!("hello", values[0]);
    assert_eq!("world", values[1]);
    assert_eq!("dizzy", values[2]);
}

#[tokio::test]
async fn unit_yield_in_select() {
    use tokio::select;

    async fn do_stuff_async() {}

    let s = stream! {
        select! {
            _ = do_stuff_async() => yield,
            else => yield,
        }
    };

    let values: Vec<_> = s.collect().await;
    assert_eq!(values.len(), 1);
}

#[tokio::test]
async fn yield_with_select() {
    use tokio::select;

    async fn do_stuff_async() {}
    async fn more_async_work() {}

    let s = stream! {
        select! {
            _ = do_stuff_async() => yield "hey",
            _ = more_async_work() => yield "hey",
            else => yield "hey",
        }
    };

    let values: Vec<_> = s.collect().await;
    assert_eq!(values, vec!["hey"]);
}

#[tokio::test]
async fn return_stream() {
    fn build_stream() -> impl Stream<Item = u32> {
        stream! {
            yield 1;
            yield 2;
            yield 3;
        }
    }

    let s = build_stream();

    let values: Vec<_> = s.collect().await;
    assert_eq!(3, values.len());
    assert_eq!(1, values[0]);
    assert_eq!(2, values[1]);
    assert_eq!(3, values[2]);
}

#[tokio::test]
async fn consume_channel() {
    let (tx, mut rx) = mpsc::channel(10);

    let s = stream! {
        while let Some(v) = rx.recv().await {
            yield v;
        }
    };

    pin_mut!(s);

    for i in 0..3 {
        assert_ok!(tx.send(i).await);
        assert_eq!(Some(i), s.next().await);
    }

    drop(tx);
    assert_eq!(None, s.next().await);
}

#[tokio::test]
async fn borrow_self() {
    struct Data(String);

    impl Data {
        fn stream<'a>(&'a self) -> impl Stream<Item = &str> + 'a {
            stream! {
                yield &self.0[..];
            }
        }
    }

    let data = Data("hello".to_string());
    let s = data.stream();
    pin_mut!(s);

    assert_eq!(Some("hello"), s.next().await);
}

#[tokio::test]
async fn stream_in_stream() {
    let s = stream! {
        let s = stream! {
            for i in 0..3 {
                yield i;
            }
        };

        pin_mut!(s);
        while let Some(v) = s.next().await {
            yield v;
        }
    };

    let values: Vec<_> = s.collect().await;
    assert_eq!(3, values.len());
}

#[tokio::test]
async fn yield_non_unpin_value() {
    let s: Vec<_> = stream! {
        for i in 0..3 {
            yield async move { i };
        }
    }
    .buffered(1)
    .collect()
    .await;

    assert_eq!(s, vec![0, 1, 2]);
}

#[test]
fn inner_try_stream() {
    use async_stream::try_stream;
    use tokio::select;

    async fn do_stuff_async() {}

    let _ = stream! {
        select! {
            _ = do_stuff_async() => {
                let another_s = try_stream! {
                    yield;
                };
                let _: Result<(), ()> = Box::pin(another_s).next().await.unwrap();
            },
            else => {},
        }
        yield
    };
}

#[rustversion::attr(not(stable), ignore)]
#[test]
fn test() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
