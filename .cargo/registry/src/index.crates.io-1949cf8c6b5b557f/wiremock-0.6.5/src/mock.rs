use crate::respond::{Respond, RespondErr};
use crate::{ErrorResponse, MockGuard, MockServer, Request, ResponseTemplate};
use std::fmt::{Debug, Formatter};
use std::ops::{
    Range, RangeBounds, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
};

/// Anything that implements `Match` can be used to constrain when a [`Mock`] is activated.
///
/// `Match` can be used to extend the set of matchers provided out-of-the-box by `wiremock` to
/// cater to your specific testing needs:
/// ```rust
/// use wiremock::{Match, MockServer, Mock, Request, ResponseTemplate};
/// use wiremock::matchers::HeaderExactMatcher;
/// use std::convert::TryInto;
///
/// // Check that a header with the specified name exists and its value has an odd length.
/// pub struct OddHeaderMatcher(http::HeaderName);
///
/// impl Match for OddHeaderMatcher {
///     fn matches(&self, request: &Request) -> bool {
///         match request.headers.get(&self.0) {
///             // We are ignoring multi-valued headers for simplicity
///             Some(value) => value.to_str().unwrap_or_default().len() % 2 == 1,
///             None => false
///         }
///     }
/// }
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(OddHeaderMatcher("custom".try_into().unwrap()))
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     let client = reqwest::Client::new();
///
///     // Even length
///     let status = client
///         .get(&mock_server.uri())
///         .header("custom", "even")
///         .send()
///         .await
///         .unwrap()
///         .status();
///     assert_eq!(status, 404);
///
///     // Odd length
///     let status = client
///         .get(&mock_server.uri())
///         .header("custom", "odd")
///         .send()
///         .await
///         .unwrap()
///         .status();
///     assert_eq!(status, 200);
/// }
/// ```
///
/// Anonymous functions that take a reference to a [`Request`] as input and return a boolean
/// as output automatically implement the `Match` trait.
///
/// The previous example could be rewritten as follows:
/// ```rust
/// use wiremock::{Match, MockServer, Mock, Request, ResponseTemplate};
/// use wiremock::matchers::HeaderExactMatcher;
/// use std::convert::TryInto;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///     
///     let header_name = http::HeaderName::from_static("custom");
///     // Check that a header with the specified name exists and its value has an odd length.
///     let matcher = move |request: &Request| {
///         match request.headers.get(&header_name) {
///             Some(value) => value.to_str().unwrap_or_default().len() % 2 == 1,
///             None => false
///         }
///     };
///
///     Mock::given(matcher)
///         .respond_with(ResponseTemplate::new(200))
///         .mount(&mock_server)
///         .await;
///
///     let client = reqwest::Client::new();
///
///     // Even length
///     let status = client
///         .get(&mock_server.uri())
///         .header("custom", "even")
///         .send()
///         .await
///         .unwrap()
///         .status();
///     assert_eq!(status, 404);
///
///     // Odd length
///     let status = client
///         .get(&mock_server.uri())
///         .header("custom", "odd")
///         .send()
///         .await
///         .unwrap()
///         .status();
///     assert_eq!(status, 200);
/// }
/// ```
pub trait Match: Send + Sync {
    /// Given a reference to a [`Request`], determine if it should match or not given
    /// a specific criterion.
    fn matches(&self, request: &Request) -> bool;
}

/// Wrapper around a `Match` trait object.
///
/// We need the wrapper to provide a (fake) implementation of `Debug`,
/// thus allowing us to pass this struct around as a `bastion` message.
/// This is because Rust's closures do not implement `Debug`.
///
/// We wouldn't need this if `bastion` didn't require `Debug` as a trait bound for its Message trait
/// or if Rust automatically implemented `Debug` for closures.
pub(crate) struct Matcher(Box<dyn Match>);

impl Match for Matcher {
    fn matches(&self, request: &Request) -> bool {
        self.0.matches(request)
    }
}

