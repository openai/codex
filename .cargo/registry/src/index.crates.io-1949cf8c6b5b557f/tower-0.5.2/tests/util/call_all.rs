use super::support;
use futures_core::Stream;
use futures_util::{
    future::{ready, Ready},
    pin_mut,
};
use std::fmt;
use std::future::Future;
use std::task::{Context, Poll};
use std::{cell::Cell, rc::Rc};
use tokio_test::{assert_pending, assert_ready, task};
use tower::util::ServiceExt;
use tower_service::*;
use tower_test::{assert_request_eq, mock, mock::Mock};

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, Eq, PartialEq)]
struct Srv {
    admit: Rc<Cell<bool>>,
    count: Rc<Cell<usize>>,
}
impl Service<&'static str> for Srv {
    type Response = &'static str;
    type Error = Error;
    type Future = Ready<Result<Self::Response, Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if !self.admit.get() {
            return Poll::Pending;
        }

        self.admit.set(false);
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: &'static str) -> Self::Future {
        self.count.set(self.count.get() + 1);
        ready(Ok(req))
    }
}

#[test]
fn ordered() {
    let _t = support::trace_init();

    let mut mock = task::spawn(());

    let admit = Rc::new(Cell::new(false));
    let count = Rc::new(Cell::new(0));
    let srv = Srv {
        count: count.clone(),
        admit: admit.clone(),
    };
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let ca = srv.call_all(support::IntoStream::new(rx));
    pin_mut!(ca);

    assert_pending!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)));
    tx.send("one").unwrap();
    mock.is_woken();
    assert_pending!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)));
    admit.set(true);
    let v = assert_ready!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)))
        .transpose()
        .unwrap();
    assert_eq!(v, Some("one"));
    assert_pending!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)));
    admit.set(true);
    tx.send("two").unwrap();
    mock.is_woken();
    tx.send("three").unwrap();
    let v = assert_ready!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)))
        .transpose()
        .unwrap();
    assert_eq!(v, Some("two"));
    assert_pending!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)));
    admit.set(true);
    let v = assert_ready!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)))
        .transpose()
        .unwrap();
    assert_eq!(v, Some("three"));
    admit.set(true);
    assert_pending!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)));
    admit.set(true);
    tx.send("four").unwrap();
    mock.is_woken();
    let v = assert_ready!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)))
        .transpose()
        .unwrap();
    assert_eq!(v, Some("four"));
    assert_pending!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)));

    // need to be ready since impl doesn't know it'll get EOF
    admit.set(true);

    // When we drop the request stream, CallAll should return None.
    drop(tx);
    mock.is_woken();
    let v = assert_ready!(mock.enter(|cx, _| ca.as_mut().poll_next(cx)))
        .transpose()
        .unwrap();
    assert!(v.is_none());
    assert_eq!(count.get(), 4);

    // We should also be able to recover the wrapped Service.
    assert_eq!(ca.take_service(), Srv { count, admit });
}

#[tokio::test(flavor = "current_thread")]
async fn unordered() {
    let _t = support::trace_init();

    let (mock, handle) = mock::pair::<_, &'static str>();
    pin_mut!(handle);

    let mut task = task::spawn(());
    let requests = futures_util::stream::iter(&["one", "two"]);

    let svc = mock.call_all(requests).unordered();
    pin_mut!(svc);

    assert_pending!(task.enter(|cx, _| svc.as_mut().poll_next(cx)));

    let resp1 = assert_request_eq!(handle, &"one");
    let resp2 = assert_request_eq!(handle, &"two");

    resp2.send_response("resp 1");

    let v = assert_ready!(task.enter(|cx, _| svc.as_mut().poll_next(cx)))
        .transpose()
        .unwrap();
    assert_eq!(v, Some("resp 1"));
    assert_pending!(task.enter(|cx, _| svc.as_mut().poll_next(cx)));

    resp1.send_response("resp 2");

    let v = assert_ready!(task.enter(|cx, _| svc.as_mut().poll_next(cx)))
        .transpose()
        .unwrap();
    assert_eq!(v, Some("resp 2"));

    let v = assert_ready!(task.enter(|cx, _| svc.as_mut().poll_next(cx)))
        .transpose()
        .unwrap();
    assert!(v.is_none());
}

