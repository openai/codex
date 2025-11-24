use super::{Action, Attempt, Policy};
use std::fmt;

/// A redirection [`Policy`] created from a closure.
///
/// See [`redirect_fn`] for more details.
#[derive(Clone, Copy)]
pub struct RedirectFn<F> {
    f: F,
}

impl<F> fmt::Debug for RedirectFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedirectFn")
            .field("f", &std::any::type_name::<F>())
            .finish()
    }
}

impl<B, E, F> Policy<B, E> for RedirectFn<F>
where
    F: FnMut(&Attempt<'_>) -> Result<Action, E>,
{
    fn redirect(&mut self, attempt: &Attempt<'_>) -> Result<Action, E> {
        (self.f)(attempt)
    }
}

/// Create a new redirection [`Policy`] from a closure
/// `F: FnMut(&Attempt<'_>) -> Result<Action, E>`.
///
/// [`redirect`][Policy::redirect] method of the returned `Policy` delegates to
/// the wrapped closure.
pub fn redirect_fn<F, E>(f: F) -> RedirectFn<F>
where
    F: FnMut(&Attempt<'_>) -> Result<Action, E>,
{
    RedirectFn { f }
}
