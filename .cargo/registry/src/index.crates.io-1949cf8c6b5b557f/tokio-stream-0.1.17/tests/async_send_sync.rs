#![allow(clippy::diverging_sub_expression)]

use std::rc::Rc;

#[allow(dead_code)]
type BoxStream<T> = std::pin::Pin<Box<dyn tokio_stream::Stream<Item = T>>>;

#[allow(dead_code)]
fn require_send<T: Send>(_t: &T) {}
#[allow(dead_code)]
fn require_sync<T: Sync>(_t: &T) {}
#[allow(dead_code)]
fn require_unpin<T: Unpin>(_t: &T) {}

#[allow(dead_code)]
struct Invalid;

#[allow(unused)]
trait AmbiguousIfSend<A> {
    fn some_item(&self) {}
}
impl<T: ?Sized> AmbiguousIfSend<()> for T {}
impl<T: ?Sized + Send> AmbiguousIfSend<Invalid> for T {}

#[allow(unused)]
trait AmbiguousIfSync<A> {
    fn some_item(&self) {}
}
impl<T: ?Sized> AmbiguousIfSync<()> for T {}
impl<T: ?Sized + Sync> AmbiguousIfSync<Invalid> for T {}

#[allow(unused)]
trait AmbiguousIfUnpin<A> {
    fn some_item(&self) {}
}
impl<T: ?Sized> AmbiguousIfUnpin<()> for T {}
impl<T: ?Sized + Unpin> AmbiguousIfUnpin<Invalid> for T {}

macro_rules! into_todo {
    ($typ:ty) => {{
        let x: $typ = todo!();
        x
    }};
}

macro_rules! async_assert_fn {
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): Send & Sync) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            require_send(&f);
            require_sync(&f);
        };
    };
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): Send & !Sync) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            require_send(&f);
            AmbiguousIfSync::some_item(&f);
        };
    };
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): !Send & Sync) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            AmbiguousIfSend::some_item(&f);
            require_sync(&f);
        };
    };
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): !Send & !Sync) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            AmbiguousIfSend::some_item(&f);
            AmbiguousIfSync::some_item(&f);
        };
    };
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): !Unpin) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            AmbiguousIfUnpin::some_item(&f);
        };
    };
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): Unpin) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            require_unpin(&f);
        };
    };
}

async_assert_fn!(tokio_stream::empty<Rc<u8>>(): Send & Sync);
async_assert_fn!(tokio_stream::pending<Rc<u8>>(): Send & Sync);
async_assert_fn!(tokio_stream::iter(std::vec::IntoIter<u8>): Send & Sync);

async_assert_fn!(tokio_stream::StreamExt::next(&mut BoxStream<()>): !Unpin);
async_assert_fn!(tokio_stream::StreamExt::try_next(&mut BoxStream<Result<(), ()>>): !Unpin);
async_assert_fn!(tokio_stream::StreamExt::all(&mut BoxStream<()>, fn(())->bool): !Unpin);
async_assert_fn!(tokio_stream::StreamExt::any(&mut BoxStream<()>, fn(())->bool): !Unpin);
async_assert_fn!(tokio_stream::StreamExt::fold(&mut BoxStream<()>, (), fn((), ())->()): !Unpin);
async_assert_fn!(tokio_stream::StreamExt::collect<Vec<()>>(&mut BoxStream<()>): !Unpin);