impl Debug for Matcher {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        // Dummy `Debug` implementation to allow us to pass `Matcher` as a message in `bastion`.
        // It's needed because closures do not implement `Debug` and we really want to enable
        // closures as matchers from an API perspective.
        // Might re-think this in the future.
        Ok(())
    }
}

/// Given a set of matchers, a `Mock` instructs an instance of [`MockServer`] to return a pre-determined response if the matching conditions are satisfied.
///
/// `Mock`s have to be mounted (or registered) with a [`MockServer`] to become effective.
/// You can use:
///
/// - [`MockServer::register`] or [`Mock::mount`] to activate a **global** `Mock`;
/// - [`MockServer::register_as_scoped`] or [`Mock::mount_as_scoped`] to activate a **scoped** `Mock`.
///
/// Check the respective documentations for more details (or look at the following examples!).
///
/// # Example (using [`register`]):
///
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::method;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     let response = ResponseTemplate::new(200);
///
///     let mock = Mock::given(method("GET")).respond_with(response.clone());
///     // Registering the mock with the mock server - it's now effective!
///     mock_server.register(mock).await;
///
///     // We won't register this mock instead.
///     let unregistered_mock = Mock::given(method("POST")).respond_with(response);
///     
///     // Act
///     let status = reqwest::get(&mock_server.uri())
///         .await
///         .unwrap()
///         .status();
///     assert_eq!(status, 200);
///
///     // This would have matched `unregistered_mock`, but we haven't registered it!
///     // Hence it returns a 404, the default response when no mocks matched on the mock server.
///     let client = reqwest::Client::new();
///     let status = client.post(&mock_server.uri())
///         .send()
///         .await
///         .unwrap()
///         .status();
///     assert_eq!(status, 404);
/// }
/// ```
///
/// # Example (using [`mount`]):
///
/// If you prefer a fluent style, you can use the [`mount`] method on the `Mock` itself
/// instead of [`register`].
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::method;
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///
///     Mock::given(method("GET"))
///         .respond_with(ResponseTemplate::new(200))
///         .up_to_n_times(1)
///         // Mounting the mock on the mock server - it's now effective!
///         .mount(&mock_server)
///         .await;
///     
///     // Act
///     let status = reqwest::get(&mock_server.uri())
///         .await
///         .unwrap()
///         .status();
///     assert_eq!(status, 200);
/// }
/// ```
///
/// # Example (using [`mount_as_scoped`]):
///
/// Sometimes you will need a `Mock` to be active within the scope of a function, but not any longer.
/// You can use [`Mock::mount_as_scoped`] to precisely control how long a `Mock` stays active.
///
/// ```rust
/// use wiremock::{MockServer, Mock, ResponseTemplate};
/// use wiremock::matchers::method;
///
/// async fn my_test_helper(mock_server: &MockServer) {
///     let mock_guard = Mock::given(method("GET"))
///         .respond_with(ResponseTemplate::new(200))
///         .expect(1)
///         .named("my_test_helper GET /")
///         .mount_as_scoped(mock_server)
///         .await;
///
///     reqwest::get(&mock_server.uri())
///         .await
///         .unwrap();
///
///     // `mock_guard` is dropped, expectations are verified!
/// }
///
/// #[async_std::main]
/// async fn main() {
///     // Arrange
///     let mock_server = MockServer::start().await;
///     my_test_helper(&mock_server).await;
///
///     // Act
///
///     // This would have returned 200 if the `Mock` in
///     // `my_test_helper` had not been scoped.
///     let status = reqwest::get(&mock_server.uri())
///         .await
///         .unwrap()
///         .status();
///     assert_eq!(status, 404);
/// }
/// ```
///
/// [`register`]: MockServer::register
/// [`mount`]: Mock::mount
/// [`mount_as_scoped`]: Mock::mount_as_scoped
#[must_use = "`Mock`s have to be mounted or registered with a `MockServer` to become effective"]
pub struct Mock {
    pub(crate) matchers: Vec<Matcher>,
    pub(crate) response: Result<Box<dyn Respond>, Box<dyn RespondErr>>,
    /// Maximum number of times (inclusive) we should return a response from this Mock on
    /// matching requests.
    /// If `None`, there is no cap and we will respond to all incoming matching requests.
    /// If `Some(max_n_matches)`, when `max_n_matches` matching incoming requests have been processed,
    /// [`crate::mounted_mock::MountedMock::matches`] should start returning `false`, regardless of the incoming request.
    pub(crate) max_n_matches: Option<u64>,
    /// Allows prioritizing a Mock over another one.
    /// `1` is the highest priority, `255` the lowest, default to `5`.
    /// When priority is the same, it fallbacks to insertion order.
    pub(crate) priority: u8,
    /// The friendly mock name specified by the user.
    /// Used in diagnostics and error messages if the mock expectations are not satisfied.
    pub(crate) name: Option<String>,
    /// The expectation is satisfied if the number of incoming requests falls within `expectation_range`.
    pub(crate) expectation_range: Times,
}

