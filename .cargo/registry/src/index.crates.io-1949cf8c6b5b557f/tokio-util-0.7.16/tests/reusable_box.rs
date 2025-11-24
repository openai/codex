use futures::future::FutureExt;
use std::alloc::Layout;
use std::future::Future;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use tokio_util::sync::ReusableBoxFuture;

#[test]
// Clippy false positive; it's useful to be able to test the trait impls for any lifetime
#[allow(clippy::extra_unused_lifetimes)]
fn traits<'a>() {
    fn assert_traits<T: Send + Sync + Unpin>() {}
    // Use a type that is !Unpin
    assert_traits::<ReusableBoxFuture<'a, PhantomPinned>>();
    // Use a type that is !Send + !Sync
    assert_traits::<ReusableBoxFuture<'a, Rc<()>>>();
}

#[test]
fn test_different_futures() {
    let fut = async move { 10 };
    // Not zero sized!
    assert_eq!(Layout::for_value(&fut).size(), 1);

    let mut b = ReusableBoxFuture::new(fut);

    assert_eq!(b.get_pin().now_or_never(), Some(10));

    b.try_set(async move { 20 })
        .unwrap_or_else(|_| panic!("incorrect size"));

    assert_eq!(b.get_pin().now_or_never(), Some(20));

    b.try_set(async move { 30 })
        .unwrap_or_else(|_| panic!("incorrect size"));

    assert_eq!(b.get_pin().now_or_never(), Some(30));
}

#[test]
fn test_different_sizes() {
    let fut1 = async move { 10 };
    let val = [0u32; 1000];
    let fut2 = async move { val[0] };
    let fut3 = ZeroSizedFuture {};

    assert_eq!(Layout::for_value(&fut1).size(), 1);
    assert_eq!(Layout::for_value(&fut2).size(), 4004);
    assert_eq!(Layout::for_value(&fut3).size(), 0);

    let mut b = ReusableBoxFuture::new(fut1);
    assert_eq!(b.get_pin().now_or_never(), Some(10));
    b.set(fut2);
    assert_eq!(b.get_pin().now_or_never(), Some(0));
    b.set(fut3);
    assert_eq!(b.get_pin().now_or_never(), Some(5));
}

struct ZeroSizedFuture {}
impl Future for ZeroSizedFuture {
    type Output = u32;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u32> {
        Poll::Ready(5)
    }
}

#[test]
fn test_zero_sized() {
    let fut = ZeroSizedFuture {};
    // Zero sized!
    assert_eq!(Layout::for_value(&fut).size(), 0);

    let mut b = ReusableBoxFuture::new(fut);

    assert_eq!(b.get_pin().now_or_never(), Some(5));
    assert_eq!(b.get_pin().now_or_never(), Some(5));

    b.try_set(ZeroSizedFuture {})
        .unwrap_or_else(|_| panic!("incorrect size"));

    assert_eq!(b.get_pin().now_or_never(), Some(5));
    assert_eq!(b.get_pin().now_or_never(), Some(5));
}
