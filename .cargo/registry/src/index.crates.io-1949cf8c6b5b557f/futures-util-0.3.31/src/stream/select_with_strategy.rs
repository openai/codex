use super::assert_stream;
use core::{fmt, pin::Pin};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

/// Type to tell [`SelectWithStrategy`] which stream to poll next.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum PollNext {
    /// Poll the first stream.
    Left,
    /// Poll the second stream.
    Right,
}

impl PollNext {
    /// Toggle the value and return the old one.
    pub fn toggle(&mut self) -> Self {
        let old = *self;
        *self = self.other();
        old
    }

    fn other(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

impl Default for PollNext {
    fn default() -> Self {
        Self::Left
    }
}

enum InternalState {
    Start,
    LeftFinished,
    RightFinished,
    BothFinished,
}

impl InternalState {
    fn finish(&mut self, ps: PollNext) {
        match (&self, ps) {
            (Self::Start, PollNext::Left) => {
                *self = Self::LeftFinished;
            }
            (Self::Start, PollNext::Right) => {
                *self = Self::RightFinished;
            }
            (Self::LeftFinished, PollNext::Right) | (Self::RightFinished, PollNext::Left) => {
                *self = Self::BothFinished;
            }
            _ => {}
        }
    }
}

pin_project! {
    /// Stream for the [`select_with_strategy()`] function. See function docs for details.
    #[must_use = "streams do nothing unless polled"]
    #[project = SelectWithStrategyProj]
    pub struct SelectWithStrategy<St1, St2, Clos, State> {
        #[pin]
        stream1: St1,
        #[pin]
        stream2: St2,
        internal_state: InternalState,
        state: State,
        clos: Clos,
    }
}

#[allow(clippy::too_long_first_doc_paragraph)]
/// This function will attempt to pull items from both streams. You provide a
/// closure to tell [`SelectWithStrategy`] which stream to poll. The closure can
/// store state on `SelectWithStrategy` to which it will receive a `&mut` on every
/// invocation. This allows basing the strategy on prior choices.
///
/// After one of the two input streams completes, the remaining one will be
/// polled exclusively. The returned stream completes when both input
/// streams have completed.
///
/// Note that this function consumes both streams and returns a wrapped
/// version of them.
///
/// ## Examples
///
/// ### Priority
/// This example shows how to always prioritize the left stream.
///
/// ```rust
/// # futures::executor::block_on(async {
/// use futures::stream::{ repeat, select_with_strategy, PollNext, StreamExt };
///
/// let left = repeat(1);
/// let right = repeat(2);
///
/// // We don't need any state, so let's make it an empty tuple.
/// // We must provide some type here, as there is no way for the compiler
/// // to infer it. As we don't need to capture variables, we can just
/// // use a function pointer instead of a closure.
/// fn prio_left(_: &mut ()) -> PollNext { PollNext::Left }
///
/// let mut out = select_with_strategy(left, right, prio_left);
///
/// for _ in 0..100 {
///     // Whenever we poll out, we will alwas get `1`.
///     assert_eq!(1, out.select_next_some().await);
/// }
/// # });
/// ```
///
/// ### Round Robin
/// This example shows how to select from both streams round robin.
/// Note: this special case is provided by [`futures-util::stream::select`].
///
/// ```rust
/// # futures::executor::block_on(async {
/// use futures::stream::{ repeat, select_with_strategy, PollNext, StreamExt };
///
/// let left = repeat(1);
/// let right = repeat(2);
///
/// let rrobin = |last: &mut PollNext| last.toggle();
///
/// let mut out = select_with_strategy(left, right, rrobin);
///
/// for _ in 0..100 {
///     // We should be alternating now.
///     assert_eq!(1, out.select_next_some().await);
///     assert_eq!(2, out.select_next_some().await);
/// }
/// # });
/// ```
pub fn select_with_strategy<St1, St2, Clos, State>(
    stream1: St1,
    stream2: St2,
    which: Clos,
) -> SelectWithStrategy<St1, St2, Clos, State>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
    State: Default,
{
    assert_stream::<St1::Item, _>(SelectWithStrategy {
        stream1,
        stream2,
        state: Default::default(),
        internal_state: InternalState::Start,
        clos: which,
    })
}

impl<St1, St2, Clos, State> SelectWithStrategy<St1, St2, Clos, State> {
    /// Acquires a reference to the underlying streams that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> (&St1, &St2) {
        (&self.stream1, &self.stream2)
    }

    /// Acquires a mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> (&mut St1, &mut St2) {
        (&mut self.stream1, &mut self.stream2)
    }

