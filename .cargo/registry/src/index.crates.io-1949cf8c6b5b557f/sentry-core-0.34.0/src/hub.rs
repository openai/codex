// NOTE: Most of the methods are noops without the `client` feature, and this will
// silence all the "unused variable" warnings related to fn arguments.
#![allow(unused)]

use std::sync::{Arc, RwLock};

use crate::protocol::{Event, Level, SessionStatus};
use crate::types::Uuid;
use crate::{Integration, IntoBreadcrumbs, Scope, ScopeGuard};

/// The central object that can manages scopes and clients.
///
/// This can be used to capture events and manage the scope.  This object is [`Send`] and
/// [`Sync`] so it can be used from multiple threads if needed.
///
/// Each thread has its own thread-local ( see [`Hub::current`]) hub, which is
/// automatically derived from the main hub ([`Hub::main`]).
///
/// In most situations developers do not need to interface with the hub directly.  Instead
/// toplevel convenience functions are expose that will automatically dispatch
/// to the thread-local ([`Hub::current`]) hub.  In some situations this might not be
/// possible in which case it might become necessary to manually work with the
/// hub.  See the main [`crate`] docs for some common use-cases and pitfalls
/// related to parallel, concurrent or async code.
///
/// Hubs that are wrapped in [`Arc`]s can be bound to the current thread with
/// the `run` static method.
///
/// Most common operations:
///
/// * [`Hub::new`]: creates a brand new hub
/// * [`Hub::current`]: returns the thread local hub
/// * [`Hub::with`]: invoke a callback with the thread local hub
/// * [`Hub::with_active`]: like `Hub::with` but does not invoke the callback if
///   the client is not in a supported state or not bound
/// * [`Hub::new_from_top`]: creates a new hub with just the top scope of another hub.
#[derive(Debug)]
pub struct Hub {
    #[cfg(feature = "client")]
    pub(crate) inner: crate::hub_impl::HubImpl,
    pub(crate) last_event_id: RwLock<Option<Uuid>>,
}

impl Hub {
    /// Like [`Hub::with`] but only calls the function if a client is bound.
    ///
    /// This is useful for integrations that want to do efficiently nothing if there is no
    /// client bound.  Additionally this internally ensures that the client can be safely
    /// synchronized.  This prevents accidental recursive calls into the client.
    pub fn with_active<F, R>(f: F) -> R
    where
        F: FnOnce(&Arc<Hub>) -> R,
        R: Default,
    {
        with_client_impl! {{
            Hub::with(|hub| {
                if hub.is_active_and_usage_safe() {
                    f(hub)
                } else {
                    Default::default()
                }
            })
        }}
    }

    /// Looks up an integration on the hub.
    ///
    /// Calls the given function with the requested integration instance when it
    /// is active on the currently active client.
    ///
    /// See the global [`capture_event`](fn.capture_event.html)
    /// for more documentation.
    pub fn with_integration<I, F, R>(&self, f: F) -> R
    where
        I: Integration,
        F: FnOnce(&I) -> R,
        R: Default,
    {
        with_client_impl! {{
            if let Some(client) = self.client() {
                if let Some(integration) = client.get_integration::<I>() {
                    return f(integration);
                }
            }
            Default::default()
        }}
    }

    /// Returns the last event id.
    pub fn last_event_id(&self) -> Option<Uuid> {
        *self.last_event_id.read().unwrap()
    }

    /// Sends the event to the current client with the current scope.
    ///
    /// In case no client is bound this does nothing instead.
    ///
    /// See the global [`capture_event`](fn.capture_event.html)
    /// for more documentation.
    pub fn capture_event(&self, event: Event<'static>) -> Uuid {
        with_client_impl! {{
            let top = self.inner.with(|stack| stack.top().clone());
            let Some(ref client) = top.client else { return Default::default() };
            let event_id = client.capture_event(event, Some(&top.scope));
            *self.last_event_id.write().unwrap() = Some(event_id);
            event_id
        }}
    }

    /// Captures an arbitrary message.
    ///
    /// See the global [`capture_message`](fn.capture_message.html)
    /// for more documentation.
    pub fn capture_message(&self, msg: &str, level: Level) -> Uuid {
        with_client_impl! {{
            let event = Event {
                message: Some(msg.to_string()),
                level,
                ..Default::default()
            };
            self.capture_event(event)
        }}
    }

