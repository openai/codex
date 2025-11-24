use crate::*;
use core::mem;
use core::pin::Pin;
use core::task::{Context, Poll};

pin_project_lite::pin_project! {
    /// A stream for the [`join`](fn.join.html) function.
    #[derive(Debug)]
    pub struct Join<A, B>
    where
        A: OrderedStream,
        B: OrderedStream<Data = A::Data, Ordering=A::Ordering>,
    {
        #[pin]
        stream_a: A,
        #[pin]
        stream_b: B,
        state: JoinState<A::Data, B::Data, A::Ordering>,
    }
}

/// Join two streams while preserving the overall ordering of elements.
///
/// You can think of this as implementing the "merge" step of a merge sort on the two streams,
/// producing a single stream that is sorted given two sorted streams.  If the streams return
/// [`PollResult::NoneBefore`] as intended, then the joined stream will be able to produce items
/// when only one of the sources has unblocked.
pub fn join<A, B>(stream_a: A, stream_b: B) -> Join<A, B>
where
    A: OrderedStream,
    B: OrderedStream<Data = A::Data, Ordering = A::Ordering>,
{
    Join {
        stream_a,
        stream_b,
        state: JoinState::None,
    }
}

#[derive(Debug)]
enum JoinState<A, B, T> {
    None,
    A(A, T),
    B(B, T),
    OnlyPollA,
    OnlyPollB,
    Terminated,
}

impl<A, B, T> JoinState<A, B, T> {
    fn take_split(&mut self) -> (PollState<A, T>, PollState<B, T>) {
        match mem::replace(self, JoinState::None) {
            JoinState::None => (PollState::Pending, PollState::Pending),
            JoinState::A(a, t) => (PollState::Item(a, t), PollState::Pending),
            JoinState::B(b, t) => (PollState::Pending, PollState::Item(b, t)),
            JoinState::OnlyPollA => (PollState::Pending, PollState::Terminated),
            JoinState::OnlyPollB => (PollState::Terminated, PollState::Pending),
            JoinState::Terminated => (PollState::Terminated, PollState::Terminated),
        }
    }
}

/// A helper equivalent to Poll<PollResult<T, I>> but easier to match
pub(crate) enum PollState<I, T> {
    Item(I, T),
    Pending,
    NoneBefore,
    Terminated,
}

impl<I, T: Ord> PollState<I, T> {
    fn ordering(&self) -> Option<&T> {
        match self {
            Self::Item(_, t) => Some(t),
            _ => None,
        }
    }

    fn update(
        &mut self,
        before: Option<&T>,
        other_token: Option<&T>,
        retry: bool,
        run: impl FnOnce(Option<&T>) -> Poll<PollResult<T, I>>,
    ) -> bool {
        match self {
            // Do not re-poll if we have an item already or if we are terminated
            Self::Item { .. } | Self::Terminated => return false,

            // No need to re-poll if we already declared no items <= before
            Self::NoneBefore if retry => return false,

            _ => {}
        }

        // Run the poll with the earlier of the two tokens to avoid transitioning to Pending (which
        // will stall the Join) when we could have transitioned to NoneBefore.
        let ordering = match (before, other_token) {
            (Some(u), Some(o)) => {
                if *u > *o {
                    // The other ordering is earlier - so a retry might let us upgrade a Pending to a
                    // NoneBefore
                    Some(o)
                } else if retry {
                    // A retry will not improve matters, so don't bother
                    return false;
                } else {
                    Some(u)
                }
            }
            (Some(t), None) | (None, Some(t)) => Some(t),
            (None, None) => None,
        };

        *self = run(ordering).into();
        matches!(self, Self::Item { .. })
    }
}

impl<I, T> From<PollState<I, T>> for Poll<PollResult<T, I>> {
    fn from(poll: PollState<I, T>) -> Self {
        match poll {
            PollState::Item(data, ordering) => Poll::Ready(PollResult::Item { data, ordering }),
            PollState::Pending => Poll::Pending,
            PollState::NoneBefore => Poll::Ready(PollResult::NoneBefore),
            PollState::Terminated => Poll::Ready(PollResult::Terminated),
        }
    }
}

impl<I, T> From<Poll<PollResult<T, I>>> for PollState<I, T> {
    fn from(poll: Poll<PollResult<T, I>>) -> Self {
        match poll {
            Poll::Ready(PollResult::Item { data, ordering }) => Self::Item(data, ordering),
            Poll::Ready(PollResult::NoneBefore) => Self::NoneBefore,
            Poll::Ready(PollResult::Terminated) => Self::Terminated,
            Poll::Pending => Self::Pending,
        }
    }
}

