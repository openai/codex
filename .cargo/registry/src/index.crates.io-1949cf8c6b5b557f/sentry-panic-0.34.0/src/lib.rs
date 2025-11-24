//! The Sentry Panic handler integration.
//!
//! The `PanicIntegration`, which is enabled by default in `sentry`, installs a
//! panic handler that will automatically dispatch all errors to Sentry that
//! are caused by a panic.
//! Additionally, panics are forwarded to the previously registered panic hook.
//!
//! # Configuration
//!
//! The panic integration can be configured with an additional extractor, which
//! might optionally create a sentry `Event` out of a `PanicInfo`.
//!
//! ```
//! let integration = sentry_panic::PanicIntegration::default().add_extractor(|info| None);
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]
#![deny(unsafe_code)]

use std::panic::{self, PanicInfo};
use std::sync::Once;

use sentry_backtrace::current_stacktrace;
use sentry_core::protocol::{Event, Exception, Level, Mechanism};
use sentry_core::{ClientOptions, Integration};

/// A panic handler that sends to Sentry.
///
/// This panic handler reports panics to Sentry. It also attempts to prevent
/// double faults in some cases where it's known to be unsafe to invoke the
/// Sentry panic handler.
pub fn panic_handler(info: &PanicInfo<'_>) {
    sentry_core::with_integration(|integration: &PanicIntegration, hub| {
        hub.capture_event(integration.event_from_panic_info(info));
        if let Some(client) = hub.client() {
            client.flush(None);
        }
    });
}

type PanicExtractor = dyn Fn(&PanicInfo<'_>) -> Option<Event<'static>> + Send + Sync;

/// The Sentry Panic handler Integration.
#[derive(Default)]
pub struct PanicIntegration {
    extractors: Vec<Box<PanicExtractor>>,
}

impl std::fmt::Debug for PanicIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanicIntegration")
            .field("extractors", &self.extractors.len())
            .finish()
    }
}

static INIT: Once = Once::new();

impl Integration for PanicIntegration {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn setup(&self, _cfg: &mut ClientOptions) {
        INIT.call_once(|| {
            let next = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                panic_handler(info);
                next(info);
            }));
        });
    }
}

/// Extract the message of a panic.
pub fn message_from_panic_info<'a>(info: &'a PanicInfo<'_>) -> &'a str {
    match info.payload().downcast_ref::<&'static str>() {
        Some(s) => s,
        None => match info.payload().downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        },
    }
}

impl PanicIntegration {
    /// Creates a new Panic Integration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new extractor.
    #[must_use]
    pub fn add_extractor<F>(mut self, f: F) -> Self
    where
        F: Fn(&PanicInfo<'_>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        self.extractors.push(Box::new(f));
        self
    }

    /// Creates an event from the given panic info.
    ///
    /// The stacktrace is calculated from the current frame.
    pub fn event_from_panic_info(&self, info: &PanicInfo<'_>) -> Event<'static> {
        for extractor in &self.extractors {
            if let Some(event) = extractor(info) {
                return event;
            }
        }

        // TODO: We would ideally want to downcast to `std::error:Error` here
        // and use `event_from_error`, but that way we won‘t get meaningful
        // backtraces yet.

        let msg = message_from_panic_info(info);
        Event {
            exception: vec![Exception {
                ty: "panic".into(),
                mechanism: Some(Mechanism {
                    ty: "panic".into(),
                    handled: Some(false),
                    ..Default::default()
                }),
                value: Some(msg.to_string()),
                stacktrace: current_stacktrace(),
                ..Default::default()
            }]
            .into(),
            level: Level::Fatal,
            ..Default::default()
        }
    }
}
