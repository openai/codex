use http::{header::USER_AGENT, HeaderValue, Request};
use std::task::{Context, Poll};
use tower_service::Service;

const TONIC_USER_AGENT: &str = concat!("tonic/", env!("CARGO_PKG_VERSION"));

#[derive(Debug)]
pub(crate) struct UserAgent<T> {
    inner: T,
    user_agent: HeaderValue,
}

impl<T> UserAgent<T> {
    pub(crate) fn new(inner: T, user_agent: Option<HeaderValue>) -> Self {
        let user_agent = user_agent
            .map(|value| {
                let mut buf = Vec::new();
                buf.extend(value.as_bytes());
                buf.push(b' ');
                buf.extend(TONIC_USER_AGENT.as_bytes());
                HeaderValue::from_bytes(&buf).expect("user-agent should be valid")
            })
            .unwrap_or_else(|| HeaderValue::from_static(TONIC_USER_AGENT));

        Self { inner, user_agent }
    }
}

impl<T, ReqBody> Service<Request<ReqBody>> for UserAgent<T>
where
    T: Service<Request<ReqBody>>,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future = T::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        if let Ok(Some(user_agent)) = req
            .headers_mut()
            .try_insert(USER_AGENT, self.user_agent.clone())
        {
            // The User-Agent header has already been set on the request. Let's
            // append our user agent to the end.
            let mut buf = Vec::new();
            buf.extend(user_agent.as_bytes());
            buf.push(b' ');
            buf.extend(self.user_agent.as_bytes());
            req.headers_mut().insert(
                USER_AGENT,
                HeaderValue::from_bytes(&buf).expect("user-agent should be valid"),
            );
        }

        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Svc;

    #[test]
    fn sets_default_if_no_custom_user_agent() {
        assert_eq!(
            UserAgent::new(Svc, None).user_agent,
            HeaderValue::from_static(TONIC_USER_AGENT)
        )
    }

    #[test]
    fn prepends_custom_user_agent_to_default() {
        assert_eq!(
            UserAgent::new(Svc, Some(HeaderValue::from_static("Greeter 1.1"))).user_agent,
            HeaderValue::from_str(&format!("Greeter 1.1 {TONIC_USER_AGENT}")).unwrap()
        )
    }

    struct TestSvc {
        pub expected_user_agent: String,
    }

    impl Service<Request<()>> for TestSvc {
        type Response = ();
        type Error = ();
        type Future = std::future::Ready<Result<(), ()>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: Request<()>) -> Self::Future {
            let user_agent = req.headers().get(USER_AGENT).unwrap().to_str().unwrap();
            assert_eq!(user_agent, self.expected_user_agent);
            std::future::ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn sets_default_user_agent_if_none_present() {
        let expected_user_agent = TONIC_USER_AGENT.to_string();
        let mut ua = UserAgent::new(
            TestSvc {
                expected_user_agent,
            },
            None,
        );
        let _ = ua.call(Request::default()).await;
    }

    #[tokio::test]
    async fn sets_custom_user_agent_if_none_present() {
        let expected_user_agent = format!("Greeter 1.1 {TONIC_USER_AGENT}");
        let mut ua = UserAgent::new(
            TestSvc {
                expected_user_agent,
            },
            Some(HeaderValue::from_static("Greeter 1.1")),
        );
        let _ = ua.call(Request::default()).await;
    }

    #[tokio::test]
    async fn appends_default_user_agent_to_request_user_agent() {
        let mut req = Request::default();
        req.headers_mut()
            .insert(USER_AGENT, HeaderValue::from_static("request-ua/x.y"));

        let expected_user_agent = format!("request-ua/x.y {TONIC_USER_AGENT}");
        let mut ua = UserAgent::new(
            TestSvc {
                expected_user_agent,
            },
            None,
        );
        let _ = ua.call(req).await;
    }

    #[tokio::test]
    async fn appends_custom_user_agent_to_request_user_agent() {
        let mut req = Request::default();
        req.headers_mut()
            .insert(USER_AGENT, HeaderValue::from_static("request-ua/x.y"));

        let expected_user_agent = format!("request-ua/x.y Greeter 1.1 {TONIC_USER_AGENT}");
        let mut ua = UserAgent::new(
            TestSvc {
                expected_user_agent,
            },
            Some(HeaderValue::from_static("Greeter 1.1")),
        );
        let _ = ua.call(req).await;
    }
}
