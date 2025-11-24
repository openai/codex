use crate::*;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::{FusedStream, Stream};

/// Helpers for chaining [`OrderedStream`]s.
pub trait OrderedStreamExt: OrderedStream {
    /// Apply a closure to the data.
    ///
    /// This does not change the ordering.
    fn map<F, R>(self, f: F) -> Map<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Data) -> R,
    {
        Map { stream: self, f }
    }

    /// Apply a closure to the items that has access to the ordering data.
    fn map_item<F, R>(self, f: F) -> MapItem<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Ordering, Self::Data) -> R,
    {
        MapItem { stream: self, f }
    }

    /// Apply a closure to the items that can change the type of the ordering value.
    ///
    /// A bidirectional mapping for ordering values is required in order to remap `before` values.
    /// It is the caller's responsibility to ensure that the items in the mapped stream still meet
    /// the ordering requirements that [`OrderedStream`] expects.
    fn map_ordering<NewOrdering, NewData, MapInto, MapFrom>(
        self,
        map_into: MapInto,
        map_from: MapFrom,
    ) -> MapOrdering<Self, MapInto, MapFrom>
    where
        Self: Sized,
        MapInto: FnMut(Self::Ordering, Self::Data) -> (NewOrdering, NewData),
        MapFrom: FnMut(&NewOrdering) -> Option<Self::Ordering>,
        NewOrdering: Ord,
    {
        MapOrdering {
            stream: self,
            map_into,
            map_from,
        }
    }

    fn filter<F>(self, filter: F) -> Filter<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Data) -> bool,
    {
        Filter {
            stream: self,
            filter,
        }
    }

    fn filter_map<F, R>(self, filter: F) -> FilterMap<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Data) -> Option<R>,
    {
        FilterMap {
            stream: self,
            filter,
        }
    }

    /// Apply a closure that produces a [`Future`] to items, running the future on each item in
    /// sequence before processing the next.
    ///
    /// This has the side effect of buffering items that are not before the requested ordering
    /// point; you can use [`ready`](core::future::ready) as the closure to take advantage of this
    /// behavior if you don't want to buffer items yourself.
    fn then<F, Fut>(self, then: F) -> Then<Self, F, Fut>
    where
        Self: Sized,
        F: FnMut(Self::Data) -> Fut,
        Fut: Future,
    {
        Then {
            stream: self,
            then,
            future: ThenItem::Idle,
        }
    }

    /// Convert this into a [`Stream`], discarding the ordering information.
    fn into_stream(self) -> IntoStream<Self>
    where
        Self: Sized,
    {
        IntoStream { stream: self }
    }

    /// Convert this into a [`Stream`], keeping the ordering objects.
    fn into_tuple_stream(self) -> IntoTupleStream<Self>
    where
        Self: Sized,
    {
        IntoTupleStream { stream: self }
    }

    /// Convert this into a [`Stream`], keeping only the ordering objects.
    fn into_ordering(self) -> IntoOrdering<Self>
    where
        Self: Sized,
    {
        IntoOrdering { stream: self }
    }

    /// Return the next item in this stream.
    fn next(&mut self) -> Next<'_, Self>
    where
        Self: Unpin,
    {
        Next {
            stream: Pin::new(self),
        }
    }

    /// Return a [`PollResult`] corresponding to the next item in the stream.
    fn next_before<'a>(&'a mut self, before: Option<&'a Self::Ordering>) -> NextBefore<'a, Self>
    where
        Self: Unpin,
    {
        NextBefore {
            stream: Pin::new(self),
            before,
        }
    }

    fn peekable(self) -> Peekable<Self>
    where
        Self: Sized,
    {
        Peekable {
            stream: self,
            item: None,
            is_terminated: false,
        }
    }
}

impl<T: ?Sized + OrderedStream> OrderedStreamExt for T {}

pin_project_lite::pin_project! {
    /// An [`OrderedStream`] wrapper around a [`Stream`].
    ///
    /// This does not use any future or past knowledge of elements, and so is suitable if the
    /// stream rarely or never blocks.  Prefer using [`FromStream`] if you plan to filter or join
    /// this stream and want other streams to be able to make progress while this one blocks.
    #[derive(Debug)]
    pub struct FromStreamDirect<S, F> {
        #[pin]
        stream: S,
        split_item: F,
    }
}

