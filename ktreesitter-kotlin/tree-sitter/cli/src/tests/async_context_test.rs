use std::{
    future::Future,
    pin::{pin, Pin},
    ptr,
    task::{self, Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use tree_sitter::Parser;

use super::helpers::fixtures::get_language;

#[test]
fn test_node_in_fut() {
    let (ret, pended) = tokio_like_spawn(async {
        let mut parser = Parser::new();
        let language = get_language("bash");
        parser.set_language(&language).unwrap();

        let tree = parser.parse("#", None).unwrap();

        let root = tree.root_node();
        let root_ref = &root;

        let fut_val_fn = || async {
            yield_now().await;
            root.child(0).unwrap().kind()
        };

        yield_now().await;

        let fut_ref_fn = || async {
            yield_now().await;
            root_ref.child(0).unwrap().kind()
        };

        let f1 = fut_val_fn().await;
        let f2 = fut_ref_fn().await;
        assert_eq!(f1, f2);

        let fut_val = async {
            yield_now().await;
            root.child(0).unwrap().kind()
        };

        let fut_ref = async {
            yield_now().await;
            root_ref.child(0).unwrap().kind()
        };

        let f1 = fut_val.await;
        let f2 = fut_ref.await;
        assert_eq!(f1, f2);

        f1
    })
    .join();
    assert_eq!(ret, "comment");
    assert_eq!(pended, 5);
}

#[test]
fn test_node_and_cursor_ref_in_fut() {
    let ((), pended) = tokio_like_spawn(async {
        let mut parser = Parser::new();
        let language = get_language("c");
        parser.set_language(&language).unwrap();

        let tree = parser.parse("#", None).unwrap();

        let root = tree.root_node();
        let root_ref = &root;

        let mut cursor = tree.walk();
        let cursor_ref = &mut cursor;

        cursor_ref.goto_first_child();

        let fut_val = async {
            yield_now().await;
            let _ = root.to_sexp();
        };

        yield_now().await;

        let fut_ref = async {
            yield_now().await;
            let _ = root_ref.to_sexp();
            cursor_ref.goto_first_child();
        };

        fut_val.await;
        fut_ref.await;

        cursor_ref.goto_first_child();
    })
    .join();
    assert_eq!(pended, 3);
}

#[test]
fn test_node_and_cursor_ref_in_fut_with_fut_fabrics() {
    let ((), pended) = tokio_like_spawn(async {
        let mut parser = Parser::new();
        let language = get_language("javascript");
        parser.set_language(&language).unwrap();

        let tree = parser.parse("#", None).unwrap();

        let root = tree.root_node();
        let root_ref = &root;

        let mut cursor = tree.walk();
        let cursor_ref = &mut cursor;

        cursor_ref.goto_first_child();

        let fut_val = || async {
            yield_now().await;
            let _ = root.to_sexp();
        };

        yield_now().await;

        let fut_ref = || async move {
            yield_now().await;
            let _ = root_ref.to_sexp();
            cursor_ref.goto_first_child();
        };

        fut_val().await;
        fut_val().await;
        fut_ref().await;
    })
    .join();
    assert_eq!(pended, 4);
}

#[test]
fn test_node_and_cursor_ref_in_fut_with_inner_spawns() {
    let (ret, pended) = tokio_like_spawn(async {
        let mut parser = Parser::new();
        let language = get_language("rust");
        parser.set_language(&language).unwrap();

        let tree = parser.parse("#", None).unwrap();

        let mut cursor = tree.walk();
        let cursor_ref = &mut cursor;

        cursor_ref.goto_first_child();

        let fut_val = || {
            let tree = tree.clone();
            async move {
                let root = tree.root_node();
                let mut cursor = tree.walk();
                let cursor_ref = &mut cursor;
                yield_now().await;
                let _ = root.to_sexp();
                cursor_ref.goto_first_child();
            }
        };

        yield_now().await;

        let fut_ref = || {
            let tree = tree.clone();
            async move {
                let root = tree.root_node();
                let root_ref = &root;
                let mut cursor = tree.walk();
                let cursor_ref = &mut cursor;
                yield_now().await;
                let _ = root_ref.to_sexp();
                cursor_ref.goto_first_child();
            }
        };

        let ((), p1) = tokio_like_spawn(fut_val()).await.unwrap();
        let ((), p2) = tokio_like_spawn(fut_ref()).await.unwrap();

        cursor_ref.goto_first_child();

        fut_val().await;
        fut_val().await;
        fut_ref().await;

        cursor_ref.goto_first_child();

        p1 + p2
    })
    .join();
    assert_eq!(pended, 4);
    assert_eq!(ret, 2);
}

fn tokio_like_spawn<T>(future: T) -> JoinHandle<(T::Output, usize)>
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    // No runtime, just noop waker

    let waker = noop_waker();
    let mut cx = task::Context::from_waker(&waker);

    let mut pending = 0;
    let mut future = pin!(future);
    let ret = loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Pending => pending += 1,
            Poll::Ready(r) => {
                break r;
            }
        }
    };
    JoinHandle::new((ret, pending))
}

async fn yield_now() {
    struct SimpleYieldNow {
        yielded: bool,
    }

    impl Future for SimpleYieldNow {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            cx.waker().wake_by_ref();
            if self.yielded {
                return Poll::Ready(());
            }
            self.yielded = true;
            Poll::Pending
        }
    }

    SimpleYieldNow { yielded: false }.await;
}

pub const fn noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        // Cloning just returns a new no-op raw waker
        |_| RAW,
        // `wake` does nothing
        |_| {},
        // `wake_by_ref` does nothing
        |_| {},
        // Dropping does nothing as we don't allocate anything
        |_| {},
    );
    const RAW: RawWaker = RawWaker::new(ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(RAW) }
}

struct JoinHandle<T> {
    data: Option<T>,
}

impl<T> JoinHandle<T> {
    #[must_use]
    const fn new(data: T) -> Self {
        Self { data: Some(data) }
    }

    fn join(&mut self) -> T {
        self.data.take().unwrap()
    }
}

impl<T: Unpin> Future for JoinHandle<T> {
    type Output = std::result::Result<T, ()>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let data = self.get_mut().data.take().unwrap();
        Poll::Ready(Ok(data))
    }
}