impl<A, B> Join<A, B>
where
    A: OrderedStream,
    B: OrderedStream<Data = A::Data, Ordering = A::Ordering>,
{
    /// Split into the source streams.
    ///
    /// This method returns the source streams along with any buffered item and its
    /// ordering.
    pub fn into_inner(self) -> (A, B, Option<(A::Data, A::Ordering)>) {
        let item = match self.state {
            JoinState::A(a, o) => Some((a, o)),
            JoinState::B(b, o) => Some((b, o)),
            _ => None,
        };

        (self.stream_a, self.stream_b, item)
    }

    /// Provide direct access to the underlying stream.
    ///
    /// This may be useful if the stream provides APIs beyond [OrderedStream].  Note that the join
    /// itself may be buffering an item from this stream, so you should consult
    /// [Self::peek_buffered] and, if needed, [Self::take_buffered] before polling it directly.
    pub fn stream_a(self: Pin<&mut Self>) -> Pin<&mut A> {
        self.project().stream_a
    }

    /// Provide direct access to the underlying stream.
    ///
    /// This may be useful if the stream provides APIs beyond [OrderedStream].  Note that the join
    /// itself may be buffering an item from this stream, so you should consult
    /// [Self::peek_buffered] and, if needed, [Self::take_buffered] before polling it directly.
    pub fn stream_b(self: Pin<&mut Self>) -> Pin<&mut B> {
        self.project().stream_b
    }

    /// Allow access to the buffered item, if any.
    ///
    /// At most one of the two sides will be `Some`.  The returned item is a candidate for being
    /// the next item returned by the joined stream, but it could not be returned by the most
    /// recent [`OrderedStream::poll_next_before`] call.
    pub fn peek_buffered(
        self: Pin<&mut Self>,
    ) -> (
        Option<(&mut A::Data, &A::Ordering)>,
        Option<(&mut B::Data, &B::Ordering)>,
    ) {
        match self.project().state {
            JoinState::A(a, o) => (Some((a, o)), None),
            JoinState::B(b, o) => (None, Some((b, o))),
            _ => (None, None),
        }
    }

    /// Remove the buffered item, if one is present.
    ///
    /// This does not poll either underlying stream.  See [Self::peek_buffered] for details on why
    /// buffering exists.
    pub fn take_buffered(self: Pin<&mut Self>) -> Option<(A::Data, A::Ordering)> {
        let state = self.project().state;
        match mem::replace(state, JoinState::None) {
            JoinState::A(a, o) => Some((a, o)),
            JoinState::B(b, o) => Some((b, o)),
            other => {
                *state = other;
                None
            }
        }
    }
}

