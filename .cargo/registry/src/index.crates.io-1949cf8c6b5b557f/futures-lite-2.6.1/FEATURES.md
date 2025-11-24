# Intentional Occlusions from `futures-lite`

[`futures-lite`] has an API that is deliberately smaller than the [`futures`]
crate. This allows it to compile significantly faster and have fewer 
dependencies.

This fact does not mean that [`futures-lite`] is not open to new feature 
requests. However it does mean that any proposed new features are subject to
scrutiny to determine whether or not they are truly necessary for this crate.
In many cases there are much simpler ways to implement these features, or they
would be a much better fit for an external crate.

This document aims to describe all intentional feature occlusions and provide
suggestions for how these features can be used in the context of 
[`futures-lite`]. If you have a feature request that you believe does not fall
under any of the following occlusions, please open an issue on the
[official `futures-lite` bug tracker](https://github.com/smol-rs/futures-lite/issues).

## Simple Combinators 

In general, anything that can be implemented in terms of `async`/`await` syntax
is not implemented in [`futures-lite`]. This is done to encourage the use of
modern `async`/`await` syntax rather than [`futures`] v1.0 combinator chaining.

As an example, take the [`map`] method in [`futures`]. It takes a future and
processes its output through a closure.

```rust
let my_future = async { 1 };

// Add one to the result of `my_future`.
let mapped_future = my_future.map(|x| x + 1);

assert_eq!(mapped_future.await, 2);
```

However, this does not need to be implemented in the form of a combinator. With
`async`/`await` syntax, you can simply `await` on `my_future` in an `async`
block, then process its output. The following code is equivalent to the above,
but doesn't use a combinator.

```rust
let my_future = async { 1 };

// Add one to the result of `my_future`.
let mapped_future = async move { my_future.await + 1 };

assert_eq!(mapped_future.await, 2);
```

By not implementing combinators that can be implemented in terms of `async`,
[`futures-lite`] has a significantly smaller API that still has roughly the
same amount of power as [`futures`].

As part of this policy, the [`TryFutureExt`] trait is not implemented. All of
its methods can be implemented by just using `async`/`await` combined with
other simpler future combinators. For instance, consider [`and_then`]:

```rust
let my_future = async { Ok(2) };

let and_then = my_future.and_then(|x| async move {
    Ok(x + 1)
});

assert_eq!(and_then.await.unwrap(), 3);
```

This can be implemented with an `async` block and the normal `and_then`
combinator.

```rust
let my_future = async { Ok(2) };

let and_then = async move {
    let x = my_future.await;
    x.and_then(|x| x + 1)
};

assert_eq!(and_then.await.unwrap(), 3);
```

One drawback of this approach is that `async` blocks are not named types. So
if a trait (like [`Service`]) requires a named future type it cannot be
returned.

```rust
impl Service for MyService {
    type Future = /* ??? */;

    fn call(&mut self) -> Self::Future {
        async { 1 + 1 }
    }    
}
```

One possible solution is to box the future and return a dynamic dispatch 
object, but in many cases this adds non trivial overhead.

```rust
impl Service for MyService {
    type Future = Pin<Box<dyn Future<Output = i32>>>;

    fn call(&mut self) -> Self::Future {
        async { 1 + 1 }.boxed_local()
    }    
}
```

This problem is expected to be resolved in the future, thanks to
[`async` fn in traits] and [TAIT]. At this point we would rather wait for these
better solutions than significantly expand [`futures-lite`]'s API. If this is a
deal breaker for you, [`futures`] is probably better for your use case.

## Asynchronous Closures

As a pattern, most combinators in [`futures-lite`] take regular closures rather
than `async` closures. For example:

```rust
// In `futures`, the `all` combinator takes a closure returning a future.
my_stream.all(|x| async move { x > 5 }).await;

// In `futures-lite`, the `all` combinator just takes a closure.
my_stream.all(|x| x > 5).await;
```

This strategy is taken for two primary reasons. 

First of all, it is significantly simpler to implement. Since we don't need to 
keep track of whether we are currently `poll`ing a future or not it makes the 
combinators an order of magnitude easier to write.

Second of all it avoids the common [`futures`] wart of needing to pass trivial
values into `async move { ... }` or `future::ready(...)` for the vast
majority of operations.

For futures, combinators that would normally require `async` closures can 
usually be implemented in terms of `async`/`await`. See the above section for
more information on that. For streams, the [`then`] combinator is one of the 
few that actually takes an `async` closure, and can therefore be used to
implement operations that would normally need `async` closures.

```rust
// In `futures`.
my_stream.all(|x| my_async_fn(x)).await;

// In `futures-lite`, use `then` and pass the result to `all`.
my_stream.then(|x| my_async_fn(x)).all(|pass| pass).await;
```

## Higher-Order Concurrency

[`futures`] provides a number of primitives and combinators that allow for
polling a significant number of futures at once. Examples of this include
[`for_each_concurrent`] and [`FuturesUnordered`].

[`futures-lite`] provides simple primitives like [`race`] and [`zip`]. However
these don't really scale to handling more than two futures at once. It has
been proposed in the past to add deeper concurrency primitives to
[`futures-lite`]. However our current stance is that such primitives would
represent a significant uptick in complexity and thus is better suited to
other crates.

[`futures-concurrency`] provides a number of simple APIs for dealing with
fixed numbers of futures. For example, here is an example for waiting on
multiple futures to complete.

```rust
let (a, b, c) = /* assume these are all futures */;

// futures
let (x, y, z) = join!(a, b, c);

// futures-concurrency
use futures_concurrency::prelude::*;
let (x, y, z) = (a, b, c).join().await;
```

For large or variable numbers of futures it is recommended to use an executor
instead. [`smol`] provides both an [`Executor`] and a [`LocalExecutor`] 
depending on the flavor of your program.

@notgull has a [blog post](https://notgull.net/futures-concurrency-in-smol/) 
describing this in greater detail.

To explicitly answer a frequently asked question, the popular [`select`] macro
can be implemented by using simple `async`/`await` and a race combinator.

```rust
let (a, b, c) = /* assume these are all futures */;

// futures
let x = select! {
   a_res = a => a_res + 1,
   _ = b => 0,
   c_res = c => c_res + 3,
};

// futures-concurrency
let x = (
    async move { a.await + 1 },
    async move { b.await; 0 },
    async move { c.await + 3 }
).race().await;
```

## Sink Trait

[`futures`] offers a [`Sink`] trait that is in many ways the opposite of the
[`Stream`] trait. Rather than asynchronously producing values, the point of the
[`Sink`] is to asynchronously receive values.

[`futures-lite`] and the rest of [`smol`] intentionally does not support the
[`Sink`] trait. [`Sink`] is a relic from the old [`futures`] v0.1 days where
I/O was tied directly into the API. The `Error` subtype is wholly unnecessary
and makes the API significantly harder to use. In addition the multi-call
requirement makes the API harder to both use and implement. It increases the
complexity of any futures that use it significantly, and its API necessitates
that implementors have an internal buffer for objects.

In short, the ideal [`Sink`] API would be if it was replaced with this trait.

*Sidenote: [`Stream`], [`AsyncRead`] and [`AsyncWrite`] suffer from this same
problem to an extent. I think they could also be fixed by transforming their
`fn poll_[X]` functions into `async fn [X]` functions. However their APIs are
not broken to the point that [`Sink`]'s is.*

In order to avoid relying on a broken API, [`futures-lite`] does not import
[`Sink`] or expose any APIs that build upon [`Sink`]. Unfortunately some crates
make their only accessible API the [`Sink`] call. Ideally instead they would
just have an `async fn send()` function.

## Out-of-scope modules

[`futures`] provides several sets of tools that are out of scope for
[`futures-lite`]. Usually these are implemented in external crates, some of
which depend on [`futures-lite`] themselves. Here are examples of these
primitives:

- **Channels:** [`async-channel`] provides an asynchronous MPMC channel, while
  [`oneshot`] provides an asynchronous oneshot channel.
- **Mutex:** [`async-lock`] provides asynchronous mutexes, alongside other
  locking primitives.
- **Atomic Wakers:** [`atomic-waker`] provides standalone atomic wakers.
- **Executors:** [`async-executor`] provides [`Executor`] to replace
  `ThreadPool` and [`LocalExecutor`] to replace `LocalPool`.

[`smol`]: https://crates.io/crates/smol
[`futures-lite`]: https://crates.io/crates/futures-lite
[`futures`]: https://crates.io/crates/futures
[`map`]: https://docs.rs/futures/latest/futures/future/trait.FutureExt.html#method.map
[`TryFutureExt`]: https://docs.rs/futures/latest/futures/future/trait.TryFutureExt.html
[`and_then`]: https://docs.rs/futures/latest/futures/future/trait.TryFutureExt.html#method.and_then
[`Service`]: https://docs.rs/tower-service/latest/tower_service/trait.Service.html
[`async` fn in traits]: https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits.html
[TAIT]: https://rust-lang.github.io/impl-trait-initiative/explainer/tait.html
[`then`]: https://docs.rs/futures-lite/latest/futures_lite/stream/trait.StreamExt.html#method.then
[`FuturesUnordered`]: https://docs.rs/futures/latest/futures/stream/struct.FuturesUnordered.html
[`for_each_concurrent`]: https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.for_each_concurrent
[`race`]: https://docs.rs/futures-lite/latest/futures_lite/future/fn.race.html
[`zip`]: https://docs.rs/futures-lite/latest/futures_lite/future/fn.zip.html 
[`futures-concurrency`]: https://docs.rs/futures-concurrency/latest/futures_concurrency/
[`Executor`]: https://docs.rs/async-executor/latest/async_executor/struct.Executor.html
[`LocalExecutor`]: https://docs.rs/async-executor/latest/async_executor/struct.LocalExecutor.html
[`select`]: https://docs.rs/futures/latest/futures/macro.select.html
[`Sink`]: https://docs.rs/futures/latest/futures/sink/trait.Sink.html
[`Stream`]: https://docs.rs/futures-core/latest/futures_core/stream/trait.Stream.html
[`AsyncRead`]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncRead.html
[`AsyncWrite`]: https://docs.rs/futures-io/latest/futures_io/trait.AsyncWrite.html
[`async-channel`]: https://crates.io/crates/async-channel
[`async-lock`]: https://crates.io/crates/async-lock
[`async-executor`]: https://crates.io/crates/async-executor
[`oneshot`]: https://crates.io/crates/oneshot
[`atomic-waker`]: https://crates.io/crates/atomic-waker