impl<S, F> FromStreamDirect<S, F> {
    /// Create a new [`OrderedStream`] by applying a `split_item` closure to each element
    /// produced by the original stream.
    pub fn new<Ordering, Data>(stream: S, split_item: F) -> Self
    where
        S: Stream,
        F: FnMut(S::Item) -> (Ordering, Data),
        Ordering: Ord,
    {
        Self { stream, split_item }
    }

    /// Helper function to simplify the creation of a stream when you have a get_ordering function.
    pub fn with_ordering<Ordering>(
        stream: S,
        mut get_ordering: F,
    ) -> FromStreamDirect<S, impl FnMut(S::Item) -> (Ordering, S::Item)>
    where
        S: Stream,
        F: FnMut(&S::Item) -> Ordering,
        Ordering: Ord,
    {
        FromStreamDirect::new(stream, move |data| {
            let ordering = get_ordering(&data);
            (ordering, data)
        })
    }
}

impl<S, F, Ordering, Data> OrderedStream for FromStreamDirect<S, F>
where
    S: Stream,
    F: FnMut(S::Item) -> (Ordering, Data),
    Ordering: Ord,
{
    type Data = Data;
    type Ordering = Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        _: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let this = self.project();
        let split_item = this.split_item;
        this.stream.poll_next(cx).map(|opt| match opt {
            None => PollResult::Terminated,
            Some(data) => {
                let (ordering, data) = split_item(data);
                PollResult::Item { data, ordering }
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl<S, F, Ordering, Data> FusedOrderedStream for FromStreamDirect<S, F>
where
    S: FusedStream,
    F: FnMut(S::Item) -> (Ordering, Data),
    Ordering: Ord,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

pin_project_lite::pin_project! {
    /// An [`OrderedStream`] wrapper around a [`Stream`].
    ///
    /// Unlike [`FromStream`], the items in the [`Stream`] are themselves ordered with no
    /// additional data.
    #[derive(Debug)]
    pub struct FromSortedStream<S> {
        #[pin]
        pub stream: S,
    }
}

impl<S> FromSortedStream<S> {
    /// Create a new [`OrderedStream`] by applying a `split_item` closure to each element
    /// produced by the original stream.
    pub fn new(stream: S) -> Self
    where
        S: Stream,
        S::Item: Ord,
    {
        Self { stream }
    }
}

impl<S> OrderedStream for FromSortedStream<S>
where
    S: Stream,
    S::Item: Ord,
{
    type Data = ();
    type Ordering = S::Item;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        _: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let this = self.project();
        this.stream.poll_next(cx).map(|opt| match opt {
            None => PollResult::Terminated,
            Some(ordering) => PollResult::Item { data: (), ordering },
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl<S> FusedOrderedStream for FromSortedStream<S>
where
    S: FusedStream,
    S::Item: Ord,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

pin_project_lite::pin_project! {
    /// An [`OrderedStream`] wrapper around a [`Stream`].
    ///
    /// This caches the last-used ordering point returned by the stream and uses it to produce
    /// NoneBefore results.  This makes it suitable for using to adapt streams that are filtered
    /// or mapped before joining.  It still relies on the original stream producing a later-ordered
    /// element to allow other streams to progress, however.
    #[derive(Debug)]
    pub struct FromStream<S, F, Ordering> {
        #[pin]
        stream: S,
        split_item: F,
        last: Option<Ordering>,
    }
}

impl<S, F, Ordering> FromStream<S, F, Ordering>
where
    S: Stream,
    Ordering: Ord + Clone,
{
    /// Create a new [`OrderedStream`] by applying a `split_item` closure to each element
    /// produced by the original stream.
    pub fn new<Data>(stream: S, split_item: F) -> Self
    where
        F: FnMut(S::Item) -> (Ordering, Data),
    {
        FromStream {
            stream,
            split_item,
            last: None,
        }
    }

    /// Helper function to simplify the creation of a stream when you have a get_ordering function.
    pub fn with_ordering(
        stream: S,
        mut get_ordering: F,
    ) -> FromStream<S, impl FnMut(S::Item) -> (Ordering, S::Item), Ordering>
    where
        F: FnMut(&S::Item) -> Ordering,
    {
        FromStream::new(stream, move |data| {
            let ordering = get_ordering(&data);
            (ordering, data)
        })
    }
}

impl<S, F, Ordering, Data> OrderedStream for FromStream<S, F, Ordering>
where
    S: Stream,
    F: FnMut(S::Item) -> (Ordering, Data),
    Ordering: Ord + Clone,
{
    type Data = Data;
    type Ordering = Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Ordering, Data>> {
        let this = self.project();
        let split_item = this.split_item;
        let last = this.last;
        if let (Some(last), Some(before)) = (last.as_ref(), before) {
            if last >= before {
                return Poll::Ready(PollResult::NoneBefore);
            }
        }
        this.stream.poll_next(cx).map(|opt| match opt {
            None => PollResult::Terminated,
            Some(item) => {
                let (ordering, data) = split_item(item);
                *last = Some(ordering.clone());
                PollResult::Item { data, ordering }
            }
        })
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        self.last.as_ref().map(MaybeBorrowed::Borrowed)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl<S, F, Ordering, Data> FusedOrderedStream for FromStream<S, F, Ordering>
where
    S: FusedStream,
    F: FnMut(S::Item) -> (Ordering, Data),
    Ordering: Ord + Clone,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

pin_project_lite::pin_project! {
    /// A [`Stream`] for the [`into_stream`](OrderedStreamExt::into_stream) function.
    #[derive(Debug)]
    pub struct IntoStream<S> {
        #[pin]
        stream: S,
    }
}

impl<S: OrderedStream> Stream for IntoStream<S> {
    type Item = S::Data;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .stream
            .poll_next_before(cx, None)
            .map(|r| r.into_data())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl<S> FusedStream for IntoStream<S>
where
    S: FusedOrderedStream,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

pin_project_lite::pin_project! {
    /// A [`Stream`] for the [`into_tuple_stream`](OrderedStreamExt::into_tuple_stream) function.
    #[derive(Debug)]
    pub struct IntoTupleStream<S> {
        #[pin]
        stream: S,
    }
}

impl<S: OrderedStream> Stream for IntoTupleStream<S> {
    type Item = (S::Ordering, S::Data);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .stream
            .poll_next_before(cx, None)
            .map(|r| r.into_tuple())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl<S> FusedStream for IntoTupleStream<S>
where
    S: FusedOrderedStream,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

pin_project_lite::pin_project! {
    /// A [`Stream`] for the [`into_ordering`](OrderedStreamExt::into_ordering) function.
    #[derive(Debug)]
    pub struct IntoOrdering<S> {
        #[pin]
        stream: S,
    }
}

impl<S: OrderedStream> Stream for IntoOrdering<S> {
    type Item = S::Ordering;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .stream
            .poll_next_before(cx, None)
            .map(|r| r.into_tuple().map(|t| t.0))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

impl<S> FusedStream for IntoOrdering<S>
where
    S: FusedOrderedStream,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

pin_project_lite::pin_project! {
    /// An [`OrderedStream`] wrapper around an [`OrderedFuture`].
    #[derive(Debug)]
    pub struct FromFuture<F> {
        #[pin]
        future: Option<F>,
    }
}

impl<F: OrderedFuture> From<F> for FromFuture<F> {
    fn from(future: F) -> Self {
        Self {
            future: Some(future),
        }
    }
}

impl<F: OrderedFuture> OrderedStream for FromFuture<F> {
    type Data = F::Output;
    type Ordering = F::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let mut this = self.project();
        match this.future.as_mut().as_pin_mut() {
            Some(future) => match future.poll_before(cx, before) {
                Poll::Ready(Some((ordering, data))) => {
                    this.future.set(None);
                    Poll::Ready(PollResult::Item { data, ordering })
                }
                Poll::Ready(None) => Poll::Ready(PollResult::NoneBefore),
                Poll::Pending => Poll::Pending,
            },
            None => Poll::Ready(PollResult::Terminated),
        }
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        self.future.as_ref().and_then(|f| f.position_hint())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.future.is_some() {
            (1, Some(1))
        } else {
            (0, Some(0))
        }
    }
}

impl<F: OrderedFuture> FusedOrderedStream for FromFuture<F> {
    fn is_terminated(&self) -> bool {
        self.future.is_none()
    }
}

pin_project_lite::pin_project! {
    /// A stream for the [`map`](OrderedStreamExt::map) function.
    #[derive(Debug)]
    pub struct Map<S, F> {
        #[pin]
        stream: S,
        f: F,
    }
}

impl<S, F> Map<S, F> {
    /// Convert to source stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S, F, R> OrderedStream for Map<S, F>
where
    S: OrderedStream,
    F: FnMut(S::Data) -> R,
{
    type Data = R;
    type Ordering = S::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let this = self.project();
        let f = this.f;
        this.stream
            .poll_next_before(cx, before)
            .map(|res| res.map_data(f))
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        self.stream.position_hint()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

pin_project_lite::pin_project! {
    /// A stream for the [`map_item`](OrderedStreamExt::map_item) function.
    #[derive(Debug)]
    pub struct MapItem<S, F> {
        #[pin]
        stream: S,
        f: F,
    }
}

impl<S, F> MapItem<S, F> {
    /// Convert to source stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S, F, R> OrderedStream for MapItem<S, F>
where
    S: OrderedStream,
    F: FnMut(&S::Ordering, S::Data) -> R,
{
    type Data = R;
    type Ordering = S::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let this = self.project();
        let f = this.f;
        this.stream
            .poll_next_before(cx, before)
            .map(|res| match res {
                PollResult::Item { data, ordering } => {
                    let data = f(&ordering, data);
                    PollResult::Item { data, ordering }
                }
                PollResult::NoneBefore => PollResult::NoneBefore,
                PollResult::Terminated => PollResult::Terminated,
            })
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        self.stream.position_hint()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

pin_project_lite::pin_project! {
    /// A stream for the [`map_ordering`](OrderedStreamExt::map_ordering) function.
    #[derive(Debug)]
    pub struct MapOrdering<S, MapInto, MapFrom> {
        #[pin]
        stream: S,
        map_into: MapInto, map_from: MapFrom,
    }
}

impl<S, I, F> MapOrdering<S, I, F> {
    /// Convert to source stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S, MapInto, MapFrom, NewOrdering, NewData> OrderedStream for MapOrdering<S, MapInto, MapFrom>
where
    S: OrderedStream,
    MapInto: FnMut(S::Ordering, S::Data) -> (NewOrdering, NewData),
    MapFrom: FnMut(&NewOrdering) -> Option<S::Ordering>,
    NewOrdering: Ord,
{
    type Data = NewData;
    type Ordering = NewOrdering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let this = self.project();
        let map_into = this.map_into;
        let before = before.and_then(this.map_from);
        this.stream
            .poll_next_before(cx, before.as_ref())
            .map(|res| match res {
                PollResult::Item { data, ordering } => {
                    let (ordering, data) = map_into(ordering, data);
                    PollResult::Item { data, ordering }
                }
                PollResult::NoneBefore => PollResult::NoneBefore,
                PollResult::Terminated => PollResult::Terminated,
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

pin_project_lite::pin_project! {
    /// A stream for the [`filter`](OrderedStreamExt::filter) function.
    #[derive(Debug)]
    pub struct Filter<S, F> {
        #[pin]
        stream: S,
        filter: F,
    }
}

impl<S, F> Filter<S, F> {
    /// Convert to source stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S, F> OrderedStream for Filter<S, F>
where
    S: OrderedStream,
    F: FnMut(&S::Data) -> bool,
{
    type Data = S::Data;
    type Ordering = S::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let mut this = self.project();
        loop {
            match this.stream.as_mut().poll_next_before(cx, before).into() {
                PollState::Pending => return Poll::Pending,
                PollState::Terminated => return Poll::Ready(PollResult::Terminated),
                PollState::NoneBefore => return Poll::Ready(PollResult::NoneBefore),
                PollState::Item(data, ordering) => {
                    if (this.filter)(&data) {
                        return Poll::Ready(PollResult::Item { data, ordering });
                    }
                }
            }
        }
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        self.stream.position_hint()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.stream.size_hint().1)
    }
}

pin_project_lite::pin_project! {
    /// A stream for the [`filter_map`](OrderedStreamExt::filter_map) function.
    #[derive(Debug)]
    pub struct FilterMap<S, F> {
        #[pin]
        stream: S,
        filter: F,
    }
}

impl<S, F> FilterMap<S, F> {
    /// Convert to source stream.
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S, F, R> OrderedStream for FilterMap<S, F>
where
    S: OrderedStream,
    F: FnMut(S::Data) -> Option<R>,
{
    type Data = R;
    type Ordering = S::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let mut this = self.project();
        loop {
            match this.stream.as_mut().poll_next_before(cx, before).into() {
                PollState::Pending => return Poll::Pending,
                PollState::Terminated => return Poll::Ready(PollResult::Terminated),
                PollState::NoneBefore => return Poll::Ready(PollResult::NoneBefore),
                PollState::Item(data, ordering) => match (this.filter)(data) {
                    Some(data) => return Poll::Ready(PollResult::Item { data, ordering }),
                    None => continue,
                },
            }
        }
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        self.stream.position_hint()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.stream.size_hint().1)
    }
}

pin_project_lite::pin_project! {
    #[project = ThenProj]
    #[project_replace = ThenDone]
    #[derive(Debug)]
    enum ThenItem<Fut, T> {
        Running { #[pin] future: Fut, ordering: T },
        Idle,
    }
}

pin_project_lite::pin_project! {
    /// A stream for the [`then`](OrderedStreamExt::then) function.
    #[derive(Debug)]
    pub struct Then<S, F, Fut>
        where S: OrderedStream
    {
        #[pin]
        stream: S,
        then: F,
        #[pin]
        future: ThenItem<Fut, S::Ordering>,
    }
}

impl<S, F, Fut> OrderedStream for Then<S, F, Fut>
where
    S: OrderedStream,
    F: FnMut(S::Data) -> Fut,
    Fut: Future,
{
    type Data = Fut::Output;
    type Ordering = S::Ordering;

    fn poll_next_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<PollResult<Self::Ordering, Self::Data>> {
        let mut this = self.project();
        loop {
            if let ThenProj::Running { future, ordering } = this.future.as_mut().project() {
                // Because we know the next ordering, we can answer questions about it now.
                if let Some(before) = before {
                    if *ordering >= *before {
                        return Poll::Ready(PollResult::NoneBefore);
                    }
                }

                if let Poll::Ready(data) = future.poll(cx) {
                    if let ThenDone::Running { ordering, .. } =
                        this.future.as_mut().project_replace(ThenItem::Idle)
                    {
                        return Poll::Ready(PollResult::Item { data, ordering });
                    }
                } else {
                    return Poll::Pending;
                }
            }
            match this.stream.as_mut().poll_next_before(cx, before).into() {
                PollState::Pending => return Poll::Pending,
                PollState::Terminated => return Poll::Ready(PollResult::Terminated),
                PollState::NoneBefore => return Poll::Ready(PollResult::NoneBefore),
                PollState::Item(data, ordering) => {
                    this.future.set(ThenItem::Running {
                        future: (this.then)(data),
                        ordering,
                    });
                }
            }
        }
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        match &self.future {
            ThenItem::Running { ordering, .. } => Some(MaybeBorrowed::Borrowed(ordering)),
            ThenItem::Idle => self.stream.position_hint(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (min, max) = self.stream.size_hint();
        match self.future {
            ThenItem::Running { .. } => (min.saturating_add(1), max.and_then(|v| v.checked_add(1))),
            ThenItem::Idle => (min, max),
        }
    }
}

/// A future for the [`next`](OrderedStreamExt::next) function.
#[derive(Debug)]
pub struct Next<'a, S: ?Sized> {
    stream: Pin<&'a mut S>,
}

impl<'a, S: ?Sized> Unpin for Next<'a, S> {}

impl<'a, S> Future for Next<'a, S>
where
    S: OrderedStream + ?Sized,
{
    type Output = Option<S::Data>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Data>> {
        self.stream
            .as_mut()
            .poll_next_before(cx, None)
            .map(PollResult::into_data)
    }
}

/// A future for the [`next_before`](OrderedStreamExt::next_before) function.
#[derive(Debug)]
pub struct NextBefore<'a, S>
where
    S: OrderedStream + ?Sized,
{
    stream: Pin<&'a mut S>,
    before: Option<&'a S::Ordering>,
}

impl<'a, S: OrderedStream + ?Sized> Unpin for NextBefore<'a, S> {}

impl<'a, S> Future for NextBefore<'a, S>
where
    S: OrderedStream + ?Sized,
{
    type Output = PollResult<S::Ordering, S::Data>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<PollResult<S::Ordering, S::Data>> {
        let before = self.before;
        self.stream.as_mut().poll_next_before(cx, before)
    }
}

pin_project_lite::pin_project! {
    /// A stream for the [`peekable`](OrderedStreamExt::peekable) function.
    #[derive(Debug)]
    pub struct Peekable<S: OrderedStream> {
        #[pin]
        stream: S,
        is_terminated: bool,
        item: Option<(S::Ordering, S::Data)>,
    }
}

impl<S: OrderedStream> Peekable<S> {
    /// Convert into the source stream.
    ///
    /// This method returns the source stream along with any buffered item and its
    /// ordering.
    pub fn into_inner(self) -> (S, Option<(S::Data, S::Ordering)>) {
        (self.stream, self.item.map(|(o, d)| (d, o)))
    }

    /// The current item, without polling
    pub(crate) fn item(&self) -> Option<&(S::Ordering, S::Data)> {
        self.item.as_ref()
    }

    /// Peek on the next item in the stream
    pub fn poll_peek_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&S::Ordering>,
    ) -> Poll<PollResult<&S::Ordering, &mut S::Data>> {
        let mut this = self.project();
        if *this.is_terminated {
            return Poll::Ready(PollResult::Terminated);
        }
        let stream = this.stream.as_mut();
        if this.item.is_none() {
            match stream.poll_next_before(cx, before) {
                Poll::Ready(PollResult::Item { ordering, data }) => {
                    *this.item = Some((ordering, data));
                }
                Poll::Ready(PollResult::NoneBefore) => return Poll::Ready(PollResult::NoneBefore),
                Poll::Ready(PollResult::Terminated) => {
                    *this.is_terminated = true;
                    return Poll::Ready(PollResult::Terminated);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
        let item = this.item.as_mut().unwrap();
        Poll::Ready(PollResult::Item {
            ordering: &item.0,
            data: &mut item.1,
        })
    }
}

impl<S: OrderedStream> OrderedStream for Peekable<S> {
    type Ordering = S::Ordering;
    type Data = S::Data;

    fn poll_next_before(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&S::Ordering>,
    ) -> Poll<PollResult<S::Ordering, S::Data>> {
        match self.as_mut().poll_peek_before(cx, before) {
            Poll::Ready(PollResult::Item { .. }) => {
                let (ordering, data) = self.project().item.take().unwrap();
                Poll::Ready(PollResult::Item { ordering, data })
            }
            Poll::Ready(PollResult::NoneBefore) => Poll::Ready(PollResult::NoneBefore),
            Poll::Ready(PollResult::Terminated) => Poll::Ready(PollResult::Terminated),
            Poll::Pending => Poll::Pending,
        }
    }

    fn position_hint(&self) -> Option<MaybeBorrowed<'_, Self::Ordering>> {
        match &self.item {
            Some((ordering, _)) => Some(MaybeBorrowed::Borrowed(ordering)),
            None => self.stream.position_hint(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (min, max) = if self.is_terminated {
            (0, Some(0))
        } else {
            self.stream.size_hint()
        };
        if self.item.is_some() {
            (min.saturating_add(1), max.and_then(|v| v.checked_add(1)))
        } else {
            (min, max)
        }
    }
}

impl<S: OrderedStream> FusedOrderedStream for Peekable<S> {
    fn is_terminated(&self) -> bool {
        self.is_terminated
    }
}
