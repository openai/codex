use crate::{metadata::GRPC_TIMEOUT_HEADER, TimeoutExpired};
use http::{HeaderMap, HeaderValue, Request};
use pin_project::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};
use tokio::time::Sleep;
use tower_service::Service;

#[derive(Debug, Clone)]
pub(crate) struct GrpcTimeout<S> {
    inner: S,
    server_timeout: Option<Duration>,
}

impl<S> GrpcTimeout<S> {
    pub(crate) fn new(inner: S, server_timeout: Option<Duration>) -> Self {
        Self {
            inner,
            server_timeout,
        }
    }
}

impl<S, ReqBody> Service<Request<ReqBody>> for GrpcTimeout<S>
where
    S: Service<Request<ReqBody>>,
    S::Error: Into<crate::BoxError>,
{
    type Response = S::Response;
    type Error = crate::BoxError;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let client_timeout = try_parse_grpc_timeout(req.headers()).unwrap_or_else(|e| {
            tracing::trace!("Error parsing `grpc-timeout` header {:?}", e);
            None
        });

        // Use the shorter of the two durations, if either are set
        let timeout_duration = match (client_timeout, self.server_timeout) {
            (None, None) => None,
            (Some(dur), None) => Some(dur),
            (None, Some(dur)) => Some(dur),
            (Some(header), Some(server)) => {
                let shorter_duration = std::cmp::min(header, server);
                Some(shorter_duration)
            }
        };

        ResponseFuture {
            inner: self.inner.call(req),
            sleep: timeout_duration.map(tokio::time::sleep),
        }
    }
}

#[pin_project]
pub(crate) struct ResponseFuture<F> {
    #[pin]
    inner: F,
    #[pin]
    sleep: Option<Sleep>,
}

impl<F, Res, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Res, E>>,
    E: Into<crate::BoxError>,
{
    type Output = Result<Res, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if let ready @ Poll::Ready(_) = this.inner.poll(cx) {
            return ready.map_err(Into::into);
        }

        if let Some(sleep) = this.sleep.as_pin_mut() {
            ready!(sleep.poll(cx));
            return Poll::Ready(Err(TimeoutExpired(()).into()));
        }

        Poll::Pending
    }
}

const SECONDS_IN_HOUR: u64 = 60 * 60;
const SECONDS_IN_MINUTE: u64 = 60;

