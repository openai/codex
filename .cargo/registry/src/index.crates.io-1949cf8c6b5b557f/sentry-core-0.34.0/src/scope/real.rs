use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use crate::performance::TransactionOrSpan;
use crate::protocol::{Attachment, Breadcrumb, Context, Event, Level, Transaction, User, Value};
use crate::session::Session;
use crate::Client;

#[derive(Debug)]
pub struct Stack {
    top: StackLayer,
    layers: Vec<StackLayer>,
}

pub type EventProcessor = Arc<dyn Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync>;

/// Holds contextual data for the current scope.
///
/// The scope is an object that can be cloned efficiently and stores data that
/// is locally relevant to an event.  For instance the scope will hold recorded
/// breadcrumbs and similar information.
///
/// The scope can be interacted with in two ways:
///
/// 1. the scope is routinely updated with information by functions such as
///    [`add_breadcrumb`] which will modify the currently top-most scope.
/// 2. the topmost scope can also be configured through the [`configure_scope`]
///    method.
///
/// Note that the scope can only be modified but not inspected.  Only the
/// client can use the scope to extract information currently.
///
/// [`add_breadcrumb`]: fn.add_breadcrumb.html
/// [`configure_scope`]: fn.configure_scope.html
#[derive(Clone, Default)]
pub struct Scope {
    pub(crate) level: Option<Level>,
    pub(crate) fingerprint: Option<Arc<[Cow<'static, str>]>>,
    pub(crate) transaction: Option<Arc<str>>,
    pub(crate) breadcrumbs: Arc<VecDeque<Breadcrumb>>,
    pub(crate) user: Option<Arc<User>>,
    pub(crate) extra: Arc<HashMap<String, Value>>,
    pub(crate) tags: Arc<HashMap<String, String>>,
    pub(crate) contexts: Arc<HashMap<String, Context>>,
    pub(crate) event_processors: Arc<Vec<EventProcessor>>,
    pub(crate) session: Arc<Mutex<Option<Session>>>,
    pub(crate) span: Arc<Option<TransactionOrSpan>>,
    pub(crate) attachments: Arc<Vec<Attachment>>,
}

impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scope")
            .field("level", &self.level)
            .field("fingerprint", &self.fingerprint)
            .field("transaction", &self.transaction)
            .field("breadcrumbs", &self.breadcrumbs)
            .field("user", &self.user)
            .field("extra", &self.extra)
            .field("tags", &self.tags)
            .field("contexts", &self.contexts)
            .field("event_processors", &self.event_processors.len())
            .field("session", &self.session)
            .field("span", &self.span)
            .field("attachments", &self.attachments.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct StackLayer {
    pub client: Option<Arc<Client>>,
    pub scope: Arc<Scope>,
}

impl Stack {
    pub fn from_client_and_scope(client: Option<Arc<Client>>, scope: Arc<Scope>) -> Stack {
        Stack {
            top: StackLayer { client, scope },
            layers: vec![],
        }
    }

    pub fn push(&mut self) {
        let layer = self.top.clone();
        self.layers.push(layer);
    }

    pub fn pop(&mut self) {
        if self.layers.is_empty() {
            panic!("Pop from empty stack");
        }
        self.top = self.layers.pop().unwrap();
    }

    #[inline(always)]
    pub fn top(&self) -> &StackLayer {
        &self.top
    }

    #[inline(always)]
    pub fn top_mut(&mut self) -> &mut StackLayer {
        &mut self.top
    }

    pub fn depth(&self) -> usize {
        self.layers.len()
    }
}

/// A scope guard.
///
/// This is returned from [`Hub::push_scope`] and will automatically pop the
/// scope on drop.
///
/// [`Hub::push_scope`]: struct.Hub.html#method.with_scope
#[derive(Default)]
pub struct ScopeGuard(pub(crate) Option<(Arc<RwLock<Stack>>, usize)>);

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        if let Some((stack, depth)) = self.0.take() {
            let popped_depth = {
                let mut stack = stack.write().unwrap_or_else(PoisonError::into_inner);
                let popped_depth = stack.depth();
                stack.pop();
                popped_depth
            };
            // NOTE: We need to drop the `stack` lock before panicing, as the
            // `PanicIntegration` will want to lock the `stack` itself
            // (through `capture_event` -> `HubImpl::with`), and would thus
            // result in a deadlock.
            // Though that deadlock itself is detected by the `RwLock` (on macOS)
            // and results in its own panic: `rwlock read lock would result in deadlock`.
            // However that panic happens in the panic handler and will thus
            // ultimately result in a `thread panicked while processing panic. aborting.`
            // Long story short, we should not panic while holding the lock :-)
            if popped_depth != depth {
                panic!("Popped scope guard out of order");
            }
        }
    }
}

impl Scope {
    /// Clear the scope.
    ///
    /// By default a scope will inherit all values from the higher scope.
    /// In some situations this might not be what a user wants.  Calling
    /// this method will wipe all data contained within.
    pub fn clear(&mut self) {
        *self = Default::default();
    }

    /// Deletes current breadcrumbs from the scope.
    pub fn clear_breadcrumbs(&mut self) {
        self.breadcrumbs = Default::default();
    }

