#![cfg(feature = "compat")]
#![allow(dead_code)]

//! Assert Send/Sync/Unpin for all public types.

use futures::{
    future::Future,
    sink::Sink,
    stream::Stream,
    task::{Context, Poll},
};
use static_assertions::{assert_impl_all as assert_impl, assert_not_impl_all as assert_not_impl};
use std::marker::PhantomPinned;
use std::{marker::PhantomData, pin::Pin};

type LocalFuture<T = *const ()> = Pin<Box<dyn Future<Output = T>>>;
type LocalTryFuture<T = *const (), E = *const ()> = LocalFuture<Result<T, E>>;
type SendFuture<T = *const ()> = Pin<Box<dyn Future<Output = T> + Send>>;
type SendTryFuture<T = *const (), E = *const ()> = SendFuture<Result<T, E>>;
type SyncFuture<T = *const ()> = Pin<Box<dyn Future<Output = T> + Sync>>;
type SyncTryFuture<T = *const (), E = *const ()> = SyncFuture<Result<T, E>>;
type SendSyncFuture<T = *const ()> = Pin<Box<dyn Future<Output = T> + Send + Sync>>;
type SendSyncTryFuture<T = *const (), E = *const ()> = SendSyncFuture<Result<T, E>>;
type UnpinFuture<T = PhantomPinned> = LocalFuture<T>;
type UnpinTryFuture<T = PhantomPinned, E = PhantomPinned> = UnpinFuture<Result<T, E>>;
struct PinnedFuture<T = PhantomPinned>(PhantomPinned, PhantomData<T>);
impl<T> Future for PinnedFuture<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        unimplemented!()
    }
}
type PinnedTryFuture<T = PhantomPinned, E = PhantomPinned> = PinnedFuture<Result<T, E>>;

type LocalStream<T = *const ()> = Pin<Box<dyn Stream<Item = T>>>;
type LocalTryStream<T = *const (), E = *const ()> = LocalStream<Result<T, E>>;
type SendStream<T = *const ()> = Pin<Box<dyn Stream<Item = T> + Send>>;
type SendTryStream<T = *const (), E = *const ()> = SendStream<Result<T, E>>;
type SyncStream<T = *const ()> = Pin<Box<dyn Stream<Item = T> + Sync>>;
type SyncTryStream<T = *const (), E = *const ()> = SyncStream<Result<T, E>>;
type SendSyncStream<T = *const ()> = Pin<Box<dyn Stream<Item = T> + Send + Sync>>;
type SendSyncTryStream<T = *const (), E = *const ()> = SendSyncStream<Result<T, E>>;
type UnpinStream<T = PhantomPinned> = LocalStream<T>;
type UnpinTryStream<T = PhantomPinned, E = PhantomPinned> = UnpinStream<Result<T, E>>;
struct PinnedStream<T = PhantomPinned>(PhantomPinned, PhantomData<T>);
impl<T> Stream for PinnedStream<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unimplemented!()
    }
}
type PinnedTryStream<T = PhantomPinned, E = PhantomPinned> = PinnedStream<Result<T, E>>;

type LocalSink<T = *const (), E = *const ()> = Pin<Box<dyn Sink<T, Error = E>>>;
type SendSink<T = *const (), E = *const ()> = Pin<Box<dyn Sink<T, Error = E> + Send>>;
type SyncSink<T = *const (), E = *const ()> = Pin<Box<dyn Sink<T, Error = E> + Sync>>;
type UnpinSink<T = PhantomPinned, E = PhantomPinned> = LocalSink<T, E>;
struct PinnedSink<T = PhantomPinned, E = PhantomPinned>(PhantomPinned, PhantomData<(T, E)>);
impl<T, E> Sink<T> for PinnedSink<T, E> {
    type Error = E;
    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }
    fn start_send(self: Pin<&mut Self>, _: T) -> Result<(), Self::Error> {
        unimplemented!()
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }
}

/// Assert Send/Sync/Unpin for all public types in `futures::channel`.
mod channel {
    use super::*;
    use futures::channel::*;

    assert_impl!(mpsc::Receiver<()>: Send);
    assert_not_impl!(mpsc::Receiver<*const ()>: Send);
    assert_impl!(mpsc::Receiver<()>: Sync);
    assert_not_impl!(mpsc::Receiver<*const ()>: Sync);
    assert_impl!(mpsc::Receiver<PhantomPinned>: Unpin);

    assert_impl!(mpsc::SendError: Send);
    assert_impl!(mpsc::SendError: Sync);
    assert_impl!(mpsc::SendError: Unpin);

    assert_impl!(mpsc::Sender<()>: Send);
    assert_not_impl!(mpsc::Sender<*const ()>: Send);
    assert_impl!(mpsc::Sender<()>: Sync);
    assert_not_impl!(mpsc::Sender<*const ()>: Sync);
    assert_impl!(mpsc::Sender<PhantomPinned>: Unpin);

    assert_impl!(mpsc::TryRecvError: Send);
    assert_impl!(mpsc::TryRecvError: Sync);
    assert_impl!(mpsc::TryRecvError: Unpin);

    assert_impl!(mpsc::TrySendError<()>: Send);
    assert_not_impl!(mpsc::TrySendError<*const ()>: Send);
    assert_impl!(mpsc::TrySendError<()>: Sync);
    assert_not_impl!(mpsc::TrySendError<*const ()>: Sync);
    assert_impl!(mpsc::TrySendError<()>: Unpin);
    assert_not_impl!(mpsc::TrySendError<PhantomPinned>: Unpin);

    assert_impl!(mpsc::UnboundedReceiver<()>: Send);
    assert_not_impl!(mpsc::UnboundedReceiver<*const ()>: Send);
    assert_impl!(mpsc::UnboundedReceiver<()>: Sync);
    assert_not_impl!(mpsc::UnboundedReceiver<*const ()>: Sync);
    assert_impl!(mpsc::UnboundedReceiver<PhantomPinned>: Unpin);

    assert_impl!(mpsc::UnboundedReceiver<()>: Send);
    assert_not_impl!(mpsc::UnboundedReceiver<*const ()>: Send);
    assert_impl!(mpsc::UnboundedReceiver<()>: Sync);
    assert_not_impl!(mpsc::UnboundedReceiver<*const ()>: Sync);
    assert_impl!(mpsc::UnboundedReceiver<PhantomPinned>: Unpin);

    assert_impl!(oneshot::Canceled: Send);
    assert_impl!(oneshot::Canceled: Sync);
    assert_impl!(oneshot::Canceled: Unpin);

