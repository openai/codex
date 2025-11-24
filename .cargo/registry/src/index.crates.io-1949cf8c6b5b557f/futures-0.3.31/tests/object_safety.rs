fn assert_is_object_safe<T>() {}

#[test]
fn future() {
    // `FutureExt`, `TryFutureExt` and `UnsafeFutureObj` are not object safe.
    use futures::future::{FusedFuture, Future, TryFuture};

    assert_is_object_safe::<&dyn Future<Output = ()>>();
    assert_is_object_safe::<&dyn FusedFuture<Output = ()>>();
    assert_is_object_safe::<&dyn TryFuture<Ok = (), Error = (), Output = Result<(), ()>>>();
}

#[test]
fn stream() {
    // `StreamExt` and `TryStreamExt` are not object safe.
    use futures::stream::{FusedStream, Stream, TryStream};

    assert_is_object_safe::<&dyn Stream<Item = ()>>();
    assert_is_object_safe::<&dyn FusedStream<Item = ()>>();
    assert_is_object_safe::<&dyn TryStream<Ok = (), Error = (), Item = Result<(), ()>>>();
}

#[test]
fn sink() {
    // `SinkExt` is not object safe.
    use futures::sink::Sink;

    assert_is_object_safe::<&dyn Sink<(), Error = ()>>();
}

#[test]
fn io() {
    // `AsyncReadExt`, `AsyncWriteExt`, `AsyncSeekExt` and `AsyncBufReadExt` are not object safe.
    use futures::io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite};

    assert_is_object_safe::<&dyn AsyncRead>();
    assert_is_object_safe::<&dyn AsyncWrite>();
    assert_is_object_safe::<&dyn AsyncSeek>();
    assert_is_object_safe::<&dyn AsyncBufRead>();
}

#[test]
fn task() {
    // `ArcWake`, `SpawnExt` and `LocalSpawnExt` are not object safe.
    use futures::task::{LocalSpawn, Spawn};

    assert_is_object_safe::<&dyn Spawn>();
    assert_is_object_safe::<&dyn LocalSpawn>();
}