impl<A, B> OrderedStream for Join<A, B>
where
    A: OrderedStream,
    B: OrderedStream<Data = A::Data, Ordering = A::Ordering>,
{
    type Data = A::Data;
    type Ordering = A::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let mut this = self.project();
        let (mut poll_a, mut poll_b) = this.state.take_split();

        poll_a.update(before, poll_b.ordering(), false, |ordering| {
            this.stream_a.as_mut().poll_next_before(cx, ordering)
        });
        if poll_b.update(before, poll_a.ordering(), false, |ordering| {
            this.stream_b.as_mut().poll_next_before(cx, ordering)
        }) {
            // If B just got an item, it's possible that A already knows that it won't have any
            // items before that item; we couldn't ask that question before.  Ask it now.
            poll_a.update(before, poll_b.ordering(), true, |ordering| {
                this.stream_a.as_mut().poll_next_before(cx, ordering)
            });
        }

        match (poll_a, poll_b) {
            // Both are ready - we can judge ordering directly (simplest case).  The first one is
            // returned while the other one is buffered for the next poll.
            (PollState::Item(a, ta), PollState::Item(b, tb)) => {
                if ta <= tb {
                    *this.state = JoinState::B(b, tb);
                    Poll::Ready(PollResult::Item {
                        data: a,
                        ordering: ta,
                    })
                } else {
                    *this.state = JoinState::A(a, ta);
                    Poll::Ready(PollResult::Item {
                        data: b,
                        ordering: tb,
                    })
                }
            }

            // If both sides are terminated, so are we.
            (PollState::Terminated, PollState::Terminated) => {
                *this.state = JoinState::Terminated;
                Poll::Ready(PollResult::Terminated)
            }

            // If one side is terminated, we can produce items directly from the other side.
            (a, PollState::Terminated) => {
                *this.state = JoinState::OnlyPollA;
                a.into()
            }
            (PollState::Terminated, b) => {
                *this.state = JoinState::OnlyPollB;
                b.into()
            }

            // If one side is pending, we can't return Ready until that gets resolved.  Because we
            // have already requested that our child streams wake us when it is possible to make
            // any kind of progress, we meet the requirements to return Poll::Pending.
            (PollState::Item(a, t), PollState::Pending) => {
                *this.state = JoinState::A(a, t);
                Poll::Pending
            }
            (PollState::Pending, PollState::Item(b, t)) => {
                *this.state = JoinState::B(b, t);
                Poll::Pending
            }
            (PollState::Pending, PollState::Pending) => Poll::Pending,
            (PollState::Pending, PollState::NoneBefore) => Poll::Pending,
            (PollState::NoneBefore, PollState::Pending) => Poll::Pending,

            // If both sides report NoneBefore, so can we.
            (PollState::NoneBefore, PollState::NoneBefore) => Poll::Ready(PollResult::NoneBefore),

            (PollState::Item(data, ordering), PollState::NoneBefore) => {
                // B was polled using either the Some value of (before) or using A's ordering.
                //
                // If before is set and is earlier than A's ordering, then B might later produce a
                // value with (bt >= before && bt < at), so we can't return A's item yet and must
                // buffer it.  However, we can return None because neither stream will produce
                // items before the ordering passed in before.
                //
                // If before is either None or after A's ordering, B's NoneBefore return represents a
                // promise to not produce an item before A's, so we can return A's item now.
                match before {
                    Some(before) if ordering > *before => {
                        *this.state = JoinState::A(data, ordering);
                        Poll::Ready(PollResult::NoneBefore)
                    }
                    _ => Poll::Ready(PollResult::Item { data, ordering }),
                }
            }

            (PollState::NoneBefore, PollState::Item(data, ordering)) => {
                // A was polled using either the Some value of (before) or using B's ordering.
                //
                // By a mirror of the above argument, this NoneBefore result gives us permission to
                // produce either B's item or NoneBefore.
                match before {
                    Some(before) if ordering > *before => {
                        *this.state = JoinState::B(data, ordering);
                        Poll::Ready(PollResult::NoneBefore)
                    }
                    _ => Poll::Ready(PollResult::Item { data, ordering }),
                }
            }
        }
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        let (a, b) = match &self.state {
            JoinState::None => (self.stream_a.position_hint(), self.stream_b.position_hint()),
            JoinState::A(_, t) => (
                Some(MaybeBorrowed::Borrowed(t)),
                self.stream_b.position_hint(),
            ),
            JoinState::B(_, t) => (
                self.stream_b.position_hint(),
                Some(MaybeBorrowed::Borrowed(t)),
            ),
            JoinState::OnlyPollA => return self.stream_a.position_hint(),
            JoinState::OnlyPollB => return self.stream_b.position_hint(),
            JoinState::Terminated => return None,
        };
        // We can only provide a hint if we have a valid hint for both sides
        match (a, b) {
            (Some(a), Some(b)) if *a <= *b => Some(a),
            (Some(_), Some(b)) => Some(b),
            _ => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let extra = match &self.state {
            JoinState::None => 0,
            JoinState::A { .. } => 1,
            JoinState::B { .. } => 1,
            JoinState::OnlyPollA => return self.stream_a.size_hint(),
            JoinState::OnlyPollB => return self.stream_b.size_hint(),
            JoinState::Terminated => return (0, Some(0)),
        };
        let (al, ah) = self.stream_a.size_hint();
        let (bl, bh) = self.stream_b.size_hint();
        let min = al.saturating_add(bl).saturating_add(extra);
        let max = ah
            .and_then(|a| bh.and_then(|b| a.checked_add(b)))
            .and_then(|h| h.checked_add(extra));
        (min, max)
    }
}

impl<A, B> FusedOrderedStream for Join<A, B>
where
    A: OrderedStream,
    B: OrderedStream<Data = A::Data, Ordering = A::Ordering>,
{
    fn is_terminated(&self) -> bool {
        matches!(self.state, JoinState::Terminated)
    }
}

#[cfg(test)]
mod test {
    extern crate alloc;
    use crate::join;
    use crate::FromStream;
    use crate::OrderedStream;
    use crate::OrderedStreamExt;
    use crate::PollResult;
    use alloc::rc::Rc;
    use core::cell::Cell;
    use core::pin::Pin;
    use core::task::{Context, Poll};
    use futures_executor::block_on;
    use futures_util::pin_mut;
    use futures_util::stream::iter;

    #[derive(Debug, PartialEq)]
    pub struct Message {
        serial: u32,
    }

    #[test]
    fn join_two() {
        block_on(async {
            let stream1 = iter([
                Message { serial: 1 },
                Message { serial: 4 },
                Message { serial: 5 },
            ]);

            let stream2 = iter([
                Message { serial: 2 },
                Message { serial: 3 },
                Message { serial: 6 },
            ]);
            let mut joined = join(
                FromStream::with_ordering(stream1, |m| m.serial),
                FromStream::with_ordering(stream2, |m| m.serial),
            );
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

            let join = join(stream1, stream2);
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