    assert_impl!(oneshot::Cancellation<'_, ()>: Send);
    assert_not_impl!(oneshot::Cancellation<'_, *const ()>: Send);
    assert_impl!(oneshot::Cancellation<'_, ()>: Sync);
    assert_not_impl!(oneshot::Cancellation<'_, *const ()>: Sync);
    assert_impl!(oneshot::Cancellation<'_, PhantomPinned>: Unpin);

    assert_impl!(oneshot::Receiver<()>: Send);
    assert_not_impl!(oneshot::Receiver<*const ()>: Send);
    assert_impl!(oneshot::Receiver<()>: Sync);
    assert_not_impl!(oneshot::Receiver<*const ()>: Sync);
    assert_impl!(oneshot::Receiver<PhantomPinned>: Unpin);

    assert_impl!(oneshot::Sender<()>: Send);
    assert_not_impl!(oneshot::Sender<*const ()>: Send);
    assert_impl!(oneshot::Sender<()>: Sync);
    assert_not_impl!(oneshot::Sender<*const ()>: Sync);
    assert_impl!(oneshot::Sender<PhantomPinned>: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::compat`.
mod compat {
    use super::*;
    use futures::compat::*;

    assert_impl!(Compat<()>: Send);
    assert_not_impl!(Compat<*const ()>: Send);
    assert_impl!(Compat<()>: Sync);
    assert_not_impl!(Compat<*const ()>: Sync);
    assert_impl!(Compat<()>: Unpin);
    assert_not_impl!(Compat<PhantomPinned>: Unpin);

    assert_impl!(Compat01As03<()>: Send);
    assert_not_impl!(Compat01As03<*const ()>: Send);
    assert_not_impl!(Compat01As03<()>: Sync);
    assert_impl!(Compat01As03<PhantomPinned>: Unpin);

    assert_impl!(Compat01As03Sink<(), ()>: Send);
    assert_not_impl!(Compat01As03Sink<(), *const ()>: Send);
    assert_not_impl!(Compat01As03Sink<*const (), ()>: Send);
    assert_not_impl!(Compat01As03Sink<(), ()>: Sync);
    assert_impl!(Compat01As03Sink<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(CompatSink<(), *const ()>: Send);
    assert_not_impl!(CompatSink<*const (), ()>: Send);
    assert_impl!(CompatSink<(), *const ()>: Sync);
    assert_not_impl!(CompatSink<*const (), ()>: Sync);
    assert_impl!(CompatSink<(), PhantomPinned>: Unpin);
    assert_not_impl!(CompatSink<PhantomPinned, ()>: Unpin);

    assert_impl!(Executor01As03<()>: Send);
    assert_not_impl!(Executor01As03<*const ()>: Send);
    assert_impl!(Executor01As03<()>: Sync);
    assert_not_impl!(Executor01As03<*const ()>: Sync);
    assert_impl!(Executor01As03<()>: Unpin);
    assert_not_impl!(Executor01As03<PhantomPinned>: Unpin);

    assert_impl!(Executor01Future: Send);
    assert_not_impl!(Executor01Future: Sync);
    assert_impl!(Executor01Future: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::executor`.
mod executor {
    use super::*;
    use futures::executor::*;

    assert_impl!(BlockingStream<SendStream>: Send);
    assert_not_impl!(BlockingStream<LocalStream>: Send);
    assert_impl!(BlockingStream<SyncStream>: Sync);
    assert_not_impl!(BlockingStream<LocalStream>: Sync);
    assert_impl!(BlockingStream<UnpinStream>: Unpin);
    // BlockingStream requires `S: Unpin`
    // assert_not_impl!(BlockingStream<PinnedStream>: Unpin);

    assert_impl!(Enter: Send);
    assert_impl!(Enter: Sync);
    assert_impl!(Enter: Unpin);

    assert_impl!(EnterError: Send);
    assert_impl!(EnterError: Sync);
    assert_impl!(EnterError: Unpin);

    assert_not_impl!(LocalPool: Send);
    assert_not_impl!(LocalPool: Sync);
    assert_impl!(LocalPool: Unpin);

    assert_not_impl!(LocalSpawner: Send);
    assert_not_impl!(LocalSpawner: Sync);
    assert_impl!(LocalSpawner: Unpin);

    assert_impl!(ThreadPool: Send);
    assert_impl!(ThreadPool: Sync);
    assert_impl!(ThreadPool: Unpin);

    assert_impl!(ThreadPoolBuilder: Send);
    assert_impl!(ThreadPoolBuilder: Sync);
    assert_impl!(ThreadPoolBuilder: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::future`.
mod future {
    use super::*;
    use futures::future::*;

    assert_impl!(AbortHandle: Send);
    assert_impl!(AbortHandle: Sync);
    assert_impl!(AbortHandle: Unpin);

    assert_impl!(AbortRegistration: Send);
    assert_impl!(AbortRegistration: Sync);
    assert_impl!(AbortRegistration: Unpin);

    assert_impl!(Abortable<SendFuture>: Send);
    assert_not_impl!(Abortable<LocalFuture>: Send);
    assert_impl!(Abortable<SyncFuture>: Sync);
    assert_not_impl!(Abortable<LocalFuture>: Sync);
    assert_impl!(Abortable<UnpinFuture>: Unpin);
    assert_not_impl!(Abortable<PinnedFuture>: Unpin);

    assert_impl!(Aborted: Send);
    assert_impl!(Aborted: Sync);
    assert_impl!(Aborted: Unpin);

    assert_impl!(AndThen<SendFuture, SendFuture, ()>: Send);
    assert_not_impl!(AndThen<SendFuture, LocalFuture, ()>: Send);
    assert_not_impl!(AndThen<LocalFuture, SendFuture, ()>: Send);
    assert_not_impl!(AndThen<SendFuture, SendFuture, *const ()>: Send);
    assert_impl!(AndThen<SyncFuture, SyncFuture, ()>: Sync);
    assert_not_impl!(AndThen<SyncFuture, LocalFuture, ()>: Sync);
    assert_not_impl!(AndThen<LocalFuture, SyncFuture, ()>: Sync);
    assert_not_impl!(AndThen<SyncFuture, SyncFuture, *const ()>: Sync);
    assert_impl!(AndThen<UnpinFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(AndThen<PinnedFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(AndThen<UnpinFuture, PinnedFuture, PhantomPinned>: Unpin);

    assert_impl!(CatchUnwind<SendFuture>: Send);
    assert_not_impl!(CatchUnwind<LocalFuture>: Send);
    assert_impl!(CatchUnwind<SyncFuture>: Sync);
    assert_not_impl!(CatchUnwind<LocalFuture>: Sync);
    assert_impl!(CatchUnwind<UnpinFuture>: Unpin);
    assert_not_impl!(CatchUnwind<PinnedFuture>: Unpin);

    assert_impl!(ErrInto<SendTryFuture, *const ()>: Send);
    assert_not_impl!(ErrInto<LocalTryFuture, ()>: Send);
    assert_impl!(ErrInto<SyncTryFuture, *const ()>: Sync);
    assert_not_impl!(ErrInto<LocalTryFuture, ()>: Sync);
    assert_impl!(ErrInto<UnpinTryFuture, PhantomPinned>: Unpin);
    assert_not_impl!(ErrInto<PinnedTryFuture, PhantomPinned>: Unpin);

    assert_impl!(Flatten<SendFuture<()>>: Send);
    assert_not_impl!(Flatten<LocalFuture>: Send);
    assert_not_impl!(Flatten<SendFuture>: Send);
    assert_impl!(Flatten<SyncFuture<()>>: Sync);
    assert_not_impl!(Flatten<LocalFuture>: Sync);
    assert_not_impl!(Flatten<SyncFuture>: Sync);
    assert_impl!(Flatten<UnpinFuture<()>>: Unpin);
    assert_not_impl!(Flatten<PinnedFuture>: Unpin);
    assert_not_impl!(Flatten<UnpinFuture>: Unpin);

    assert_impl!(FlattenSink<SendFuture, ()>: Send);
    assert_not_impl!(FlattenSink<SendFuture, *const ()>: Send);
    assert_not_impl!(FlattenSink<LocalFuture, ()>: Send);
    assert_impl!(FlattenSink<SyncFuture, ()>: Sync);
    assert_not_impl!(FlattenSink<SyncFuture, *const ()>: Sync);
    assert_not_impl!(FlattenSink<LocalFuture, ()>: Sync);
    assert_impl!(FlattenSink<UnpinFuture, ()>: Unpin);
    assert_not_impl!(FlattenSink<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(FlattenSink<PinnedFuture, ()>: Unpin);

    assert_impl!(FlattenStream<SendFuture<()>>: Send);
    assert_not_impl!(FlattenStream<LocalFuture>: Send);
    assert_not_impl!(FlattenStream<SendFuture>: Send);
    assert_impl!(FlattenStream<SyncFuture<()>>: Sync);
    assert_not_impl!(FlattenStream<LocalFuture>: Sync);
    assert_not_impl!(FlattenStream<SyncFuture>: Sync);
    assert_impl!(FlattenStream<UnpinFuture<()>>: Unpin);
    assert_not_impl!(FlattenStream<PinnedFuture>: Unpin);
    assert_not_impl!(FlattenStream<UnpinFuture>: Unpin);

    assert_impl!(Fuse<SendFuture>: Send);
    assert_not_impl!(Fuse<LocalFuture>: Send);
    assert_impl!(Fuse<SyncFuture>: Sync);
    assert_not_impl!(Fuse<LocalFuture>: Sync);
    assert_impl!(Fuse<UnpinFuture>: Unpin);
    assert_not_impl!(Fuse<PinnedFuture>: Unpin);

    assert_impl!(FutureObj<'_, *const ()>: Send);
    assert_not_impl!(FutureObj<'_, ()>: Sync);
    assert_impl!(FutureObj<'_, PhantomPinned>: Unpin);

    assert_impl!(Inspect<SendFuture, ()>: Send);
    assert_not_impl!(Inspect<SendFuture, *const ()>: Send);
    assert_not_impl!(Inspect<LocalFuture, ()>: Send);
    assert_impl!(Inspect<SyncFuture, ()>: Sync);
    assert_not_impl!(Inspect<SyncFuture, *const ()>: Sync);
    assert_not_impl!(Inspect<LocalFuture, ()>: Sync);
    assert_impl!(Inspect<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(Inspect<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(InspectErr<SendFuture, ()>: Send);
    assert_not_impl!(InspectErr<SendFuture, *const ()>: Send);
    assert_not_impl!(InspectErr<LocalFuture, ()>: Send);
    assert_impl!(InspectErr<SyncFuture, ()>: Sync);
    assert_not_impl!(InspectErr<SyncFuture, *const ()>: Sync);
    assert_not_impl!(InspectErr<LocalFuture, ()>: Sync);
    assert_impl!(InspectErr<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(InspectErr<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(InspectOk<SendFuture, ()>: Send);
    assert_not_impl!(InspectOk<SendFuture, *const ()>: Send);
    assert_not_impl!(InspectOk<LocalFuture, ()>: Send);
    assert_impl!(InspectOk<SyncFuture, ()>: Sync);
    assert_not_impl!(InspectOk<SyncFuture, *const ()>: Sync);
    assert_not_impl!(InspectOk<LocalFuture, ()>: Sync);
    assert_impl!(InspectOk<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(InspectOk<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(IntoFuture<SendFuture>: Send);
    assert_not_impl!(IntoFuture<LocalFuture>: Send);
    assert_impl!(IntoFuture<SyncFuture>: Sync);
    assert_not_impl!(IntoFuture<LocalFuture>: Sync);
    assert_impl!(IntoFuture<UnpinFuture>: Unpin);
    assert_not_impl!(IntoFuture<PinnedFuture>: Unpin);

    assert_impl!(IntoStream<SendFuture>: Send);
    assert_not_impl!(IntoStream<LocalFuture>: Send);
    assert_impl!(IntoStream<SyncFuture>: Sync);
    assert_not_impl!(IntoStream<LocalFuture>: Sync);
    assert_impl!(IntoStream<UnpinFuture>: Unpin);
    assert_not_impl!(IntoStream<PinnedFuture>: Unpin);

    assert_impl!(Join<SendFuture<()>, SendFuture<()>>: Send);
    assert_not_impl!(Join<SendFuture<()>, SendFuture>: Send);
    assert_not_impl!(Join<SendFuture, SendFuture<()>>: Send);
    assert_not_impl!(Join<SendFuture, LocalFuture>: Send);
    assert_not_impl!(Join<LocalFuture, SendFuture>: Send);
    assert_impl!(Join<SyncFuture<()>, SyncFuture<()>>: Sync);
    assert_not_impl!(Join<SyncFuture<()>, SyncFuture>: Sync);
    assert_not_impl!(Join<SyncFuture, SyncFuture<()>>: Sync);
    assert_not_impl!(Join<SyncFuture, LocalFuture>: Sync);
    assert_not_impl!(Join<LocalFuture, SyncFuture>: Sync);
    assert_impl!(Join<UnpinFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Join<PinnedFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Join<UnpinFuture, PinnedFuture>: Unpin);

    // Join3, Join4, Join5 are the same as Join

    assert_impl!(JoinAll<SendFuture<()>>: Send);
    assert_not_impl!(JoinAll<LocalFuture>: Send);
    assert_not_impl!(JoinAll<SendFuture>: Send);
    assert_impl!(JoinAll<SendSyncFuture<()>>: Sync);
    assert_not_impl!(JoinAll<SendFuture<()>>: Sync);
    assert_not_impl!(JoinAll<SyncFuture<()>>: Sync);
    assert_not_impl!(JoinAll<SendSyncFuture>: Sync);
    assert_impl!(JoinAll<PinnedFuture>: Unpin);

    assert_impl!(Lazy<()>: Send);
    assert_not_impl!(Lazy<*const ()>: Send);
    assert_impl!(Lazy<()>: Sync);
    assert_not_impl!(Lazy<*const ()>: Sync);
    assert_impl!(Lazy<PhantomPinned>: Unpin);

    assert_not_impl!(LocalFutureObj<'_, ()>: Send);
    assert_not_impl!(LocalFutureObj<'_, ()>: Sync);
    assert_impl!(LocalFutureObj<'_, PhantomPinned>: Unpin);

    assert_impl!(Map<SendFuture, ()>: Send);
    assert_not_impl!(Map<SendFuture, *const ()>: Send);
    assert_not_impl!(Map<LocalFuture, ()>: Send);
    assert_impl!(Map<SyncFuture, ()>: Sync);
    assert_not_impl!(Map<SyncFuture, *const ()>: Sync);
    assert_not_impl!(Map<LocalFuture, ()>: Sync);
    assert_impl!(Map<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(Map<PhantomPinned, ()>: Unpin);

    assert_impl!(MapErr<SendFuture, ()>: Send);
    assert_not_impl!(MapErr<SendFuture, *const ()>: Send);
    assert_not_impl!(MapErr<LocalFuture, ()>: Send);
    assert_impl!(MapErr<SyncFuture, ()>: Sync);
    assert_not_impl!(MapErr<SyncFuture, *const ()>: Sync);
    assert_not_impl!(MapErr<LocalFuture, ()>: Sync);
    assert_impl!(MapErr<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(MapErr<PhantomPinned, ()>: Unpin);

    assert_impl!(MapInto<SendFuture, *const ()>: Send);
    assert_not_impl!(MapInto<LocalFuture, ()>: Send);
    assert_impl!(MapInto<SyncFuture, *const ()>: Sync);
    assert_not_impl!(MapInto<LocalFuture, ()>: Sync);
    assert_impl!(MapInto<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(MapInto<PhantomPinned, ()>: Unpin);

    assert_impl!(MapOk<SendFuture, ()>: Send);
    assert_not_impl!(MapOk<SendFuture, *const ()>: Send);
    assert_not_impl!(MapOk<LocalFuture, ()>: Send);
    assert_impl!(MapOk<SyncFuture, ()>: Sync);
    assert_not_impl!(MapOk<SyncFuture, *const ()>: Sync);
    assert_not_impl!(MapOk<LocalFuture, ()>: Sync);
    assert_impl!(MapOk<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(MapOk<PhantomPinned, ()>: Unpin);

    assert_impl!(MapOkOrElse<SendFuture, (), ()>: Send);
    assert_not_impl!(MapOkOrElse<SendFuture, (), *const ()>: Send);
    assert_not_impl!(MapOkOrElse<SendFuture, *const (), ()>: Send);
    assert_not_impl!(MapOkOrElse<LocalFuture, (), ()>: Send);
    assert_impl!(MapOkOrElse<SyncFuture, (), ()>: Sync);
    assert_not_impl!(MapOkOrElse<SyncFuture, (), *const ()>: Sync);
    assert_not_impl!(MapOkOrElse<SyncFuture, *const (), ()>: Sync);
    assert_not_impl!(MapOkOrElse<LocalFuture, (), ()>: Sync);
    assert_impl!(MapOkOrElse<UnpinFuture, PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(MapOkOrElse<PhantomPinned, (), ()>: Unpin);

    assert_impl!(NeverError<SendFuture>: Send);
    assert_not_impl!(NeverError<LocalFuture>: Send);
    assert_impl!(NeverError<SyncFuture>: Sync);
    assert_not_impl!(NeverError<LocalFuture>: Sync);
    assert_impl!(NeverError<UnpinFuture>: Unpin);
    assert_not_impl!(NeverError<PinnedFuture>: Unpin);

    assert_impl!(OkInto<SendFuture, *const ()>: Send);
    assert_not_impl!(OkInto<LocalFuture, ()>: Send);
    assert_impl!(OkInto<SyncFuture, *const ()>: Sync);
    assert_not_impl!(OkInto<LocalFuture, ()>: Sync);
    assert_impl!(OkInto<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(OkInto<PhantomPinned, ()>: Unpin);

    assert_impl!(OptionFuture<SendFuture>: Send);
    assert_not_impl!(OptionFuture<LocalFuture>: Send);
    assert_impl!(OptionFuture<SyncFuture>: Sync);
    assert_not_impl!(OptionFuture<LocalFuture>: Sync);
    assert_impl!(OptionFuture<UnpinFuture>: Unpin);
    assert_not_impl!(OptionFuture<PinnedFuture>: Unpin);

    assert_impl!(OrElse<SendFuture, SendFuture, ()>: Send);
    assert_not_impl!(OrElse<SendFuture, LocalFuture, ()>: Send);
    assert_not_impl!(OrElse<LocalFuture, SendFuture, ()>: Send);
    assert_not_impl!(OrElse<SendFuture, SendFuture, *const ()>: Send);
    assert_impl!(OrElse<SyncFuture, SyncFuture, ()>: Sync);
    assert_not_impl!(OrElse<SyncFuture, LocalFuture, ()>: Sync);
    assert_not_impl!(OrElse<LocalFuture, SyncFuture, ()>: Sync);
    assert_not_impl!(OrElse<SyncFuture, SyncFuture, *const ()>: Sync);
    assert_impl!(OrElse<UnpinFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(OrElse<PinnedFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(OrElse<UnpinFuture, PinnedFuture, PhantomPinned>: Unpin);

    assert_impl!(Pending<()>: Send);
    assert_not_impl!(Pending<*const ()>: Send);
    assert_impl!(Pending<()>: Sync);
    assert_not_impl!(Pending<*const ()>: Sync);
    assert_impl!(Pending<PhantomPinned>: Unpin);

    assert_impl!(PollFn<()>: Send);
    assert_not_impl!(PollFn<*const ()>: Send);
    assert_impl!(PollFn<()>: Sync);
    assert_not_impl!(PollFn<*const ()>: Sync);
    assert_impl!(PollFn<PhantomPinned>: Unpin);

    assert_impl!(PollImmediate<SendStream>: Send);
    assert_not_impl!(PollImmediate<LocalStream<()>>: Send);
    assert_impl!(PollImmediate<SyncStream>: Sync);
    assert_not_impl!(PollImmediate<LocalStream<()>>: Sync);
    assert_impl!(PollImmediate<UnpinStream>: Unpin);
    assert_not_impl!(PollImmediate<PinnedStream>: Unpin);

    assert_impl!(Ready<()>: Send);
    assert_not_impl!(Ready<*const ()>: Send);
    assert_impl!(Ready<()>: Sync);
    assert_not_impl!(Ready<*const ()>: Sync);
    assert_impl!(Ready<PhantomPinned>: Unpin);

    assert_impl!(Remote<SendFuture<()>>: Send);
    assert_not_impl!(Remote<LocalFuture>: Send);
    assert_not_impl!(Remote<SendFuture>: Send);
    assert_impl!(Remote<SyncFuture<()>>: Sync);
    assert_not_impl!(Remote<LocalFuture>: Sync);
    assert_not_impl!(Remote<SyncFuture>: Sync);
    assert_impl!(Remote<UnpinFuture>: Unpin);
    assert_not_impl!(Remote<PinnedFuture>: Unpin);

    assert_impl!(RemoteHandle<()>: Send);
    assert_not_impl!(RemoteHandle<*const ()>: Send);
    assert_impl!(RemoteHandle<()>: Sync);
    assert_not_impl!(RemoteHandle<*const ()>: Sync);
    assert_impl!(RemoteHandle<PhantomPinned>: Unpin);

    assert_impl!(Select<SendFuture, SendFuture>: Send);
    assert_not_impl!(Select<SendFuture, LocalFuture>: Send);
    assert_not_impl!(Select<LocalFuture, SendFuture>: Send);
    assert_impl!(Select<SyncFuture, SyncFuture>: Sync);
    assert_not_impl!(Select<SyncFuture, LocalFuture>: Sync);
    assert_not_impl!(Select<LocalFuture, SyncFuture>: Sync);
    assert_impl!(Select<UnpinFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Select<PinnedFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Select<UnpinFuture, PinnedFuture>: Unpin);

    assert_impl!(SelectAll<SendFuture>: Send);
    assert_not_impl!(SelectAll<LocalFuture>: Send);
    assert_impl!(SelectAll<SyncFuture>: Sync);
    assert_not_impl!(SelectAll<LocalFuture>: Sync);
    assert_impl!(SelectAll<UnpinFuture>: Unpin);
    assert_not_impl!(SelectAll<PinnedFuture>: Unpin);

    assert_impl!(SelectOk<SendFuture>: Send);
    assert_not_impl!(SelectOk<LocalFuture>: Send);
    assert_impl!(SelectOk<SyncFuture>: Sync);
    assert_not_impl!(SelectOk<LocalFuture>: Sync);
    assert_impl!(SelectOk<UnpinFuture>: Unpin);
    assert_not_impl!(SelectOk<PinnedFuture>: Unpin);

    assert_impl!(Shared<SendFuture<()>>: Send);
    assert_not_impl!(Shared<SendFuture>: Send);
    assert_not_impl!(Shared<LocalFuture>: Send);
    assert_not_impl!(Shared<SyncFuture<()>>: Sync);
    assert_impl!(Shared<PinnedFuture>: Unpin);

    assert_impl!(Then<SendFuture, SendFuture, ()>: Send);
    assert_not_impl!(Then<SendFuture, SendFuture, *const ()>: Send);
    assert_not_impl!(Then<SendFuture, LocalFuture, ()>: Send);
    assert_not_impl!(Then<LocalFuture, SendFuture, ()>: Send);
    assert_impl!(Then<SyncFuture, SyncFuture, ()>: Sync);
    assert_not_impl!(Then<SyncFuture, SyncFuture, *const ()>: Sync);
    assert_not_impl!(Then<SyncFuture, LocalFuture, ()>: Sync);
    assert_not_impl!(Then<LocalFuture, SyncFuture, ()>: Sync);
    assert_impl!(Then<UnpinFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(Then<PinnedFuture, UnpinFuture, ()>: Unpin);
    assert_not_impl!(Then<UnpinFuture, PinnedFuture, ()>: Unpin);

    assert_impl!(TryFlatten<SendTryFuture<()>, ()>: Send);
    assert_not_impl!(TryFlatten<LocalTryFuture, ()>: Send);
    assert_not_impl!(TryFlatten<SendTryFuture, *const ()>: Send);
    assert_impl!(TryFlatten<SyncTryFuture<()>, ()>: Sync);
    assert_not_impl!(TryFlatten<LocalTryFuture, ()>: Sync);
    assert_not_impl!(TryFlatten<SyncTryFuture, *const ()>: Sync);
    assert_impl!(TryFlatten<UnpinTryFuture<()>, ()>: Unpin);
    assert_not_impl!(TryFlatten<PinnedTryFuture, ()>: Unpin);
    assert_not_impl!(TryFlatten<UnpinTryFuture, PhantomPinned>: Unpin);

    assert_impl!(TryFlattenStream<SendTryFuture<()>>: Send);
    assert_not_impl!(TryFlattenStream<LocalTryFuture>: Send);
    assert_not_impl!(TryFlattenStream<SendTryFuture>: Send);
    assert_impl!(TryFlattenStream<SyncTryFuture<()>>: Sync);
    assert_not_impl!(TryFlattenStream<LocalTryFuture>: Sync);
    assert_not_impl!(TryFlattenStream<SyncTryFuture>: Sync);
    assert_impl!(TryFlattenStream<UnpinTryFuture<()>>: Unpin);
    assert_not_impl!(TryFlattenStream<PinnedTryFuture>: Unpin);
    assert_not_impl!(TryFlattenStream<UnpinTryFuture>: Unpin);

    assert_impl!(TryJoin<SendTryFuture<()>, SendTryFuture<()>>: Send);
    assert_not_impl!(TryJoin<SendTryFuture<()>, SendTryFuture>: Send);
    assert_not_impl!(TryJoin<SendTryFuture, SendTryFuture<()>>: Send);
    assert_not_impl!(TryJoin<SendTryFuture, LocalTryFuture>: Send);
    assert_not_impl!(TryJoin<LocalTryFuture, SendTryFuture>: Send);
    assert_impl!(TryJoin<SyncTryFuture<()>, SyncTryFuture<()>>: Sync);
    assert_not_impl!(TryJoin<SyncTryFuture<()>, SyncTryFuture>: Sync);
    assert_not_impl!(TryJoin<SyncTryFuture, SyncTryFuture<()>>: Sync);
    assert_not_impl!(TryJoin<SyncTryFuture, LocalTryFuture>: Sync);
    assert_not_impl!(TryJoin<LocalTryFuture, SyncTryFuture>: Sync);
    assert_impl!(TryJoin<UnpinTryFuture, UnpinTryFuture>: Unpin);
    assert_not_impl!(TryJoin<PinnedTryFuture, UnpinTryFuture>: Unpin);
    assert_not_impl!(TryJoin<UnpinTryFuture, PinnedTryFuture>: Unpin);

    // TryJoin3, TryJoin4, TryJoin5 are the same as TryJoin

    assert_impl!(TryJoinAll<SendTryFuture<(), ()>>: Send);
    assert_not_impl!(TryJoinAll<LocalTryFuture>: Send);
    assert_not_impl!(TryJoinAll<SendTryFuture>: Send);
    assert_impl!(TryJoinAll<SendSyncTryFuture<(), ()>>: Sync);
    assert_not_impl!(TryJoinAll<SendTryFuture<(), ()>>: Sync);
    assert_not_impl!(TryJoinAll<SyncTryFuture<(), ()>>: Sync);
    assert_not_impl!(TryJoinAll<SendSyncTryFuture>: Sync);
    assert_impl!(TryJoinAll<PinnedTryFuture>: Unpin);

    assert_impl!(TrySelect<SendFuture, SendFuture>: Send);
    assert_not_impl!(TrySelect<SendFuture, LocalFuture>: Send);
    assert_not_impl!(TrySelect<LocalFuture, SendFuture>: Send);
    assert_impl!(TrySelect<SyncFuture, SyncFuture>: Sync);
    assert_not_impl!(TrySelect<SyncFuture, LocalFuture>: Sync);
    assert_not_impl!(TrySelect<LocalFuture, SyncFuture>: Sync);
    assert_impl!(TrySelect<UnpinFuture, UnpinFuture>: Unpin);
    assert_not_impl!(TrySelect<PinnedFuture, UnpinFuture>: Unpin);
    assert_not_impl!(TrySelect<UnpinFuture, PinnedFuture>: Unpin);

    assert_impl!(UnitError<SendFuture>: Send);
    assert_not_impl!(UnitError<LocalFuture>: Send);
    assert_impl!(UnitError<SyncFuture>: Sync);
    assert_not_impl!(UnitError<LocalFuture>: Sync);
    assert_impl!(UnitError<UnpinFuture>: Unpin);
    assert_not_impl!(UnitError<PinnedFuture>: Unpin);

    assert_impl!(UnwrapOrElse<SendFuture, ()>: Send);
    assert_not_impl!(UnwrapOrElse<SendFuture, *const ()>: Send);
    assert_not_impl!(UnwrapOrElse<LocalFuture, ()>: Send);
    assert_impl!(UnwrapOrElse<SyncFuture, ()>: Sync);
    assert_not_impl!(UnwrapOrElse<SyncFuture, *const ()>: Sync);
    assert_not_impl!(UnwrapOrElse<LocalFuture, ()>: Sync);
    assert_impl!(UnwrapOrElse<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(UnwrapOrElse<PhantomPinned, ()>: Unpin);

    assert_impl!(WeakShared<SendFuture<()>>: Send);
    assert_not_impl!(WeakShared<SendFuture>: Send);
    assert_not_impl!(WeakShared<LocalFuture>: Send);
    assert_not_impl!(WeakShared<SyncFuture<()>>: Sync);
    assert_impl!(WeakShared<PinnedFuture>: Unpin);

    assert_impl!(Either<SendFuture, SendFuture>: Send);
    assert_not_impl!(Either<SendFuture, LocalFuture>: Send);
    assert_not_impl!(Either<LocalFuture, SendFuture>: Send);
    assert_impl!(Either<SyncFuture, SyncFuture>: Sync);
    assert_not_impl!(Either<SyncFuture, LocalFuture>: Sync);
    assert_not_impl!(Either<LocalFuture, SyncFuture>: Sync);
    assert_impl!(Either<UnpinFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Either<UnpinFuture, PinnedFuture>: Unpin);
    assert_not_impl!(Either<PinnedFuture, UnpinFuture>: Unpin);

    assert_impl!(MaybeDone<SendFuture<()>>: Send);
    assert_not_impl!(MaybeDone<SendFuture>: Send);
    assert_not_impl!(MaybeDone<LocalFuture>: Send);
    assert_impl!(MaybeDone<SyncFuture<()>>: Sync);
    assert_not_impl!(MaybeDone<SyncFuture>: Sync);
    assert_not_impl!(MaybeDone<LocalFuture>: Sync);
    assert_impl!(MaybeDone<UnpinFuture>: Unpin);
    assert_not_impl!(MaybeDone<PinnedFuture>: Unpin);

    assert_impl!(TryMaybeDone<SendTryFuture<()>>: Send);
    assert_not_impl!(TryMaybeDone<SendTryFuture>: Send);
    assert_not_impl!(TryMaybeDone<LocalTryFuture>: Send);
    assert_impl!(TryMaybeDone<SyncTryFuture<()>>: Sync);
    assert_not_impl!(TryMaybeDone<SyncTryFuture>: Sync);
    assert_not_impl!(TryMaybeDone<LocalTryFuture>: Sync);
    assert_impl!(TryMaybeDone<UnpinTryFuture>: Unpin);
    assert_not_impl!(TryMaybeDone<PinnedTryFuture>: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::io`.
mod io {
    use super::*;
    use futures::io::{Sink, *};

    assert_impl!(AllowStdIo<()>: Send);
    assert_not_impl!(AllowStdIo<*const ()>: Send);
    assert_impl!(AllowStdIo<()>: Sync);
    assert_not_impl!(AllowStdIo<*const ()>: Sync);
    assert_impl!(AllowStdIo<PhantomPinned>: Unpin);

    assert_impl!(BufReader<()>: Send);
    assert_not_impl!(BufReader<*const ()>: Send);
    assert_impl!(BufReader<()>: Sync);
    assert_not_impl!(BufReader<*const ()>: Sync);
    assert_impl!(BufReader<()>: Unpin);
    assert_not_impl!(BufReader<PhantomPinned>: Unpin);

    assert_impl!(BufWriter<()>: Send);
    assert_not_impl!(BufWriter<*const ()>: Send);
    assert_impl!(BufWriter<()>: Sync);
    assert_not_impl!(BufWriter<*const ()>: Sync);
    assert_impl!(BufWriter<()>: Unpin);
    assert_not_impl!(BufWriter<PhantomPinned>: Unpin);

    assert_impl!(Chain<(), ()>: Send);
    assert_not_impl!(Chain<(), *const ()>: Send);
    assert_not_impl!(Chain<*const (), ()>: Send);
    assert_impl!(Chain<(), ()>: Sync);
    assert_not_impl!(Chain<(), *const ()>: Sync);
    assert_not_impl!(Chain<*const (), ()>: Sync);
    assert_impl!(Chain<(), ()>: Unpin);
    assert_not_impl!(Chain<(), PhantomPinned>: Unpin);
    assert_not_impl!(Chain<PhantomPinned, ()>: Unpin);

    assert_impl!(Close<'_, ()>: Send);
    assert_not_impl!(Close<'_, *const ()>: Send);
    assert_impl!(Close<'_, ()>: Sync);
    assert_not_impl!(Close<'_, *const ()>: Sync);
    assert_impl!(Close<'_, ()>: Unpin);
    assert_not_impl!(Close<'_, PhantomPinned>: Unpin);

    assert_impl!(Copy<'_, (), ()>: Send);
    assert_not_impl!(Copy<'_, (), *const ()>: Send);
    assert_not_impl!(Copy<'_, *const (), ()>: Send);
    assert_impl!(Copy<'_, (), ()>: Sync);
    assert_not_impl!(Copy<'_, (), *const ()>: Sync);
    assert_not_impl!(Copy<'_, *const (), ()>: Sync);
    assert_impl!(Copy<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(Copy<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(CopyBuf<'_, (), ()>: Send);
    assert_not_impl!(CopyBuf<'_, (), *const ()>: Send);
    assert_not_impl!(CopyBuf<'_, *const (), ()>: Send);
    assert_impl!(CopyBuf<'_, (), ()>: Sync);
    assert_not_impl!(CopyBuf<'_, (), *const ()>: Sync);
    assert_not_impl!(CopyBuf<'_, *const (), ()>: Sync);
    assert_impl!(CopyBuf<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(CopyBuf<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(Cursor<()>: Send);
    assert_not_impl!(Cursor<*const ()>: Send);
    assert_impl!(Cursor<()>: Sync);
    assert_not_impl!(Cursor<*const ()>: Sync);
    assert_impl!(Cursor<()>: Unpin);
    assert_not_impl!(Cursor<PhantomPinned>: Unpin);

    assert_impl!(Empty: Send);
    assert_impl!(Empty: Sync);
    assert_impl!(Empty: Unpin);

    assert_impl!(FillBuf<'_, ()>: Send);
    assert_not_impl!(FillBuf<'_, *const ()>: Send);
    assert_impl!(FillBuf<'_, ()>: Sync);
    assert_not_impl!(FillBuf<'_, *const ()>: Sync);
    assert_impl!(FillBuf<'_, PhantomPinned>: Unpin);

    assert_impl!(Flush<'_, ()>: Send);
    assert_not_impl!(Flush<'_, *const ()>: Send);
    assert_impl!(Flush<'_, ()>: Sync);
    assert_not_impl!(Flush<'_, *const ()>: Sync);
    assert_impl!(Flush<'_, ()>: Unpin);
    assert_not_impl!(Flush<'_, PhantomPinned>: Unpin);

    assert_impl!(IntoSink<(), ()>: Send);
    assert_not_impl!(IntoSink<(), *const ()>: Send);
    assert_not_impl!(IntoSink<*const (), ()>: Send);
    assert_impl!(IntoSink<(), ()>: Sync);
    assert_not_impl!(IntoSink<(), *const ()>: Sync);
    assert_not_impl!(IntoSink<*const (), ()>: Sync);
    assert_impl!(IntoSink<(), PhantomPinned>: Unpin);
    assert_not_impl!(IntoSink<PhantomPinned, ()>: Unpin);

    assert_impl!(Lines<()>: Send);
    assert_not_impl!(Lines<*const ()>: Send);
    assert_impl!(Lines<()>: Sync);
    assert_not_impl!(Lines<*const ()>: Sync);
    assert_impl!(Lines<()>: Unpin);
    assert_not_impl!(Lines<PhantomPinned>: Unpin);

    assert_impl!(Read<'_, ()>: Send);
    assert_not_impl!(Read<'_, *const ()>: Send);
    assert_impl!(Read<'_, ()>: Sync);
    assert_not_impl!(Read<'_, *const ()>: Sync);
    assert_impl!(Read<'_, ()>: Unpin);
    assert_not_impl!(Read<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadExact<'_, ()>: Send);
    assert_not_impl!(ReadExact<'_, *const ()>: Send);
    assert_impl!(ReadExact<'_, ()>: Sync);
    assert_not_impl!(ReadExact<'_, *const ()>: Sync);
    assert_impl!(ReadExact<'_, ()>: Unpin);
    assert_not_impl!(ReadExact<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadHalf<()>: Send);
    assert_not_impl!(ReadHalf<*const ()>: Send);
    assert_impl!(ReadHalf<()>: Sync);
    assert_not_impl!(ReadHalf<*const ()>: Sync);
    assert_impl!(ReadHalf<PhantomPinned>: Unpin);

    assert_impl!(ReadLine<'_, ()>: Send);
    assert_not_impl!(ReadLine<'_, *const ()>: Send);
    assert_impl!(ReadLine<'_, ()>: Sync);
    assert_not_impl!(ReadLine<'_, *const ()>: Sync);
    assert_impl!(ReadLine<'_, ()>: Unpin);
    assert_not_impl!(ReadLine<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadToEnd<'_, ()>: Send);
    assert_not_impl!(ReadToEnd<'_, *const ()>: Send);
    assert_impl!(ReadToEnd<'_, ()>: Sync);
    assert_not_impl!(ReadToEnd<'_, *const ()>: Sync);
    assert_impl!(ReadToEnd<'_, ()>: Unpin);
    assert_not_impl!(ReadToEnd<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadToString<'_, ()>: Send);
    assert_not_impl!(ReadToString<'_, *const ()>: Send);
    assert_impl!(ReadToString<'_, ()>: Sync);
    assert_not_impl!(ReadToString<'_, *const ()>: Sync);
    assert_impl!(ReadToString<'_, ()>: Unpin);
    assert_not_impl!(ReadToString<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadUntil<'_, ()>: Send);
    assert_not_impl!(ReadUntil<'_, *const ()>: Send);
    assert_impl!(ReadUntil<'_, ()>: Sync);
    assert_not_impl!(ReadUntil<'_, *const ()>: Sync);
    assert_impl!(ReadUntil<'_, ()>: Unpin);
    assert_not_impl!(ReadUntil<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadVectored<'_, ()>: Send);
    assert_not_impl!(ReadVectored<'_, *const ()>: Send);
    assert_impl!(ReadVectored<'_, ()>: Sync);
    assert_not_impl!(ReadVectored<'_, *const ()>: Sync);
    assert_impl!(ReadVectored<'_, ()>: Unpin);
    assert_not_impl!(ReadVectored<'_, PhantomPinned>: Unpin);

    assert_impl!(Repeat: Send);
    assert_impl!(Repeat: Sync);
    assert_impl!(Repeat: Unpin);

    assert_impl!(ReuniteError<()>: Send);
    assert_not_impl!(ReuniteError<*const ()>: Send);
    assert_impl!(ReuniteError<()>: Sync);
    assert_not_impl!(ReuniteError<*const ()>: Sync);
    assert_impl!(ReuniteError<PhantomPinned>: Unpin);

    assert_impl!(Seek<'_, ()>: Send);
    assert_not_impl!(Seek<'_, *const ()>: Send);
    assert_impl!(Seek<'_, ()>: Sync);
    assert_not_impl!(Seek<'_, *const ()>: Sync);
    assert_impl!(Seek<'_, ()>: Unpin);
    assert_not_impl!(Seek<'_, PhantomPinned>: Unpin);

    assert_impl!(SeeKRelative<'_, ()>: Send);
    assert_not_impl!(SeeKRelative<'_, *const ()>: Send);
    assert_impl!(SeeKRelative<'_, ()>: Sync);
    assert_not_impl!(SeeKRelative<'_, *const ()>: Sync);
    assert_impl!(SeeKRelative<'_, PhantomPinned>: Unpin);

    assert_impl!(Sink: Send);
    assert_impl!(Sink: Sync);
    assert_impl!(Sink: Unpin);

    assert_impl!(Take<()>: Send);
    assert_not_impl!(Take<*const ()>: Send);
    assert_impl!(Take<()>: Sync);
    assert_not_impl!(Take<*const ()>: Sync);
    assert_impl!(Take<()>: Unpin);
    assert_not_impl!(Take<PhantomPinned>: Unpin);

    assert_impl!(Window<()>: Send);
    assert_not_impl!(Window<*const ()>: Send);
    assert_impl!(Window<()>: Sync);
    assert_not_impl!(Window<*const ()>: Sync);
    assert_impl!(Window<()>: Unpin);
    assert_not_impl!(Window<PhantomPinned>: Unpin);

    assert_impl!(Write<'_, ()>: Send);
    assert_not_impl!(Write<'_, *const ()>: Send);
    assert_impl!(Write<'_, ()>: Sync);
    assert_not_impl!(Write<'_, *const ()>: Sync);
    assert_impl!(Write<'_, ()>: Unpin);
    assert_not_impl!(Write<'_, PhantomPinned>: Unpin);

    assert_impl!(WriteAll<'_, ()>: Send);
    assert_not_impl!(WriteAll<'_, *const ()>: Send);
    assert_impl!(WriteAll<'_, ()>: Sync);
    assert_not_impl!(WriteAll<'_, *const ()>: Sync);
    assert_impl!(WriteAll<'_, ()>: Unpin);
    assert_not_impl!(WriteAll<'_, PhantomPinned>: Unpin);

    #[cfg(feature = "write-all-vectored")]
    assert_impl!(WriteAllVectored<'_, ()>: Send);
    #[cfg(feature = "write-all-vectored")]
    assert_not_impl!(WriteAllVectored<'_, *const ()>: Send);
    #[cfg(feature = "write-all-vectored")]
    assert_impl!(WriteAllVectored<'_, ()>: Sync);
    #[cfg(feature = "write-all-vectored")]
    assert_not_impl!(WriteAllVectored<'_, *const ()>: Sync);
    #[cfg(feature = "write-all-vectored")]
    assert_impl!(WriteAllVectored<'_, ()>: Unpin);
    // WriteAllVectored requires `W: Unpin`
    // #[cfg(feature = "write-all-vectored")]
    // assert_not_impl!(WriteAllVectored<'_, PhantomPinned>: Unpin);

    assert_impl!(WriteHalf<()>: Send);
    assert_not_impl!(WriteHalf<*const ()>: Send);
    assert_impl!(WriteHalf<()>: Sync);
    assert_not_impl!(WriteHalf<*const ()>: Sync);
    assert_impl!(WriteHalf<PhantomPinned>: Unpin);

    assert_impl!(WriteVectored<'_, ()>: Send);
    assert_not_impl!(WriteVectored<'_, *const ()>: Send);
    assert_impl!(WriteVectored<'_, ()>: Sync);
    assert_not_impl!(WriteVectored<'_, *const ()>: Sync);
    assert_impl!(WriteVectored<'_, ()>: Unpin);
    assert_not_impl!(WriteVectored<'_, PhantomPinned>: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::lock`.
mod lock {
    use super::*;
    use futures::lock::*;

    #[cfg(feature = "bilock")]
    assert_impl!(BiLock<()>: Send);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLock<*const ()>: Send);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLock<()>: Sync);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLock<*const ()>: Sync);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLock<PhantomPinned>: Unpin);

    #[cfg(feature = "bilock")]
    assert_impl!(BiLockAcquire<'_, ()>: Send);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLockAcquire<'_, *const ()>: Send);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLockAcquire<'_, ()>: Sync);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLockAcquire<'_, *const ()>: Sync);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLockAcquire<'_, PhantomPinned>: Unpin);

    #[cfg(feature = "bilock")]
    assert_impl!(BiLockGuard<'_, ()>: Send);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLockGuard<'_, *const ()>: Send);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLockGuard<'_, ()>: Sync);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLockGuard<'_, *const ()>: Sync);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLockGuard<'_, PhantomPinned>: Unpin);

    assert_impl!(MappedMutexGuard<'_, (), ()>: Send);
    assert_not_impl!(MappedMutexGuard<'_, (), *const ()>: Send);
    assert_not_impl!(MappedMutexGuard<'_, *const (), ()>: Send);
    assert_impl!(MappedMutexGuard<'_, (), ()>: Sync);
    assert_not_impl!(MappedMutexGuard<'_, (), *const ()>: Sync);
    assert_not_impl!(MappedMutexGuard<'_, *const (), ()>: Sync);
    assert_impl!(MappedMutexGuard<'_, PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(Mutex<()>: Send);
    assert_not_impl!(Mutex<*const ()>: Send);
    assert_impl!(Mutex<()>: Sync);
    assert_not_impl!(Mutex<*const ()>: Sync);
    assert_impl!(Mutex<()>: Unpin);
    assert_not_impl!(Mutex<PhantomPinned>: Unpin);

    assert_impl!(MutexGuard<'_, ()>: Send);
    assert_not_impl!(MutexGuard<'_, *const ()>: Send);
    assert_impl!(MutexGuard<'_, ()>: Sync);
    assert_not_impl!(MutexGuard<'_, *const ()>: Sync);
    assert_impl!(MutexGuard<'_, PhantomPinned>: Unpin);

    assert_impl!(MutexLockFuture<'_, ()>: Send);
    assert_not_impl!(MutexLockFuture<'_, *const ()>: Send);
    assert_impl!(MutexLockFuture<'_, *const ()>: Sync);
    assert_impl!(MutexLockFuture<'_, PhantomPinned>: Unpin);

    #[cfg(feature = "bilock")]
    assert_impl!(ReuniteError<()>: Send);
    #[cfg(feature = "bilock")]
    assert_not_impl!(ReuniteError<*const ()>: Send);
    #[cfg(feature = "bilock")]
    assert_impl!(ReuniteError<()>: Sync);
    #[cfg(feature = "bilock")]
    assert_not_impl!(ReuniteError<*const ()>: Sync);
    #[cfg(feature = "bilock")]
    assert_impl!(ReuniteError<PhantomPinned>: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::sink`.
mod sink {
    use super::*;
    use futures::sink::{self, *};
    use std::marker::Send;

    assert_impl!(Buffer<(), ()>: Send);
    assert_not_impl!(Buffer<(), *const ()>: Send);
    assert_not_impl!(Buffer<*const (), ()>: Send);
    assert_impl!(Buffer<(), ()>: Sync);
    assert_not_impl!(Buffer<(), *const ()>: Sync);
    assert_not_impl!(Buffer<*const (), ()>: Sync);
    assert_impl!(Buffer<(), PhantomPinned>: Unpin);
    assert_not_impl!(Buffer<PhantomPinned, ()>: Unpin);

    assert_impl!(Close<'_, (), *const ()>: Send);
    assert_not_impl!(Close<'_, *const (), ()>: Send);
    assert_impl!(Close<'_, (), *const ()>: Sync);
    assert_not_impl!(Close<'_, *const (), ()>: Sync);
    assert_impl!(Close<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(Close<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(Drain<()>: Send);
    assert_not_impl!(Drain<*const ()>: Send);
    assert_impl!(Drain<()>: Sync);
    assert_not_impl!(Drain<*const ()>: Sync);
    assert_impl!(Drain<PhantomPinned>: Unpin);

    assert_impl!(Fanout<(), ()>: Send);
    assert_not_impl!(Fanout<(), *const ()>: Send);
    assert_not_impl!(Fanout<*const (), ()>: Send);
    assert_impl!(Fanout<(), ()>: Sync);
    assert_not_impl!(Fanout<(), *const ()>: Sync);
    assert_not_impl!(Fanout<*const (), ()>: Sync);
    assert_impl!(Fanout<(), ()>: Unpin);
    assert_not_impl!(Fanout<(), PhantomPinned>: Unpin);
    assert_not_impl!(Fanout<PhantomPinned, ()>: Unpin);

    assert_impl!(Feed<'_, (), ()>: Send);
    assert_not_impl!(Feed<'_, (), *const ()>: Send);
    assert_not_impl!(Feed<'_, *const (), ()>: Send);
    assert_impl!(Feed<'_, (), ()>: Sync);
    assert_not_impl!(Feed<'_, (), *const ()>: Sync);
    assert_not_impl!(Feed<'_, *const (), ()>: Sync);
    assert_impl!(Feed<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(Feed<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(Flush<'_, (), *const ()>: Send);
    assert_not_impl!(Flush<'_, *const (), ()>: Send);
    assert_impl!(Flush<'_, (), *const ()>: Sync);
    assert_not_impl!(Flush<'_, *const (), ()>: Sync);
    assert_impl!(Flush<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(Flush<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(sink::Send<'_, (), ()>: Send);
    assert_not_impl!(sink::Send<'_, (), *const ()>: Send);
    assert_not_impl!(sink::Send<'_, *const (), ()>: Send);
    assert_impl!(sink::Send<'_, (), ()>: Sync);
    assert_not_impl!(sink::Send<'_, (), *const ()>: Sync);
    assert_not_impl!(sink::Send<'_, *const (), ()>: Sync);
    assert_impl!(sink::Send<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(sink::Send<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(SendAll<'_, (), SendTryStream<()>>: Send);
    assert_not_impl!(SendAll<'_, (), SendTryStream>: Send);
    assert_not_impl!(SendAll<'_, (), LocalTryStream>: Send);
    assert_not_impl!(SendAll<'_, *const (), SendTryStream<()>>: Send);
    assert_impl!(SendAll<'_, (), SyncTryStream<()>>: Sync);
    assert_not_impl!(SendAll<'_, (), SyncTryStream>: Sync);
    assert_not_impl!(SendAll<'_, (), LocalTryStream>: Sync);
    assert_not_impl!(SendAll<'_, *const (), SyncTryStream<()>>: Sync);
    assert_impl!(SendAll<'_, (), UnpinTryStream>: Unpin);
    assert_not_impl!(SendAll<'_, PhantomPinned, UnpinTryStream>: Unpin);
    assert_not_impl!(SendAll<'_, (), PinnedTryStream>: Unpin);

    assert_impl!(SinkErrInto<SendSink, *const (), *const ()>: Send);
    assert_not_impl!(SinkErrInto<LocalSink<()>, (), ()>: Send);
    assert_impl!(SinkErrInto<SyncSink, *const (), *const ()>: Sync);
    assert_not_impl!(SinkErrInto<LocalSink<()>, (), ()>: Sync);
    assert_impl!(SinkErrInto<UnpinSink, PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(SinkErrInto<PinnedSink<()>, (), ()>: Unpin);

    assert_impl!(SinkMapErr<SendSink, ()>: Send);
    assert_not_impl!(SinkMapErr<SendSink, *const ()>: Send);
    assert_not_impl!(SinkMapErr<LocalSink<()>, ()>: Send);
    assert_impl!(SinkMapErr<SyncSink, ()>: Sync);
    assert_not_impl!(SinkMapErr<SyncSink, *const ()>: Sync);
    assert_not_impl!(SinkMapErr<LocalSink<()>, ()>: Sync);
    assert_impl!(SinkMapErr<UnpinSink, PhantomPinned>: Unpin);
    assert_not_impl!(SinkMapErr<PinnedSink<()>, ()>: Unpin);

    assert_impl!(Unfold<(), (), ()>: Send);
    assert_not_impl!(Unfold<*const (), (), ()>: Send);
    assert_not_impl!(Unfold<(), *const (), ()>: Send);
    assert_not_impl!(Unfold<(), (), *const ()>: Send);
    assert_impl!(Unfold<(), (), ()>: Sync);
    assert_not_impl!(Unfold<*const (), (), ()>: Sync);
    assert_not_impl!(Unfold<(), *const (), ()>: Sync);
    assert_not_impl!(Unfold<(), (), *const ()>: Sync);
    assert_impl!(Unfold<PhantomPinned, PhantomPinned, ()>: Unpin);
    assert_not_impl!(Unfold<PinnedSink<()>, (), PhantomPinned>: Unpin);

    assert_impl!(With<(), *const (), *const (), (), ()>: Send);
    assert_not_impl!(With<*const (), (), (), (), ()>: Send);
    assert_not_impl!(With<(), (), (), *const (), ()>: Send);
    assert_not_impl!(With<(), (), (), (), *const ()>: Send);
    assert_impl!(With<(), *const (), *const (), (), ()>: Sync);
    assert_not_impl!(With<*const (), (), (), (), ()>: Sync);
    assert_not_impl!(With<(), (), (), *const (), ()>: Sync);
    assert_not_impl!(With<(), (), (), (), *const ()>: Sync);
    assert_impl!(With<(), PhantomPinned, PhantomPinned, (), PhantomPinned>: Unpin);
    assert_not_impl!(With<PhantomPinned, (), (), (), ()>: Unpin);
    assert_not_impl!(With<(), (), (), PhantomPinned, ()>: Unpin);

    assert_impl!(WithFlatMap<(), (), *const (), (), ()>: Send);
    assert_not_impl!(WithFlatMap<*const (), (), (), (), ()>: Send);
    assert_not_impl!(WithFlatMap<(), *const (), (), (), ()>: Send);
    assert_not_impl!(WithFlatMap<(), (), (), *const (), ()>: Send);
    assert_not_impl!(WithFlatMap<(), (), (), (), *const ()>: Send);
    assert_impl!(WithFlatMap<(), (), *const (), (), ()>: Sync);
    assert_not_impl!(WithFlatMap<*const (), (), (), (), ()>: Sync);
    assert_not_impl!(WithFlatMap<(), *const (), (), (), ()>: Sync);
    assert_not_impl!(WithFlatMap<(), (), (), *const (), ()>: Sync);
    assert_not_impl!(WithFlatMap<(), (), (), (), *const ()>: Sync);
    assert_impl!(WithFlatMap<(), PhantomPinned, PhantomPinned, (), PhantomPinned>: Unpin);
    assert_not_impl!(WithFlatMap<PhantomPinned, (), (), (), ()>: Unpin);
    assert_not_impl!(WithFlatMap<(), (), (), PhantomPinned, ()>: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::stream`.
mod stream {
    use super::*;
    use futures::{io, stream::*};

    assert_impl!(AndThen<(), (), ()>: Send);
    assert_not_impl!(AndThen<*const (), (), ()>: Send);
    assert_not_impl!(AndThen<(), *const (), ()>: Send);
    assert_not_impl!(AndThen<(), (), *const ()>: Send);
    assert_impl!(AndThen<(), (), ()>: Sync);
    assert_not_impl!(AndThen<*const (), (), ()>: Sync);
    assert_not_impl!(AndThen<(), *const (), ()>: Sync);
    assert_not_impl!(AndThen<(), (), *const ()>: Sync);
    assert_impl!(AndThen<(), (), PhantomPinned>: Unpin);
    assert_not_impl!(AndThen<PhantomPinned, (), ()>: Unpin);
    assert_not_impl!(AndThen<(), PhantomPinned, ()>: Unpin);

    assert_impl!(BufferUnordered<SendStream<()>>: Send);
    assert_not_impl!(BufferUnordered<SendStream>: Send);
    assert_not_impl!(BufferUnordered<LocalStream>: Send);
    assert_impl!(BufferUnordered<SyncStream<()>>: Sync);
    assert_not_impl!(BufferUnordered<SyncStream>: Sync);
    assert_not_impl!(BufferUnordered<LocalStream>: Sync);
    assert_impl!(BufferUnordered<UnpinStream>: Unpin);
    assert_not_impl!(BufferUnordered<PinnedStream>: Unpin);

    assert_impl!(Buffered<SendStream<SendFuture<()>>>: Send);
    assert_not_impl!(Buffered<SendStream<SendFuture>>: Send);
    assert_not_impl!(Buffered<SendStream<LocalFuture>>: Send);
    assert_not_impl!(Buffered<LocalStream<SendFuture<()>>>: Send);
    assert_impl!(Buffered<SyncStream<SendSyncFuture<()>>>: Sync);
    assert_not_impl!(Buffered<SyncStream<SyncFuture<()>>>: Sync);
    assert_not_impl!(Buffered<LocalStream<SendSyncFuture<()>>>: Sync);
    assert_impl!(Buffered<UnpinStream<PinnedFuture>>: Unpin);
    assert_not_impl!(Buffered<PinnedStream<PinnedFuture>>: Unpin);

    assert_impl!(CatchUnwind<SendStream>: Send);
    assert_not_impl!(CatchUnwind<LocalStream>: Send);
    assert_impl!(CatchUnwind<SyncStream>: Sync);
    assert_not_impl!(CatchUnwind<LocalStream>: Sync);
    assert_impl!(CatchUnwind<UnpinStream>: Unpin);
    assert_not_impl!(CatchUnwind<PinnedStream>: Unpin);

    assert_impl!(Chain<(), ()>: Send);
    assert_not_impl!(Chain<(), *const ()>: Send);
    assert_not_impl!(Chain<*const (), ()>: Send);
    assert_impl!(Chain<(), ()>: Sync);
    assert_not_impl!(Chain<(), *const ()>: Sync);
    assert_not_impl!(Chain<*const (), ()>: Sync);
    assert_impl!(Chain<(), ()>: Unpin);
    assert_not_impl!(Chain<(), PhantomPinned>: Unpin);
    assert_not_impl!(Chain<PhantomPinned, ()>: Unpin);

    assert_impl!(Chunks<SendStream<()>>: Send);
    assert_not_impl!(Chunks<SendStream>: Send);
    assert_not_impl!(Chunks<LocalStream>: Send);
    assert_impl!(Chunks<SyncStream<()>>: Sync);
    assert_not_impl!(Chunks<SyncStream>: Sync);
    assert_not_impl!(Chunks<LocalStream>: Sync);
    assert_impl!(Chunks<UnpinStream>: Unpin);
    assert_not_impl!(Chunks<PinnedStream>: Unpin);

    assert_impl!(Collect<(), ()>: Send);
    assert_not_impl!(Collect<*const (), ()>: Send);
    assert_not_impl!(Collect<(), *const ()>: Send);
    assert_impl!(Collect<(), ()>: Sync);
    assert_not_impl!(Collect<*const (), ()>: Sync);
    assert_not_impl!(Collect<(), *const ()>: Sync);
    assert_impl!(Collect<(), PhantomPinned>: Unpin);
    assert_not_impl!(Collect<PhantomPinned, ()>: Unpin);

    assert_impl!(Concat<SendStream<()>>: Send);
    assert_not_impl!(Concat<SendStream>: Send);
    assert_not_impl!(Concat<LocalStream>: Send);
    assert_impl!(Concat<SyncStream<()>>: Sync);
    assert_not_impl!(Concat<SyncStream>: Sync);
    assert_not_impl!(Concat<LocalStream>: Sync);
    assert_impl!(Concat<UnpinStream>: Unpin);
    assert_not_impl!(Concat<PinnedStream>: Unpin);

    assert_impl!(Cycle<()>: Send);
    assert_not_impl!(Cycle<*const ()>: Send);
    assert_impl!(Cycle<()>: Sync);
    assert_not_impl!(Cycle<*const ()>: Sync);
    assert_impl!(Cycle<()>: Unpin);
    assert_not_impl!(Cycle<PhantomPinned>: Unpin);

    assert_impl!(Empty<()>: Send);
    assert_not_impl!(Empty<*const ()>: Send);
    assert_impl!(Empty<()>: Sync);
    assert_not_impl!(Empty<*const ()>: Sync);
    assert_impl!(Empty<PhantomPinned>: Unpin);

    assert_impl!(Enumerate<()>: Send);
    assert_not_impl!(Enumerate<*const ()>: Send);
    assert_impl!(Enumerate<()>: Sync);
    assert_not_impl!(Enumerate<*const ()>: Sync);
    assert_impl!(Enumerate<()>: Unpin);
    assert_not_impl!(Enumerate<PhantomPinned>: Unpin);

    assert_impl!(ErrInto<(), *const ()>: Send);
    assert_not_impl!(ErrInto<*const (), ()>: Send);
    assert_impl!(ErrInto<(), *const ()>: Sync);
    assert_not_impl!(ErrInto<*const (), ()>: Sync);
    assert_impl!(ErrInto<(), PhantomPinned>: Unpin);
    assert_not_impl!(ErrInto<PhantomPinned, ()>: Unpin);

    assert_impl!(Filter<SendStream<()>, (), ()>: Send);
    assert_not_impl!(Filter<LocalStream<()>, (), ()>: Send);
    assert_not_impl!(Filter<SendStream, (), ()>: Send);
    assert_not_impl!(Filter<SendStream<()>, *const (), ()>: Send);
    assert_not_impl!(Filter<SendStream<()>, (), *const ()>: Send);
    assert_impl!(Filter<SyncStream<()>, (), ()>: Sync);
    assert_not_impl!(Filter<LocalStream<()>, (), ()>: Sync);
    assert_not_impl!(Filter<SyncStream, (), ()>: Sync);
    assert_not_impl!(Filter<SyncStream<()>, *const (), ()>: Sync);
    assert_not_impl!(Filter<SyncStream<()>, (), *const ()>: Sync);
    assert_impl!(Filter<UnpinStream, (), PhantomPinned>: Unpin);
    assert_not_impl!(Filter<PinnedStream, (), ()>: Unpin);
    assert_not_impl!(Filter<UnpinStream, PhantomPinned, ()>: Unpin);

    assert_impl!(FilterMap<(), (), ()>: Send);
    assert_not_impl!(FilterMap<*const (), (), ()>: Send);
    assert_not_impl!(FilterMap<(), *const (), ()>: Send);
    assert_not_impl!(FilterMap<(), (), *const ()>: Send);
    assert_impl!(FilterMap<(), (), ()>: Sync);
    assert_not_impl!(FilterMap<*const (), (), ()>: Sync);
    assert_not_impl!(FilterMap<(), *const (), ()>: Sync);
    assert_not_impl!(FilterMap<(), (), *const ()>: Sync);
    assert_impl!(FilterMap<(), (), PhantomPinned>: Unpin);
    assert_not_impl!(FilterMap<PhantomPinned, (), ()>: Unpin);
    assert_not_impl!(FilterMap<(), PhantomPinned, ()>: Unpin);

    assert_impl!(FlatMap<(), (), ()>: Send);
    assert_not_impl!(FlatMap<*const (), (), ()>: Send);
    assert_not_impl!(FlatMap<(), *const (), ()>: Send);
    assert_not_impl!(FlatMap<(), (), *const ()>: Send);
    assert_impl!(FlatMap<(), (), ()>: Sync);
    assert_not_impl!(FlatMap<*const (), (), ()>: Sync);
    assert_not_impl!(FlatMap<(), *const (), ()>: Sync);
    assert_not_impl!(FlatMap<(), (), *const ()>: Sync);
    assert_impl!(FlatMap<(), (), PhantomPinned>: Unpin);
    assert_not_impl!(FlatMap<PhantomPinned, (), ()>: Unpin);
    assert_not_impl!(FlatMap<(), PhantomPinned, ()>: Unpin);

    assert_impl!(Flatten<SendStream<()>>: Send);
    assert_not_impl!(Flatten<SendStream>: Send);
    assert_not_impl!(Flatten<SendStream>: Send);
    assert_impl!(Flatten<SyncStream<()>>: Sync);
    assert_not_impl!(Flatten<LocalStream<()>>: Sync);
    assert_not_impl!(Flatten<LocalStream<()>>: Sync);
    assert_impl!(Flatten<UnpinStream<()>>: Unpin);
    assert_not_impl!(Flatten<UnpinStream>: Unpin);
    assert_not_impl!(Flatten<PinnedStream>: Unpin);

    assert_impl!(Fold<(), (), (), ()>: Send);
    assert_not_impl!(Fold<*const (), (), (), ()>: Send);
    assert_not_impl!(Fold<(), *const (), (), ()>: Send);
    assert_not_impl!(Fold<(), (), *const (), ()>: Send);
    assert_not_impl!(Fold<(), (), (), *const ()>: Send);
    assert_impl!(Fold<(), (), (), ()>: Sync);
    assert_not_impl!(Fold<*const (), (), (), ()>: Sync);
    assert_not_impl!(Fold<(), *const (), (), ()>: Sync);
    assert_not_impl!(Fold<(), (), *const (), ()>: Sync);
    assert_not_impl!(Fold<(), (), (), *const ()>: Sync);
    assert_impl!(Fold<(), (), PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(Fold<PhantomPinned, (), (), ()>: Unpin);
    assert_not_impl!(Fold<(), PhantomPinned, (), ()>: Unpin);

    assert_impl!(ForEach<(), (), ()>: Send);
    assert_not_impl!(ForEach<*const (), (), ()>: Send);
    assert_not_impl!(ForEach<(), *const (), ()>: Send);
    assert_not_impl!(ForEach<(), (), *const ()>: Send);
    assert_impl!(ForEach<(), (), ()>: Sync);
    assert_not_impl!(ForEach<*const (), (), ()>: Sync);
    assert_not_impl!(ForEach<(), *const (), ()>: Sync);
    assert_not_impl!(ForEach<(), (), *const ()>: Sync);
    assert_impl!(ForEach<(), (), PhantomPinned>: Unpin);
    assert_not_impl!(ForEach<PhantomPinned, (), ()>: Unpin);
    assert_not_impl!(ForEach<(), PhantomPinned, ()>: Unpin);

    assert_impl!(ForEachConcurrent<(), (), ()>: Send);
    assert_not_impl!(ForEachConcurrent<*const (), (), ()>: Send);
    assert_not_impl!(ForEachConcurrent<(), *const (), ()>: Send);
    assert_not_impl!(ForEachConcurrent<(), (), *const ()>: Send);
    assert_impl!(ForEachConcurrent<(), (), ()>: Sync);
    assert_not_impl!(ForEachConcurrent<*const (), (), ()>: Sync);
    assert_not_impl!(ForEachConcurrent<(), *const (), ()>: Sync);
    assert_not_impl!(ForEachConcurrent<(), (), *const ()>: Sync);
    assert_impl!(ForEachConcurrent<(), PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(ForEachConcurrent<PhantomPinned, (), ()>: Unpin);

    assert_impl!(Forward<SendTryStream<()>, ()>: Send);
    assert_not_impl!(Forward<SendTryStream, ()>: Send);
    assert_not_impl!(Forward<SendTryStream<()>, *const ()>: Send);
    assert_not_impl!(Forward<LocalTryStream, ()>: Send);
    assert_impl!(Forward<SyncTryStream<()>, ()>: Sync);
    assert_not_impl!(Forward<SyncTryStream, ()>: Sync);
    assert_not_impl!(Forward<SyncTryStream<()>, *const ()>: Sync);
    assert_not_impl!(Forward<LocalTryStream, ()>: Sync);
    assert_impl!(Forward<UnpinTryStream, ()>: Unpin);
    assert_not_impl!(Forward<UnpinTryStream, PhantomPinned>: Unpin);
    assert_not_impl!(Forward<PinnedTryStream, ()>: Unpin);

    assert_impl!(Fuse<()>: Send);
    assert_not_impl!(Fuse<*const ()>: Send);
    assert_impl!(Fuse<()>: Sync);
    assert_not_impl!(Fuse<*const ()>: Sync);
    assert_impl!(Fuse<()>: Unpin);
    assert_not_impl!(Fuse<PhantomPinned>: Unpin);

    assert_impl!(FuturesOrdered<SendFuture<()>>: Send);
    assert_not_impl!(FuturesOrdered<SendFuture>: Send);
    assert_not_impl!(FuturesOrdered<SendFuture>: Send);
    assert_impl!(FuturesOrdered<SendSyncFuture<()>>: Sync);
    assert_not_impl!(FuturesOrdered<SyncFuture<()>>: Sync);
    assert_not_impl!(FuturesOrdered<SendFuture<()>>: Sync);
    assert_not_impl!(FuturesOrdered<SendSyncFuture>: Sync);
    assert_impl!(FuturesOrdered<PinnedFuture>: Unpin);

    assert_impl!(FuturesUnordered<()>: Send);
    assert_not_impl!(FuturesUnordered<*const ()>: Send);
    assert_impl!(FuturesUnordered<()>: Sync);
    assert_not_impl!(FuturesUnordered<*const ()>: Sync);
    assert_impl!(FuturesUnordered<PhantomPinned>: Unpin);

    assert_impl!(Inspect<(), ()>: Send);
    assert_not_impl!(Inspect<*const (), ()>: Send);
    assert_not_impl!(Inspect<(), *const ()>: Send);
    assert_impl!(Inspect<(), ()>: Sync);
    assert_not_impl!(Inspect<*const (), ()>: Sync);
    assert_not_impl!(Inspect<(), *const ()>: Sync);
    assert_impl!(Inspect<(), PhantomPinned>: Unpin);
    assert_not_impl!(Inspect<PhantomPinned, ()>: Unpin);

    assert_impl!(InspectErr<(), ()>: Send);
    assert_not_impl!(InspectErr<*const (), ()>: Send);
    assert_not_impl!(InspectErr<(), *const ()>: Send);
    assert_impl!(InspectErr<(), ()>: Sync);
    assert_not_impl!(InspectErr<*const (), ()>: Sync);
    assert_not_impl!(InspectErr<(), *const ()>: Sync);
    assert_impl!(InspectErr<(), PhantomPinned>: Unpin);
    assert_not_impl!(InspectErr<PhantomPinned, ()>: Unpin);

    assert_impl!(InspectOk<(), ()>: Send);
    assert_not_impl!(InspectOk<*const (), ()>: Send);
    assert_not_impl!(InspectOk<(), *const ()>: Send);
    assert_impl!(InspectOk<(), ()>: Sync);
    assert_not_impl!(InspectOk<*const (), ()>: Sync);
    assert_not_impl!(InspectOk<(), *const ()>: Sync);
    assert_impl!(InspectOk<(), PhantomPinned>: Unpin);
    assert_not_impl!(InspectOk<PhantomPinned, ()>: Unpin);

    assert_impl!(IntoAsyncRead<SendTryStream<Vec<u8>, io::Error>>: Send);
    assert_not_impl!(IntoAsyncRead<LocalTryStream<Vec<u8>, io::Error>>: Send);
    assert_impl!(IntoAsyncRead<SyncTryStream<Vec<u8>, io::Error>>: Sync);
    assert_not_impl!(IntoAsyncRead<LocalTryStream<Vec<u8>, io::Error>>: Sync);
    assert_impl!(IntoAsyncRead<UnpinTryStream<Vec<u8>, io::Error>>: Unpin);
    // IntoAsyncRead requires `St: Unpin`
    // assert_not_impl!(IntoAsyncRead<PinnedTryStream<Vec<u8>, io::Error>>: Unpin);

    assert_impl!(IntoStream<()>: Send);
    assert_not_impl!(IntoStream<*const ()>: Send);
    assert_impl!(IntoStream<()>: Sync);
    assert_not_impl!(IntoStream<*const ()>: Sync);
    assert_impl!(IntoStream<()>: Unpin);
    assert_not_impl!(IntoStream<PhantomPinned>: Unpin);

    assert_impl!(Iter<()>: Send);
    assert_not_impl!(Iter<*const ()>: Send);
    assert_impl!(Iter<()>: Sync);
    assert_not_impl!(Iter<*const ()>: Sync);
    assert_impl!(Iter<PhantomPinned>: Unpin);

    assert_impl!(Map<(), ()>: Send);
    assert_not_impl!(Map<*const (), ()>: Send);
    assert_not_impl!(Map<(), *const ()>: Send);
    assert_impl!(Map<(), ()>: Sync);
    assert_not_impl!(Map<*const (), ()>: Sync);
    assert_not_impl!(Map<(), *const ()>: Sync);
    assert_impl!(Map<(), PhantomPinned>: Unpin);
    assert_not_impl!(Map<PhantomPinned, ()>: Unpin);

    assert_impl!(MapErr<(), ()>: Send);
    assert_not_impl!(MapErr<*const (), ()>: Send);
    assert_not_impl!(MapErr<(), *const ()>: Send);
    assert_impl!(MapErr<(), ()>: Sync);
    assert_not_impl!(MapErr<*const (), ()>: Sync);
    assert_not_impl!(MapErr<(), *const ()>: Sync);
    assert_impl!(MapErr<(), PhantomPinned>: Unpin);
    assert_not_impl!(MapErr<PhantomPinned, ()>: Unpin);

    assert_impl!(MapOk<(), ()>: Send);
    assert_not_impl!(MapOk<*const (), ()>: Send);
    assert_not_impl!(MapOk<(), *const ()>: Send);
    assert_impl!(MapOk<(), ()>: Sync);
    assert_not_impl!(MapOk<*const (), ()>: Sync);
    assert_not_impl!(MapOk<(), *const ()>: Sync);
    assert_impl!(MapOk<(), PhantomPinned>: Unpin);
    assert_not_impl!(MapOk<PhantomPinned, ()>: Unpin);

    assert_impl!(Next<'_, ()>: Send);
    assert_not_impl!(Next<'_, *const ()>: Send);
    assert_impl!(Next<'_, ()>: Sync);
    assert_not_impl!(Next<'_, *const ()>: Sync);
    assert_impl!(Next<'_, ()>: Unpin);
    assert_not_impl!(Next<'_, PhantomPinned>: Unpin);

    assert_impl!(NextIf<'_, SendStream<()>, ()>: Send);
    assert_not_impl!(NextIf<'_, SendStream<()>, *const ()>: Send);
    assert_not_impl!(NextIf<'_, SendStream, ()>: Send);
    assert_not_impl!(NextIf<'_, LocalStream<()>, ()>: Send);
    assert_impl!(NextIf<'_, SyncStream<()>, ()>: Sync);
    assert_not_impl!(NextIf<'_, SyncStream<()>, *const ()>: Sync);
    assert_not_impl!(NextIf<'_, SyncStream, ()>: Sync);
    assert_not_impl!(NextIf<'_, LocalStream<()>, ()>: Send);
    assert_impl!(NextIf<'_, PinnedStream, PhantomPinned>: Unpin);

    assert_impl!(NextIfEq<'_, SendStream<()>, ()>: Send);
    assert_not_impl!(NextIfEq<'_, SendStream<()>, *const ()>: Send);
    assert_not_impl!(NextIfEq<'_, SendStream, ()>: Send);
    assert_not_impl!(NextIfEq<'_, LocalStream<()>, ()>: Send);
    assert_impl!(NextIfEq<'_, SyncStream<()>, ()>: Sync);
    assert_not_impl!(NextIfEq<'_, SyncStream<()>, *const ()>: Sync);
    assert_not_impl!(NextIfEq<'_, SyncStream, ()>: Sync);
    assert_not_impl!(NextIfEq<'_, LocalStream<()>, ()>: Send);
    assert_impl!(NextIfEq<'_, PinnedStream, PhantomPinned>: Unpin);

    assert_impl!(Once<()>: Send);
    assert_not_impl!(Once<*const ()>: Send);
    assert_impl!(Once<()>: Sync);
    assert_not_impl!(Once<*const ()>: Sync);
    assert_impl!(Once<()>: Unpin);
    assert_not_impl!(Once<PhantomPinned>: Unpin);

    assert_impl!(OrElse<(), (), ()>: Send);
    assert_not_impl!(OrElse<*const (), (), ()>: Send);
    assert_not_impl!(OrElse<(), *const (), ()>: Send);
    assert_not_impl!(OrElse<(), (), *const ()>: Send);
    assert_impl!(OrElse<(), (), ()>: Sync);
    assert_not_impl!(OrElse<*const (), (), ()>: Sync);
    assert_not_impl!(OrElse<(), *const (), ()>: Sync);
    assert_not_impl!(OrElse<(), (), *const ()>: Sync);
    assert_impl!(OrElse<(), (), PhantomPinned>: Unpin);
    assert_not_impl!(OrElse<PhantomPinned, (), ()>: Unpin);
    assert_not_impl!(OrElse<(), PhantomPinned, ()>: Unpin);

    assert_impl!(Peek<'_, SendStream<()>>: Send);
    assert_not_impl!(Peek<'_, SendStream>: Send);
    assert_not_impl!(Peek<'_, LocalStream<()>>: Send);
    assert_impl!(Peek<'_, SyncStream<()>>: Sync);
    assert_not_impl!(Peek<'_, SyncStream>: Sync);
    assert_not_impl!(Peek<'_, LocalStream<()>>: Sync);
    assert_impl!(Peek<'_, PinnedStream>: Unpin);

    assert_impl!(PeekMut<'_, SendStream<()>>: Send);
    assert_not_impl!(PeekMut<'_, SendStream>: Send);
    assert_not_impl!(PeekMut<'_, LocalStream<()>>: Send);
    assert_impl!(PeekMut<'_, SyncStream<()>>: Sync);
    assert_not_impl!(PeekMut<'_, SyncStream>: Sync);
    assert_not_impl!(PeekMut<'_, LocalStream<()>>: Sync);
    assert_impl!(PeekMut<'_, PinnedStream>: Unpin);

    assert_impl!(Peekable<SendStream<()>>: Send);
    assert_not_impl!(Peekable<SendStream>: Send);
    assert_not_impl!(Peekable<LocalStream>: Send);
    assert_impl!(Peekable<SyncStream<()>>: Sync);
    assert_not_impl!(Peekable<SyncStream>: Sync);
    assert_not_impl!(Peekable<LocalStream>: Sync);
    assert_impl!(Peekable<UnpinStream>: Unpin);
    assert_not_impl!(Peekable<PinnedStream>: Unpin);

    assert_impl!(Pending<()>: Send);
    assert_not_impl!(Pending<*const ()>: Send);
    assert_impl!(Pending<()>: Sync);
    assert_not_impl!(Pending<*const ()>: Sync);
    assert_impl!(Pending<PhantomPinned>: Unpin);

    assert_impl!(PollFn<()>: Send);
    assert_not_impl!(PollFn<*const ()>: Send);
    assert_impl!(PollFn<()>: Sync);
    assert_not_impl!(PollFn<*const ()>: Sync);
    assert_impl!(PollFn<PhantomPinned>: Unpin);

    assert_impl!(PollImmediate<SendStream>: Send);
    assert_not_impl!(PollImmediate<LocalStream<()>>: Send);
    assert_impl!(PollImmediate<SyncStream>: Sync);
    assert_not_impl!(PollImmediate<LocalStream<()>>: Sync);
    assert_impl!(PollImmediate<UnpinStream>: Unpin);
    assert_not_impl!(PollImmediate<PinnedStream>: Unpin);

    assert_impl!(ReadyChunks<SendStream<()>>: Send);
    assert_impl!(ReadyChunks<SendStream>: Send);
    assert_not_impl!(ReadyChunks<LocalStream>: Send);
    assert_impl!(ReadyChunks<SyncStream<()>>: Sync);
    assert_impl!(ReadyChunks<SyncStream>: Sync);
    assert_not_impl!(ReadyChunks<LocalStream>: Sync);
    assert_impl!(ReadyChunks<UnpinStream>: Unpin);
    assert_not_impl!(ReadyChunks<PinnedStream>: Unpin);

    assert_impl!(Repeat<()>: Send);
    assert_not_impl!(Repeat<*const ()>: Send);
    assert_impl!(Repeat<()>: Sync);
    assert_not_impl!(Repeat<*const ()>: Sync);
    assert_impl!(Repeat<PhantomPinned>: Unpin);

    assert_impl!(RepeatWith<()>: Send);
    assert_not_impl!(RepeatWith<*const ()>: Send);
    assert_impl!(RepeatWith<()>: Sync);
    assert_not_impl!(RepeatWith<*const ()>: Sync);
    // RepeatWith requires `F: FnMut() -> A`
    assert_impl!(RepeatWith<fn() -> ()>: Unpin);
    // assert_impl!(RepeatWith<PhantomPinned>: Unpin);

    assert_impl!(ReuniteError<(), ()>: Send);
    assert_not_impl!(ReuniteError<*const (), ()>: Send);
    assert_not_impl!(ReuniteError<(), *const ()>: Send);
    assert_impl!(ReuniteError<(), ()>: Sync);
    assert_not_impl!(ReuniteError<*const (), ()>: Sync);
    assert_not_impl!(ReuniteError<(), *const ()>: Sync);
    assert_impl!(ReuniteError<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(Scan<SendStream, (), (), ()>: Send);
    assert_not_impl!(Scan<LocalStream<()>, (), (), ()>: Send);
    assert_not_impl!(Scan<SendStream<()>, *const (), (), ()>: Send);
    assert_not_impl!(Scan<SendStream<()>, (), *const (), ()>: Send);
    assert_not_impl!(Scan<SendStream<()>, (), (), *const ()>: Send);
    assert_impl!(Scan<SyncStream, (), (), ()>: Sync);
    assert_not_impl!(Scan<LocalStream<()>, (), (), ()>: Sync);
    assert_not_impl!(Scan<SyncStream<()>, *const (), (), ()>: Sync);
    assert_not_impl!(Scan<SyncStream<()>, (), *const (), ()>: Sync);
    assert_not_impl!(Scan<SyncStream<()>, (), (), *const ()>: Sync);
    assert_impl!(Scan<UnpinStream, PhantomPinned, (), PhantomPinned>: Unpin);
    assert_not_impl!(Scan<PinnedStream, (), (), ()>: Unpin);
    assert_not_impl!(Scan<UnpinStream, (), PhantomPinned, ()>: Unpin);

    assert_impl!(Select<(), ()>: Send);
    assert_not_impl!(Select<*const (), ()>: Send);
    assert_not_impl!(Select<(), *const ()>: Send);
    assert_impl!(Select<(), ()>: Sync);
    assert_not_impl!(Select<*const (), ()>: Sync);
    assert_not_impl!(Select<(), *const ()>: Sync);
    assert_impl!(Select<(), ()>: Unpin);
    assert_not_impl!(Select<PhantomPinned, ()>: Unpin);
    assert_not_impl!(Select<(), PhantomPinned>: Unpin);

    assert_impl!(SelectAll<()>: Send);
    assert_not_impl!(SelectAll<*const ()>: Send);
    assert_impl!(SelectAll<()>: Sync);
    assert_not_impl!(SelectAll<*const ()>: Sync);
    assert_impl!(SelectAll<PhantomPinned>: Unpin);

    assert_impl!(SelectNextSome<'_, ()>: Send);
    assert_not_impl!(SelectNextSome<'_, *const ()>: Send);
    assert_impl!(SelectNextSome<'_, ()>: Sync);
    assert_not_impl!(SelectNextSome<'_, *const ()>: Sync);
    assert_impl!(SelectNextSome<'_, PhantomPinned>: Unpin);

    assert_impl!(Skip<()>: Send);
    assert_not_impl!(Skip<*const ()>: Send);
    assert_impl!(Skip<()>: Sync);
    assert_not_impl!(Skip<*const ()>: Sync);
    assert_impl!(Skip<()>: Unpin);
    assert_not_impl!(Skip<PhantomPinned>: Unpin);

    assert_impl!(SkipWhile<SendStream<()>, (), ()>: Send);
    assert_not_impl!(SkipWhile<LocalStream<()>, (), ()>: Send);
    assert_not_impl!(SkipWhile<SendStream, (), ()>: Send);
    assert_not_impl!(SkipWhile<SendStream<()>, *const (), ()>: Send);
    assert_not_impl!(SkipWhile<SendStream<()>, (), *const ()>: Send);
    assert_impl!(SkipWhile<SyncStream<()>, (), ()>: Sync);
    assert_not_impl!(SkipWhile<LocalStream<()>, (), ()>: Sync);
    assert_not_impl!(SkipWhile<SyncStream, (), ()>: Sync);
    assert_not_impl!(SkipWhile<SyncStream<()>, *const (), ()>: Sync);
    assert_not_impl!(SkipWhile<SyncStream<()>, (), *const ()>: Sync);
    assert_impl!(SkipWhile<UnpinStream, (), PhantomPinned>: Unpin);
    assert_not_impl!(SkipWhile<PinnedStream, (), ()>: Unpin);
    assert_not_impl!(SkipWhile<UnpinStream, PhantomPinned, ()>: Unpin);

    assert_impl!(SplitSink<(), ()>: Send);
    assert_not_impl!(SplitSink<*const (), ()>: Send);
    assert_not_impl!(SplitSink<(), *const ()>: Send);
    assert_impl!(SplitSink<(), ()>: Sync);
    assert_not_impl!(SplitSink<*const (), ()>: Sync);
    assert_not_impl!(SplitSink<(), *const ()>: Sync);
    assert_impl!(SplitSink<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(SplitStream<()>: Send);
    assert_not_impl!(SplitStream<*const ()>: Send);
    assert_impl!(SplitStream<()>: Sync);
    assert_not_impl!(SplitStream<*const ()>: Sync);
    assert_impl!(SplitStream<PhantomPinned>: Unpin);

    assert_impl!(StreamFuture<()>: Send);
    assert_not_impl!(StreamFuture<*const ()>: Send);
    assert_impl!(StreamFuture<()>: Sync);
    assert_not_impl!(StreamFuture<*const ()>: Sync);
    assert_impl!(StreamFuture<()>: Unpin);
    assert_not_impl!(StreamFuture<PhantomPinned>: Unpin);

    assert_impl!(Take<()>: Send);
    assert_not_impl!(Take<*const ()>: Send);
    assert_impl!(Take<()>: Sync);
    assert_not_impl!(Take<*const ()>: Sync);
    assert_impl!(Take<()>: Unpin);
    assert_not_impl!(Take<PhantomPinned>: Unpin);

    assert_impl!(TakeUntil<SendStream, SendFuture<()>>: Send);
    assert_not_impl!(TakeUntil<SendStream, SendFuture>: Send);
    assert_not_impl!(TakeUntil<SendStream, LocalFuture<()>>: Send);
    assert_not_impl!(TakeUntil<LocalStream, SendFuture<()>>: Send);
    assert_impl!(TakeUntil<SyncStream, SyncFuture<()>>: Sync);
    assert_not_impl!(TakeUntil<SyncStream, SyncFuture>: Sync);
    assert_not_impl!(TakeUntil<SyncStream, LocalFuture<()>>: Sync);
    assert_not_impl!(TakeUntil<LocalStream, SyncFuture<()>>: Sync);
    assert_impl!(TakeUntil<UnpinStream, UnpinFuture>: Unpin);
    assert_not_impl!(TakeUntil<PinnedStream, UnpinFuture>: Unpin);
    assert_not_impl!(TakeUntil<UnpinStream, PinnedFuture>: Unpin);

    assert_impl!(TakeWhile<SendStream<()>, (), ()>: Send);
    assert_not_impl!(TakeWhile<LocalStream<()>, (), ()>: Send);
    assert_not_impl!(TakeWhile<SendStream, (), ()>: Send);
    assert_not_impl!(TakeWhile<SendStream<()>, *const (), ()>: Send);
    assert_not_impl!(TakeWhile<SendStream<()>, (), *const ()>: Send);
    assert_impl!(TakeWhile<SyncStream<()>, (), ()>: Sync);
    assert_not_impl!(TakeWhile<LocalStream<()>, (), ()>: Sync);
    assert_not_impl!(TakeWhile<SyncStream, (), ()>: Sync);
    assert_not_impl!(TakeWhile<SyncStream<()>, *const (), ()>: Sync);
    assert_not_impl!(TakeWhile<SyncStream<()>, (), *const ()>: Sync);
    assert_impl!(TakeWhile<UnpinStream, (), PhantomPinned>: Unpin);
    assert_not_impl!(TakeWhile<PinnedStream, (), ()>: Unpin);
    assert_not_impl!(TakeWhile<UnpinStream, PhantomPinned, ()>: Unpin);

    assert_impl!(Then<SendStream, (), ()>: Send);
    assert_not_impl!(Then<LocalStream<()>, (), ()>: Send);
    assert_not_impl!(Then<SendStream<()>, *const (), ()>: Send);
    assert_not_impl!(Then<SendStream<()>, (), *const ()>: Send);
    assert_impl!(Then<SyncStream, (), ()>: Sync);
    assert_not_impl!(Then<LocalStream<()>, (), ()>: Sync);
    assert_not_impl!(Then<SyncStream<()>, *const (), ()>: Sync);
    assert_not_impl!(Then<SyncStream<()>, (), *const ()>: Sync);
    assert_impl!(Then<UnpinStream, (), PhantomPinned>: Unpin);
    assert_not_impl!(Then<PinnedStream, (), ()>: Unpin);
    assert_not_impl!(Then<UnpinStream, PhantomPinned, ()>: Unpin);

    assert_impl!(TryBufferUnordered<SendTryStream<()>>: Send);
    assert_not_impl!(TryBufferUnordered<SendTryStream>: Send);
    assert_not_impl!(TryBufferUnordered<LocalTryStream>: Send);
    assert_impl!(TryBufferUnordered<SyncTryStream<()>>: Sync);
    assert_not_impl!(TryBufferUnordered<SyncTryStream>: Sync);
    assert_not_impl!(TryBufferUnordered<LocalTryStream>: Sync);
    assert_impl!(TryBufferUnordered<UnpinTryStream>: Unpin);
    assert_not_impl!(TryBufferUnordered<PinnedTryStream>: Unpin);

    assert_impl!(TryBuffered<SendTryStream<SendTryFuture<(), ()>>>: Send);
    assert_not_impl!(TryBuffered<SendTryStream<SendTryFuture<*const (), ()>>>: Send);
    assert_not_impl!(TryBuffered<SendTryStream<SendTryFuture<(), *const ()>>>: Send);
    assert_not_impl!(TryBuffered<SendTryStream<LocalTryFuture<(), ()>>>: Send);
    assert_not_impl!(TryBuffered<LocalTryStream<SendTryFuture<(), ()>>>: Send);
    assert_impl!(TryBuffered<SyncTryStream<SendSyncTryFuture<(), ()>>>: Sync);
    assert_not_impl!(TryBuffered<SyncTryStream<SendSyncTryFuture<*const (), ()>>>: Sync);
    assert_not_impl!(TryBuffered<SyncTryStream<SendSyncTryFuture<(), *const ()>>>: Sync);
    assert_not_impl!(TryBuffered<SyncTryStream<SendTryFuture<(), ()>>>: Sync);
    assert_not_impl!(TryBuffered<SyncTryStream<SyncTryFuture<(), ()>>>: Sync);
    assert_not_impl!(TryBuffered<LocalTryStream<SendSyncTryFuture<(), ()>>>: Sync);
    assert_impl!(TryBuffered<UnpinTryStream<PinnedTryFuture>>: Unpin);
    assert_not_impl!(TryBuffered<PinnedTryStream<UnpinTryFuture>>: Unpin);

    assert_impl!(TryCollect<(), ()>: Send);
    assert_not_impl!(TryCollect<*const (), ()>: Send);
    assert_not_impl!(TryCollect<(), *const ()>: Send);
    assert_impl!(TryCollect<(), ()>: Sync);
    assert_not_impl!(TryCollect<*const (), ()>: Sync);
    assert_not_impl!(TryCollect<(), *const ()>: Sync);
    assert_impl!(TryCollect<(), PhantomPinned>: Unpin);
    assert_not_impl!(TryCollect<PhantomPinned, ()>: Unpin);

    assert_impl!(TryConcat<SendTryStream<()>>: Send);
    assert_not_impl!(TryConcat<SendTryStream>: Send);
    assert_not_impl!(TryConcat<LocalTryStream>: Send);
    assert_impl!(TryConcat<SyncTryStream<()>>: Sync);
    assert_not_impl!(TryConcat<SyncTryStream>: Sync);
    assert_not_impl!(TryConcat<LocalTryStream>: Sync);
    assert_impl!(TryConcat<UnpinTryStream>: Unpin);
    assert_not_impl!(TryConcat<PinnedTryStream>: Unpin);

    assert_impl!(TryFilter<SendTryStream<()>, (), ()>: Send);
    assert_not_impl!(TryFilter<LocalTryStream<()>, (), ()>: Send);
    assert_not_impl!(TryFilter<SendTryStream, (), ()>: Send);
    assert_not_impl!(TryFilter<SendTryStream<()>, *const (), ()>: Send);
    assert_not_impl!(TryFilter<SendTryStream<()>, (), *const ()>: Send);
    assert_impl!(TryFilter<SyncTryStream<()>, (), ()>: Sync);
    assert_not_impl!(TryFilter<LocalTryStream<()>, (), ()>: Sync);
    assert_not_impl!(TryFilter<SyncTryStream, (), ()>: Sync);
    assert_not_impl!(TryFilter<SyncTryStream<()>, *const (), ()>: Sync);
    assert_not_impl!(TryFilter<SyncTryStream<()>, (), *const ()>: Sync);
    assert_impl!(TryFilter<UnpinTryStream, (), PhantomPinned>: Unpin);
    assert_not_impl!(TryFilter<PinnedTryStream, (), ()>: Unpin);
    assert_not_impl!(TryFilter<UnpinTryStream, PhantomPinned, ()>: Unpin);

    assert_impl!(TryFilterMap<(), (), ()>: Send);
    assert_not_impl!(TryFilterMap<*const (), (), ()>: Send);
    assert_not_impl!(TryFilterMap<(), *const (), ()>: Send);
    assert_not_impl!(TryFilterMap<(), (), *const ()>: Send);
    assert_impl!(TryFilterMap<(), (), ()>: Sync);
    assert_not_impl!(TryFilterMap<*const (), (), ()>: Sync);
    assert_not_impl!(TryFilterMap<(), *const (), ()>: Sync);
    assert_not_impl!(TryFilterMap<(), (), *const ()>: Sync);
    assert_impl!(TryFilterMap<(), (), PhantomPinned>: Unpin);
    assert_not_impl!(TryFilterMap<PhantomPinned, (), ()>: Unpin);
    assert_not_impl!(TryFilterMap<(), PhantomPinned, ()>: Unpin);

    assert_impl!(TryFlatten<SendTryStream<()>>: Send);
    assert_not_impl!(TryFlatten<SendTryStream>: Send);
    assert_not_impl!(TryFlatten<SendTryStream>: Send);
    assert_impl!(TryFlatten<SyncTryStream<()>>: Sync);
    assert_not_impl!(TryFlatten<LocalTryStream<()>>: Sync);
    assert_not_impl!(TryFlatten<LocalTryStream<()>>: Sync);
    assert_impl!(TryFlatten<UnpinTryStream<()>>: Unpin);
    assert_not_impl!(TryFlatten<UnpinTryStream>: Unpin);
    assert_not_impl!(TryFlatten<PinnedTryStream>: Unpin);

    assert_impl!(TryFold<(), (), (), ()>: Send);
    assert_not_impl!(TryFold<*const (), (), (), ()>: Send);
    assert_not_impl!(TryFold<(), *const (), (), ()>: Send);
    assert_not_impl!(TryFold<(), (), *const (), ()>: Send);
    assert_not_impl!(TryFold<(), (), (), *const ()>: Send);
    assert_impl!(TryFold<(), (), (), ()>: Sync);
    assert_not_impl!(TryFold<*const (), (), (), ()>: Sync);
    assert_not_impl!(TryFold<(), *const (), (), ()>: Sync);
    assert_not_impl!(TryFold<(), (), *const (), ()>: Sync);
    assert_not_impl!(TryFold<(), (), (), *const ()>: Sync);
    assert_impl!(TryFold<(), (), PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(TryFold<PhantomPinned, (), (), ()>: Unpin);
    assert_not_impl!(TryFold<(), PhantomPinned, (), ()>: Unpin);

    assert_impl!(TryForEach<(), (), ()>: Send);
    assert_not_impl!(TryForEach<*const (), (), ()>: Send);
    assert_not_impl!(TryForEach<(), *const (), ()>: Send);
    assert_not_impl!(TryForEach<(), (), *const ()>: Send);
    assert_impl!(TryForEach<(), (), ()>: Sync);
    assert_not_impl!(TryForEach<*const (), (), ()>: Sync);
    assert_not_impl!(TryForEach<(), *const (), ()>: Sync);
    assert_not_impl!(TryForEach<(), (), *const ()>: Sync);
    assert_impl!(TryForEach<(), (), PhantomPinned>: Unpin);
    assert_not_impl!(TryForEach<PhantomPinned, (), ()>: Unpin);
    assert_not_impl!(TryForEach<(), PhantomPinned, ()>: Unpin);

    assert_impl!(TryForEachConcurrent<(), (), ()>: Send);
    assert_not_impl!(TryForEachConcurrent<*const (), (), ()>: Send);
    assert_not_impl!(TryForEachConcurrent<(), *const (), ()>: Send);
    assert_not_impl!(TryForEachConcurrent<(), (), *const ()>: Send);
    assert_impl!(TryForEachConcurrent<(), (), ()>: Sync);
    assert_not_impl!(TryForEachConcurrent<*const (), (), ()>: Sync);
    assert_not_impl!(TryForEachConcurrent<(), *const (), ()>: Sync);
    assert_not_impl!(TryForEachConcurrent<(), (), *const ()>: Sync);
    assert_impl!(TryForEachConcurrent<(), PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(TryForEachConcurrent<PhantomPinned, (), ()>: Unpin);

    assert_impl!(TryNext<'_, ()>: Send);
    assert_not_impl!(TryNext<'_, *const ()>: Send);
    assert_impl!(TryNext<'_, ()>: Sync);
    assert_not_impl!(TryNext<'_, *const ()>: Sync);
    assert_impl!(TryNext<'_, ()>: Unpin);
    assert_not_impl!(TryNext<'_, PhantomPinned>: Unpin);

    assert_impl!(TrySkipWhile<SendTryStream<()>, (), ()>: Send);
    assert_not_impl!(TrySkipWhile<LocalTryStream<()>, (), ()>: Send);
    assert_not_impl!(TrySkipWhile<SendTryStream, (), ()>: Send);
    assert_not_impl!(TrySkipWhile<SendTryStream<()>, *const (), ()>: Send);
    assert_not_impl!(TrySkipWhile<SendTryStream<()>, (), *const ()>: Send);
    assert_impl!(TrySkipWhile<SyncTryStream<()>, (), ()>: Sync);
    assert_not_impl!(TrySkipWhile<LocalTryStream<()>, (), ()>: Sync);
    assert_not_impl!(TrySkipWhile<SyncTryStream, (), ()>: Sync);
    assert_not_impl!(TrySkipWhile<SyncTryStream<()>, *const (), ()>: Sync);
    assert_not_impl!(TrySkipWhile<SyncTryStream<()>, (), *const ()>: Sync);
    assert_impl!(TrySkipWhile<UnpinTryStream, (), PhantomPinned>: Unpin);
    assert_not_impl!(TrySkipWhile<PinnedTryStream, (), ()>: Unpin);
    assert_not_impl!(TrySkipWhile<UnpinTryStream, PhantomPinned, ()>: Unpin);

    assert_impl!(TryTakeWhile<SendTryStream<()>, (), ()>: Send);
    assert_not_impl!(TryTakeWhile<LocalTryStream<()>, (), ()>: Send);
    assert_not_impl!(TryTakeWhile<SendTryStream, (), ()>: Send);
    assert_not_impl!(TryTakeWhile<SendTryStream<()>, *const (), ()>: Send);
    assert_not_impl!(TryTakeWhile<SendTryStream<()>, (), *const ()>: Send);
    assert_impl!(TryTakeWhile<SyncTryStream<()>, (), ()>: Sync);
    assert_not_impl!(TryTakeWhile<LocalTryStream<()>, (), ()>: Sync);
    assert_not_impl!(TryTakeWhile<SyncTryStream, (), ()>: Sync);
    assert_not_impl!(TryTakeWhile<SyncTryStream<()>, *const (), ()>: Sync);
    assert_not_impl!(TryTakeWhile<SyncTryStream<()>, (), *const ()>: Sync);
    assert_impl!(TryTakeWhile<UnpinTryStream, (), PhantomPinned>: Unpin);
    assert_not_impl!(TryTakeWhile<PinnedTryStream, (), ()>: Unpin);
    assert_not_impl!(TryTakeWhile<UnpinTryStream, PhantomPinned, ()>: Unpin);

    assert_impl!(TryUnfold<(), (), ()>: Send);
    assert_not_impl!(TryUnfold<*const (), (), ()>: Send);
    assert_not_impl!(TryUnfold<(), *const (), ()>: Send);
    assert_not_impl!(TryUnfold<(), (), *const ()>: Send);
    assert_impl!(TryUnfold<(), (), ()>: Sync);
    assert_not_impl!(TryUnfold<*const (), (), ()>: Sync);
    assert_not_impl!(TryUnfold<(), *const (), ()>: Sync);
    assert_not_impl!(TryUnfold<(), (), *const ()>: Sync);
    assert_impl!(TryUnfold<PhantomPinned, PhantomPinned, ()>: Unpin);
    assert_not_impl!(TryUnfold<(), (), PhantomPinned>: Unpin);

    assert_impl!(Unfold<(), (), ()>: Send);
    assert_not_impl!(Unfold<*const (), (), ()>: Send);
    assert_not_impl!(Unfold<(), *const (), ()>: Send);
    assert_not_impl!(Unfold<(), (), *const ()>: Send);
    assert_impl!(Unfold<(), (), ()>: Sync);
    assert_not_impl!(Unfold<*const (), (), ()>: Sync);
    assert_not_impl!(Unfold<(), *const (), ()>: Sync);
    assert_not_impl!(Unfold<(), (), *const ()>: Sync);
    assert_impl!(Unfold<PhantomPinned, PhantomPinned, ()>: Unpin);
    assert_not_impl!(Unfold<(), (), PhantomPinned>: Unpin);

    assert_impl!(Unzip<(), (), ()>: Send);
    assert_not_impl!(Unzip<*const (), (), ()>: Send);
    assert_not_impl!(Unzip<(), *const (), ()>: Send);
    assert_not_impl!(Unzip<(), (), *const ()>: Send);
    assert_impl!(Unzip<(), (), ()>: Sync);
    assert_not_impl!(Unzip<*const (), (), ()>: Sync);
    assert_not_impl!(Unzip<(), *const (), ()>: Sync);
    assert_not_impl!(Unzip<(), (), *const ()>: Sync);
    assert_impl!(Unzip<(), PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(Unzip<PhantomPinned, (), ()>: Unpin);

    assert_impl!(Zip<SendStream<()>, SendStream<()>>: Send);
    assert_not_impl!(Zip<SendStream, SendStream<()>>: Send);
    assert_not_impl!(Zip<SendStream<()>, SendStream>: Send);
    assert_not_impl!(Zip<LocalStream, SendStream<()>>: Send);
    assert_not_impl!(Zip<SendStream<()>, LocalStream>: Send);
    assert_impl!(Zip<SyncStream<()>, SyncStream<()>>: Sync);
    assert_not_impl!(Zip<SyncStream, SyncStream<()>>: Sync);
    assert_not_impl!(Zip<SyncStream<()>, SyncStream>: Sync);
    assert_not_impl!(Zip<LocalStream, SyncStream<()>>: Sync);
    assert_not_impl!(Zip<SyncStream<()>, LocalStream>: Sync);
    assert_impl!(Zip<UnpinStream, UnpinStream>: Unpin);
    assert_not_impl!(Zip<UnpinStream, PinnedStream>: Unpin);
    assert_not_impl!(Zip<PinnedStream, UnpinStream>: Unpin);

    assert_impl!(futures_unordered::Iter<'_, ()>: Send);
    assert_not_impl!(futures_unordered::Iter<'_, *const ()>: Send);
    assert_impl!(futures_unordered::Iter<'_, ()>: Sync);
    assert_not_impl!(futures_unordered::Iter<'_, *const ()>: Sync);
    assert_impl!(futures_unordered::Iter<'_, ()>: Unpin);
    // The definition of futures_unordered::Iter has `Fut: Unpin` bounds.
    // assert_not_impl!(futures_unordered::Iter<'_, PhantomPinned>: Unpin);

    assert_impl!(futures_unordered::IterMut<'_, ()>: Send);
    assert_not_impl!(futures_unordered::IterMut<'_, *const ()>: Send);
    assert_impl!(futures_unordered::IterMut<'_, ()>: Sync);
    assert_not_impl!(futures_unordered::IterMut<'_, *const ()>: Sync);
    assert_impl!(futures_unordered::IterMut<'_, ()>: Unpin);
    // The definition of futures_unordered::IterMut has `Fut: Unpin` bounds.
    // assert_not_impl!(futures_unordered::IterMut<'_, PhantomPinned>: Unpin);

    assert_impl!(futures_unordered::IterPinMut<'_, ()>: Send);
    assert_not_impl!(futures_unordered::IterPinMut<'_, *const ()>: Send);
    assert_impl!(futures_unordered::IterPinMut<'_, ()>: Sync);
    assert_not_impl!(futures_unordered::IterPinMut<'_, *const ()>: Sync);
    assert_impl!(futures_unordered::IterPinMut<'_, PhantomPinned>: Unpin);

    assert_impl!(futures_unordered::IterPinRef<'_, ()>: Send);
    assert_not_impl!(futures_unordered::IterPinRef<'_, *const ()>: Send);
    assert_impl!(futures_unordered::IterPinRef<'_, ()>: Sync);
    assert_not_impl!(futures_unordered::IterPinRef<'_, *const ()>: Sync);
    assert_impl!(futures_unordered::IterPinRef<'_, PhantomPinned>: Unpin);

    assert_impl!(futures_unordered::IntoIter<()>: Send);
    assert_not_impl!(futures_unordered::IntoIter<*const ()>: Send);
    assert_impl!(futures_unordered::IntoIter<()>: Sync);
    assert_not_impl!(futures_unordered::IntoIter<*const ()>: Sync);
    // The definition of futures_unordered::IntoIter has `Fut: Unpin` bounds.
    // assert_not_impl!(futures_unordered::IntoIter<PhantomPinned>: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::task`.
mod task {
    use super::*;
    use futures::task::*;

    assert_impl!(AtomicWaker: Send);
    assert_impl!(AtomicWaker: Sync);
    assert_impl!(AtomicWaker: Unpin);

    assert_impl!(FutureObj<'_, *const ()>: Send);
    assert_not_impl!(FutureObj<'_, ()>: Sync);
    assert_impl!(FutureObj<'_, PhantomPinned>: Unpin);

    assert_not_impl!(LocalFutureObj<'_, ()>: Send);
    assert_not_impl!(LocalFutureObj<'_, ()>: Sync);
    assert_impl!(LocalFutureObj<'_, PhantomPinned>: Unpin);

    assert_impl!(SpawnError: Send);
    assert_impl!(SpawnError: Sync);
    assert_impl!(SpawnError: Unpin);

    assert_impl!(WakerRef<'_>: Send);
    assert_impl!(WakerRef<'_>: Sync);
    assert_impl!(WakerRef<'_>: Unpin);
}