/// A fluent builder to construct a [`Mock`] instance given matchers and a [`ResponseTemplate`].
#[derive(Debug)]
pub struct MockBuilder {
    pub(crate) matchers: Vec<Matcher>,
}

impl Mock {
    /// Start building a [`Mock`] specifying the first matcher.
    ///
    /// It returns an instance of [`MockBuilder`].
    pub fn given<M: 'static + Match>(matcher: M) -> MockBuilder {
        MockBuilder {
            matchers: vec![Matcher(Box::new(matcher))],
        }
    }

    /// Specify an upper limit to the number of times you would like this [`Mock`] to respond to
    /// incoming requests that satisfy the conditions imposed by your [`matchers`].
    ///
    /// ### Example:
    ///
    /// ```rust
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///
    ///     Mock::given(method("GET"))
    ///         .respond_with(ResponseTemplate::new(200))
    ///         // Default behaviour will have this Mock responding to any incoming request
    ///         // that satisfied our matcher (e.g. being a GET request).
    ///         // We can opt out of the default behaviour by setting a cap on the number of
    ///         // matching requests this Mock should respond to.
    ///         //
    ///         // In this case, once one matching request has been received, the mock will stop
    ///         // matching additional requests and you will receive a 404 if no other mock
    ///         // matches on those requests.
    ///         .up_to_n_times(1)
    ///         .mount(&mock_server)
    ///         .await;
    ///     
    ///     // Act
    ///
    ///     // The first request matches, as expected.
    ///     let status = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap()
    ///         .status();
    ///     assert_eq!(status, 200);
    ///
    ///     // The second request does NOT match given our `up_to_n_times(1)` setting.
    ///     let status = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap()
    ///         .status();
    ///     assert_eq!(status, 404);
    /// }
    /// ```
    ///
    /// [`matchers`]: crate::matchers
    pub fn up_to_n_times(mut self, n: u64) -> Mock {
        assert!(n > 0, "n must be strictly greater than 0!");
        self.max_n_matches = Some(n);
        self
    }

    /// Specify a priority for this [`Mock`].
    /// Use this when you mount many [`Mock`] in a [`MockServer`]
    /// and those mocks have interlaced request matching conditions
    /// e.g. `mock A` accepts path `/abcd` and `mock B` a path regex `[a-z]{4}`
    /// It is recommended to set the highest priority (1) for mocks with exact conditions (`mock A` in this case)
    /// `1` is the highest priority, `255` the lowest, default to `5`
    /// If two mocks have the same priority, priority is defined by insertion order (first one mounted has precedence over the others).
    ///
    /// ### Example:
    ///
    /// ```rust
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::{method, path, path_regex};
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///
    ///     Mock::given(method("GET"))
    ///         .and(path("abcd"))
    ///         .respond_with(ResponseTemplate::new(200))
    ///         .with_priority(1) // highest priority
    ///         .mount(&mock_server)
    ///         .await;
    ///
    ///     Mock::given(method("GET"))
    ///         .and(path_regex("[a-z]{4}"))
    ///         .respond_with(ResponseTemplate::new(201))
    ///         .with_priority(2)
    ///         .mount(&mock_server)
    ///         .await;
    ///
    ///     // Act
    ///
    ///     // The request with highest priority, as expected.
    ///     let status = reqwest::get(&format!("{}/abcd", mock_server.uri()))
    ///         .await
    ///         .unwrap()
    ///         .status();
    ///     assert_eq!(status, 200);
    /// }
    /// ```
    ///
    /// [`matchers`]: crate::matchers
    pub fn with_priority(mut self, p: u8) -> Mock {
        assert!(p > 0, "priority must be strictly greater than 0!");
        self.priority = p;
        self
    }

    /// Set an expectation on the number of times this [`Mock`] should match in the current
    /// test case.
    /// Expectations are verified when the [`MockServer`] is shutting down: if the expectation
    /// is not satisfied, the [`MockServer`] will panic and the `error_message` is shown.
    ///
    /// By default, no expectation is set for [`Mock`]s.
    ///
    /// ### When is this useful?
    ///
    /// `expect` can turn out handy when you'd like to verify that a certain side-effect has
    /// (or has not!) taken place.
    ///
    /// For example:
    /// - check that a 3rd party notification API (e.g. email service) is called when an event
    ///   in your application is supposed to trigger a notification;
    /// - check that a 3rd party API is NOT called when the response of a call is expected
    ///   to be retrieved from a cache (`.expect(0)`).
    ///
    /// This technique is also called [spying](https://martinfowler.com/bliki/TestDouble.html).
    ///
    /// ### Example:
    ///
    /// ```rust
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///
    ///     Mock::given(method("GET"))
    ///         .respond_with(ResponseTemplate::new(200))
    ///         .up_to_n_times(2)
    ///         // We expect the mock to be called at least once.
    ///         // If that does not happen, the `MockServer` will panic on shutdown,
    ///         // causing the whole test to fail.
    ///         .expect(1..)
    ///         // We assign a name to the mock - it will be shown in error messages
    ///         // if our expectation is not verified!
    ///         .named("Root GET")
    ///         .mount(&mock_server)
    ///         .await;
    ///     
    ///     // Act
    ///     let status = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap()
    ///         .status();
    ///     assert_eq!(status, 200);
    ///
    ///     // Assert
    ///     // We made at least one matching request, the expectation is satisfied.
    ///     // The `MockServer` will shutdown peacefully, without panicking.
    /// }
    /// ```
    pub fn expect<T: Into<Times>>(mut self, r: T) -> Self {
        let range = r.into();
        self.expectation_range = range;

        self
    }

    /// Assign a name to your mock.  
    ///
    /// The mock name will be used in error messages (e.g. if the mock expectation
    /// is not satisfied) and debug logs to help you identify what failed.
    ///
    /// ### Example:
    ///
    /// ```should_panic
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///
    ///     // We have two mocks in the same test - how do we find out
    ///     // which one failed when the test panics?
    ///     // Assigning a name to each mock with `named` gives us better error
    ///     // messages and makes it much easier to debug why a test is failing!
    ///     Mock::given(method("GET"))
    ///         .respond_with(ResponseTemplate::new(200))
    ///         .up_to_n_times(2)
    ///         .expect(1..)
    ///         // We assign a name to the mock - it will be shown in error messages
    ///         // if our expectation is not verified!
    ///         .named("Root GET")
    ///         .mount(&mock_server)
    ///         .await;
    ///
    ///     Mock::given(method("POST"))
    ///         .respond_with(ResponseTemplate::new(200))
    ///         .up_to_n_times(2)
    ///         .expect(1..)
    ///         // We assign a name to the mock - it will be shown in error messages
    ///         // if our expectation is not verified!
    ///         .named("Root POST")
    ///         .mount(&mock_server)
    ///         .await;
    ///     
    ///     // Act
    ///     let status = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap()
    ///         .status();
    ///     assert_eq!(status, 200);
    ///
    ///     // Assert
    ///     // We did not make a POST request, therefore the expectation on `Root POST`
    ///     // is not satisfied and the test will panic.
    /// }
    /// ```
    pub fn named<T: Into<String>>(mut self, mock_name: T) -> Self {
        self.name = Some(mock_name.into());
        self
    }

    /// Mount a [`Mock`] on an instance of [`MockServer`].
    /// The [`Mock`] will remain active until [`MockServer`] is shut down. If you want to control or limit how
    /// long your [`Mock`] stays active, check out [`Mock::mount_as_scoped`].
    ///
    /// Be careful! [`Mock`]s are not effective until they are [`mount`]ed or [`register`]ed on a [`MockServer`].
    /// [`mount`] is an asynchronous method, make sure to `.await` it!
    ///
    /// [`register`]: MockServer::register
    /// [`mount`]: Mock::mount
    pub async fn mount(self, server: &MockServer) {
        server.register(self).await;
    }

    /// Mount a [`Mock`] as **scoped**  on an instance of [`MockServer`].
    ///
    /// When using [`mount`], your [`Mock`]s will be active until the [`MockServer`] is shut down.  
    /// When using `mount_as_scoped`, your [`Mock`]s will be active as long as the returned [`MockGuard`] is not dropped.
    /// When the returned [`MockGuard`] is dropped, [`MockServer`] will verify that the expectations set on the scoped [`Mock`] were
    /// verified - if not, it will panic.
    ///
    /// `mount_as_scoped` is the ideal solution when you need a [`Mock`] within a test helper
    /// but you do not want it to linger around after the end of the function execution.
    ///
    /// # Limitations
    ///
    /// When expectations of a scoped [`Mock`] are not verified, it will trigger a panic - just like a normal [`Mock`].
    /// Due to [limitations](https://internals.rust-lang.org/t/should-drop-glue-use-track-caller/13682) in Rust's [`Drop`] trait,
    /// the panic message will not include the filename and the line location
    /// where the corresponding [`MockGuard`] was dropped - it will point into `wiremock`'s source code.  
    ///
    /// This can be an issue when you are using more than one scoped [`Mock`] in a single test - which of them panicked?  
    /// To improve your debugging experience it is strongly recommended to use [`Mock::named`] to assign a unique
    /// identifier to your scoped [`Mock`]s, which will in turn be referenced in the panic message if their expectations are
    /// not met.
    ///
    /// # Example:
    ///
    /// - The behaviour of the scoped mock is invisible outside of `my_test_helper`.
    ///
    /// ```rust
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// async fn my_test_helper(mock_server: &MockServer) {
    ///     let mock_guard = Mock::given(method("GET"))
    ///         .respond_with(ResponseTemplate::new(200))
    ///         .expect(1)
    ///         .named("my_test_helper GET /")
    ///         .mount_as_scoped(mock_server)
    ///         .await;
    ///
    ///     reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap();
    ///
    ///     // `mock_guard` is dropped, expectations are verified!
    /// }
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///     my_test_helper(&mock_server).await;
    ///
    ///     // Act
    ///
    ///     // This would have returned 200 if the `Mock` in
    ///     // `my_test_helper` had not been scoped.
    ///     let status = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap()
    ///         .status();
    ///     assert_eq!(status, 404);
    /// }
    /// ```
    ///
    /// - The expectations for the scoped mock are not verified, it panics at the end of `my_test_helper`.
    ///
    /// ```rust,should_panic
    /// use wiremock::{MockServer, Mock, ResponseTemplate};
    /// use wiremock::matchers::method;
    ///
    /// async fn my_test_helper(mock_server: &MockServer) {
    ///     let mock_guard = Mock::given(method("GET"))
    ///         .respond_with(ResponseTemplate::new(200))
    ///         .expect(1)
    ///         .named("my_test_helper GET /")
    ///         .mount_as_scoped(mock_server)
    ///         .await;
    ///     // `mock_guard` is dropped, expectations are NOT verified!
    ///     // Panic!
    /// }
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     // Arrange
    ///     let mock_server = MockServer::start().await;
    ///     my_test_helper(&mock_server).await;
    ///
    ///     // Act
    ///     let status = reqwest::get(&mock_server.uri())
    ///         .await
    ///         .unwrap()
    ///         .status();
    ///     assert_eq!(status, 404);
    /// }
    /// ```
    ///
    /// [`mount`]: Mock::mount
    pub async fn mount_as_scoped(self, server: &MockServer) -> MockGuard {
        server.register_as_scoped(self).await
    }

    /// Given a [`Request`] build an instance a [`ResponseTemplate`] using
    /// the responder associated with the `Mock`.
    pub(crate) fn response_template(
        &self,
        request: &Request,
    ) -> Result<ResponseTemplate, ErrorResponse> {
        match &self.response {
            Ok(responder) => Ok(responder.respond(request)),
            Err(responder_err) => Err(responder_err.respond_err(request)),
        }
    }
}

