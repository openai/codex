#![allow(clippy::needless_doctest_main)]
//! `wiremock` provides HTTP mocking to perform black-box testing of Rust applications that
//! interact with third-party APIs.
//!
//! It provides mocking of HTTP responses using request matching and response templating.
//!
//! ## How to install
//!
//! Add `wiremock` to your development dependencies:
//! ```toml
//! [dev-dependencies]
//! # ...
//! wiremock = "0.5"
//! ```
//! If you are using [`cargo-edit`](https://github.com/killercup/cargo-edit), run
//! ```bash
//! cargo add wiremock --dev
//! ```
//!
//! ## Getting started
//!
//! ```rust
//! use wiremock::{MockServer, Mock, ResponseTemplate};
//! use wiremock::matchers::{method, path};
//!
//! #[async_std::main]
//! async fn main() {
//!     // Start a background HTTP server on a random local port
//!     let mock_server = MockServer::start().await;
//!
//!     // Arrange the behaviour of the MockServer adding a Mock:
//!     // when it receives a GET request on '/hello' it will respond with a 200.
//!     Mock::given(method("GET"))
//!         .and(path("/hello"))
//!         .respond_with(ResponseTemplate::new(200))
//!         // Mounting the mock on the mock server - it's now effective!
//!         .mount(&mock_server)
//!         .await;
//!     
//!     // If we probe the MockServer using any HTTP client it behaves as expected.
//!     let status = reqwest::get(format!("{}/hello", &mock_server.uri()))
//!         .await
//!         .unwrap()
//!         .status();
//!     assert_eq!(status, 200);
//!
//!     // If the request doesn't match any `Mock` mounted on our `MockServer` a 404 is returned.
//!     let status = reqwest::get(format!("{}/missing", &mock_server.uri()))
//!         .await
//!         .unwrap()
//!         .status();
//!     assert_eq!(status, 404);
//! }
//! ```
//!
//! ## Matchers
//!
//! `wiremock` provides a set of matching strategies out of the box - check the [`matchers`] module
//! for a complete list.
//!
//! You can define your own matchers using the [`Match`] trait, as well as using `Fn` closures.  
//! Check [`Match`]'s documentation for more details and examples.
//!
//! ## Spying
//!
//! `wiremock` empowers you to set expectations on the number of invocations to your [`Mock`]s -
//! check the [`expect`] method for more details.
//!
//! Expectations can be used to verify that a side-effect has (or has not) taken place!
//!
//! Expectations are automatically verified during the shutdown of each [`MockServer`] instance,
//! at the end of your test. A failed verification will trigger a panic.  
//! By default, no expectations are set on your [`Mock`]s.
//!
//! ## Responses
//!
//! `wiremock` lets you specify pre-determined responses using [`ResponseTemplate`] and
//! [`respond_with`].
//!
//! You also given the option to have [`Mock`]s that return different responses based on the matched
//! [`Request`] using the [`Respond`] trait.  
//! Check [`Respond`]'s documentation for more details and examples.
//!
//! ## Test isolation
//!
//! Each instance of [`MockServer`] is fully isolated: [`start`] takes care of finding a random port
//! available on your local machine which is assigned to the new [`MockServer`].
//!
//! To ensure full isolation and no cross-test interference, [`MockServer`]s shouldn't be
//! shared between tests. Instead, [`MockServer`]s should be created in the test where they are used.
//!
//! When a [`MockServer`] instance goes out of scope (e.g. the test finishes), the corresponding
//! HTTP server running in the background is shut down to free up the port it was using.
//!
//! ## Runtime compatibility
//!
//! `wiremock` can be used (and it is tested to work) with both [`async_std`] and [`tokio`] as
//! futures runtimes.  
//! If you encounter any compatibility bug, please open an issue on our [GitHub repository].
//!
//! ## Efficiency
//!
//! `wiremock` maintains a pool of mock servers in the background to minimise the number of
//! connections and the time spent starting up a new [`MockServer`].  
//! Pooling reduces the likelihood of you having to tune your OS configurations (e.g. ulimit).
//!
//! The pool is designed to be invisible: it makes your life easier and your tests faster. If you
//! end up having to worry about it, it's a bug: open an issue!
//!
//! ## Prior art
//!
//! [`mockito`] and [`httpmock`] provide HTTP mocking for Rust.
//!
//! Check the table below to see how `wiremock` compares to them across the following dimensions:
//! - Test execution strategy (do tests have to be executed sequentially or can they be executed in parallel?);
//! - How many APIs can I mock in a test?
//! - Out-of-the-box request matchers;
//! - Extensible request matching (i.e. you can define your own matchers);
//! - Sync/Async API;
//! - Spying (e.g. verify that a mock has/hasn't been called in a test);
//! - Standalone mode (i.e. can I launch an HTTP mock server outside of a test suite?).
//!
//! |           | Test execution strategy | How many APIs can I mock? | Out-of-the-box request matchers | Extensible request matching | API   | Spying | Standalone mode |
//! |-----------|-------------------------|---------------------------|---------------------------------|----------------------------|-------|----------|-----------------|
//! | mockito   | ❌ Sequential           | ❌ 1                        | ✔                           | ❌                        | Sync  | ✔     | ❌              |
//! | httpmock  | ✔ Parallel              | ✔ Unbounded                | ✔                           | ✔                        | Async/Sync  | ✔     | ✔              |
//! | wiremock  | ✔ Parallel ️             | ✔ Unbounded                | ✔                           | ✔                       | Async | ✔      | ❌              |
//!
//!
//! ## Future evolution
//!
//! More request matchers can be added to those provided out-of-the-box to handle common usecases.
//!
//! ## Related projects
//!
//! * [`stubr`](https://github.com/beltram/stubr) for mounting [`Wiremock`](http://wiremock.org/) json stubs in a [`MockServer`]. Also works as a cli.
//!
//! [`MockServer`]: MockServer
//! [`start`]: MockServer::start
//! [`expect`]: Mock::expect
//! [`respond_with`]: MockBuilder::respond_with
//! [GitHub repository]: https://github.com/LukeMathWalker/wiremock-rs
//! [`mockito`]: https://docs.rs/mockito/
//! [`httpmock`]: https://docs.rs/httpmock/
//! [`async_std`]: https://docs.rs/async-std/
//! [`tokio`]: https://docs.rs/tokio/
pub mod http;
pub mod matchers;
mod mock;
mod mock_server;
mod mock_set;
mod mounted_mock;
mod request;
mod respond;
mod response_template;
mod verification;

pub type ErrorResponse = Box<dyn std::error::Error + Send + Sync + 'static>;

pub use mock::{Match, Mock, MockBuilder, Times};
pub use mock_server::{MockGuard, MockServer, MockServerBuilder};
pub use request::{BodyPrintLimit, Request};
pub use respond::Respond;
pub use response_template::ResponseTemplate;
