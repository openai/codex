//! Tools for customizing the behavior of a [`FollowRedirect`][super::FollowRedirect] middleware.

mod and;
mod clone_body_fn;
mod filter_credentials;
mod limited;
mod or;
mod redirect_fn;
mod same_origin;

pub use self::{
    and::And,
    clone_body_fn::{clone_body_fn, CloneBodyFn},
    filter_credentials::FilterCredentials,
    limited::Limited,
    or::Or,
    redirect_fn::{redirect_fn, RedirectFn},
    same_origin::SameOrigin,
};

use http::{uri::Scheme, Request, StatusCode, Uri};

/// Trait for the policy on handling redirection responses.
///
/// # Example
///
/// Detecting a cyclic redirection:
///
/// ```
/// use http::{Request, Uri};
/// use std::collections::HashSet;
/// use tower_http::follow_redirect::policy::{Action, Attempt, Policy};
///
/// #[derive(Clone)]
/// pub struct DetectCycle {
///     uris: HashSet<Uri>,
/// }
///
/// impl<B, E> Policy<B, E> for DetectCycle {
///     fn redirect(&mut self, attempt: &Attempt<'_>) -> Result<Action, E> {
///         if self.uris.contains(attempt.location()) {
///             Ok(Action::Stop)
///         } else {
///             self.uris.insert(attempt.previous().clone());
///             Ok(Action::Follow)
///         }
///     }
/// }
/// ```
pub trait Policy<B, E> {
    /// Invoked when the service received a response with a redirection status code (`3xx`).
    ///
    /// This method returns an [`Action`] which indicates whether the service should follow
    /// the redirection.
    fn redirect(&mut self, attempt: &Attempt<'_>) -> Result<Action, E>;

    /// Invoked right before the service makes a request, regardless of whether it is redirected
    /// or not.
    ///
    /// This can for example be used to remove sensitive headers from the request
    /// or prepare the request in other ways.
    ///
    /// The default implementation does nothing.
    fn on_request(&mut self, _request: &mut Request<B>) {}

    /// Try to clone a request body before the service makes a redirected request.
    ///
    /// If the request body cannot be cloned, return `None`.
    ///
    /// This is not invoked when [`B::size_hint`][http_body::Body::size_hint] returns zero,
    /// in which case `B::default()` will be used to create a new request body.
    ///
    /// The default implementation returns `None`.
    fn clone_body(&self, _body: &B) -> Option<B> {
        None
    }
}

impl<B, E, P> Policy<B, E> for &mut P
where
    P: Policy<B, E> + ?Sized,
{
    fn redirect(&mut self, attempt: &Attempt<'_>) -> Result<Action, E> {
        (**self).redirect(attempt)
    }

    fn on_request(&mut self, request: &mut Request<B>) {
        (**self).on_request(request)
    }

    fn clone_body(&self, body: &B) -> Option<B> {
        (**self).clone_body(body)
    }
}

impl<B, E, P> Policy<B, E> for Box<P>
where
    P: Policy<B, E> + ?Sized,
{
    fn redirect(&mut self, attempt: &Attempt<'_>) -> Result<Action, E> {
        (**self).redirect(attempt)
    }

    fn on_request(&mut self, request: &mut Request<B>) {
        (**self).on_request(request)
    }

    fn clone_body(&self, body: &B) -> Option<B> {
        (**self).clone_body(body)
    }
}

/// An extension trait for `Policy` that provides additional adapters.
pub trait PolicyExt {
    /// Create a new `Policy` that returns [`Action::Follow`] only if `self` and `other` return
    /// `Action::Follow`.
    ///
    /// [`clone_body`][Policy::clone_body] method of the returned `Policy` tries to clone the body
    /// with both policies.
    ///
    /// # Example
    ///
    /// ```
    /// use bytes::Bytes;
    /// use http_body_util::Full;
    /// use tower_http::follow_redirect::policy::{self, clone_body_fn, Limited, PolicyExt};
    ///
    /// enum MyBody {
    ///     Bytes(Bytes),
    ///     Full(Full<Bytes>),
    /// }
    ///
    /// let policy = Limited::default().and::<_, _, ()>(clone_body_fn(|body| {
    ///     if let MyBody::Bytes(buf) = body {
    ///         Some(MyBody::Bytes(buf.clone()))
    ///     } else {
    ///         None
    ///     }
    /// }));
    /// ```
    fn and<P, B, E>(self, other: P) -> And<Self, P>
    where
        Self: Policy<B, E> + Sized,
        P: Policy<B, E>;

