use std::any::{type_name, Any};

use crate::protocol::Event;
use crate::ClientOptions;

/// Integration abstraction.
///
/// An Integration in sentry has two primary purposes.
/// It can act as an *Event Source*, which will capture new events;
/// or as an *Event Processor*, which can modify every `Event` flowing through
/// the pipeline.
///
/// # Examples
///
/// ```
/// use sentry::protocol::{Event, Level};
/// use sentry::ClientOptions;
///
/// struct MyProcessorIntegration {
///     override_environment: &'static str,
///     override_level: Level,
/// }
///
/// impl sentry::Integration for MyProcessorIntegration {
///     fn setup(&self, options: &mut ClientOptions) {
///         options.environment = Some(self.override_environment.into());
///     }
///     fn process_event(
///         &self,
///         mut event: Event<'static>,
///         _options: &ClientOptions,
///     ) -> Option<Event<'static>> {
///         event.level = self.override_level;
///         Some(event)
///     }
/// }
///
/// let options = ClientOptions::new().add_integration(MyProcessorIntegration {
///     override_environment: "my_env",
///     override_level: Level::Error,
/// });
///
/// let events = sentry::test::with_captured_events_options(
///     || {
///         sentry::capture_message("some message", Level::Info);
///     },
///     options,
/// );
/// let captured_event = events.into_iter().next().unwrap();
///
/// assert_eq!(captured_event.level, Level::Error);
/// assert_eq!(captured_event.environment, Some("my_env".into()));
/// ```
// NOTE: we need `Any` here so that the `TypeId` machinery works correctly.
pub trait Integration: Sync + Send + Any + AsAny {
    /// Name of this integration.
    ///
    /// This will be added to the SDK information sent to sentry.
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }

    /// Called whenever the integration is attached to a Client.
    fn setup(&self, options: &mut ClientOptions) {
        let _ = options;
    }

    /// The Integrations Event Processor Hook.
    ///
    /// An integration can process, or even completely drop an `Event`.
    /// Examples include adding or processing a backtrace, obfuscate some
    /// personal information, or add additional information.
    fn process_event(
        &self,
        event: Event<'static>,
        options: &ClientOptions,
    ) -> Option<Event<'static>> {
        let _ = options;
        Some(event)
    }
}

// This is needed as a workaround to be able to safely downcast integrations
#[doc(hidden)]
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
