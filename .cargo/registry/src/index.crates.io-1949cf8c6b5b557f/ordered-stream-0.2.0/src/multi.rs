use crate::*;
use core::ops::DerefMut;
use core::pin::Pin;
use core::task::{Context, Poll};

fn poll_multiple_step<I, P, S>(
    streams: I,
    cx: &mut Context<'_>,
    before: Option<&S::Ordering>,
    mut retry: Option<&mut Option<S::Ordering>>,
) -> Poll<PollResult<S::Ordering, S::Data>>
where
    I: IntoIterator<Item = Pin<P>>,
    P: DerefMut<Target = Peekable<S>>,
    S: OrderedStream,
    S::Ordering: Clone,
{
    // The stream with the earliest item that is actually before the given point
    let mut best: Option<Pin<P>> = None;
    // true if we have a stream that has not terminated
    let mut has_data = false;
    let mut has_pending = false;
    let mut skip_retry = false;
    for mut stream in streams {
        let best_before = best.as_ref().and_then(|p| p.item().map(|i| &i.0));
        let current_bound = match (before, best_before) {
            (Some(given), Some(best)) if given <= best => Some(given),
            (_, Some(best)) => Some(best),
            (given, None) => given,
        };
        // improved is true if have improved the `before` bound from the initial value

        match stream.as_mut().poll_peek_before(cx, current_bound) {
            Poll::Pending => {
                has_pending = true;
                skip_retry = true;
            }
            Poll::Ready(PollResult::Terminated) => continue,
            Poll::Ready(PollResult::NoneBefore) => {
                has_data = true;
            }
            Poll::Ready(PollResult::Item { ordering, .. }) => {
                has_data = true;
                match current_bound {
                    Some(max) if max < ordering => continue,
                    _ => {}
                }
                match (&mut retry, before, has_pending) {
                    (Some(retry), Some(initial_bound), true) if ordering < initial_bound => {
                        // We have just improved the initial bound, so the streams that
                        // previously returned Pending might be able to return NoneBefore in a
                        // retry.  This is only useful if there are no later Pending returns, so
                        // those will set skip_retry.
                        **retry = Some(ordering.clone());
                        skip_retry = false;
                    }
                    (Some(retry), None, true) => {
                        **retry = Some(ordering.clone());
                        skip_retry = false;
                    }
                    _ => {}
                }
                best = Some(stream);
            }
        }
    }
    if skip_retry {
        retry.map(|r| *r = None);
    }
    match best {
        _ if has_pending => Poll::Pending,
        None if has_data => Poll::Ready(PollResult::NoneBefore),
        None => Poll::Ready(PollResult::Terminated),
        // This is guaranteed to return PollResult::Item
        Some(mut stream) => stream.as_mut().poll_next_before(cx, before),
    }
}

/// Join a collection of [`OrderedStream`]s.
///
/// This is similar to repeatedly using [`join()`] on all the streams in the contained collection.
/// It is not optimized to avoid polling streams that are not ready, so it works best if the number
/// of streams is relatively small.
//
// Unlike `FutureUnordered` or `SelectAll`, the ordering properties that this struct provides can
// easily require that all items in the collection be consulted before returning any item.  An
// example of such a situation is a series of streams that all generate timestamps (locally) for
// their items and only return `NoneBefore` for past timestamps.  If only one stream produces an
// item for each call to `JoinMultiple::poll_next_before`, that timestamp must be checked against
// every other stream, and no amount of preparatory work or hints will help this.
//
// On the other hand, if all streams provide a position hint that matches their next item, it is
// possible to build a priority queue to sort the streams and reduce the cost of a single poll from
// `n` to `log(n)`.  This does require maintaining a snapshot of the hints (so S::Ordering: Clone),
// and will significantly increase the worst-case workload, so it should be a distinct type.
#[derive(Debug, Default, Clone)]
pub struct JoinMultiple<C>(pub C);
impl<C> Unpin for JoinMultiple<C> {}

impl<C, S> OrderedStream for JoinMultiple<C>
where
    for<'a> &'a mut C: IntoIterator<Item = &'a mut Peekable<S>>,
    S: OrderedStream + Unpin,
    S::Ordering: Clone,
{
    type Ordering = S::Ordering;
    type Data = S::Data;
    fn poll_next_before(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&S::Ordering>,
    ) -> Poll<PollResult<S::Ordering, S::Data>> {
        let mut retry = None;
        let rv = poll_multiple_step(
            self.as_mut().get_mut().0.into_iter().map(Pin::new),
            cx,
            before,
            Some(&mut retry),
        );
        if rv.is_pending() && retry.is_some() {
            poll_multiple_step(
                self.get_mut().0.into_iter().map(Pin::new),
                cx,
                retry.as_ref(),
                None,
            )
        } else {
            rv
        }
    }
}

impl<C, S> FusedOrderedStream for JoinMultiple<C>
where
    for<'a> &'a mut C: IntoIterator<Item = &'a mut Peekable<S>>,
    for<'a> &'a C: IntoIterator<Item = &'a Peekable<S>>,
    S: OrderedStream + Unpin,
    S::Ordering: Clone,
{
    fn is_terminated(&self) -> bool {
        self.0.into_iter().all(|peekable| peekable.is_terminated())
    }
}

pin_project_lite::pin_project! {
    /// Join a collection of pinned [`OrderedStream`]s.
    ///
    /// This is identical to [`JoinMultiple`], but accepts [`OrderedStream`]s that are not [`Unpin`] by
    /// requiring that the collection provide a pinned [`IntoIterator`] implementation.
    ///
    /// This is not a feature available in most `std` collections.  If you wish to use them, you
    /// should use `Box::pin` to make the stream [`Unpin`] before inserting it in the collection,
    /// and then use [`JoinMultiple`] on the resulting collection.
    #[derive(Debug,Default,Clone)]
    pub struct JoinMultiplePin<C> {
        #[pin]
        pub streams: C,
    }
}

