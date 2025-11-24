/// Async equivalent of [`Drop`].
///
/// This trait is similar to [`Drop`], but it is async so types that need to perform async
/// operations when they're dropped should implement this. Unfortunately, the `async_drop` method
/// won't be implicitly called by the compiler for the user so types implementing this trait will
/// also have to implement [`Drop`] to clean up (for example, by passing the async cleanup to
/// another async task). This is also why the `async_drop` method consumes `self` instead of taking
/// a `&mut self` like [`Drop`].
///
/// Hopefully this will be unnecessary [in the future][itf] when Rust gain an `AsyncDrop` itself.
///
/// [itf]: https://rust-lang.github.io/async-fundamentals-initiative/roadmap/async_drop.html
#[async_trait::async_trait]
pub trait AsyncDrop {
    /// Perform the async cleanup.
    async fn async_drop(self);
}
