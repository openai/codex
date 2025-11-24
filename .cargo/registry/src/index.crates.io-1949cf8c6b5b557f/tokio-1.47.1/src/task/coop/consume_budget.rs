/// Consumes a unit of budget and returns the execution back to the Tokio
/// runtime *if* the task's coop budget was exhausted.
///
/// The task will only yield if its entire coop budget has been exhausted.
/// This function can be used in order to insert optional yield points into long
/// computations that do not use Tokio resources like sockets or semaphores,
/// without redundantly yielding to the runtime each time.
///
/// # Examples
///
/// Make sure that a function which returns a sum of (potentially lots of)
/// iterated values is cooperative.
///
/// ```
/// async fn sum_iterator(input: &mut impl std::iter::Iterator<Item=i64>) -> i64 {
///     let mut sum: i64 = 0;
///     while let Some(i) = input.next() {
///         sum += i;
///         tokio::task::consume_budget().await
///     }
///     sum
/// }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
pub async fn consume_budget() {
    let mut status = std::task::Poll::Pending;

    std::future::poll_fn(move |cx| {
        std::task::ready!(crate::trace::trace_leaf(cx));
        if status.is_ready() {
            return status;
        }
        status = crate::task::coop::poll_proceed(cx).map(|restore| {
            restore.made_progress();
        });
        status
    })
    .await
}
