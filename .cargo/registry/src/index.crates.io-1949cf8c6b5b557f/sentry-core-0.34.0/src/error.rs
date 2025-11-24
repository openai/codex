use std::error::Error;

use crate::protocol::{Event, Exception, Level};
use crate::types::Uuid;
use crate::Hub;

impl Hub {
    /// Capture any `std::error::Error`.
    ///
    /// See the global [`capture_error`](fn.capture_error.html)
    /// for more documentation.
    #[allow(unused)]
    pub fn capture_error<E: Error + ?Sized>(&self, error: &E) -> Uuid {
        with_client_impl! {{
            self.inner.with(|stack| {
                let top = stack.top();
                if top.client.is_some() {
                    let event = event_from_error(error);
                    self.capture_event(event)
                } else {
                    Uuid::nil()
                }
            })
        }}
    }
}

/// Captures a `std::error::Error`.
///
/// Creates an event from the given error and sends it to the current hub.
/// A chain of errors will be resolved as well, and sorted oldest to newest, as
/// described in the [sentry event payloads].
///
/// # Examples
///
/// ```
/// let err = "NaN".parse::<usize>().unwrap_err();
///
/// # let events = sentry::test::with_captured_events(|| {
/// sentry::capture_error(&err);
/// # });
/// # let captured_event = events.into_iter().next().unwrap();
///
/// assert_eq!(captured_event.exception.len(), 1);
/// assert_eq!(&captured_event.exception[0].ty, "ParseIntError");
/// ```
///
/// [sentry event payloads]: https://develop.sentry.dev/sdk/event-payloads/exception/
#[allow(unused_variables)]
pub fn capture_error<E: Error + ?Sized>(error: &E) -> Uuid {
    Hub::with_active(|hub| hub.capture_error(error))
}

/// Create a sentry `Event` from a `std::error::Error`.
///
/// A chain of errors will be resolved as well, and sorted oldest to newest, as
/// described in the [sentry event payloads].
///
/// # Examples
///
/// ```
/// use thiserror::Error;
///
/// #[derive(Debug, Error)]
/// #[error("inner")]
/// struct InnerError;
///
/// #[derive(Debug, Error)]
/// #[error("outer")]
/// struct OuterError(#[from] InnerError);
///
/// let event = sentry::event_from_error(&OuterError(InnerError));
/// assert_eq!(event.level, sentry::protocol::Level::Error);
/// assert_eq!(event.exception.len(), 2);
/// assert_eq!(&event.exception[0].ty, "InnerError");
/// assert_eq!(event.exception[0].value, Some("inner".into()));
/// assert_eq!(&event.exception[1].ty, "OuterError");
/// assert_eq!(event.exception[1].value, Some("outer".into()));
/// ```
///
/// [sentry event payloads]: https://develop.sentry.dev/sdk/event-payloads/exception/
pub fn event_from_error<E: Error + ?Sized>(err: &E) -> Event<'static> {
    let mut exceptions = vec![exception_from_error(err)];

    let mut source = err.source();
    while let Some(err) = source {
        exceptions.push(exception_from_error(err));
        source = err.source();
    }

    exceptions.reverse();
    Event {
        exception: exceptions.into(),
        level: Level::Error,
        ..Default::default()
    }
}

fn exception_from_error<E: Error + ?Sized>(err: &E) -> Exception {
    let dbg = format!("{err:?}");
    let value = err.to_string();

    // A generic `anyhow::msg` will just `Debug::fmt` the `String` that you feed
    // it. Trying to parse the type name from that will result in a leading quote
    // and the first word, so quite useless.
    // To work around this, we check if the `Debug::fmt` of the complete error
    // matches its `Display::fmt`, in which case there is no type to parse and
    // we will just be using `Error`.
    let ty = if dbg == format!("{value:?}") {
        String::from("Error")
    } else {
        parse_type_from_debug(&dbg).to_owned()
    };
    Exception {
        ty,
        value: Some(err.to_string()),
        ..Default::default()
    }
}

/// Parse the types name from `Debug` output.
///
/// # Examples
///
/// ```
/// use sentry::parse_type_from_debug;
///
/// let err = format!("{:?}", "NaN".parse::<usize>().unwrap_err());
/// assert_eq!(parse_type_from_debug(&err), "ParseIntError");
/// ```
pub fn parse_type_from_debug(d: &str) -> &str {
    d.split(&[' ', '(', '{', '\r', '\n'][..])
        .next()
        .unwrap()
        .trim()
}

#[test]
fn test_parse_type_from_debug() {
    use parse_type_from_debug as parse;
    #[derive(Debug)]
    struct MyStruct;
    let err = format!("{MyStruct:?}");
    assert_eq!(parse(&err), "MyStruct");

    let err = format!("{:?}", "NaN".parse::<usize>().unwrap_err());
    assert_eq!(parse(&err), "ParseIntError");

    let err = format!(
        "{:?}",
        sentry_types::ParseDsnError::from(sentry_types::ParseProjectIdError::EmptyValue)
    );
    assert_eq!(parse(&err), "InvalidProjectId");

    // `anyhow` is using extended debug formatting
    let err = format!(
        "{:#?}",
        anyhow::Error::from("NaN".parse::<usize>().unwrap_err())
    );
    assert_eq!(parse(&err), "ParseIntError");
}

#[test]
fn test_parse_anyhow_as_error() {
    let anyhow_err = anyhow::anyhow!("Ooops, something bad happened");
    let err: &dyn Error = anyhow_err.as_ref();

    let exc = exception_from_error(err);

    assert_eq!(&exc.ty, "Error");
    assert_eq!(exc.value.as_deref(), Some("Ooops, something bad happened"));
}
