use super::{Action, Attempt, Policy};

/// A redirection [`Policy`] that limits the number of successive redirections.
#[derive(Clone, Copy, Debug)]
pub struct Limited {
    remaining: usize,
}

impl Limited {
    /// Create a new [`Limited`] with a limit of `max` redirections.
    pub fn new(max: usize) -> Self {
        Limited { remaining: max }
    }
}

impl Default for Limited {
    /// Returns the default [`Limited`] with a limit of `20` redirections.
    fn default() -> Self {
        // This is the (default) limit of Firefox and the Fetch API.
        // https://hg.mozilla.org/mozilla-central/file/6264f13d54a1caa4f5b60303617a819efd91b8ee/modules/libpref/init/all.js#l1371
        // https://fetch.spec.whatwg.org/#http-redirect-fetch
        Limited::new(20)
    }
}

impl<B, E> Policy<B, E> for Limited {
    fn redirect(&mut self, _: &Attempt<'_>) -> Result<Action, E> {
        if self.remaining > 0 {
            self.remaining -= 1;
            Ok(Action::Follow)
        } else {
            Ok(Action::Stop)
        }
    }
}

#[cfg(test)]
mod tests {
    use http::{Request, Uri};

    use super::*;

    #[test]
    fn works() {
        let uri = Uri::from_static("https://example.com/");
        let mut policy = Limited::new(2);

        for _ in 0..2 {
            let mut request = Request::builder().uri(uri.clone()).body(()).unwrap();
            Policy::<(), ()>::on_request(&mut policy, &mut request);

            let attempt = Attempt {
                status: Default::default(),
                location: &uri,
                previous: &uri,
            };
            assert!(Policy::<(), ()>::redirect(&mut policy, &attempt)
                .unwrap()
                .is_follow());
        }

        let mut request = Request::builder().uri(uri.clone()).body(()).unwrap();
        Policy::<(), ()>::on_request(&mut policy, &mut request);

        let attempt = Attempt {
            status: Default::default(),
            location: &uri,
            previous: &uri,
        };
        assert!(Policy::<(), ()>::redirect(&mut policy, &attempt)
            .unwrap()
            .is_stop());
    }
}