impl<C> JoinMultiplePin<C> {
    pub fn as_pin_mut(self: Pin<&mut Self>) -> Pin<&mut C> {
        self.project().streams
    }
}

impl<C, S> OrderedStream for JoinMultiplePin<C>
where
    for<'a> Pin<&'a mut C>: IntoIterator<Item = Pin<&'a mut Peekable<S>>>,
    S: OrderedStream,
    S::Ordering: Clone,
{
    type Ordering = S::Ordering;
    type Data = S::Data;
    fn poll_next_before(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&S::Ordering>,
    ) -> Poll<PollResult<S::Ordering, S::Data>> {
        let mut retry = None;
        let rv = poll_multiple_step(self.as_mut().as_pin_mut(), cx, before, Some(&mut retry));
        if rv.is_pending() && retry.is_some() {
            poll_multiple_step(self.as_pin_mut(), cx, retry.as_ref(), None)
        } else {
            rv
        }
    }
}

#[cfg(test)]
mod test {
    extern crate alloc;

    use crate::{FromStream, JoinMultiple, OrderedStream, OrderedStreamExt, PollResult};
    use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
    use core::{cell::Cell, pin::Pin, task::Context, task::Poll};
    use futures_core::Stream;
    use futures_util::{pin_mut, stream::iter};

    #[derive(Debug, PartialEq)]
    pub struct Message {
        serial: u32,
    }

    #[test]
    fn join_mutiple() {
        futures_executor::block_on(async {
            pub struct RemoteLogSource {
                stream: Pin<Box<dyn Stream<Item = Message>>>,
            }

            let mut logs = [
                RemoteLogSource {
                    stream: Box::pin(iter([
                        Message { serial: 1 },
                        Message { serial: 4 },
                        Message { serial: 5 },
                    ])),
                },
                RemoteLogSource {
                    stream: Box::pin(iter([
                        Message { serial: 2 },
                        Message { serial: 3 },
                        Message { serial: 6 },
                    ])),
                },
            ];
            let streams: Vec<_> = logs
                .iter_mut()
                .map(|s| FromStream::with_ordering(&mut s.stream, |m| m.serial).peekable())
                .collect();
            let mut joined = JoinMultiple(streams);
            for i in 0..6 {
                let msg = joined.next().await.unwrap();
                assert_eq!(msg.serial, i as u32 + 1);
            }
        });
    }

    #[test]
    fn join_one_slow() {
        futures_executor::block_on(async {
            pub struct DelayStream(Rc<Cell<u8>>);

            impl OrderedStream for DelayStream {
                type Ordering = u32;
                type Data = Message;
                fn poll_next_before(
                    self: Pin<&mut Self>,
                    _: &mut Context<'_>,
                    before: Option<&Self::Ordering>,
                ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
                    match self.0.get() {
                        0 => Poll::Pending,
                        1 if matches!(before, Some(&1)) => Poll::Ready(PollResult::NoneBefore),
                        1 => Poll::Pending,

                        2 => {
                            self.0.set(3);
                            Poll::Ready(PollResult::Item {
                                data: Message { serial: 4 },
                                ordering: 4,
                            })
                        }
                        _ => Poll::Ready(PollResult::Terminated),
                    }
                }
            }

            let stream1 = iter([
                Message { serial: 1 },
                Message { serial: 3 },
                Message { serial: 5 },
            ]);

            let stream1 = FromStream::with_ordering(stream1, |m| m.serial);
            let go = Rc::new(Cell::new(0));
            let stream2 = DelayStream(go.clone());

            let stream1: Pin<Box<dyn OrderedStream<Ordering = u32, Data = Message>>> =
                Box::pin(stream1);
            let stream2: Pin<Box<dyn OrderedStream<Ordering = u32, Data = Message>>> =
                Box::pin(stream2);
            let streams = vec![stream1.peekable(), stream2.peekable()];
            let join = JoinMultiple(streams);
            let waker = futures_util::task::noop_waker();
            let mut ctx = core::task::Context::from_waker(&waker);

            pin_mut!(join);

            // When the DelayStream has no information about what it contains, join returns Pending
            // (since there could be a serial-0 message output of DelayStream)
            assert_eq!(
                join.as_mut().poll_next_before(&mut ctx, None),
                Poll::Pending
            );

            go.set(1);
            // Now the DelayStream will return NoneBefore on serial 1
            assert_eq!(
                join.as_mut().poll_next_before(&mut ctx, None),
                Poll::Ready(PollResult::Item {
                    data: Message { serial: 1 },
                    ordering: 1,
                })
            );
            // however, it does not (yet) do so for serial 3
            assert_eq!(
                join.as_mut().poll_next_before(&mut ctx, None),
                Poll::Pending
            );

            go.set(2);
            assert_eq!(
                join.as_mut().poll_next_before(&mut ctx, None),
                Poll::Ready(PollResult::Item {
                    data: Message { serial: 3 },
                    ordering: 3,
                })
            );
            assert_eq!(
                join.as_mut().poll_next_before(&mut ctx, None),
                Poll::Ready(PollResult::Item {
                    data: Message { serial: 4 },
                    ordering: 4,
                })
            );
            assert_eq!(
                join.as_mut().poll_next_before(&mut ctx, None),
                Poll::Ready(PollResult::Item {
                    data: Message { serial: 5 },
                    ordering: 5,
                })
            );

            assert_eq!(
                join.as_mut().poll_next_before(&mut ctx, None),
                Poll::Ready(PollResult::Terminated)
            );
        });
    }
}