/// Tries to parse the `grpc-timeout` header if it is present. If we fail to parse, returns
/// the value we attempted to parse.
///
/// Follows the [gRPC over HTTP2 spec](https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md).
fn try_parse_grpc_timeout(
    headers: &HeaderMap<HeaderValue>,
) -> Result<Option<Duration>, &HeaderValue> {
    let Some(val) = headers.get(GRPC_TIMEOUT_HEADER) else {
        return Ok(None);
    };

    let (timeout_value, timeout_unit) = val
        .to_str()
        .map_err(|_| val)
        .and_then(|s| if s.is_empty() { Err(val) } else { Ok(s) })?
        // `HeaderValue::to_str` only returns `Ok` if the header contains ASCII so this
        // `split_at` will never panic from trying to split in the middle of a character.
        // See https://docs.rs/http/1/http/header/struct.HeaderValue.html#method.to_str
        //
        // `len - 1` also wont panic since we just checked `s.is_empty`.
        .split_at(val.len() - 1);

    // gRPC spec specifies `TimeoutValue` will be at most 8 digits
    // Caping this at 8 digits also prevents integer overflow from ever occurring
    if timeout_value.len() > 8 {
        return Err(val);
    }

    let timeout_value: u64 = timeout_value.parse().map_err(|_| val)?;

    let duration = match timeout_unit {
        // Hours
        "H" => Duration::from_secs(timeout_value * SECONDS_IN_HOUR),
        // Minutes
        "M" => Duration::from_secs(timeout_value * SECONDS_IN_MINUTE),
        // Seconds
        "S" => Duration::from_secs(timeout_value),
        // Milliseconds
        "m" => Duration::from_millis(timeout_value),
        // Microseconds
        "u" => Duration::from_micros(timeout_value),
        // Nanoseconds
        "n" => Duration::from_nanos(timeout_value),
        _ => return Err(val),
    };

    Ok(Some(duration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;

    // Helper function to reduce the boiler plate of our test cases
    fn setup_map_try_parse(val: Option<&str>) -> Result<Option<Duration>, HeaderValue> {
        let mut hm = HeaderMap::new();
        if let Some(v) = val {
            let hv = HeaderValue::from_str(v).unwrap();
            hm.insert(GRPC_TIMEOUT_HEADER, hv);
        };

        try_parse_grpc_timeout(&hm).map_err(|e| e.clone())
    }

    #[test]
    fn test_hours() {
        let parsed_duration = setup_map_try_parse(Some("3H")).unwrap().unwrap();
        assert_eq!(Duration::from_secs(3 * 60 * 60), parsed_duration);
    }

    #[test]
    fn test_minutes() {
        let parsed_duration = setup_map_try_parse(Some("1M")).unwrap().unwrap();
        assert_eq!(Duration::from_secs(60), parsed_duration);
    }

    #[test]
    fn test_seconds() {
        let parsed_duration = setup_map_try_parse(Some("42S")).unwrap().unwrap();
        assert_eq!(Duration::from_secs(42), parsed_duration);
    }

    #[test]
    fn test_milliseconds() {
        let parsed_duration = setup_map_try_parse(Some("13m")).unwrap().unwrap();
        assert_eq!(Duration::from_millis(13), parsed_duration);
    }

    #[test]
    fn test_microseconds() {
        let parsed_duration = setup_map_try_parse(Some("2u")).unwrap().unwrap();
        assert_eq!(Duration::from_micros(2), parsed_duration);
    }

    #[test]
    fn test_nanoseconds() {
        let parsed_duration = setup_map_try_parse(Some("82n")).unwrap().unwrap();
        assert_eq!(Duration::from_nanos(82), parsed_duration);
    }

    #[test]
    fn test_header_not_present() {
        let parsed_duration = setup_map_try_parse(None).unwrap();
        assert!(parsed_duration.is_none());
    }

    #[test]
    #[should_panic(expected = "82f")]
    fn test_invalid_unit() {
        // "f" is not a valid TimeoutUnit
        setup_map_try_parse(Some("82f")).unwrap().unwrap();
    }

    #[test]
    #[should_panic(expected = "123456789H")]
    fn test_too_many_digits() {
        // gRPC spec states TimeoutValue will be at most 8 digits
        setup_map_try_parse(Some("123456789H")).unwrap().unwrap();
    }

    #[test]
    #[should_panic(expected = "oneH")]
    fn test_invalid_digits() {
        // gRPC spec states TimeoutValue will be at most 8 digits
        setup_map_try_parse(Some("oneH")).unwrap().unwrap();
    }

    #[quickcheck]
    fn fuzz(header_value: HeaderValueGen) -> bool {
        let header_value = header_value.0;

        // this just shouldn't panic
        let _ = setup_map_try_parse(Some(&header_value));

        true
    }

    /// Newtype to implement `Arbitrary` for generating `String`s that are valid `HeaderValue`s.
    #[derive(Clone, Debug)]
    struct HeaderValueGen(String);

    impl Arbitrary for HeaderValueGen {
        fn arbitrary(g: &mut Gen) -> Self {
            let max = g.choose(&(1..70).collect::<Vec<_>>()).copied().unwrap();
            Self(gen_string(g, 0, max))
        }
    }

    // copied from https://github.com/hyperium/http/blob/master/tests/header_map_fuzz.rs
    fn gen_string(g: &mut Gen, min: usize, max: usize) -> String {
        let bytes: Vec<_> = (min..max)
            .map(|_| {
                // Chars to pick from
                g.choose(b"ABCDEFGHIJKLMNOPQRSTUVabcdefghilpqrstuvwxyz----")
                    .copied()
                    .unwrap()
            })
            .collect();

        String::from_utf8(bytes).unwrap()
    }
}