impl MockBuilder {
    /// Add another request matcher to the mock you are building.
    ///
    /// **All** specified [`matchers`] must match for the overall [`Mock`] to match an incoming request.
    ///
    /// [`matchers`]: crate::matchers
    pub fn and<M: Match + 'static>(mut self, matcher: M) -> Self {
        self.matchers.push(Matcher(Box::new(matcher)));
        self
    }

    /// Establish what [`ResponseTemplate`] should be used to generate a response when an incoming
    /// request matches.
    ///
    /// `respond_with` finalises the `MockBuilder` and returns you a [`Mock`] instance, ready to
    /// be [`register`]ed or [`mount`]ed on a [`MockServer`]!
    ///
    /// [`register`]: MockServer::register
    /// [`mount`]: Mock::mount
    pub fn respond_with<R: Respond + 'static>(self, responder: R) -> Mock {
        Mock {
            matchers: self.matchers,
            response: Ok(Box::new(responder)),
            max_n_matches: None,
            priority: 5,
            name: None,
            expectation_range: Times(TimesEnum::Unbounded(RangeFull)),
        }
    }

    /// Instead of response with an HTTP reply, return a Rust error.
    ///
    /// This can simulate lower level errors, e.g., a [`ConnectionReset`] IO Error.
    ///
    /// [`ConnectionReset`]: std::io::ErrorKind::ConnectionReset
    pub fn respond_with_err<R: RespondErr + 'static>(self, responder_err: R) -> Mock {
        Mock {
            matchers: self.matchers,
            response: Err(Box::new(responder_err)),
            max_n_matches: None,
            priority: 5,
            name: None,
            expectation_range: Times(TimesEnum::Unbounded(RangeFull)),
        }
    }
}

