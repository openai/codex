//! This provides testing functionality for building tests.
//!
//! **Feature:** `test` (*disabled by default*)
//!
//! If the sentry crate has been compiled with the test support feature this
//! module becomes available and provides functionality to capture events
//! in a block.
//!
//! # Example usage
//!
//! ```
//! use sentry::test::with_captured_events;
//! use sentry::{capture_message, Level};
//!
//! let events = with_captured_events(|| {
//!     capture_message("Hello World!", Level::Warning);
//! });
//! assert_eq!(events.len(), 1);
//! assert_eq!(events[0].message.as_ref().unwrap(), "Hello World!");
//! ```

use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;

use crate::protocol::Event;
use crate::types::Dsn;
use crate::{ClientOptions, Envelope, Hub, Transport};

static TEST_DSN: Lazy<Dsn> = Lazy::new(|| "https://public@sentry.invalid/1".parse().unwrap());

/// Collects events instead of sending them.
///
/// # Examples
///
/// ```
/// use sentry::test::TestTransport;
/// use sentry::{ClientOptions, Hub};
/// use std::sync::Arc;
///
/// let transport = TestTransport::new();
/// let options = ClientOptions {
///     dsn: Some("https://public@example.com/1".parse().unwrap()),
///     transport: Some(Arc::new(transport.clone())),
///     ..ClientOptions::default()
/// };
/// Hub::current().bind_client(Some(Arc::new(options.into())));
/// ```
pub struct TestTransport {
    collected: Mutex<Vec<Envelope>>,
}

impl TestTransport {
    /// Creates a new test transport.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Arc<TestTransport> {
        Arc::new(TestTransport {
            collected: Mutex::new(vec![]),
        })
    }

    /// Fetches and clears the contained events.
    pub fn fetch_and_clear_events(&self) -> Vec<Event<'static>> {
        self.fetch_and_clear_envelopes()
            .into_iter()
            .filter_map(|envelope| envelope.event().cloned())
            .collect()
    }

    /// Fetches and clears the contained envelopes.
    pub fn fetch_and_clear_envelopes(&self) -> Vec<Envelope> {
        let mut guard = self.collected.lock().unwrap();
        std::mem::take(&mut *guard)
    }
}

impl Transport for TestTransport {
    fn send_envelope(&self, envelope: Envelope) {
        self.collected.lock().unwrap().push(envelope);
    }
}

/// Runs some code with the default test hub and returns the captured events.
///
/// See [`with_captured_envelopes_options`](fn.with_captured_envelopes_options.html)
pub fn with_captured_events<F: FnOnce()>(f: F) -> Vec<Event<'static>> {
    with_captured_events_options(f, ClientOptions::default())
}

/// Runs some code with the default test hub with the given options and
/// returns the captured events.
///
/// See [`with_captured_envelopes_options`](fn.with_captured_envelopes_options.html)
pub fn with_captured_events_options<F: FnOnce(), O: Into<ClientOptions>>(
    f: F,
    options: O,
) -> Vec<Event<'static>> {
    with_captured_envelopes_options(f, options)
        .into_iter()
        .filter_map(|envelope| envelope.event().cloned())
        .collect()
}

/// Runs some code with the default test hub and returns the captured envelopes.
///
/// See [`with_captured_envelopes_options`](fn.with_captured_envelopes_options.html)
pub fn with_captured_envelopes<F: FnOnce()>(f: F) -> Vec<Envelope> {
    with_captured_envelopes_options(f, ClientOptions::default())
}

/// Runs some code with the default test hub with the given options and
/// returns the captured envelopes.
///
/// If no DSN is set on the options a default test DSN is inserted.  The
/// transport on the options is also overridden with a `TestTransport`.
///
/// This is a shortcut for creating a testable client with the supplied options
/// and `TestTransport`, and bind it to a newly created hub for the duration of
/// the call.
pub fn with_captured_envelopes_options<F: FnOnce(), O: Into<ClientOptions>>(
    f: F,
    options: O,
) -> Vec<Envelope> {
    let transport = TestTransport::new();
    let mut options = options.into();
    options.dsn = Some(options.dsn.unwrap_or_else(|| TEST_DSN.clone()));
    options.transport = Some(Arc::new(transport.clone()));
    Hub::run(
        Arc::new(Hub::new(
            Some(Arc::new(options.into())),
            Arc::new(Default::default()),
        )),
        f,
    );
    transport.fetch_and_clear_envelopes()
}
