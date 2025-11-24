use std::cell::{Cell, UnsafeCell};
use std::sync::{Arc, PoisonError, RwLock};
use std::thread;

use crate::Scope;
use crate::{scope::Stack, Client, Hub};

use once_cell::sync::Lazy;

static PROCESS_HUB: Lazy<(Arc<Hub>, thread::ThreadId)> = Lazy::new(|| {
    (
        Arc::new(Hub::new(None, Arc::new(Default::default()))),
        thread::current().id(),
    )
});

thread_local! {
    static THREAD_HUB: (UnsafeCell<Arc<Hub>>, Cell<bool>) = (
        UnsafeCell::new(Arc::new(Hub::new_from_top(&PROCESS_HUB.0))),
        Cell::new(PROCESS_HUB.1 == thread::current().id())
    );
}

pub(crate) struct SwitchGuard {
    inner: Option<(Arc<Hub>, bool)>,
}

impl SwitchGuard {
    pub(crate) fn new(mut hub: Arc<Hub>) -> Self {
        let inner = THREAD_HUB.with(|(thread_hub, is_process_hub)| {
            // SAFETY: `thread_hub` will always be a valid thread local hub,
            // by definition not shared between threads.
            let thread_hub = unsafe { &mut *thread_hub.get() };
            if std::ptr::eq(thread_hub.as_ref(), hub.as_ref()) {
                return None;
            }
            std::mem::swap(thread_hub, &mut hub);
            let was_process_hub = is_process_hub.replace(false);
            Some((hub, was_process_hub))
        });
        SwitchGuard { inner }
    }

    fn swap(&mut self) -> Option<Arc<Hub>> {
        if let Some((mut hub, was_process_hub)) = self.inner.take() {
            Some(THREAD_HUB.with(|(thread_hub, is_process_hub)| {
                let thread_hub = unsafe { &mut *thread_hub.get() };
                std::mem::swap(thread_hub, &mut hub);
                if was_process_hub {
                    is_process_hub.set(true);
                }
                hub
            }))
        } else {
            None
        }
    }
}

impl Drop for SwitchGuard {
    fn drop(&mut self) {
        let _ = self.swap();
    }
}

#[derive(Debug)]
pub(crate) struct HubImpl {
    pub(crate) stack: Arc<RwLock<Stack>>,
}

impl HubImpl {
    pub(crate) fn with<F: FnOnce(&Stack) -> R, R>(&self, f: F) -> R {
        let guard = self.stack.read().unwrap_or_else(PoisonError::into_inner);
        f(&guard)
    }

    pub(crate) fn with_mut<F: FnOnce(&mut Stack) -> R, R>(&self, f: F) -> R {
        let mut guard = self.stack.write().unwrap_or_else(PoisonError::into_inner);
        f(&mut guard)
    }

    pub(crate) fn is_active_and_usage_safe(&self) -> bool {
        let guard = match self.stack.read() {
            Err(err) => err.into_inner(),
            Ok(guard) => guard,
        };

        guard
            .top()
            .client
            .as_ref()
            .map_or(false, |c| c.is_enabled())
    }
}

impl Hub {
    /// Creates a new hub from the given client and scope.
    pub fn new(client: Option<Arc<Client>>, scope: Arc<Scope>) -> Hub {
        Hub {
            inner: HubImpl {
                stack: Arc::new(RwLock::new(Stack::from_client_and_scope(client, scope))),
            },
            last_event_id: RwLock::new(None),
        }
    }

    /// Creates a new hub based on the top scope of the given hub.
    pub fn new_from_top<H: AsRef<Hub>>(other: H) -> Hub {
        let hub = other.as_ref();
        hub.inner.with(|stack| {
            let top = stack.top();
            Hub::new(top.client.clone(), top.scope.clone())
        })
    }

    /// Returns the current, thread-local hub.
    ///
    /// Invoking this will return the current thread-local hub.  The first
    /// time it is called on a thread, a new thread-local hub will be
    /// created based on the topmost scope of the hub on the main thread as
    /// returned by [`Hub::main`].  If the main thread did not yet have a
    /// hub it will be created when invoking this function.
    ///
    /// To have control over which hub is installed as the current
    /// thread-local hub, use [`Hub::run`].
    ///
    /// This method is unavailable if the client implementation is disabled.
    /// When using the minimal API set use [`Hub::with_active`] instead.
    pub fn current() -> Arc<Hub> {
        Hub::with(Arc::clone)
    }

    /// Returns the main thread's hub.
    ///
    /// This is similar to [`Hub::current`] but instead of picking the
    /// current thread's hub it returns the main thread's hub instead.
    pub fn main() -> Arc<Hub> {
        PROCESS_HUB.0.clone()
    }

    /// Invokes the callback with the default hub.
    ///
    /// This is a slightly more efficient version than [`Hub::current`] and
    /// also unavailable in minimal mode.
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Arc<Hub>) -> R,
    {
        THREAD_HUB.with(|(hub, is_process_hub)| {
            if is_process_hub.get() {
                f(&PROCESS_HUB.0)
            } else {
                f(unsafe { &*hub.get() })
            }
        })
    }

    /// Binds a hub to the current thread for the duration of the call.
    ///
    /// During the execution of `f` the given hub will be installed as the
    /// thread-local hub.  So any call to [`Hub::current`] during this time
    /// will return the provided hub.
    ///
    /// Once the function is finished executing, including after it
    /// paniced, the original hub is re-installed if one was present.
    pub fn run<F: FnOnce() -> R, R>(hub: Arc<Hub>, f: F) -> R {
        let _guard = SwitchGuard::new(hub);
        f()
    }

    /// Returns the currently bound client.
    pub fn client(&self) -> Option<Arc<Client>> {
        self.inner.with(|stack| stack.top().client.clone())
    }

    /// Binds a new client to the hub.
    pub fn bind_client(&self, client: Option<Arc<Client>>) {
        self.inner.with_mut(|stack| {
            stack.top_mut().client = client;
        })
    }

    pub(crate) fn is_active_and_usage_safe(&self) -> bool {
        self.inner.is_active_and_usage_safe()
    }

    pub(crate) fn with_current_scope<F: FnOnce(&Scope) -> R, R>(&self, f: F) -> R {
        self.inner.with(|stack| f(&stack.top().scope))
    }

    pub(crate) fn with_current_scope_mut<F: FnOnce(&mut Scope) -> R, R>(&self, f: F) -> R {
        self.inner
            .with_mut(|stack| f(Arc::make_mut(&mut stack.top_mut().scope)))
    }
}