/// Specify how many times we expect a [`Mock`] to match via [`expect`].
/// It is used to set expectations on the usage of a [`Mock`] in a test case.
///
/// You can either specify an exact value, e.g.
/// ```rust
/// use wiremock::Times;
///
/// let times: Times = 10.into();
/// ```
/// or a range
/// ```rust
/// use wiremock::Times;
///
/// // Between 10 and 15 (not included) times
/// let times: Times = (10..15).into();
/// // Between 10 and 15 (included) times
/// let times: Times = (10..=15).into();
/// // At least 10 times
/// let times: Times = (10..).into();
/// // Strictly less than 15 times
/// let times: Times = (..15).into();
/// // Strictly less than 16 times
/// let times: Times = (..=15).into();
/// ```
///
/// [`expect`]: Mock::expect
#[derive(Clone, Debug)]
pub struct Times(TimesEnum);

impl Times {
    pub(crate) fn contains(&self, n_calls: u64) -> bool {
        match &self.0 {
            TimesEnum::Exact(e) => e == &n_calls,
            TimesEnum::Unbounded(r) => r.contains(&n_calls),
            TimesEnum::Range(r) => r.contains(&n_calls),
            TimesEnum::RangeFrom(r) => r.contains(&n_calls),
            TimesEnum::RangeTo(r) => r.contains(&n_calls),
            TimesEnum::RangeToInclusive(r) => r.contains(&n_calls),
            TimesEnum::RangeInclusive(r) => r.contains(&n_calls),
        }
    }
}

