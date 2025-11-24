use crate::discover::ServiceList;
use crate::load;
use futures_util::pin_mut;
use std::task::Poll;
use tokio_test::{assert_pending, assert_ready, assert_ready_ok, task};
use tower_test::{assert_request_eq, mock};

use super::*;

#[tokio::test]
async fn empty() {
    let empty: Vec<load::Constant<mock::Mock<(), &'static str>, usize>> = vec![];
    let disco = ServiceList::new(empty);
    let mut svc = mock::Spawn::new(Balance::new(disco));
    assert_pending!(svc.poll_ready());
}

#[tokio::test]
async fn single_endpoint() {
    let (mut svc, mut handle) = mock::spawn_with(|s| {
        let mock = load::Constant::new(s, 0);
        let disco = ServiceList::new(vec![mock].into_iter());
        Balance::new(disco)
    });

    handle.allow(0);
    assert_pending!(svc.poll_ready());
    assert_eq!(
        svc.get_ref().len(),
        1,
        "balancer must have discovered endpoint"
    );

    handle.allow(1);
    assert_ready_ok!(svc.poll_ready());

    let mut fut = task::spawn(svc.call(()));

    assert_request_eq!(handle, ()).send_response(1);

    assert_eq!(assert_ready_ok!(fut.poll()), 1);
    handle.allow(1);
    assert_ready_ok!(svc.poll_ready());

    handle.send_error("endpoint lost");
    assert_pending!(svc.poll_ready());
    assert!(
        svc.get_ref().is_empty(),
        "balancer must drop failed endpoints"
    );
}

#[tokio::test]
async fn two_endpoints_with_equal_load() {
    let (mock_a, handle_a) = mock::pair();
    let (mock_b, handle_b) = mock::pair();
    let mock_a = load::Constant::new(mock_a, 1);
    let mock_b = load::Constant::new(mock_b, 1);

    pin_mut!(handle_a);
    pin_mut!(handle_b);

    let disco = ServiceList::new(vec![mock_a, mock_b].into_iter());
    let mut svc = mock::Spawn::new(Balance::new(disco));

    handle_a.allow(0);
    handle_b.allow(0);
    assert_pending!(svc.poll_ready());
    assert_eq!(
        svc.get_ref().len(),
        2,
        "balancer must have discovered both endpoints"
    );

    handle_a.allow(1);
    handle_b.allow(0);
    assert_ready_ok!(
        svc.poll_ready(),
        "must be ready when one of two services is ready"
    );
    {
        let mut fut = task::spawn(svc.call(()));
        assert_request_eq!(handle_a, ()).send_response("a");
        assert_eq!(assert_ready_ok!(fut.poll()), "a");
    }

    handle_a.allow(0);
    handle_b.allow(1);
    assert_ready_ok!(
        svc.poll_ready(),
        "must be ready when both endpoints are ready"
    );
    {
        let mut fut = task::spawn(svc.call(()));
        assert_request_eq!(handle_b, ()).send_response("b");
        assert_eq!(assert_ready_ok!(fut.poll()), "b");
    }

    handle_a.allow(1);
    handle_b.allow(1);
    for _ in 0..2 {
        assert_ready_ok!(
            svc.poll_ready(),
            "must be ready when both endpoints are ready"
        );
        let mut fut = task::spawn(svc.call(()));

        for (ref mut h, c) in &mut [(&mut handle_a, "a"), (&mut handle_b, "b")] {
            if let Poll::Ready(Some((_, tx))) = h.as_mut().poll_request() {
                tracing::info!("using {}", c);
                tx.send_response(c);
                h.allow(0);
            }
        }
        assert_ready_ok!(fut.poll());
    }

    handle_a.send_error("endpoint lost");
    assert_pending!(svc.poll_ready());
    assert_eq!(
        svc.get_ref().len(),
        1,
        "balancer must drop failed endpoints",
    );
}
