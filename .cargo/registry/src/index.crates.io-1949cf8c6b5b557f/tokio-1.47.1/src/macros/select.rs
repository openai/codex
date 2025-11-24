macro_rules! doc {
    ($select:item) => {
        /// Waits on multiple concurrent branches, returning when the **first** branch
        /// completes, cancelling the remaining branches.
        ///
        /// The `select!` macro must be used inside of async functions, closures, and
        /// blocks.
        ///
        /// The `select!` macro accepts one or more branches with the following pattern:
        ///
        /// ```text
        /// <pattern> = <async expression> (, if <precondition>)? => <handler>,
        /// ```
        ///
        /// Additionally, the `select!` macro may include a single, optional `else`
        /// branch, which evaluates if none of the other branches match their patterns:
        ///
        /// ```text
        /// else => <expression>
        /// ```
        ///
        /// The macro aggregates all `<async expression>` expressions and runs them
        /// concurrently on the **current** task. Once the **first** expression
        /// completes with a value that matches its `<pattern>`, the `select!` macro
        /// returns the result of evaluating the completed branch's `<handler>`
        /// expression.
        ///
        /// Additionally, each branch may include an optional `if` precondition. If the
        /// precondition returns `false`, then the branch is disabled. The provided
        /// `<async expression>` is still evaluated but the resulting future is never
        /// polled. This capability is useful when using `select!` within a loop.
        ///
        /// The complete lifecycle of a `select!` expression is as follows:
        ///
        /// 1. Evaluate all provided `<precondition>` expressions. If the precondition
        ///    returns `false`, disable the branch for the remainder of the current call
        ///    to `select!`. Re-entering `select!` due to a loop clears the "disabled"
        ///    state.
        /// 2. Aggregate the `<async expression>`s from each branch, including the
        ///    disabled ones. If the branch is disabled, `<async expression>` is still
        ///    evaluated, but the resulting future is not polled.
        /// 3. If **all** branches are disabled: go to step 6.
        /// 4. Concurrently await on the results for all remaining `<async expression>`s.
        /// 5. Once an `<async expression>` returns a value, attempt to apply the value to the
        ///    provided `<pattern>`. If the pattern matches, evaluate the `<handler>` and return.
        ///    If the pattern **does not** match, disable the current branch for the remainder of
        ///    the current call to `select!`. Continue from step 3.
        /// 6. Evaluate the `else` expression. If no else expression is provided, panic.
        ///
        /// # Runtime characteristics
        ///
        /// By running all async expressions on the current task, the expressions are
        /// able to run **concurrently** but not in **parallel**. This means all
        /// expressions are run on the same thread and if one branch blocks the thread,
        /// all other expressions will be unable to continue. If parallelism is
        /// required, spawn each async expression using [`tokio::spawn`] and pass the
        /// join handle to `select!`.
        ///
        /// [`tokio::spawn`]: crate::spawn
        ///
        /// # Fairness
        ///
        /// By default, `select!` randomly picks a branch to check first. This provides
        /// some level of fairness when calling `select!` in a loop with branches that
        /// are always ready.
        ///
        /// This behavior can be overridden by adding `biased;` to the beginning of the
        /// macro usage. See the examples for details. This will cause `select` to poll
        /// the futures in the order they appear from top to bottom. There are a few
        /// reasons you may want this:
        ///
        /// - The random number generation of `tokio::select!` has a non-zero CPU cost
        /// - Your futures may interact in a way where known polling order is significant
        ///
        /// But there is an important caveat to this mode. It becomes your responsibility
        /// to ensure that the polling order of your futures is fair. If for example you
        /// are selecting between a stream and a shutdown future, and the stream has a
        /// huge volume of messages and zero or nearly zero time between them, you should
        /// place the shutdown future earlier in the `select!` list to ensure that it is
        /// always polled, and will not be ignored due to the stream being constantly
        /// ready.
        ///
        /// # Panics
        ///
        /// The `select!` macro panics if all branches are disabled **and** there is no
        /// provided `else` branch. A branch is disabled when the provided `if`
        /// precondition returns `false` **or** when the pattern does not match the
        /// result of `<async expression>`.
        ///
        /// # Cancellation safety
        ///
        /// When using `select!` in a loop to receive messages from multiple sources,
        /// you should make sure that the receive call is cancellation safe to avoid
        /// losing messages. This section goes through various common methods and
        /// describes whether they are cancel safe.  The lists in this section are not
        /// exhaustive.
        ///
        /// The following methods are cancellation safe:
        ///
        ///  * [`tokio::sync::mpsc::Receiver::recv`](crate::sync::mpsc::Receiver::recv)
        ///  * [`tokio::sync::mpsc::UnboundedReceiver::recv`](crate::sync::mpsc::UnboundedReceiver::recv)
        ///  * [`tokio::sync::broadcast::Receiver::recv`](crate::sync::broadcast::Receiver::recv)
        ///  * [`tokio::sync::watch::Receiver::changed`](crate::sync::watch::Receiver::changed)
        ///  * [`tokio::net::TcpListener::accept`](crate::net::TcpListener::accept)
        ///  * [`tokio::net::UnixListener::accept`](crate::net::UnixListener::accept)
        ///  * [`tokio::signal::unix::Signal::recv`](crate::signal::unix::Signal::recv)
        ///  * [`tokio::io::AsyncReadExt::read`](crate::io::AsyncReadExt::read) on any `AsyncRead`
        ///  * [`tokio::io::AsyncReadExt::read_buf`](crate::io::AsyncReadExt::read_buf) on any `AsyncRead`
        ///  * [`tokio::io::AsyncWriteExt::write`](crate::io::AsyncWriteExt::write) on any `AsyncWrite`
        ///  * [`tokio::io::AsyncWriteExt::write_buf`](crate::io::AsyncWriteExt::write_buf) on any `AsyncWrite`
        ///  * [`tokio_stream::StreamExt::next`](https://docs.rs/tokio-stream/0.1/tokio_stream/trait.StreamExt.html#method.next) on any `Stream`
        ///  * [`futures::stream::StreamExt::next`](https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.next) on any `Stream`
        ///
        /// The following methods are not cancellation safe and can lead to loss of data:
        ///
        ///  * [`tokio::io::AsyncReadExt::read_exact`](crate::io::AsyncReadExt::read_exact)
        ///  * [`tokio::io::AsyncReadExt::read_to_end`](crate::io::AsyncReadExt::read_to_end)
        ///  * [`tokio::io::AsyncReadExt::read_to_string`](crate::io::AsyncReadExt::read_to_string)
        ///  * [`tokio::io::AsyncWriteExt::write_all`](crate::io::AsyncWriteExt::write_all)
        ///
        /// The following methods are not cancellation safe because they use a queue for
        /// fairness and cancellation makes you lose your place in the queue:
        ///
        ///  * [`tokio::sync::Mutex::lock`](crate::sync::Mutex::lock)
        ///  * [`tokio::sync::RwLock::read`](crate::sync::RwLock::read)
        ///  * [`tokio::sync::RwLock::write`](crate::sync::RwLock::write)
        ///  * [`tokio::sync::Semaphore::acquire`](crate::sync::Semaphore::acquire)
        ///  * [`tokio::sync::Notify::notified`](crate::sync::Notify::notified)
        ///
        /// To determine whether your own methods are cancellation safe, look for the
        /// location of uses of `.await`. This is because when an asynchronous method is
        /// cancelled, that always happens at an `.await`. If your function behaves
        /// correctly even if it is restarted while waiting at an `.await`, then it is
        /// cancellation safe.
        ///
        /// Cancellation safety can be defined in the following way: If you have a
        /// future that has not yet completed, then it must be a no-op to drop that
        /// future and recreate it. This definition is motivated by the situation where
        /// a `select!` is used in a loop. Without this guarantee, you would lose your
        /// progress when another branch completes and you restart the `select!` by
        /// going around the loop.
        ///
        /// Be aware that cancelling something that is not cancellation safe is not
        /// necessarily wrong. For example, if you are cancelling a task because the
        /// application is shutting down, then you probably don't care that partially
        /// read data is lost.
        ///
        /// # Examples
        ///
        /// Basic select with two branches.
        ///
        /// ```
        /// async fn do_stuff_async() {
        ///     // async work
        /// }
        ///
        /// async fn more_async_work() {
        ///     // more here
        /// }
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     tokio::select! {
        ///         _ = do_stuff_async() => {
        ///             println!("do_stuff_async() completed first")
        ///         }
        ///         _ = more_async_work() => {
        ///             println!("more_async_work() completed first")
        ///         }
        ///     };
        /// }
        /// ```
        ///
        /// Basic stream selecting.
        ///
        /// ```
        /// use tokio_stream::{self as stream, StreamExt};
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let mut stream1 = stream::iter(vec![1, 2, 3]);
        ///     let mut stream2 = stream::iter(vec![4, 5, 6]);
        ///
        ///     let next = tokio::select! {
        ///         v = stream1.next() => v.unwrap(),
        ///         v = stream2.next() => v.unwrap(),
        ///     };
        ///
        ///     assert!(next == 1 || next == 4);
        /// }
        /// ```
        ///
        /// Collect the contents of two streams. In this example, we rely on pattern
        /// matching and the fact that `stream::iter` is "fused", i.e. once the stream
        /// is complete, all calls to `next()` return `None`.
        ///
        /// ```
        /// use tokio_stream::{self as stream, StreamExt};
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let mut stream1 = stream::iter(vec![1, 2, 3]);
        ///     let mut stream2 = stream::iter(vec![4, 5, 6]);
        ///
        ///     let mut values = vec![];
        ///
        ///     loop {
        ///         tokio::select! {
        ///             Some(v) = stream1.next() => values.push(v),
        ///             Some(v) = stream2.next() => values.push(v),
        ///             else => break,
        ///         }
        ///     }
        ///
        ///     values.sort();
        ///     assert_eq!(&[1, 2, 3, 4, 5, 6], &values[..]);
        /// }
        /// ```
        ///
        /// Using the same future in multiple `select!` expressions can be done by passing
        /// a reference to the future. Doing so requires the future to be [`Unpin`]. A
        /// future can be made [`Unpin`] by either using [`Box::pin`] or stack pinning.
        ///
        /// [`Unpin`]: std::marker::Unpin
        /// [`Box::pin`]: std::boxed::Box::pin
        ///
        /// Here, a stream is consumed for at most 1 second.
        ///
        /// ```
        /// use tokio_stream::{self as stream, StreamExt};
        /// use tokio::time::{self, Duration};
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let mut stream = stream::iter(vec![1, 2, 3]);
        ///     let sleep = time::sleep(Duration::from_secs(1));
        ///     tokio::pin!(sleep);
        ///
        ///     loop {
        ///         tokio::select! {
        ///             maybe_v = stream.next() => {
        ///                 if let Some(v) = maybe_v {
        ///                     println!("got = {}", v);
        ///                 } else {
        ///                     break;
        ///                 }
        ///             }
        ///             _ = &mut sleep => {
        ///                 println!("timeout");
        ///                 break;
        ///             }
        ///         }
        ///     }
        /// }
        /// ```
        ///
        /// Joining two values using `select!`.
        ///
        /// ```
        /// use tokio::sync::oneshot;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let (tx1, mut rx1) = oneshot::channel();
        ///     let (tx2, mut rx2) = oneshot::channel();
        ///
        ///     tokio::spawn(async move {
        ///         tx1.send("first").unwrap();
        ///     });
        ///
        ///     tokio::spawn(async move {
        ///         tx2.send("second").unwrap();
        ///     });
        ///
        ///     let mut a = None;
        ///     let mut b = None;
        ///
        ///     while a.is_none() || b.is_none() {
        ///         tokio::select! {
        ///             v1 = (&mut rx1), if a.is_none() => a = Some(v1.unwrap()),
        ///             v2 = (&mut rx2), if b.is_none() => b = Some(v2.unwrap()),
        ///         }
        ///     }
        ///
        ///     let res = (a.unwrap(), b.unwrap());
        ///
        ///     assert_eq!(res.0, "first");
        ///     assert_eq!(res.1, "second");
        /// }
        /// ```
        ///
        /// Using the `biased;` mode to control polling order.
        ///
        /// ```
        /// #[tokio::main]
        /// async fn main() {
        ///     let mut count = 0u8;
        ///
        ///     loop {
        ///         tokio::select! {
        ///             // If you run this example without `biased;`, the polling order is
        ///             // pseudo-random, and the assertions on the value of count will
        ///             // (probably) fail.
        ///             biased;
        ///
        ///             _ = async {}, if count < 1 => {
        ///                 count += 1;
        ///                 assert_eq!(count, 1);
        ///             }
        ///             _ = async {}, if count < 2 => {
        ///                 count += 1;
        ///                 assert_eq!(count, 2);
        ///             }
        ///             _ = async {}, if count < 3 => {
        ///                 count += 1;
        ///                 assert_eq!(count, 3);
        ///             }
        ///             _ = async {}, if count < 4 => {
        ///                 count += 1;
        ///                 assert_eq!(count, 4);
        ///             }
        ///
        ///             else => {
        ///                 break;
        ///             }
        ///         };
        ///     }
        /// }
        /// ```
        ///
        /// ## Avoid racy `if` preconditions
        ///
        /// Given that `if` preconditions are used to disable `select!` branches, some
        /// caution must be used to avoid missing values.
        ///
        /// For example, here is **incorrect** usage of `sleep` with `if`. The objective
        /// is to repeatedly run an asynchronous task for up to 50 milliseconds.
        /// However, there is a potential for the `sleep` completion to be missed.
        ///
        /// ```no_run,should_panic
        /// use tokio::time::{self, Duration};
        ///
        /// async fn some_async_work() {
        ///     // do work
        /// }
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let sleep = time::sleep(Duration::from_millis(50));
        ///     tokio::pin!(sleep);
        ///
        ///     while !sleep.is_elapsed() {
        ///         tokio::select! {
        ///             _ = &mut sleep, if !sleep.is_elapsed() => {
        ///                 println!("operation timed out");
        ///             }
        ///             _ = some_async_work() => {
        ///                 println!("operation completed");
        ///             }
        ///         }
        ///     }
        ///
        ///     panic!("This example shows how not to do it!");
        /// }
        /// ```
        ///
        /// In the above example, `sleep.is_elapsed()` may return `true` even if
        /// `sleep.poll()` never returned `Ready`. This opens up a potential race
        /// condition where `sleep` expires between the `while !sleep.is_elapsed()`
        /// check and the call to `select!` resulting in the `some_async_work()` call to
        /// run uninterrupted despite the sleep having elapsed.
        ///
        /// One way to write the above example without the race would be:
        ///
        /// ```
        /// use tokio::time::{self, Duration};
        ///
        /// async fn some_async_work() {
        /// # time::sleep(Duration::from_millis(10)).await;
        ///     // do work
        /// }
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let sleep = time::sleep(Duration::from_millis(50));
        ///     tokio::pin!(sleep);
        ///
        ///     loop {
        ///         tokio::select! {
        ///             _ = &mut sleep => {
        ///                 println!("operation timed out");
        ///                 break;
        ///             }
        ///             _ = some_async_work() => {
        ///                 println!("operation completed");
        ///             }
        ///         }
        ///     }
        /// }
        /// ```
        /// # Alternatives from the Ecosystem
        ///
        /// The `select!` macro is a powerful tool for managing multiple asynchronous
        /// branches, enabling tasks to run concurrently within the same thread. However,
        /// its use can introduce challenges, particularly around cancellation safety, which
        /// can lead to subtle and hard-to-debug errors. For many use cases, ecosystem
        /// alternatives may be preferable as they mitigate these concerns by offering
        /// clearer syntax, more predictable control flow, and reducing the need to manually
        /// handle issues like fuse semantics or cancellation safety.
        ///
        /// ## Merging Streams
        ///
        /// For cases where `loop { select! { ... } }` is used to poll multiple tasks,
        /// stream merging offers a concise alternative, inherently handle cancellation-safe
        /// processing, removing the risk of data loss. Libraries such as [`tokio_stream`],
        /// [`futures::stream`] and [`futures_concurrency`] provide tools for merging
        /// streams and handling their outputs sequentially.
        ///
        /// [`tokio_stream`]: https://docs.rs/tokio-stream/latest/tokio_stream/
        /// [`futures::stream`]: https://docs.rs/futures/latest/futures/stream/
        /// [`futures_concurrency`]: https://docs.rs/futures-concurrency/latest/futures_concurrency/
        ///
        /// ### Example with `select!`
        ///
        /// ```
        /// struct File;
        /// struct Channel;
        /// struct Socket;
        ///
        /// impl Socket {
        ///     async fn read_packet(&mut self) -> Vec<u8> {
        ///         vec![]
        ///     }
        /// }
        ///
        /// async fn read_send(_file: &mut File, _channel: &mut Channel) {
        ///     // do work that is not cancel safe
        /// }
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     // open our IO types
        ///     let mut file = File;
        ///     let mut channel = Channel;
        ///     let mut socket = Socket;
        ///
        ///     loop {
        ///         tokio::select! {
        ///             _ = read_send(&mut file, &mut channel) => { /* ... */ },
        ///             _data = socket.read_packet() => { /* ... */ }
        ///             _ = futures::future::ready(()) => break
        ///         }
        ///     }
        /// }
        ///
        /// ```
        ///
        /// ### Moving to `merge`
        ///
        /// By using merge, you can unify multiple asynchronous tasks into a single stream,
        /// eliminating the need to manage tasks manually and reducing the risk of
        /// unintended behavior like data loss.
        ///
        /// ```
        /// use std::pin::pin;
        ///
        /// use futures::stream::unfold;
        /// use tokio_stream::StreamExt;
        ///
        /// struct File;
        /// struct Channel;
        /// struct Socket;
        ///
        /// impl Socket {
        ///     async fn read_packet(&mut self) -> Vec<u8> {
        ///         vec![]
        ///     }
        /// }
        ///
        /// async fn read_send(_file: &mut File, _channel: &mut Channel) {
        ///     // do work that is not cancel safe
        /// }
        ///
        /// enum Message {
        ///     Stop,
        ///     Sent,
        ///     Data(Vec<u8>),
        /// }
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     // open our IO types
        ///     let file = File;
        ///     let channel = Channel;
        ///     let socket = Socket;
        ///
        ///     let a = unfold((file, channel), |(mut file, mut channel)| async {
        ///         read_send(&mut file, &mut channel).await;
        ///         Some((Message::Sent, (file, channel)))
        ///     });
        ///     let b = unfold(socket, |mut socket| async {
        ///         let data = socket.read_packet().await;
        ///         Some((Message::Data(data), socket))
        ///     });
        ///     let c = tokio_stream::iter([Message::Stop]);
        ///
        ///     let mut s = pin!(a.merge(b).merge(c));
        ///     while let Some(msg) = s.next().await {
        ///         match msg {
        ///             Message::Data(_data) => { /* ... */ }
        ///             Message::Sent => continue,
        ///             Message::Stop => break,
        ///         }
        ///     }
        /// }
        /// ```
        ///
        /// ## Racing Futures
        ///
        /// If you need to wait for the first completion among several asynchronous tasks,
        /// ecosystem utilities such as
        /// [`futures`](https://docs.rs/futures/latest/futures/),
        /// [`futures-lite`](https://docs.rs/futures-lite/latest/futures_lite/) or
        /// [`futures-concurrency`](https://docs.rs/futures-concurrency/latest/futures_concurrency/)
        /// provide streamlined syntax for racing futures:
        ///
        /// - [`futures_concurrency::future::Race`](https://docs.rs/futures-concurrency/latest/futures_concurrency/future/trait.Race.html)
        /// - [`futures::select`](https://docs.rs/futures/latest/futures/macro.select.html)
        /// - [`futures::stream::select_all`](https://docs.rs/futures/latest/futures/stream/select_all/index.html) (for streams)
        /// - [`futures_lite::future::or`](https://docs.rs/futures-lite/latest/futures_lite/future/fn.or.html)
        /// - [`futures_lite::future::race`](https://docs.rs/futures-lite/latest/futures_lite/future/fn.race.html)
        ///
        /// ```
        /// use futures_concurrency::future::Race;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let task_a = async { Ok("ok") };
        ///     let task_b = async { Err("error") };
        ///     let result = (task_a, task_b).race().await;
        ///
        ///     match result {
        ///         Ok(output) => println!("First task completed with: {output}"),
        ///         Err(err) => eprintln!("Error occurred: {err}"),
        ///     }
        /// }
        /// ```
        #[macro_export]
        #[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
        $select
    };
}

#[cfg(doc)]
doc! {macro_rules! select {
    {
        $(
            biased;
        )?
        $(
            $bind:pat = $fut:expr $(, if $cond:expr)? => $handler:expr,
        )*
        $(
            else => $els:expr $(,)?
        )?
    } => {
        unimplemented!()
    };
}}

#[cfg(not(doc))]
doc! {macro_rules! select {
    // Uses a declarative macro to do **most** of the work. While it is possible
    // to implement fully with a declarative macro, a procedural macro is used
    // to enable improved error messages.
    //
    // The macro is structured as a tt-muncher. All branches are processed and
    // normalized. Once the input is normalized, it is passed to the top-most
    // rule. When entering the macro, `@{ }` is inserted at the front. This is
    // used to collect the normalized input.
    //
    // The macro only recurses once per branch. This allows using `select!`
    // without requiring the user to increase the recursion limit.

    // All input is normalized, now transform.
    (@ {
        // The index of the future to poll first (in bias mode), or the RNG
        // expression to use to pick a future to poll first.
        start=$start:expr;

        // One `_` for each branch in the `select!` macro. Passing this to
        // `count!` converts $skip to an integer.
        ( $($count:tt)* )

        // Normalized select branches. `( $skip )` is a set of `_` characters.
        // There is one `_` for each select branch **before** this one. Given
        // that all input futures are stored in a tuple, $skip is useful for
        // generating a pattern to reference the future for the current branch.
        // $skip is also used as an argument to `count!`, returning the index of
        // the current select branch.
        $( ( $($skip:tt)* ) $bind:pat = $fut:expr, if $c:expr => $handle:expr, )+

        // Fallback expression used when all select branches have been disabled.
        ; $else:expr

    }) => {{
        // Enter a context where stable "function-like" proc macros can be used.
        //
        // This module is defined within a scope and should not leak out of this
        // macro.
        #[doc(hidden)]
        mod __tokio_select_util {
            // Generate an enum with one variant per select branch
            $crate::select_priv_declare_output_enum!( ( $($count)* ) );
        }

        // `tokio::macros::support` is a public, but doc(hidden) module
        // including a re-export of all types needed by this macro.
        use $crate::macros::support::Future;
        use $crate::macros::support::Pin;
        use $crate::macros::support::Poll::{Ready, Pending};

        const BRANCHES: u32 = $crate::count!( $($count)* );

        let mut disabled: __tokio_select_util::Mask = Default::default();

        // First, invoke all the pre-conditions. For any that return true,
        // set the appropriate bit in `disabled`.
        $(
            if !$c {
                let mask: __tokio_select_util::Mask = 1 << $crate::count!( $($skip)* );
                disabled |= mask;
            }
        )*

        // Create a scope to separate polling from handling the output. This
        // adds borrow checker flexibility when using the macro.
        let mut output = {
            // Store each future directly first (that is, without wrapping the future in a call to
            // `IntoFuture::into_future`). This allows the `$fut` expression to make use of
            // temporary lifetime extension.
            //
            // https://doc.rust-lang.org/1.58.1/reference/destructors.html#temporary-lifetime-extension
            let futures_init = ($( $fut, )+);

            // Safety: Nothing must be moved out of `futures`. This is to
            // satisfy the requirement of `Pin::new_unchecked` called below.
            //
            // We can't use the `pin!` macro for this because `futures` is a
            // tuple and the standard library provides no way to pin-project to
            // the fields of a tuple.
            let mut futures = ($( $crate::macros::support::IntoFuture::into_future(
                        $crate::count_field!( futures_init.$($skip)* )
            ),)+);

            // This assignment makes sure that the `poll_fn` closure only has a
            // reference to the futures, instead of taking ownership of them.
            // This mitigates the issue described in
            // <https://internals.rust-lang.org/t/surprising-soundness-trouble-around-pollfn/17484>
            let mut futures = &mut futures;

            $crate::macros::support::poll_fn(|cx| {
                // Return `Pending` when the task budget is depleted since budget-aware futures
                // are going to yield anyway and other futures will not cooperate.
                ::std::task::ready!($crate::macros::support::poll_budget_available(cx));

                // Track if any branch returns pending. If no branch completes
                // **or** returns pending, this implies that all branches are
                // disabled.
                let mut is_pending = false;

                // Choose a starting index to begin polling the futures at. In
                // practice, this will either be a pseudo-randomly generated
                // number by default, or the constant 0 if `biased;` is
                // supplied.
                let start = $start;

                for i in 0..BRANCHES {
                    let branch;
                    #[allow(clippy::modulo_one)]
                    {
                        branch = (start + i) % BRANCHES;
                    }
                    match branch {
                        $(
                            #[allow(unreachable_code)]
                            $crate::count!( $($skip)* ) => {
                                // First, if the future has previously been
                                // disabled, do not poll it again. This is done
                                // by checking the associated bit in the
                                // `disabled` bit field.
                                let mask = 1 << branch;

                                if disabled & mask == mask {
                                    // The future has been disabled.
                                    continue;
                                }

                                // Extract the future for this branch from the
                                // tuple
                                let ( $($skip,)* fut, .. ) = &mut *futures;

                                // Safety: future is stored on the stack above
                                // and never moved.
                                let mut fut = unsafe { Pin::new_unchecked(fut) };

                                // Try polling it
                                let out = match Future::poll(fut, cx) {
                                    Ready(out) => out,
                                    Pending => {
                                        // Track that at least one future is
                                        // still pending and continue polling.
                                        is_pending = true;
                                        continue;
                                    }
                                };

                                // Disable the future from future polling.
                                disabled |= mask;

                                // The future returned a value, check if matches
                                // the specified pattern.
                                #[allow(unused_variables)]
                                #[allow(unused_mut)]
                                match &out {
                                    $crate::select_priv_clean_pattern!($bind) => {}
                                    _ => continue,
                                }

                                // The select is complete, return the value
                                return Ready($crate::select_variant!(__tokio_select_util::Out, ($($skip)*))(out));
                            }
                        )*
                        _ => unreachable!("reaching this means there probably is an off by one bug"),
                    }
                }

                if is_pending {
                    Pending
                } else {
                    // All branches have been disabled.
                    Ready(__tokio_select_util::Out::Disabled)
                }
            }).await
        };

        match output {
            $(
                $crate::select_variant!(__tokio_select_util::Out, ($($skip)*) ($bind)) => $handle,
            )*
            __tokio_select_util::Out::Disabled => $else,
            _ => unreachable!("failed to match bind"),
        }
    }};

    // ==== Normalize =====

    // These rules match a single `select!` branch and normalize it for
    // processing by the first rule.

    (@ { start=$start:expr; $($t:tt)* } ) => {
        // No `else` branch
        $crate::select!(@{ start=$start; $($t)*; panic!("all branches are disabled and there is no else branch") })
    };
    (@ { start=$start:expr; $($t:tt)* } else => $else:expr $(,)?) => {
        $crate::select!(@{ start=$start; $($t)*; $else })
    };
    (@ { start=$start:expr; ( $($s:tt)* ) $($t:tt)* } $p:pat = $f:expr, if $c:expr => $h:block, $($r:tt)* ) => {
        $crate::select!(@{ start=$start; ($($s)* _) $($t)* ($($s)*) $p = $f, if $c => $h, } $($r)*)
    };
    (@ { start=$start:expr; ( $($s:tt)* ) $($t:tt)* } $p:pat = $f:expr => $h:block, $($r:tt)* ) => {
        $crate::select!(@{ start=$start; ($($s)* _) $($t)* ($($s)*) $p = $f, if true => $h, } $($r)*)
    };
    (@ { start=$start:expr; ( $($s:tt)* ) $($t:tt)* } $p:pat = $f:expr, if $c:expr => $h:block $($r:tt)* ) => {
        $crate::select!(@{ start=$start; ($($s)* _) $($t)* ($($s)*) $p = $f, if $c => $h, } $($r)*)
    };
    (@ { start=$start:expr; ( $($s:tt)* ) $($t:tt)* } $p:pat = $f:expr => $h:block $($r:tt)* ) => {
        $crate::select!(@{ start=$start; ($($s)* _) $($t)* ($($s)*) $p = $f, if true => $h, } $($r)*)
    };
    (@ { start=$start:expr; ( $($s:tt)* ) $($t:tt)* } $p:pat = $f:expr, if $c:expr => $h:expr ) => {
        $crate::select!(@{ start=$start; ($($s)* _) $($t)* ($($s)*) $p = $f, if $c => $h, })
    };
    (@ { start=$start:expr; ( $($s:tt)* ) $($t:tt)* } $p:pat = $f:expr => $h:expr ) => {
        $crate::select!(@{ start=$start; ($($s)* _) $($t)* ($($s)*) $p = $f, if true => $h, })
    };
    (@ { start=$start:expr; ( $($s:tt)* ) $($t:tt)* } $p:pat = $f:expr, if $c:expr => $h:expr, $($r:tt)* ) => {
        $crate::select!(@{ start=$start; ($($s)* _) $($t)* ($($s)*) $p = $f, if $c => $h, } $($r)*)
    };
    (@ { start=$start:expr; ( $($s:tt)* ) $($t:tt)* } $p:pat = $f:expr => $h:expr, $($r:tt)* ) => {
        $crate::select!(@{ start=$start; ($($s)* _) $($t)* ($($s)*) $p = $f, if true => $h, } $($r)*)
    };

    // ===== Entry point =====

    ($(biased;)? else => $else:expr $(,)? ) => {{
        $else
    }};

    (biased; $p:pat = $($t:tt)* ) => {
        $crate::select!(@{ start=0; () } $p = $($t)*)
    };

    ( $p:pat = $($t:tt)* ) => {
        // Randomly generate a starting point. This makes `select!` a bit more
        // fair and avoids always polling the first future.
        $crate::select!(@{ start={ $crate::macros::support::thread_rng_n(BRANCHES) }; () } $p = $($t)*)
    };

    () => {
        compile_error!("select! requires at least one branch.")
    };
}}

// And here... we manually list out matches for up to 64 branches... I'm not
// happy about it either, but this is how we manage to use a declarative macro!

#[macro_export]
#[doc(hidden)]
macro_rules! count {
    () => {
        0
    };
    (_) => {
        1
    };
    (_ _) => {
        2
    };
    (_ _ _) => {
        3
    };
    (_ _ _ _) => {
        4
    };
    (_ _ _ _ _) => {
        5
    };
    (_ _ _ _ _ _) => {
        6
    };
    (_ _ _ _ _ _ _) => {
        7
    };
    (_ _ _ _ _ _ _ _) => {
        8
    };
    (_ _ _ _ _ _ _ _ _) => {
        9
    };
    (_ _ _ _ _ _ _ _ _ _) => {
        10
    };
    (_ _ _ _ _ _ _ _ _ _ _) => {
        11
    };
    (_ _ _ _ _ _ _ _ _ _ _ _) => {
        12
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _) => {
        13
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        14
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        15
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        16
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        17
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        18
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        19
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        20
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        21
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        22
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        23
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        24
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        25
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        26
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        27
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        28
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        29
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        30
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        31
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        32
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        33
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        34
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        35
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        36
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        37
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        38
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        39
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        40
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        41
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        42
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        43
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        44
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        45
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        46
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        47
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        48
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        49
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        50
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        51
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        52
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        53
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        54
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        55
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        56
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        57
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        58
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        59
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        60
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        61
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        62
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        63
    };
    (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        64
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! count_field {
    ($var:ident. ) => {
        $var.0
    };
    ($var:ident. _) => {
        $var.1
    };
    ($var:ident. _ _) => {
        $var.2
    };
    ($var:ident. _ _ _) => {
        $var.3
    };
    ($var:ident. _ _ _ _) => {
        $var.4
    };
    ($var:ident. _ _ _ _ _) => {
        $var.5
    };
    ($var:ident. _ _ _ _ _ _) => {
        $var.6
    };
    ($var:ident. _ _ _ _ _ _ _) => {
        $var.7
    };
    ($var:ident. _ _ _ _ _ _ _ _) => {
        $var.8
    };
    ($var:ident. _ _ _ _ _ _ _ _ _) => {
        $var.9
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _) => {
        $var.10
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _) => {
        $var.11
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.12
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.13
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.14
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.15
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.16
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.17
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.18
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.19
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.20
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.21
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.22
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.23
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.24
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.25
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.26
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.27
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.28
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.29
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.30
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.31
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.32
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.33
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.34
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.35
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.36
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.37
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.38
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.39
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.40
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.41
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.42
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.43
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.44
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.45
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.46
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.47
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.48
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.49
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.50
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.51
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.52
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.53
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.54
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.55
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.56
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.57
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.58
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.59
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.60
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.61
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.62
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.63
    };
    ($var:ident. _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) => {
        $var.64
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! select_variant {
    ($($p:ident)::*, () $($t:tt)*) => {
        $($p)::*::_0 $($t)*
    };
    ($($p:ident)::*, (_) $($t:tt)*) => {
        $($p)::*::_1 $($t)*
    };
    ($($p:ident)::*, (_ _) $($t:tt)*) => {
        $($p)::*::_2 $($t)*
    };
    ($($p:ident)::*, (_ _ _) $($t:tt)*) => {
        $($p)::*::_3 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _) $($t:tt)*) => {
        $($p)::*::_4 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_5 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_6 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_7 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_8 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_9 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_10 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_11 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_12 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_13 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_14 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_15 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_16 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_17 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_18 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_19 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_20 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_21 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_22 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_23 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_24 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_25 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_26 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_27 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_28 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_29 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_30 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_31 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_32 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_33 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_34 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_35 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_36 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_37 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_38 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_39 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_40 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_41 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_42 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_43 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_44 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_45 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_46 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_47 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_48 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_49 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_50 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_51 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_52 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_53 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_54 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_55 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_56 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_57 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_58 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_59 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_60 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_61 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_62 $($t)*
    };
    ($($p:ident)::*, (_ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _ _) $($t:tt)*) => {
        $($p)::*::_63 $($t)*
    };
}
