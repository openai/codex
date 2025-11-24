use sentry_types::protocol::v7::SessionStatus;

use crate::protocol::{Event, Level};
use crate::types::Uuid;
use crate::{Hub, Integration, IntoBreadcrumbs, Scope};

/// Captures an event on the currently active client if any.
///
/// The event must already be assembled. Typically code would instead use
/// the utility methods like [`capture_message`], [`capture_error`], or an
/// integration specific function.
///
/// The return value is the event ID. If the event was discarded for any reason,
/// return value will be the nil UUID (`Uuid::nil`).
///
/// # Examples
///
/// ```
/// use sentry::protocol::{Event, Level};
/// use sentry::types::{Uuid, random_uuid};
///
/// let uuid = random_uuid();
/// let event = Event {
///     event_id: uuid,
///     message: Some("Hello World!".into()),
///     level: Level::Info,
///     ..Default::default()
/// };
///
/// assert_eq!(sentry::capture_event(event.clone()), Uuid::nil());
///
/// let events = sentry::test::with_captured_events(|| {
///     assert_eq!(sentry::capture_event(event), uuid);
/// });
/// assert_eq!(events.len(), 1);
/// ```
///
/// [`capture_message`]: fn.capture_message.html
/// [`capture_error`]: fn.capture_error.html
pub fn capture_event(event: Event<'static>) -> Uuid {
    Hub::with_active(|hub| hub.capture_event(event))
}

/// Captures an arbitrary message.
///
/// This creates an event from the given message and sends it via
/// [`capture_event`](fn.capture_event.html).
///
/// # Examples
///
/// ```
/// use sentry::protocol::Level;
///
/// # let events = sentry::test::with_captured_events(|| {
/// sentry::capture_message("some message", Level::Info);
/// # });
/// # let captured_event = events.into_iter().next().unwrap();
///
/// assert_eq!(captured_event.message.as_deref(), Some("some message"));
/// ```
pub fn capture_message(msg: &str, level: Level) -> Uuid {
    Hub::with_active(|hub| hub.capture_message(msg, level))
}

/// Records a breadcrumb by calling a function.
///
/// The total number of breadcrumbs that can be recorded are limited by the
/// configuration on the client.  This function accepts any object that
/// implements [`IntoBreadcrumbs`], which is implemented for a varienty of
/// common types.  For efficiency reasons you can also pass a closure returning
/// a breadcrumb in which case the closure is only called if the client is
/// enabled.
///
/// The most common implementations that can be passed:
///
/// * `Breadcrumb`: to record a breadcrumb
/// * `Vec<Breadcrumb>`: to record more than one breadcrumb in one go.
/// * `Option<Breadcrumb>`: to record a breadcrumb or not
/// * additionally all of these can also be returned from an `FnOnce()`
///
/// # Examples
///
/// ```
/// use sentry::protocol::{Breadcrumb, Level, Map};
///
/// let breadcrumb = Breadcrumb {
///     ty: "http".into(),
///     category: Some("request".into()),
///     data: {
///         let mut map = Map::new();
///         map.insert("method".into(), "GET".into());
///         map.insert("url".into(), "https://example.com/".into());
///         map
///     },
///     ..Default::default()
/// };
///
/// # let events = sentry::test::with_captured_events(|| {
/// sentry::add_breadcrumb(breadcrumb.clone());
///
/// sentry::capture_message("some message", Level::Info);
/// # });
/// # let captured_event = events.into_iter().next().unwrap();
///
/// assert_eq!(captured_event.breadcrumbs.values, vec![breadcrumb]);
/// ```
///
/// [`IntoBreadcrumbs`]: trait.IntoBreadcrumbs.html
pub fn add_breadcrumb<B: IntoBreadcrumbs>(breadcrumb: B) {
    Hub::with_active(|hub| hub.add_breadcrumb(breadcrumb))
}

/// Invokes a function that can modify the current scope.
///
/// The function is passed a mutable reference to the [`Scope`] so that modifications
/// can be performed.  Because there might currently not be a scope or client active
/// it's possible that the callback might not be called at all.  As a result of this
/// the return value of this closure must have a default that is returned in such
/// cases.
///
/// # Examples
///
/// ```
/// use sentry::protocol::{Level, User};
///
/// let user = Some(User {
///     username: Some("john_doe".into()),
///     ..Default::default()
/// });
///
/// # let events = sentry::test::with_captured_events(|| {
/// sentry::configure_scope(|scope| {
///     scope.set_user(user.clone());
/// });
///
/// sentry::capture_message("some message", Level::Info);
/// # });
/// # let captured_event = events.into_iter().next().unwrap();
///
/// assert_eq!(captured_event.user, user);
/// ```
///
/// # Panics
///
/// While the scope is being configured accessing scope related functionality is
/// not permitted.  In this case a wide range of panics will be raised.  It's
/// unsafe to call into `sentry::bind_client` or similar functions from within
/// the callback as a result of this.
///
/// [`Scope`]: struct.Scope.html
pub fn configure_scope<F, R>(f: F) -> R
where
    R: Default,
    F: FnOnce(&mut Scope) -> R,
{
    Hub::with_active(|hub| hub.configure_scope(f))
}

