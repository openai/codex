#![cfg(feature = "retry")]
#[path = "../support.rs"]
mod support;

use futures_util::future;
use tokio_test::{assert_pending, assert_ready_err, assert_ready_ok, task};
use tower::retry::Policy;
use tower_test::{assert_request_eq, mock};

#[tokio::test(flavor = "current_thread")]
async fn retry_errors() {
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service(RetryErrors);

    assert_ready_ok!(service.poll_ready());

    let mut fut = task::spawn(service.call("hello"));

    assert_request_eq!(handle, "hello").send_error("retry me");

    assert_pending!(fut.poll());

    assert_request_eq!(handle, "hello").send_response("world");

    assert_eq!(fut.into_inner().await.unwrap(), "world");
}

#[tokio::test(flavor = "current_thread")]
async fn retry_limit() {
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service(Limit(2));

    assert_ready_ok!(service.poll_ready());

    let mut fut = task::spawn(service.call("hello"));

    assert_request_eq!(handle, "hello").send_error("retry 1");
    assert_pending!(fut.poll());

    assert_request_eq!(handle, "hello").send_error("retry 2");
    assert_pending!(fut.poll());

    assert_request_eq!(handle, "hello").send_error("retry 3");
    assert_eq!(assert_ready_err!(fut.poll()).to_string(), "retry 3");
}

#[tokio::test(flavor = "current_thread")]
async fn retry_error_inspection() {
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service(UnlessErr("reject"));

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call("hello"));

    assert_request_eq!(handle, "hello").send_error("retry 1");
    assert_pending!(fut.poll());

    assert_request_eq!(handle, "hello").send_error("reject");
    assert_eq!(assert_ready_err!(fut.poll()).to_string(), "reject");
}

#[tokio::test(flavor = "current_thread")]
async fn retry_cannot_clone_request() {
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service(CannotClone);

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call("hello"));

    assert_request_eq!(handle, "hello").send_error("retry 1");
    assert_eq!(assert_ready_err!(fut.poll()).to_string(), "retry 1");
}

#[tokio::test(flavor = "current_thread")]
async fn success_with_cannot_clone() {
    let _t = support::trace_init();

    // Even though the request couldn't be cloned, if the first request succeeds,
    // it should succeed overall.
    let (mut service, mut handle) = new_service(CannotClone);

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call("hello"));

    assert_request_eq!(handle, "hello").send_response("world");
    assert_ready_ok!(fut.poll(), "world");
}

#[tokio::test(flavor = "current_thread")]
async fn retry_mutating_policy() {
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service(MutatingPolicy { remaining: 2 });

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call("hello"));

    assert_request_eq!(handle, "hello").send_response("world");
    assert_pending!(fut.poll());
    // the policy alters the request. in real life, this might be setting a header
    assert_request_eq!(handle, "retrying").send_response("world");

    assert_pending!(fut.poll());

    assert_request_eq!(handle, "retrying").send_response("world");

    assert_ready_err!(fut.poll(), "out of retries");
}

type Req = &'static str;
type Res = &'static str;
type InnerError = &'static str;
type Error = Box<dyn std::error::Error + Send + Sync>;
type Mock = mock::Mock<Req, Res>;
type Handle = mock::Handle<Req, Res>;

#[derive(Clone)]
struct RetryErrors;

impl Policy<Req, Res, Error> for RetryErrors {
    type Future = future::Ready<()>;
    fn retry(&mut self, _: &mut Req, result: &mut Result<Res, Error>) -> Option<Self::Future> {
        if result.is_err() {
            Some(future::ready(()))
        } else {
            None
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(*req)
    }
}

#[derive(Clone)]
struct Limit(usize);

impl Policy<Req, Res, Error> for Limit {
    type Future = future::Ready<()>;
    fn retry(&mut self, _: &mut Req, result: &mut Result<Res, Error>) -> Option<Self::Future> {
        if result.is_err() && self.0 > 0 {
            self.0 -= 1;
            Some(future::ready(()))
        } else {
            None
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(*req)
    }
}

#[derive(Clone)]
struct UnlessErr(InnerError);

impl Policy<Req, Res, Error> for UnlessErr {
    type Future = future::Ready<()>;
    fn retry(&mut self, _: &mut Req, result: &mut Result<Res, Error>) -> Option<Self::Future> {
        result.as_ref().err().and_then(|err| {
            if err.to_string() != self.0 {
                Some(future::ready(()))
            } else {
                None
            }
        })
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(*req)
    }
}

#[derive(Clone)]
struct CannotClone;

impl Policy<Req, Res, Error> for CannotClone {
    type Future = future::Ready<()>;
    fn retry(&mut self, _: &mut Req, _: &mut Result<Res, Error>) -> Option<Self::Future> {
        unreachable!("retry cannot be called since request isn't cloned");
    }

    fn clone_request(&mut self, _req: &Req) -> Option<Req> {
        None
    }
}

/// Test policy that changes the request to `retrying` during retries and the result to `"out of retries"`
/// when retries are exhausted.
#[derive(Clone)]
struct MutatingPolicy {
    remaining: usize,
}

impl Policy<Req, Res, Error> for MutatingPolicy
where
    Error: From<&'static str>,
{
    type Future = future::Ready<()>;

    fn retry(&mut self, req: &mut Req, result: &mut Result<Res, Error>) -> Option<Self::Future> {
        if self.remaining == 0 {
            *result = Err("out of retries".into());
            None
        } else {
            *req = "retrying";
            self.remaining -= 1;
            Some(future::ready(()))
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(*req)
    }
}

fn new_service<P: Policy<Req, Res, Error> + Clone>(
    policy: P,
) -> (mock::Spawn<tower::retry::Retry<P, Mock>>, Handle) {
    let retry = tower::retry::RetryLayer::new(policy);
    mock::spawn_layer(retry)
}
