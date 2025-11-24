use std::fmt;

use crate::protocol::{Context, Event, Level, User, Value};
use crate::TransactionOrSpan;

/// A minimal API scope guard.
///
/// Doesn't do anything but can be debug formatted.
#[derive(Default)]
pub struct ScopeGuard;

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

/// The minimal scope.
///
/// In minimal API mode all modification functions are available as normally
/// just that generally calling them is impossible.
#[derive(Debug, Clone)]
pub struct Scope;

impl Scope {
    /// Clear the scope.
    ///
    /// By default a scope will inherit all values from the higher scope.
    /// In some situations this might not be what a user wants.  Calling
    /// this method will wipe all data contained within.
    pub fn clear(&mut self) {
        minimal_unreachable!();
    }

    /// Sets a level override.
    pub fn set_level(&mut self, level: Option<Level>) {
        let _level = level;
        minimal_unreachable!();
    }

    /// Sets the fingerprint.
    pub fn set_fingerprint(&mut self, fingerprint: Option<&[&str]>) {
        let _fingerprint = fingerprint;
        minimal_unreachable!();
    }

    /// Sets the transaction.
    pub fn set_transaction(&mut self, transaction: Option<&str>) {
        let _transaction = transaction;
        minimal_unreachable!();
    }

    /// Sets the user for the current scope.
    pub fn set_user(&mut self, user: Option<User>) {
        let _user = user;
        minimal_unreachable!();
    }

    /// Sets a tag to a specific value.
    pub fn set_tag<V: ToString>(&mut self, key: &str, value: V) {
        let _key = key;
        let _value = value;
        minimal_unreachable!();
    }

    /// Removes a tag.
    pub fn remove_tag(&mut self, key: &str) {
        let _key = key;
        minimal_unreachable!();
    }

    /// Sets a context for a key.
    pub fn set_context<C: Into<Context>>(&mut self, key: &str, value: C) {
        let _key = key;
        let _value = value;
        minimal_unreachable!();
    }

    /// Removes a context for a key.
    pub fn remove_context(&mut self, key: &str) {
        let _key = key;
        minimal_unreachable!();
    }

    /// Sets a extra to a specific value.
    pub fn set_extra(&mut self, key: &str, value: Value) {
        let _key = key;
        let _value = value;
        minimal_unreachable!();
    }

    /// Removes a extra.
    pub fn remove_extra(&mut self, key: &str) {
        let _key = key;
        minimal_unreachable!();
    }

    /// Add an event processor to the scope.
    pub fn add_event_processor<F>(&mut self, f: F)
    where
        F: Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        let _f = f;
        minimal_unreachable!();
    }

    /// Applies the contained scoped data to fill an event.
    pub fn apply_to_event(&self, event: Event<'static>) -> Option<Event<'static>> {
        let _event = event;
        minimal_unreachable!();
    }

    /// Set the given [`TransactionOrSpan`] as the active span for this scope.
    pub fn set_span(&mut self, span: Option<TransactionOrSpan>) {
        let _ = span;
        minimal_unreachable!();
    }

    /// Returns the currently active span.
    pub fn get_span(&self) -> Option<TransactionOrSpan> {
        None
    }
}