impl std::fmt::Display for Times {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            TimesEnum::Exact(e) => write!(f, "== {}", e),
            TimesEnum::Unbounded(_) => write!(f, "0 <= x"),
            TimesEnum::Range(r) => write!(f, "{} <= x < {}", r.start, r.end),
            TimesEnum::RangeFrom(r) => write!(f, "{} <= x", r.start),
            TimesEnum::RangeTo(r) => write!(f, "0 <= x < {}", r.end),
            TimesEnum::RangeToInclusive(r) => write!(f, "0 <= x <= {}", r.end),
            TimesEnum::RangeInclusive(r) => write!(f, "{} <= x <= {}", r.start(), r.end()),
        }
    }
}

// Implementation notes: this has gone through a couple of iterations before landing to
// what you see now.
//
// The original draft had Times itself as an enum with two variants (Exact and Range), with
// the Range variant generic over `R: RangeBounds<u64>`.
//
// We switched to a generic struct wrapper around a private `R: RangeBounds<u64>` when we realised
// that you would have had to specify a range type when creating the Exact variant
// (e.g. as you do for `Option` when creating a `None` variant).
//
// We achieved the same functionality with a struct wrapper, but exact values had to converted
// to ranges with a single element (e.g. 15 -> 15..16).
// Not the most expressive representation, but we would have lived with it.
//
// We changed once again when we started to update our `MockActor`: we are storing all `Mock`s
// in a vector. Being generic over `R`, the range type leaked into the overall `Mock` (and `MountedMock`)
// type, thus making those generic as well over `R`.
// To store them in a vector all mocks would have had to use the same range internally, which is
// obviously an unreasonable restrictions.
// At the same time, we can't have a Box<dyn RangeBounds<u64>> because `contains` is a generic
// method hence the requirements for object safety are not satisfied.
//
// Thus we ended up creating this master enum that wraps all range variants with the addition
// of the Exact variant.
// If you can do better, please submit a PR.
// We keep them enum private to the crate to allow for future refactoring.
#[derive(Clone, Debug)]
pub(crate) enum TimesEnum {
    Exact(u64),
    Unbounded(RangeFull),
    Range(Range<u64>),
    RangeFrom(RangeFrom<u64>),
    RangeTo(RangeTo<u64>),
    RangeToInclusive(RangeToInclusive<u64>),
    RangeInclusive(RangeInclusive<u64>),
}

impl From<u64> for Times {
    fn from(x: u64) -> Self {
        Times(TimesEnum::Exact(x))
    }
}

impl From<RangeFull> for Times {
    fn from(x: RangeFull) -> Self {
        Times(TimesEnum::Unbounded(x))
    }
}

// A quick macro to help easing the implementation pain.
macro_rules! impl_from_for_range {
    ($type_name:ident) => {
        impl From<$type_name<u64>> for Times {
            fn from(r: $type_name<u64>) -> Self {
                Times(TimesEnum::$type_name(r))
            }
        }
    };
}

impl_from_for_range!(Range);
impl_from_for_range!(RangeTo);
impl_from_for_range!(RangeFrom);
impl_from_for_range!(RangeInclusive);
impl_from_for_range!(RangeToInclusive);
