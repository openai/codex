use super::{Action, Attempt, Policy};
use std::fmt;

/// A redirection [`Policy`] created from a closure.
///
/// See [`clone_body_fn`] for more details.
#[derive(Clone, Copy)]
pub struct CloneBodyFn<F> {
    f: F,
}

impl<F> fmt::Debug for CloneBodyFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloneBodyFn")
            .field("f", &std::any::type_name::<F>())
            .finish()
    }
}

impl<F, B, E> Policy<B, E> for CloneBodyFn<F>
where
    F: Fn(&B) -> Option<B>,
{
    fn redirect(&mut self, _: &Attempt<'_>) -> Result<Action, E> {
        Ok(Action::Follow)
    }

    fn clone_body(&self, body: &B) -> Option<B> {
        (self.f)(body)
    }
}

/// Create a new redirection [`Policy`] from a closure `F: Fn(&B) -> Option<B>`.
///
/// [`clone_body`][Policy::clone_body] method of the returned `Policy` delegates to the wrapped
/// closure and [`redirect`][Policy::redirect] method always returns [`Action::Follow`].
pub fn clone_body_fn<F, B>(f: F) -> CloneBodyFn<F>
where
    F: Fn(&B) -> Option<B>,
{
    CloneBodyFn { f }
}