/// Temporarily pushes a scope for a single call optionally reconfiguring it.
///
/// This function takes two arguments: the first is a callback that is passed
/// a scope and can reconfigure it.  The second is callback that then executes
/// in the context of that scope.
///
/// This is useful when extra data should be send with a single capture call
/// for instance a different level or tags:
///
/// # Examples
///
/// ```
/// use sentry::protocol::Level;
///
/// # let events = sentry::test::with_captured_events(|| {
/// sentry::with_scope(
///     |scope| scope.set_level(Some(Level::Warning)),
///     || sentry::capture_message("some message", Level::Info),
/// );
/// # });
/// # let captured_event = events.into_iter().next().unwrap();
///
/// assert_eq!(captured_event.level, Level::Warning);
/// ```
pub fn with_scope<C, F, R>(scope_config: C, callback: F) -> R
where
    C: FnOnce(&mut Scope),
    F: FnOnce() -> R,
{
    #[cfg(feature = "client")]
    {
        Hub::with(|hub| {
            if hub.is_active_and_usage_safe() {
                hub.with_scope(scope_config, callback)
            } else {
                callback()
            }
        })
    }
    #[cfg(not(feature = "client"))]
    {
        let _scope_config = scope_config;
        callback()
    }
}

/// Looks up an integration on the current Hub.
///
/// Calls the given function with the requested integration instance when it
/// is active on the currently active client. When multiple instances of the
/// same integration are added, the function will be called with the first one.
///
/// # Examples
///
/// ```
/// use sentry::{ClientOptions, Integration};
///
/// struct MyIntegration(usize);
/// impl Integration for MyIntegration {}
///
/// let options = ClientOptions::default()
///     .add_integration(MyIntegration(10))
///     .add_integration(MyIntegration(20));
/// # let _options = options.clone();
///
/// let _sentry = sentry::init(options);
///
/// # sentry::test::with_captured_events_options(|| {
/// let value = sentry::with_integration(|integration: &MyIntegration, _| integration.0);
/// assert_eq!(value, 10);
/// # }, _options);
/// ```
pub fn with_integration<I, F, R>(f: F) -> R
where
    I: Integration,
    F: FnOnce(&I, &Hub) -> R,
    R: Default,
{
    Hub::with_active(|hub| hub.with_integration(|i| f(i, hub)))
}

/// Returns the last event ID captured.
///
/// This uses the current thread local [`Hub`], and will return `None` if no
/// event has been captured yet on this [`Hub`].
///
/// # Examples
///
/// ```
/// use sentry::protocol::Level;
/// use sentry::types::Uuid;
///
/// # sentry::test::with_captured_events(|| {
/// assert_eq!(sentry::last_event_id(), None);
///
/// sentry::capture_message("some message", Level::Info);
///
/// assert!(sentry::last_event_id().is_some());
/// # });
/// ```
///
/// [`Hub`]: struct.Hub.html
pub fn last_event_id() -> Option<Uuid> {
    with_client_impl! {{
        Hub::with(|hub| hub.last_event_id())
    }}
}

/// Start a new session for Release Health.
///
/// This is still **experimental** for the moment and is not recommended to be
/// used with a very high volume of sessions (_request-mode_ sessions).
///
/// # Examples
///
/// ```
/// sentry::start_session();
///
/// // capturing any event / error here will update the sessions `errors` count,
/// // up until we call `sentry::end_session`.
///
/// sentry::end_session();
/// ```
pub fn start_session() {
    Hub::with_active(|hub| hub.start_session())
}

/// End the current Release Health Session.
pub fn end_session() {
    end_session_with_status(SessionStatus::Exited)
}

/// End the current Release Health Session with the given [`SessionStatus`].
///
/// By default, the SDK will only consider the `Exited` and `Crashed` status
/// based on the type of events that were captured during the session.
///
/// When an `Abnormal` session should be captured, it has to be done explicitly
/// using this function.
pub fn end_session_with_status(status: SessionStatus) {
    Hub::with_active(|hub| hub.end_session_with_status(status))
}
