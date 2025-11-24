macro_rules! doc {
    ($try_join:item) => {
        /// Waits on multiple concurrent branches, returning when **all** branches
        /// complete with `Ok(_)` or on the first `Err(_)`.
        ///
        /// The `try_join!` macro must be used inside of async functions, closures, and
        /// blocks.
        ///
        /// Similar to [`join!`], the `try_join!` macro takes a list of async
        /// expressions and evaluates them concurrently on the same task. Each async
        /// expression evaluates to a future and the futures from each expression are
        /// multiplexed on the current task. The `try_join!` macro returns when **all**
        /// branches return with `Ok` or when the **first** branch returns with `Err`.
        ///
        /// [`join!`]: macro@join
        ///
        /// # Notes
        ///
        /// The supplied futures are stored inline and do not require allocating a
        /// `Vec`.
        ///
        /// ## Runtime characteristics
        ///
        /// By running all async expressions on the current task, the expressions are
        /// able to run **concurrently** but not in **parallel**. This means all
        /// expressions are run on the same thread and if one branch blocks the thread,
        /// all other expressions will be unable to continue. If parallelism is
        /// required, spawn each async expression using [`tokio::spawn`] and pass the
        /// join handle to `try_join!`.
        ///
        /// [`tokio::spawn`]: crate::spawn
        ///
        /// ## Fairness
        ///
        /// By default, `try_join!`'s generated future rotates which
        /// contained future is polled first whenever it is woken.
        ///
        /// This behavior can be overridden by adding `biased;` to the beginning of the
        /// macro usage. See the examples for details. This will cause `try_join` to poll
        /// the futures in the order they appear from top to bottom.
        ///
        /// You may want this if your futures may interact in a way where known polling order is significant.
        ///
        /// But there is an important caveat to this mode. It becomes your responsibility
        /// to ensure that the polling order of your futures is fair. If for example you
        /// are joining a stream and a shutdown future, and the stream has a
        /// huge volume of messages that takes a long time to finish processing per poll, you should
        /// place the shutdown future earlier in the `try_join!` list to ensure that it is
        /// always polled, and will not be delayed due to the stream future taking a long time to return
        /// `Poll::Pending`.
        ///
        /// # Examples
        ///
        /// Basic `try_join` with two branches.
        ///
        /// ```
        /// async fn do_stuff_async() -> Result<(), &'static str> {
        ///     // async work
        /// # Ok(())
        /// }
        ///
        /// async fn more_async_work() -> Result<(), &'static str> {
        ///     // more here
        /// # Ok(())
        /// }
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let res = tokio::try_join!(
        ///         do_stuff_async(),
        ///         more_async_work());
        ///
        ///     match res {
        ///          Ok((first, second)) => {
        ///              // do something with the values
        ///          }
        ///          Err(err) => {
        ///             println!("processing failed; error = {}", err);
        ///          }
        ///     }
        /// }
        /// ```
        ///
        /// Using `try_join!` with spawned tasks.
        ///
        /// ```
        /// use tokio::task::JoinHandle;
        ///
        /// async fn do_stuff_async() -> Result<(), &'static str> {
        ///     // async work
        /// # Err("failed")
        /// }
        ///
        /// async fn more_async_work() -> Result<(), &'static str> {
        ///     // more here
        /// # Ok(())
        /// }
        ///
        /// async fn flatten<T>(handle: JoinHandle<Result<T, &'static str>>) -> Result<T, &'static str> {
        ///     match handle.await {
        ///         Ok(Ok(result)) => Ok(result),
        ///         Ok(Err(err)) => Err(err),
        ///         Err(err) => Err("handling failed"),
        ///     }
        /// }
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let handle1 = tokio::spawn(do_stuff_async());
        ///     let handle2 = tokio::spawn(more_async_work());
        ///     match tokio::try_join!(flatten(handle1), flatten(handle2)) {
        ///         Ok(val) => {
        ///             // do something with the values
        ///         }
        ///         Err(err) => {
        ///             println!("Failed with {}.", err);
        ///             # assert_eq!(err, "failed");
        ///         }
        ///     }
        /// }
        /// ```
        /// Using the `biased;` mode to control polling order.
        ///
        /// ```
        /// async fn do_stuff_async() -> Result<(), &'static str> {
        ///     // async work
        /// # Ok(())
        /// }
        ///
        /// async fn more_async_work() -> Result<(), &'static str> {
        ///     // more here
        /// # Ok(())
        /// }
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let res = tokio::try_join!(
        ///         biased;
        ///         do_stuff_async(),
        ///         more_async_work()
        ///     );
        ///
        ///     match res {
        ///          Ok((first, second)) => {
        ///              // do something with the values
        ///          }
        ///          Err(err) => {
        ///             println!("processing failed; error = {}", err);
        ///          }
        ///     }
        /// }
        /// ```
        #[macro_export]
        #[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
        $try_join
    };
}

#[cfg(doc)]
doc! {macro_rules! try_join {
    ($(biased;)? $($future:expr),*) => { unimplemented!() }
}}

#[cfg(not(doc))]
doc! {macro_rules! try_join {
    (@ {
        // Type of rotator that controls which inner future to start with
        // when polling our output future.
        rotator=$rotator:ty;

        // One `_` for each branch in the `try_join!` macro. This is not used once
        // normalization is complete.
        ( $($count:tt)* )

        // The expression `0+1+1+ ... +1` equal to the number of branches.
        ( $($total:tt)* )

        // Normalized try_join! branches
        $( ( $($skip:tt)* ) $e:expr, )*

    }) => {{
        use $crate::macros::support::{maybe_done, poll_fn, Future, Pin};
        use $crate::macros::support::Poll::{Ready, Pending};

        // Safety: nothing must be moved out of `futures`. This is to satisfy
        // the requirement of `Pin::new_unchecked` called below.
        //
        // We can't use the `pin!` macro for this because `futures` is a tuple
        // and the standard library provides no way to pin-project to the fields
        // of a tuple.
        let mut futures = ( $( maybe_done($e), )* );

        // This assignment makes sure that the `poll_fn` closure only has a
        // reference to the futures, instead of taking ownership of them. This
        // mitigates the issue described in
        // <https://internals.rust-lang.org/t/surprising-soundness-trouble-around-pollfn/17484>
        let mut futures = &mut futures;

        const COUNT: u32 = $($total)*;

        // Each time the future created by poll_fn is polled, if not using biased mode,
        // a different future is polled first to ensure every future passed to try_join!
        // can make progress even if one of the futures consumes the whole budget.
        let mut rotator = <$rotator>::default();

        poll_fn(move |cx| {
            let mut is_pending = false;
            let mut to_run = COUNT;

            // The number of futures that will be skipped in the first loop iteration.
            let mut skip = rotator.num_skip();

            // This loop runs twice and the first `skip` futures
            // are not polled in the first iteration.
            loop {
            $(
                if skip == 0 {
                    if to_run == 0 {
                        // Every future has been polled
                        break;
                    }
                    to_run -= 1;

                    // Extract the future for this branch from the tuple.
                    let ( $($skip,)* fut, .. ) = &mut *futures;

                    // Safety: future is stored on the stack above
                    // and never moved.
                    let mut fut = unsafe { Pin::new_unchecked(fut) };

                    // Try polling
                    if fut.as_mut().poll(cx).is_pending() {
                        is_pending = true;
                    } else if fut.as_mut().output_mut().expect("expected completed future").is_err() {
                        return Ready(Err(fut.take_output().expect("expected completed future").err().unwrap()))
                    }
                } else {
                    // Future skipped, one less future to skip in the next iteration
                    skip -= 1;
                }
            )*
            }

            if is_pending {
                Pending
            } else {
                Ready(Ok(($({
                    // Extract the future for this branch from the tuple.
                    let ( $($skip,)* fut, .. ) = &mut futures;

                    // Safety: future is stored on the stack above
                    // and never moved.
                    let mut fut = unsafe { Pin::new_unchecked(fut) };

                    fut
                        .take_output()
                        .expect("expected completed future")
                        .ok()
                        .expect("expected Ok(_)")
                },)*)))
            }
        }).await
    }};

    // ===== Normalize =====

    (@ { rotator=$rotator:ty;  ( $($s:tt)* ) ( $($n:tt)* ) $($t:tt)* } $e:expr, $($r:tt)* ) => {
      $crate::try_join!(@{ rotator=$rotator; ($($s)* _) ($($n)* + 1) $($t)* ($($s)*) $e, } $($r)*)
    };

    // ===== Entry point =====
    ( biased; $($e:expr),+ $(,)?) => {
        $crate::try_join!(@{ rotator=$crate::macros::support::BiasedRotator;  () (0) } $($e,)*)
    };

    ( $($e:expr),+ $(,)?) => {
        $crate::try_join!(@{ rotator=$crate::macros::support::Rotator<COUNT>; () (0) } $($e,)*)
    };

    (biased;) => { async { Ok(()) }.await };

    () => { async { Ok(()) }.await }
}}
