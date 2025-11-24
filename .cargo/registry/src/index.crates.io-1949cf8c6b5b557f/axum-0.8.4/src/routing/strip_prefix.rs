use http::{Request, Uri};
use std::{
    sync::Arc,
    task::{Context, Poll},
};
use tower::Layer;
use tower_layer::layer_fn;
use tower_service::Service;

#[derive(Clone)]
pub(super) struct StripPrefix<S> {
    inner: S,
    prefix: Arc<str>,
}

impl<S> StripPrefix<S> {
    pub(super) fn layer(prefix: &str) -> impl Layer<S, Service = Self> + Clone {
        let prefix = Arc::from(prefix);
        layer_fn(move |inner| Self {
            inner,
            prefix: Arc::clone(&prefix),
        })
    }
}

impl<S, B> Service<Request<B>> for StripPrefix<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if let Some(new_uri) = strip_prefix(req.uri(), &self.prefix) {
            *req.uri_mut() = new_uri;
        }
        self.inner.call(req)
    }
}

fn strip_prefix(uri: &Uri, prefix: &str) -> Option<Uri> {
    let path_and_query = uri.path_and_query()?;

    // Check whether the prefix matches the path and if so how long the matching prefix is.
    //
    // For example:
    //
    // prefix = /api
    // path   = /api/users
    //          ^^^^ this much is matched and the length is 4. Thus if we chop off the first 4
    //          characters we get the remainder
    //
    // prefix = /api/{version}
    // path   = /api/v0/users
    //          ^^^^^^^ this much is matched and the length is 7.
    let mut matching_prefix_length = Some(0);
    for item in zip_longest(segments(path_and_query.path()), segments(prefix)) {
        // count the `/`
        *matching_prefix_length.as_mut().unwrap() += 1;

        match item {
            Item::Both(path_segment, prefix_segment) => {
                if is_capture(prefix_segment) || path_segment == prefix_segment {
                    // the prefix segment is either a param, which matches anything, or
                    // it actually matches the path segment
                    *matching_prefix_length.as_mut().unwrap() += path_segment.len();
                } else if prefix_segment.is_empty() {
                    // the prefix ended in a `/` so we got a match.
                    //
                    // For example:
                    //
                    // prefix = /foo/
                    // path   = /foo/bar
                    //
                    // The prefix matches and the new path should be `/bar`
                    break;
                } else {
                    // the prefix segment didn't match so there is no match
                    matching_prefix_length = None;
                    break;
                }
            }
            // the path had more segments than the prefix but we got a match.
            //
            // For example:
            //
            // prefix = /foo
            // path   = /foo/bar
            Item::First(_) => {
                break;
            }
            // the prefix had more segments than the path so there is no match
            Item::Second(_) => {
                matching_prefix_length = None;
                break;
            }
        }
    }

    // if the prefix matches it will always do so up until a `/`, it cannot match only
    // part of a segment. Therefore this will always be at a char boundary and `split_at` won't
    // panic
    let after_prefix = uri.path().split_at(matching_prefix_length?).1;

    let new_path_and_query = match (after_prefix.starts_with('/'), path_and_query.query()) {
        (true, None) => after_prefix.parse().unwrap(),
        (true, Some(query)) => format!("{after_prefix}?{query}").parse().unwrap(),
        (false, None) => format!("/{after_prefix}").parse().unwrap(),
        (false, Some(query)) => format!("/{after_prefix}?{query}").parse().unwrap(),
    };

    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(new_path_and_query);

    Some(Uri::from_parts(parts).unwrap())
}

fn segments(s: &str) -> impl Iterator<Item = &str> {
    assert!(
        s.starts_with('/'),
        "path didn't start with '/'. axum should have caught this higher up."
    );

    s.split('/')
        // skip one because paths always start with `/` so `/a/b` would become ["", "a", "b"]
        // otherwise
        .skip(1)
}

fn zip_longest<I, I2>(a: I, b: I2) -> impl Iterator<Item = Item<I::Item>>
where
    I: Iterator,
    I2: Iterator<Item = I::Item>,
{
    let a = a.map(Some).chain(std::iter::repeat_with(|| None));
    let b = b.map(Some).chain(std::iter::repeat_with(|| None));
    a.zip(b).map_while(|(a, b)| match (a, b) {
        (Some(a), Some(b)) => Some(Item::Both(a, b)),
        (Some(a), None) => Some(Item::First(a)),
        (None, Some(b)) => Some(Item::Second(b)),
        (None, None) => None,
    })
}

fn is_capture(segment: &str) -> bool {
    segment.starts_with('{')
        && segment.ends_with('}')
        && !segment.starts_with("{{")
        && !segment.ends_with("}}")
        && !segment.starts_with("{*")
}

#[derive(Debug)]
enum Item<T> {
    Both(T, T),
    First(T),
    Second(T),
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use quickcheck::Arbitrary;
    use quickcheck_macros::quickcheck;

    macro_rules! test {
        (
            $name:ident,
            uri = $uri:literal,
            prefix = $prefix:literal,
            expected = $expected:expr,
        ) => {
            #[test]
            fn $name() {
                let uri = $uri.parse().unwrap();
                let new_uri = strip_prefix(&uri, $prefix).map(|uri| uri.to_string());
                assert_eq!(new_uri.as_deref(), $expected);
            }
        };
    }

    test!(empty, uri = "/", prefix = "/", expected = Some("/"),);

    test!(
        single_segment,
        uri = "/a",
        prefix = "/a",
        expected = Some("/"),
    );