    /// Start a new session for Release Health.
    ///
    /// See the global [`start_session`](fn.start_session.html)
    /// for more documentation.
    pub fn start_session(&self) {
        with_client_impl! {{
            self.inner.with_mut(|stack| {
                let top = stack.top_mut();
                if let Some(session) = crate::session::Session::from_stack(top) {
                    // When creating a *new* session, we make sure it is unique,
                    // as to no inherit *backwards* to any parents.
                    let mut scope = Arc::make_mut(&mut top.scope);
                    scope.session = Arc::new(std::sync::Mutex::new(Some(session)));
                }
            })
        }}
    }

    /// End the current Release Health Session.
    ///
    /// See the global [`sentry::end_session`](crate::end_session) for more documentation.
    pub fn end_session(&self) {
        self.end_session_with_status(SessionStatus::Exited)
    }

    /// End the current Release Health Session with the given [`SessionStatus`].
    ///
    /// See the global [`end_session_with_status`](crate::end_session_with_status)
    /// for more documentation.
    pub fn end_session_with_status(&self, status: SessionStatus) {
        with_client_impl! {{
            self.inner.with_mut(|stack| {
                let top = stack.top_mut();
                // drop will close and enqueue the session
                if let Some(mut session) = top.scope.session.lock().unwrap().take() {
                    session.close(status);
                }
            })
        }}
    }

    /// Pushes a new scope.
    ///
    /// This returns a guard that when dropped will pop the scope again.
    pub fn push_scope(&self) -> ScopeGuard {
        with_client_impl! {{
            self.inner.with_mut(|stack| {
                stack.push();
                ScopeGuard(Some((self.inner.stack.clone(), stack.depth())))
            })
        }}
    }

    /// Temporarily pushes a scope for a single call optionally reconfiguring it.
    ///
    /// See the global [`with_scope`](fn.with_scope.html)
    /// for more documentation.
    pub fn with_scope<C, F, R>(&self, scope_config: C, callback: F) -> R
    where
        C: FnOnce(&mut Scope),
        F: FnOnce() -> R,
    {
        #[cfg(feature = "client")]
        {
            let _guard = self.push_scope();
            self.configure_scope(scope_config);
            callback()
        }
        #[cfg(not(feature = "client"))]
        {
            let _scope_config = scope_config;
            callback()
        }
    }

    /// Invokes a function that can modify the current scope.
    ///
    /// See the global [`configure_scope`](fn.configure_scope.html)
    /// for more documentation.
    pub fn configure_scope<F, R>(&self, f: F) -> R
    where
        R: Default,
        F: FnOnce(&mut Scope) -> R,
    {
        with_client_impl! {{
            let mut new_scope = self.with_current_scope(|scope| scope.clone());
            let rv = f(&mut new_scope);
            self.with_current_scope_mut(|ptr| *ptr = new_scope);
            rv
        }}
    }

    /// Adds a new breadcrumb to the current scope.
    ///
    /// See the global [`add_breadcrumb`](fn.add_breadcrumb.html)
    /// for more documentation.
    pub fn add_breadcrumb<B: IntoBreadcrumbs>(&self, breadcrumb: B) {
        with_client_impl! {{
            self.inner.with_mut(|stack| {
                let top = stack.top_mut();
                if let Some(ref client) = top.client {
                    let scope = Arc::make_mut(&mut top.scope);
                    let options = client.options();
                    let breadcrumbs = Arc::make_mut(&mut scope.breadcrumbs);
                    for breadcrumb in breadcrumb.into_breadcrumbs() {
                        let breadcrumb_opt = match options.before_breadcrumb {
                            Some(ref callback) => callback(breadcrumb),
                            None => Some(breadcrumb)
                        };
                        if let Some(breadcrumb) = breadcrumb_opt {
                            breadcrumbs.push_back(breadcrumb);
                        }
                        while breadcrumbs.len() > options.max_breadcrumbs {
                            breadcrumbs.pop_front();
                        }
                    }
                }
            })
        }}
    }
}
