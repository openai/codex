use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct OnInformational(Arc<dyn OnInformationalCallback + Send + Sync>);

/// Add a callback for 1xx informational responses.
///
/// # Example
///
/// ```
/// # let some_body = ();
/// let mut req = hyper::Request::new(some_body);
///
/// hyper::ext::on_informational(&mut req, |res| {
///     println!("informational: {:?}", res.status());
/// });
///
/// // send request on a client connection...
/// ```
pub fn on_informational<B, F>(req: &mut http::Request<B>, callback: F)
where
    F: Fn(Response<'_>) + Send + Sync + 'static,
{
    on_informational_raw(req, OnInformationalClosure(callback));
}

pub(crate) fn on_informational_raw<B, C>(req: &mut http::Request<B>, callback: C)
where
    C: OnInformationalCallback + Send + Sync + 'static,
{
    req.extensions_mut()
        .insert(OnInformational(Arc::new(callback)));
}

// Sealed, not actually nameable bounds
pub(crate) trait OnInformationalCallback {
    fn on_informational(&self, res: http::Response<()>);
}

impl OnInformational {
    pub(crate) fn call(&self, res: http::Response<()>) {
        self.0.on_informational(res);
    }
}

struct OnInformationalClosure<F>(F);

impl<F> OnInformationalCallback for OnInformationalClosure<F>
where
    F: Fn(Response<'_>) + Send + Sync + 'static,
{
    fn on_informational(&self, res: http::Response<()>) {
        let res = Response(&res);
        (self.0)(res);
    }
}

// A facade over http::Response.
//
// It purposefully hides being able to move the response out of the closure,
// while also not being able to expect it to be a reference `&Response`.
// (Otherwise, a closure can be written as `|res: &_|`, and then be broken if
// we make the closure take ownership.)
//
// With the type not being nameable, we could change from being a facade to
// being either a real reference, or moving the http::Response into the closure,
// in a backwards-compatible change in the future.
#[derive(Debug)]
pub struct Response<'a>(&'a http::Response<()>);

impl Response<'_> {
    #[inline]
    pub fn status(&self) -> http::StatusCode {
        self.0.status()
    }

    #[inline]
    pub fn version(&self) -> http::Version {
        self.0.version()
    }

    #[inline]
    pub fn headers(&self) -> &http::HeaderMap {
        self.0.headers()
    }
}
