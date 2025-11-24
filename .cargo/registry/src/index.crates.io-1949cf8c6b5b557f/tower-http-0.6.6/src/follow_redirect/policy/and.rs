use super::{Action, Attempt, Policy};
use http::Request;

/// A redirection [`Policy`] that combines the results of two `Policy`s.
///
/// See [`PolicyExt::and`][super::PolicyExt::and] for more details.
#[derive(Clone, Copy, Debug, Default)]
pub struct And<A, B> {
    a: A,
    b: B,
}

impl<A, B> And<A, B> {
    pub(crate) fn new<Bd, E>(a: A, b: B) -> Self
    where
        A: Policy<Bd, E>,
        B: Policy<Bd, E>,
    {
        And { a, b }
    }
}

impl<Bd, E, A, B> Policy<Bd, E> for And<A, B>
where
    A: Policy<Bd, E>,
    B: Policy<Bd, E>,
{
    fn redirect(&mut self, attempt: &Attempt<'_>) -> Result<Action, E> {
        match self.a.redirect(attempt) {
            Ok(Action::Follow) => self.b.redirect(attempt),
            a => a,
        }
    }

    fn on_request(&mut self, request: &mut Request<Bd>) {
        self.a.on_request(request);
        self.b.on_request(request);
    }

    fn clone_body(&self, body: &Bd) -> Option<Bd> {
        self.a.clone_body(body).or_else(|| self.b.clone_body(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Uri;

    struct Taint<P> {
        policy: P,
        used: bool,
    }

    impl<P> Taint<P> {
        fn new(policy: P) -> Self {
            Taint {
                policy,
                used: false,
            }
        }
    }

    impl<B, E, P> Policy<B, E> for Taint<P>
    where
        P: Policy<B, E>,
    {
        fn redirect(&mut self, attempt: &Attempt<'_>) -> Result<Action, E> {
            self.used = true;
            self.policy.redirect(attempt)
        }
    }

    #[test]
    fn redirect() {
        let attempt = Attempt {
            status: Default::default(),
            location: &Uri::from_static("*"),
            previous: &Uri::from_static("*"),
        };

        let mut a = Taint::new(Action::Follow);
        let mut b = Taint::new(Action::Follow);
        let mut policy = And::new::<(), ()>(&mut a, &mut b);
        assert!(Policy::<(), ()>::redirect(&mut policy, &attempt)
            .unwrap()
            .is_follow());
        assert!(a.used);
        assert!(b.used);

        let mut a = Taint::new(Action::Stop);
        let mut b = Taint::new(Action::Follow);
        let mut policy = And::new::<(), ()>(&mut a, &mut b);
        assert!(Policy::<(), ()>::redirect(&mut policy, &attempt)
            .unwrap()
            .is_stop());
        assert!(a.used);
        assert!(!b.used); // short-circuiting

        let mut a = Taint::new(Action::Follow);
        let mut b = Taint::new(Action::Stop);
        let mut policy = And::new::<(), ()>(&mut a, &mut b);
        assert!(Policy::<(), ()>::redirect(&mut policy, &attempt)
            .unwrap()
            .is_stop());
        assert!(a.used);
        assert!(b.used);

        let mut a = Taint::new(Action::Stop);
        let mut b = Taint::new(Action::Stop);
        let mut policy = And::new::<(), ()>(&mut a, &mut b);
        assert!(Policy::<(), ()>::redirect(&mut policy, &attempt)
            .unwrap()
            .is_stop());
        assert!(a.used);
        assert!(!b.used);
    }
}