    test!(
        single_segment_root_uri,
        uri = "/",
        prefix = "/a",
        expected = None,
    );

    // the prefix is empty, so removing it should have no effect
    test!(
        single_segment_root_prefix,
        uri = "/a",
        prefix = "/",
        expected = Some("/a"),
    );

    test!(
        single_segment_no_match,
        uri = "/a",
        prefix = "/b",
        expected = None,
    );

    test!(
        single_segment_trailing_slash,
        uri = "/a/",
        prefix = "/a/",
        expected = Some("/"),
    );

    test!(
        single_segment_trailing_slash_2,
        uri = "/a",
        prefix = "/a/",
        expected = None,
    );

    test!(
        single_segment_trailing_slash_3,
        uri = "/a/",
        prefix = "/a",
        expected = Some("/"),
    );

    test!(
        multi_segment,
        uri = "/a/b",
        prefix = "/a",
        expected = Some("/b"),
    );

    test!(
        multi_segment_2,
        uri = "/b/a",
        prefix = "/a",
        expected = None,
    );

    test!(
        multi_segment_3,
        uri = "/a",
        prefix = "/a/b",
        expected = None,
    );

    test!(
        multi_segment_4,
        uri = "/a/b",
        prefix = "/b",
        expected = None,
    );

    test!(
        multi_segment_trailing_slash,
        uri = "/a/b/",
        prefix = "/a/b/",
        expected = Some("/"),
    );

    test!(
        multi_segment_trailing_slash_2,
        uri = "/a/b",
        prefix = "/a/b/",
        expected = None,
    );

    test!(
        multi_segment_trailing_slash_3,
        uri = "/a/b/",
        prefix = "/a/b",
        expected = Some("/"),
    );

    test!(
        param_0,
        uri = "/",
        prefix = "/{param}",
        expected = Some("/"),
    );

    test!(
        param_1,
        uri = "/a",
        prefix = "/{param}",
        expected = Some("/"),
    );

    test!(
        param_2,
        uri = "/a/b",
        prefix = "/{param}",
        expected = Some("/b"),
    );

    test!(
        param_3,
        uri = "/b/a",
        prefix = "/{param}",
        expected = Some("/a"),
    );

    test!(
        param_4,
        uri = "/a/b",
        prefix = "/a/{param}",
        expected = Some("/"),
    );

    test!(
        param_5,
        uri = "/b/a",
        prefix = "/a/{param}",
        expected = None,
    );

    test!(
        param_6,
        uri = "/a/b",
        prefix = "/{param}/a",
        expected = None,
    );

    test!(
        param_7,
        uri = "/b/a",
        prefix = "/{param}/a",
        expected = Some("/"),
    );

    test!(
        param_8,
        uri = "/a/b/c",
        prefix = "/a/{param}/c",
        expected = Some("/"),
    );

    test!(
        param_9,
        uri = "/c/b/a",
        prefix = "/a/{param}/c",
        expected = None,
    );

    test!(
        param_10,
        uri = "/a/",
        prefix = "/{param}",
        expected = Some("/"),
    );

    test!(param_11, uri = "/a", prefix = "/{param}/", expected = None,);

    test!(
        param_12,
        uri = "/a/",
        prefix = "/{param}/",
        expected = Some("/"),
    );

    test!(
        param_13,
        uri = "/a/a",
        prefix = "/a/",
        expected = Some("/a"),
    );

    #[quickcheck]
    fn does_not_panic(uri_and_prefix: UriAndPrefix) -> bool {
        let UriAndPrefix { uri, prefix } = uri_and_prefix;
        strip_prefix(&uri, &prefix);
        true
    }

    #[derive(Clone, Debug)]
    struct UriAndPrefix {
        uri: Uri,
        prefix: String,
    }

    impl Arbitrary for UriAndPrefix {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut uri = String::new();
            let mut prefix = String::new();

            let size = u8_between(1, 20, g);

            for _ in 0..size {
                let segment = ascii_alphanumeric(g);

                uri.push('/');
                uri.push_str(&segment);

                prefix.push('/');

                let make_matching_segment = bool::arbitrary(g);
                let make_capture = bool::arbitrary(g);

                match (make_matching_segment, make_capture) {
                    (_, true) => {
                        prefix.push_str(":a");
                    }
                    (true, false) => {
                        prefix.push_str(&segment);
                    }
                    (false, false) => {
                        prefix.push_str(&ascii_alphanumeric(g));
                    }
                }
            }

            if bool::arbitrary(g) {
                uri.push('/');
            }

            if bool::arbitrary(g) {
                prefix.push('/');
            }

            Self {
                uri: uri.parse().unwrap(),
                prefix,
            }
        }
    }

    fn ascii_alphanumeric(g: &mut quickcheck::Gen) -> String {
        #[derive(Clone)]
        struct AsciiAlphanumeric(String);

        impl Arbitrary for AsciiAlphanumeric {
            fn arbitrary(g: &mut quickcheck::Gen) -> Self {
                let mut out = String::new();

                let size = u8_between(1, 20, g) as usize;

                while out.len() < size {
                    let c = char::arbitrary(g);
                    if c.is_ascii_alphanumeric() {
                        out.push(c);
                    }
                }
                Self(out)
            }
        }

        let out = AsciiAlphanumeric::arbitrary(g).0;
        assert!(!out.is_empty());
        out
    }

    fn u8_between(lower: u8, upper: u8, g: &mut quickcheck::Gen) -> u8 {
        loop {
            let size = u8::arbitrary(g);
            if size > lower && size <= upper {
                break size;
            }
        }
    }
}
