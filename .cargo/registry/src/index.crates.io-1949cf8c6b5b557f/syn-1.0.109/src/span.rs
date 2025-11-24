use proc_macro2::Span;

pub trait IntoSpans<S> {
    fn into_spans(self) -> S;
}

impl IntoSpans<[Span; 1]> for Span {
    fn into_spans(self) -> [Span; 1] {
        [self]
    }
}

impl IntoSpans<[Span; 2]> for Span {
    fn into_spans(self) -> [Span; 2] {
        [self, self]
    }
}

impl IntoSpans<[Span; 3]> for Span {
    fn into_spans(self) -> [Span; 3] {
        [self, self, self]
    }
}

impl IntoSpans<[Span; 1]> for [Span; 1] {
    fn into_spans(self) -> [Span; 1] {
        self
    }
}

impl IntoSpans<[Span; 2]> for [Span; 2] {
    fn into_spans(self) -> [Span; 2] {
        self
    }
}

impl IntoSpans<[Span; 3]> for [Span; 3] {
    fn into_spans(self) -> [Span; 3] {
        self
    }
}

#[cfg(feature = "parsing")]
pub trait FromSpans: Sized {
    fn from_spans(spans: &[Span]) -> Self;
}

#[cfg(feature = "parsing")]
impl FromSpans for [Span; 1] {
    fn from_spans(spans: &[Span]) -> Self {
        [spans[0]]
    }
}

#[cfg(feature = "parsing")]
impl FromSpans for [Span; 2] {
    fn from_spans(spans: &[Span]) -> Self {
        [spans[0], spans[1]]
    }
}

#[cfg(feature = "parsing")]
impl FromSpans for [Span; 3] {
    fn from_spans(spans: &[Span]) -> Self {
        [spans[0], spans[1], spans[2]]
    }
}