    /// Sets a level override.
    pub fn set_level(&mut self, level: Option<Level>) {
        self.level = level;
    }

    /// Sets the fingerprint.
    pub fn set_fingerprint(&mut self, fingerprint: Option<&[&str]>) {
        self.fingerprint =
            fingerprint.map(|fp| fp.iter().map(|s| Cow::Owned((*s).into())).collect())
    }

    /// Sets the transaction.
    pub fn set_transaction(&mut self, transaction: Option<&str>) {
        self.transaction = transaction.map(Arc::from);
        if let Some(name) = transaction {
            let trx = match self.span.as_ref() {
                Some(TransactionOrSpan::Span(span)) => &span.transaction,
                Some(TransactionOrSpan::Transaction(trx)) => &trx.inner,
                _ => return,
            };

            if let Some(trx) = trx.lock().unwrap().transaction.as_mut() {
                trx.name = Some(name.into());
            }
        }
    }

    /// Sets the user for the current scope.
    pub fn set_user(&mut self, user: Option<User>) {
        self.user = user.map(Arc::new);
    }

    /// Sets a tag to a specific value.
    pub fn set_tag<V: ToString>(&mut self, key: &str, value: V) {
        Arc::make_mut(&mut self.tags).insert(key.to_string(), value.to_string());
    }

    /// Removes a tag.
    ///
    /// If the tag is not set, does nothing.
    pub fn remove_tag(&mut self, key: &str) {
        Arc::make_mut(&mut self.tags).remove(key);
    }

    /// Sets a context for a key.
    pub fn set_context<C: Into<Context>>(&mut self, key: &str, value: C) {
        Arc::make_mut(&mut self.contexts).insert(key.to_string(), value.into());
    }

    /// Removes a context for a key.
    pub fn remove_context(&mut self, key: &str) {
        Arc::make_mut(&mut self.contexts).remove(key);
    }

    /// Sets a extra to a specific value.
    pub fn set_extra(&mut self, key: &str, value: Value) {
        Arc::make_mut(&mut self.extra).insert(key.to_string(), value);
    }

    /// Removes a extra.
    pub fn remove_extra(&mut self, key: &str) {
        Arc::make_mut(&mut self.extra).remove(key);
    }

    /// Add an event processor to the scope.
    pub fn add_event_processor<F>(&mut self, f: F)
    where
        F: Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        Arc::make_mut(&mut self.event_processors).push(Arc::new(f));
    }

    /// Adds an attachment to the scope
    pub fn add_attachment(&mut self, attachment: Attachment) {
        Arc::make_mut(&mut self.attachments).push(attachment);
    }

    /// Clears attachments from the scope
    pub fn clear_attachments(&mut self) {
        Arc::make_mut(&mut self.attachments).clear();
    }

    /// Applies the contained scoped data to fill an event.
    pub fn apply_to_event(&self, mut event: Event<'static>) -> Option<Event<'static>> {
        // TODO: event really should have an optional level
        if self.level.is_some() {
            event.level = self.level.unwrap();
        }

        if event.user.is_none() {
            if let Some(user) = self.user.as_deref() {
                event.user = Some(user.clone());
            }
        }

        event.breadcrumbs.extend(self.breadcrumbs.iter().cloned());
        event
            .extra
            .extend(self.extra.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
        event
            .tags
            .extend(self.tags.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
        event.contexts.extend(
            self.contexts
                .iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned())),
        );

        if let Some(span) = self.span.as_ref() {
            span.apply_to_event(&mut event);
        }

        if event.transaction.is_none() {
            if let Some(txn) = self.transaction.as_deref() {
                event.transaction = Some(txn.to_owned());
            }
        }

        if event.fingerprint.len() == 1
            && (event.fingerprint[0] == "{{ default }}" || event.fingerprint[0] == "{{default}}")
        {
            if let Some(fp) = self.fingerprint.as_deref() {
                event.fingerprint = Cow::Owned(fp.to_owned());
            }
        }

        for processor in self.event_processors.as_ref() {
            let id = event.event_id;
            event = match processor(event) {
                Some(event) => event,
                None => {
                    sentry_debug!("event processor dropped event {}", id);
                    return None;
                }
            }
        }

        Some(event)
    }

    /// Applies the contained scoped data to fill a transaction.
    pub fn apply_to_transaction(&self, transaction: &mut Transaction<'static>) {
        if transaction.user.is_none() {
            if let Some(user) = self.user.as_deref() {
                transaction.user = Some(user.clone());
            }
        }

        transaction
            .extra
            .extend(self.extra.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
        transaction
            .tags
            .extend(self.tags.iter().map(|(k, v)| (k.to_owned(), v.to_owned())));
        transaction.contexts.extend(
            self.contexts
                .iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned())),
        );
    }

    /// Set the given [`TransactionOrSpan`] as the active span for this scope.
    pub fn set_span(&mut self, span: Option<TransactionOrSpan>) {
        self.span = Arc::new(span);
    }

    /// Returns the currently active span.
    pub fn get_span(&self) -> Option<TransactionOrSpan> {
        self.span.as_ref().clone()
    }

    pub(crate) fn update_session_from_event(&self, event: &Event<'static>) {
        if let Some(session) = self.session.lock().unwrap().as_mut() {
            session.update_from_event(event);
        }
    }
}
