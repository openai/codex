/// Used to do reference-to-value conversions thus not consuming the input value.
///
/// This is mainly used with [`State`] to extract "substates" from a reference to main application
/// state.
///
/// See [`State`] for more details on how library authors should use this trait.
///
/// This trait can be derived using `#[derive(FromRef)]`.
///
/// [`State`]: https://docs.rs/axum/0.8/axum/extract/struct.State.html
// NOTE: This trait is defined in axum-core, even though it is mainly used with `State` which is
// defined in axum. That allows crate authors to use it when implementing extractors.
pub trait FromRef<T> {
    /// Converts to this type from a reference to the input type.
    fn from_ref(input: &T) -> Self;
}

impl<T> FromRef<T> for T
where
    T: Clone,
{
    fn from_ref(input: &T) -> Self {
        input.clone()
    }
}
