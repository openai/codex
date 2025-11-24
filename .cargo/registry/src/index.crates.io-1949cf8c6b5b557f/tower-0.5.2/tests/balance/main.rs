#![cfg(feature = "balance")]
#[path = "../support.rs"]
mod support;

use std::future::Future;
use std::task::{Context, Poll};
use tokio_test::{assert_pending, assert_ready, task};
use tower::balance::p2c::Balance;
use tower::discover::Change;
use tower_service::Service;
use tower_test::mock;

type Req = &'static str;
struct Mock(mock::Mock<Req, Req>);

impl Service<Req> for Mock {
    type Response = <mock::Mock<Req, Req> as Service<Req>>::Response;
    type Error = <mock::Mock<Req, Req> as Service<Req>>::Error;
    type Future = <mock::Mock<Req, Req> as Service<Req>>::Future;
    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }
    fn call(&mut self, req: Req) -> Self::Future {
        self.0.call(req)
    }
}

impl tower::load::Load for Mock {
    type Metric = usize;
    fn load(&self) -> Self::Metric {
        rand::random()
    }
}

#[test]
fn stress() {
    let _t = support::trace_init();
    let mut task = task::spawn(());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<_, &'static str>>();
    let mut cache = Balance::<_, Req>::new(support::IntoStream::new(rx));

    let mut nready = 0;
    let mut services = slab::Slab::<(mock::Handle<Req, Req>, bool)>::new();
    let mut retired = Vec::<mock::Handle<Req, Req>>::new();
    for _ in 0..100_000 {
        for _ in 0..(rand::random::<u8>() % 8) {
            if !services.is_empty() && rand::random() {
                if nready == 0 || rand::random::<u8>() > u8::max_value() / 4 {
                    // ready a service
                    // TODO: sometimes ready a removed service?
                    for (_, (handle, ready)) in &mut services {
                        if !*ready {
                            handle.allow(1);
                            *ready = true;
                            nready += 1;
                            break;
                        }
                    }
                } else {
                    // use a service
                    use std::task::Poll;
                    match task.enter(|cx, _| cache.poll_ready(cx)) {
                        Poll::Ready(Ok(())) => {
                            assert_ne!(nready, 0, "got ready when no service is ready");
                            let mut fut = cache.call("hello");
                            let mut fut = std::pin::Pin::new(&mut fut);
                            assert_pending!(task.enter(|cx, _| fut.as_mut().poll(cx)));
                            let mut found = false;
                            for (_, (handle, ready)) in &mut services {
                                if *ready {
                                    if let Poll::Ready(Some((req, res))) = handle.poll_request() {
                                        assert_eq!(req, "hello");
                                        res.send_response("world");
                                        *ready = false;
                                        nready -= 1;
                                        found = true;
                                        break;
                                    }
                                }
                            }
                            if !found {
                                // we must have been given a retired service
                                let mut at = None;
                                for (i, handle) in retired.iter_mut().enumerate() {
                                    if let Poll::Ready(Some((req, res))) = handle.poll_request() {
                                        assert_eq!(req, "hello");
                                        res.send_response("world");
                                        at = Some(i);
                                        break;
                                    }
                                }
                                let _ = retired.swap_remove(
                                    at.expect("request was not sent to a ready service"),
                                );
                                nready -= 1;
                            }
                            assert_ready!(task.enter(|cx, _| fut.as_mut().poll(cx))).unwrap();
                        }
                        Poll::Ready(_) => unreachable!("discover stream has not failed"),
                        Poll::Pending => {
                            // assert_eq!(nready, 0, "got pending when a service is ready");
                        }
                    }
                }
            } else if services.is_empty() || rand::random() {
                if services.is_empty() || nready == 0 || rand::random() {
                    // add
                    let (svc, mut handle) = mock::pair::<Req, Req>();
                    let svc = Mock(svc);
                    handle.allow(0);
                    let k = services.insert((handle, false));
                    let ok = tx.send(Ok(Change::Insert(k, svc)));
                    assert!(ok.is_ok());
                } else {
                    // remove
                    while !services.is_empty() {
                        let k = rand::random::<usize>() % (services.iter().last().unwrap().0 + 1);
                        if services.contains(k) {
                            let (handle, ready) = services.remove(k);
                            if ready {
                                retired.push(handle);
                            }
                            let ok = tx.send(Ok(Change::Remove(k)));
                            assert!(ok.is_ok());
                            break;
                        }
                    }
                }
            } else {
                // fail a service
                while !services.is_empty() {
                    let k = rand::random::<usize>() % (services.iter().last().unwrap().0 + 1);
                    if services.contains(k) {
                        let (mut handle, ready) = services.remove(k);
                        if ready {
                            nready -= 1;
                        }
                        handle.send_error("doom");
                        break;
                    }
                }
            }
        }

        let r = task.enter(|cx, _| cache.poll_ready(cx));

        // drop any retired services that the p2c has gotten rid of
        let mut removed = Vec::new();
        for (i, handle) in retired.iter_mut().enumerate() {
            if let Poll::Ready(None) = handle.poll_request() {
                removed.push(i);
            }
        }
        for i in removed.into_iter().rev() {
            retired.swap_remove(i);
            nready -= 1;
        }

        use std::task::Poll;
        match r {
            Poll::Ready(Ok(())) => {
                assert_ne!(nready, 0, "got ready when no service is ready");
            }
            Poll::Ready(_) => unreachable!("discover stream has not failed"),
            Poll::Pending => {
                assert_eq!(nready, 0, "got pending when a service is ready");
            }
        }
    }
}
