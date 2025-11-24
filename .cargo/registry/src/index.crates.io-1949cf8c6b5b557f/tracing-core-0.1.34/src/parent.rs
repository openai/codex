use crate::span::Id;

#[derive(Debug)]
pub(crate) enum Parent {
    /// The new span will be a root span.
    Root,
    /// The new span will be rooted in the current span.
    Current,
    /// The new span has an explicitly-specified parent.
    Explicit(Id),
}