    /// Create a new `Policy` that returns [`Action::Follow`] if either `self` or `other` returns
    /// `Action::Follow`.
    ///
    /// [`clone_body`][Policy::clone_body] method of the returned `Policy` tries to clone the body
    /// with both policies.
    ///
    /// # Example
    ///
    /// ```
    /// use tower_http::follow_redirect::policy::{self, Action, Limited, PolicyExt};
    ///
    /// #[derive(Clone)]
    /// enum MyError {
    ///     TooManyRedirects,
    ///     // ...
    /// }
    ///
    /// let policy = Limited::default().or::<_, (), _>(Err(MyError::TooManyRedirects));
    /// ```
    fn or<P, B, E>(self, other: P) -> Or<Self, P>
    where
        Self: Policy<B, E> + Sized,
        P: Policy<B, E>;
}

impl<T> PolicyExt for T
where
    T: ?Sized,
{
    fn and<P, B, E>(self, other: P) -> And<Self, P>
    where
        Self: Policy<B, E> + Sized,
        P: Policy<B, E>,
    {
        And::new(self, other)
    }

    fn or<P, B, E>(self, other: P) -> Or<Self, P>
    where
        Self: Policy<B, E> + Sized,
        P: Policy<B, E>,
    {
        Or::new(self, other)
    }
}

/// A redirection [`Policy`] with a reasonable set of standard behavior.
///
/// This policy limits the number of successive redirections ([`Limited`])
/// and removes credentials from requests in cross-origin redirections ([`FilterCredentials`]).
pub type Standard = And<Limited, FilterCredentials>;

/// A type that holds information on a redirection attempt.
pub struct Attempt<'a> {
    pub(crate) status: StatusCode,
    pub(crate) location: &'a Uri,
    pub(crate) previous: &'a Uri,
}

impl<'a> Attempt<'a> {
    /// Returns the redirection response.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the destination URI of the redirection.
    pub fn location(&self) -> &'a Uri {
        self.location
    }

    /// Returns the URI of the original request.
    pub fn previous(&self) -> &'a Uri {
        self.previous
    }
}

/// A value returned by [`Policy::redirect`] which indicates the action
/// [`FollowRedirect`][super::FollowRedirect] should take for a redirection response.
#[derive(Clone, Copy, Debug)]
pub enum Action {
    /// Follow the redirection.
    Follow,
    /// Do not follow the redirection, and return the redirection response as-is.
    Stop,
}

impl Action {
    /// Returns `true` if the `Action` is a `Follow` value.
    pub fn is_follow(&self) -> bool {
        if let Action::Follow = self {
            true
        } else {
            false
        }
    }

    /// Returns `true` if the `Action` is a `Stop` value.
    pub fn is_stop(&self) -> bool {
        if let Action::Stop = self {
            true
        } else {
            false
        }
    }
}

impl<B, E> Policy<B, E> for Action {
    fn redirect(&mut self, _: &Attempt<'_>) -> Result<Action, E> {
        Ok(*self)
    }
}

impl<B, E> Policy<B, E> for Result<Action, E>
where
    E: Clone,
{
    fn redirect(&mut self, _: &Attempt<'_>) -> Result<Action, E> {
        self.clone()
    }
}

/// Compares the origins of two URIs as per RFC 6454 sections 4. through 5.
fn eq_origin(lhs: &Uri, rhs: &Uri) -> bool {
    let default_port = match (lhs.scheme(), rhs.scheme()) {
        (Some(l), Some(r)) if l == r => {
            if l == &Scheme::HTTP {
                80
            } else if l == &Scheme::HTTPS {
                443
            } else {
                return false;
            }
        }
        _ => return false,
    };
    match (lhs.host(), rhs.host()) {
        (Some(l), Some(r)) if l == r => {}
        _ => return false,
    }
    lhs.port_u16().unwrap_or(default_port) == rhs.port_u16().unwrap_or(default_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_origin_works() {
        assert!(eq_origin(
            &Uri::from_static("https://example.com/1"),
            &Uri::from_static("https://example.com/2")
        ));
        assert!(eq_origin(
            &Uri::from_static("https://example.com:443/"),
            &Uri::from_static("https://example.com/")
        ));
        assert!(eq_origin(
            &Uri::from_static("https://example.com/"),
            &Uri::from_static("https://user@example.com/")
        ));

        assert!(!eq_origin(
            &Uri::from_static("https://example.com/"),
            &Uri::from_static("https://www.example.com/")
        ));
        assert!(!eq_origin(
            &Uri::from_static("https://example.com/"),
            &Uri::from_static("http://example.com/")
        ));
    }
}