#[tokio::test]
async fn pending() {
    let _t = support::trace_init();

    let (mock, mut handle) = mock::pair::<_, &'static str>();

    let mut task = task::spawn(());

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let ca = mock.call_all(support::IntoStream::new(rx));
    pin_mut!(ca);

    assert_pending!(task.enter(|cx, _| ca.as_mut().poll_next(cx)));
    tx.send("req").unwrap();
    assert_pending!(task.enter(|cx, _| ca.as_mut().poll_next(cx)));
    assert_request_eq!(handle, "req").send_response("res");
    let res = assert_ready!(task.enter(|cx, _| ca.as_mut().poll_next(cx)));
    assert_eq!(res.transpose().unwrap(), Some("res"));
    assert_pending!(task.enter(|cx, _| ca.as_mut().poll_next(cx)));
}

#[tokio::test]
async fn poll_ready_error() {
    struct ReadyOnceThenErr {
        polled: bool,
        inner: Mock<&'static str, &'static str>,
    }

    #[derive(Debug)]
    pub struct StringErr(String);

    impl fmt::Display for StringErr {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Display::fmt(&self.0, f)
        }
    }

    impl std::error::Error for StringErr {}

    impl Service<&'static str> for ReadyOnceThenErr {
        type Response = &'static str;
        type Error = Error;
        type Future = <Mock<&'static str, &'static str> as Service<&'static str>>::Future;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            match self.polled {
                false => {
                    self.polled = true;
                    self.inner.poll_ready(cx)
                }
                true => Poll::Ready(Err(Box::new(StringErr("poll_ready error".to_string())))),
            }
        }

        fn call(&mut self, req: &'static str) -> Self::Future {
            self.inner.call(req)
        }
    }

    let _t = support::trace_init();

    let (mock, mut handle) = mock::pair::<_, &'static str>();
    let svc = ReadyOnceThenErr {
        polled: false,
        inner: mock,
    };
    let mut task = task::spawn(());

    // "req0" is called, then "req1" receives a poll_ready error so "req2" will never be called.
    // Still the response from "req0" is waited on before ending the `call_all` stream.
    let requests = futures_util::stream::iter(vec!["req0", "req1", "req2"]);
    let ca = svc.call_all(requests);
    pin_mut!(ca);
    let err = assert_ready!(task.enter(|cx, _| ca.as_mut().poll_next(cx)));
    assert_eq!(err.unwrap().unwrap_err().to_string(), "poll_ready error");
    assert_request_eq!(handle, "req0").send_response("res0");
    let res = assert_ready!(task.enter(|cx, _| ca.as_mut().poll_next(cx)));
    assert_eq!(res.transpose().unwrap(), Some("res0"));
    let res = assert_ready!(task.enter(|cx, _| ca.as_mut().poll_next(cx)));
    assert_eq!(res.transpose().unwrap(), None);
}

#[tokio::test]
async fn stream_does_not_block_service() {
    use tower::buffer::Buffer;
    use tower::limit::ConcurrencyLimit;

    let _t = support::trace_init();
    let (mock, mut handle) = mock::pair::<_, &'static str>();
    let mut task = task::spawn(());

    let svc = Buffer::new(ConcurrencyLimit::new(mock, 1), 1);

    // Always pending, but should not occupy a concurrency slot.
    let pending = svc.clone().call_all(futures_util::stream::pending());
    pin_mut!(pending);
    assert_pending!(task.enter(|cx, _| pending.as_mut().poll_next(cx)));

    let call = svc.oneshot("req");
    pin_mut!(call);
    assert_pending!(task.enter(|cx, _| call.as_mut().poll(cx)));
    assert_request_eq!(handle, "req").send_response("res");
    let res = assert_ready!(task.enter(|cx, _| call.as_mut().poll(cx)));
    assert_eq!(res.unwrap(), "res");
}