    /// Acquires a pinned mutable reference to the underlying streams that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> (Pin<&mut St1>, Pin<&mut St2>) {
        let this = self.project();
        (this.stream1, this.stream2)
    }

    /// Consumes this combinator, returning the underlying streams.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> (St1, St2) {
        (self.stream1, self.stream2)
    }
}

impl<St1, St2, Clos, State> FusedStream for SelectWithStrategy<St1, St2, Clos, State>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
{
    fn is_terminated(&self) -> bool {
        match self.internal_state {
            InternalState::BothFinished => true,
            _ => false,
        }
    }
}

#[inline]
fn poll_side<St1, St2, Clos, State>(
    select: &mut SelectWithStrategyProj<'_, St1, St2, Clos, State>,
    side: PollNext,
    cx: &mut Context<'_>,
) -> Poll<Option<St1::Item>>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
{
    match side {
        PollNext::Left => select.stream1.as_mut().poll_next(cx),
        PollNext::Right => select.stream2.as_mut().poll_next(cx),
    }
}

#[inline]
fn poll_inner<St1, St2, Clos, State>(
    select: &mut SelectWithStrategyProj<'_, St1, St2, Clos, State>,
    side: PollNext,
    cx: &mut Context<'_>,
) -> Poll<Option<St1::Item>>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
{
    let first_done = match poll_side(select, side, cx) {
        Poll::Ready(Some(item)) => return Poll::Ready(Some(item)),
        Poll::Ready(None) => {
            select.internal_state.finish(side);
            true
        }
        Poll::Pending => false,
    };
    let other = side.other();
    match poll_side(select, other, cx) {
        Poll::Ready(None) => {
            select.internal_state.finish(other);
            if first_done {
                Poll::Ready(None)
            } else {
                Poll::Pending
            }
        }
        a => a,
    }
}

impl<St1, St2, Clos, State> Stream for SelectWithStrategy<St1, St2, Clos, State>
where
    St1: Stream,
    St2: Stream<Item = St1::Item>,
    Clos: FnMut(&mut State) -> PollNext,
{
    type Item = St1::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St1::Item>> {
        let mut this = self.project();

        match this.internal_state {
            InternalState::Start => {
                let next_side = (this.clos)(this.state);
                poll_inner(&mut this, next_side, cx)
            }
            InternalState::LeftFinished => match this.stream2.poll_next(cx) {
                Poll::Ready(None) => {
                    *this.internal_state = InternalState::BothFinished;
                    Poll::Ready(None)
                }
                a => a,
            },
            InternalState::RightFinished => match this.stream1.poll_next(cx) {
                Poll::Ready(None) => {
                    *this.internal_state = InternalState::BothFinished;
                    Poll::Ready(None)
                }
                a => a,
            },
            InternalState::BothFinished => Poll::Ready(None),
        }
    }
}

impl<St1, St2, Clos, State> fmt::Debug for SelectWithStrategy<St1, St2, Clos, State>
where
    St1: fmt::Debug,
    St2: fmt::Debug,
    State: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectWithStrategy")
            .field("stream1", &self.stream1)
            .field("stream2", &self.stream2)
            .field("state", &self.state)
            .finish()
    }
}
