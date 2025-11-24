//! Execution-scoped context propagation.
//!
//! The `context` module provides mechanisms for propagating values across API boundaries and between
//! logically associated execution units. It enables cross-cutting concerns to access their data in-process
//! using a shared context object.
//!
//! # Main Types
//!
//! - [`Context`]: An immutable, execution-scoped collection of values.
//!

use crate::otel_warn;
#[cfg(feature = "trace")]
use crate::trace::context::SynchronizedSpan;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

#[cfg(feature = "futures")]
mod future_ext;

#[cfg(feature = "futures")]
pub use future_ext::{FutureExt, WithContext};

thread_local! {
    static CURRENT_CONTEXT: RefCell<ContextStack> = RefCell::new(ContextStack::default());
}

/// An execution-scoped collection of values.
///
/// A [`Context`] is a propagation mechanism which carries execution-scoped
/// values across API boundaries and between logically associated execution
/// units. Cross-cutting concerns access their data in-process using the same
/// shared context object.
///
/// [`Context`]s are immutable, and their write operations result in the creation
/// of a new context containing the original values and the new specified values.
///
/// ## Context state
///
/// Concerns can create and retrieve their local state in the current execution
/// state represented by a context through the [`get`] and [`with_value`]
/// methods. It is recommended to use application-specific types when storing new
/// context values to avoid unintentionally overwriting existing state.
///
/// ## Managing the current context
///
/// Contexts can be associated with the caller's current execution unit on a
/// given thread via the [`attach`] method, and previous contexts can be restored
/// by dropping the returned [`ContextGuard`]. Context can be nested, and will
/// restore their parent outer context when detached on drop. To access the
/// values of the context, a snapshot can be created via the [`Context::current`]
/// method.
///
/// [`Context::current`]: Context::current()
/// [`get`]: Context::get()
/// [`with_value`]: Context::with_value()
/// [`attach`]: Context::attach()
///
/// # Examples
///
/// ```
/// use opentelemetry::Context;
///
/// // Application-specific `a` and `b` values
/// #[derive(Debug, PartialEq)]
/// struct ValueA(&'static str);
/// #[derive(Debug, PartialEq)]
/// struct ValueB(u64);
///
/// let _outer_guard = Context::new().with_value(ValueA("a")).attach();
///
/// // Only value a has been set
/// let current = Context::current();
/// assert_eq!(current.get::<ValueA>(), Some(&ValueA("a")));
/// assert_eq!(current.get::<ValueB>(), None);
///
/// {
///     let _inner_guard = Context::current_with_value(ValueB(42)).attach();
///     // Both values are set in inner context
///     let current = Context::current();
///     assert_eq!(current.get::<ValueA>(), Some(&ValueA("a")));
///     assert_eq!(current.get::<ValueB>(), Some(&ValueB(42)));
/// }
///
/// // Resets to only the `a` value when inner guard is dropped
/// let current = Context::current();
/// assert_eq!(current.get::<ValueA>(), Some(&ValueA("a")));
/// assert_eq!(current.get::<ValueB>(), None);
/// ```
#[derive(Clone, Default)]
pub struct Context {
    #[cfg(feature = "trace")]
    pub(crate) span: Option<Arc<SynchronizedSpan>>,
    entries: Option<Arc<EntryMap>>,
    suppress_telemetry: bool,
}

type EntryMap = HashMap<TypeId, Arc<dyn Any + Sync + Send>, BuildHasherDefault<IdHasher>>;

impl Context {
    /// Creates an empty `Context`.
    ///
    /// The context is initially created with a capacity of 0, so it will not
    /// allocate. Use [`with_value`] to create a new context that has entries.
    ///
    /// [`with_value`]: Context::with_value()
    pub fn new() -> Self {
        Context::default()
    }

    /// Returns an immutable snapshot of the current thread's context.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    ///
    /// fn do_work() {
    ///     assert_eq!(Context::current().get(), Some(&ValueA("a")));
    /// }
    ///
    /// let _guard = Context::new().with_value(ValueA("a")).attach();
    /// do_work()
    /// ```
    pub fn current() -> Self {
        Self::map_current(|cx| cx.clone())
    }

    /// Applies a function to the current context returning its value.
    ///
    /// This can be used to build higher performing algebraic expressions for
    /// optionally creating a new context without the overhead of cloning the
    /// current one and dropping it.
    ///
    /// Note: This function will panic if you attempt to attach another context
    /// while the current one is still borrowed.
    pub fn map_current<T>(f: impl FnOnce(&Context) -> T) -> T {
        CURRENT_CONTEXT.with(|cx| cx.borrow().map_current_cx(f))
    }

    /// Returns a clone of the current thread's context with the given value.
    ///
    /// This is a more efficient form of `Context::current().with_value(value)`
    /// as it avoids the intermediate context clone.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// // Given some value types defined in your application
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    /// #[derive(Debug, PartialEq)]
    /// struct ValueB(u64);
    ///
    /// // You can create and attach context with the first value set to "a"
    /// let _guard = Context::new().with_value(ValueA("a")).attach();
    ///
    /// // And create another context based on the fist with a new value
    /// let all_current_and_b = Context::current_with_value(ValueB(42));
    ///
    /// // The second context now contains all the current values and the addition
    /// assert_eq!(all_current_and_b.get::<ValueA>(), Some(&ValueA("a")));
    /// assert_eq!(all_current_and_b.get::<ValueB>(), Some(&ValueB(42)));
    /// ```
    pub fn current_with_value<T: 'static + Send + Sync>(value: T) -> Self {
        Self::map_current(|cx| cx.with_value(value))
    }

    /// Returns a reference to the entry for the corresponding value type.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// // Given some value types defined in your application
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    /// #[derive(Debug, PartialEq)]
    /// struct MyUser();
    ///
    /// let cx = Context::new().with_value(ValueA("a"));
    ///
    /// // Values can be queried by type
    /// assert_eq!(cx.get::<ValueA>(), Some(&ValueA("a")));
    ///
    /// // And return none if not yet set
    /// assert_eq!(cx.get::<MyUser>(), None);
    /// ```
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.entries
            .as_ref()?
            .get(&TypeId::of::<T>())?
            .downcast_ref()
    }

    /// Returns a copy of the context with the new value included.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// // Given some value types defined in your application
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    /// #[derive(Debug, PartialEq)]
    /// struct ValueB(u64);
    ///
    /// // You can create a context with the first value set to "a"
    /// let cx_with_a = Context::new().with_value(ValueA("a"));
    ///
    /// // And create another context based on the fist with a new value
    /// let cx_with_a_and_b = cx_with_a.with_value(ValueB(42));
    ///
    /// // The first context is still available and unmodified
    /// assert_eq!(cx_with_a.get::<ValueA>(), Some(&ValueA("a")));
    /// assert_eq!(cx_with_a.get::<ValueB>(), None);
    ///
    /// // The second context now contains both values
    /// assert_eq!(cx_with_a_and_b.get::<ValueA>(), Some(&ValueA("a")));
    /// assert_eq!(cx_with_a_and_b.get::<ValueB>(), Some(&ValueB(42)));
    /// ```
    pub fn with_value<T: 'static + Send + Sync>(&self, value: T) -> Self {
        let entries = if let Some(current_entries) = &self.entries {
            let mut inner_entries = (**current_entries).clone();
            inner_entries.insert(TypeId::of::<T>(), Arc::new(value));
            Some(Arc::new(inner_entries))
        } else {
            let mut entries = EntryMap::default();
            entries.insert(TypeId::of::<T>(), Arc::new(value));
            Some(Arc::new(entries))
        };
        Context {
            entries,
            #[cfg(feature = "trace")]
            span: self.span.clone(),
            suppress_telemetry: self.suppress_telemetry,
        }
    }

    /// Replaces the current context on this thread with this context.
    ///
    /// Dropping the returned [`ContextGuard`] will reset the current context to the
    /// previous value.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    ///
    /// let my_cx = Context::new().with_value(ValueA("a"));
    ///
    /// // Set the current thread context
    /// let cx_guard = my_cx.attach();
    /// assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA("a")));
    ///
    /// // Drop the guard to restore the previous context
    /// drop(cx_guard);
    /// assert_eq!(Context::current().get::<ValueA>(), None);
    /// ```
    ///
    /// Guards do not need to be explicitly dropped:
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    ///
    /// fn my_function() -> String {
    ///     // attach a context the duration of this function.
    ///     let my_cx = Context::new().with_value(ValueA("a"));
    ///     // NOTE: a variable name after the underscore is **required** or rust
    ///     // will drop the guard, restoring the previous context _immediately_.
    ///     let _guard = my_cx.attach();
    ///
    ///     // anything happening in functions we call can still access my_cx...
    ///     my_other_function();
    ///
    ///     // returning from the function drops the guard, exiting the span.
    ///     return "Hello world".to_owned();
    /// }
    ///
    /// fn my_other_function() {
    ///     // ...
    /// }
    /// ```
    /// Sub-scopes may be created to limit the duration for which the span is
    /// entered:
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// #[derive(Debug, PartialEq)]
    /// struct ValueA(&'static str);
    ///
    /// let my_cx = Context::new().with_value(ValueA("a"));
    ///
    /// {
    ///     let _guard = my_cx.attach();
    ///
    ///     // the current context can access variables in
    ///     assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA("a")));
    ///
    ///     // exiting the scope drops the guard, detaching the context.
    /// }
    ///
    /// // this is back in the default empty context
    /// assert_eq!(Context::current().get::<ValueA>(), None);
    /// ```
    pub fn attach(self) -> ContextGuard {
        let cx_id = CURRENT_CONTEXT.with(|cx| cx.borrow_mut().push(self));

        ContextGuard {
            cx_pos: cx_id,
            _marker: PhantomData,
        }
    }

    /// Returns whether telemetry is suppressed in this context.
    #[inline]
    pub fn is_telemetry_suppressed(&self) -> bool {
        self.suppress_telemetry
    }

    /// Returns a new context with telemetry suppression enabled.
    pub fn with_telemetry_suppressed(&self) -> Self {
        Context {
            entries: self.entries.clone(),
            #[cfg(feature = "trace")]
            span: self.span.clone(),
            suppress_telemetry: true,
        }
    }

    /// Enters a scope where telemetry is suppressed.
    ///
    /// This method is specifically designed for OpenTelemetry components (like Exporters,
    /// Processors etc.) to prevent generating recursive or self-referential
    /// telemetry data when performing their own operations.
    ///
    /// Without suppression, we have a telemetry-induced-telemetry situation
    /// where, operations like exporting telemetry could generate new telemetry
    /// about the export process itself, potentially causing:
    /// - Infinite telemetry feedback loops
    /// - Excessive resource consumption
    ///
    /// This method:
    /// 1. Takes the current context
    /// 2. Creates a new context from current, with `suppress_telemetry` set to `true`
    /// 3. Attaches it to the current thread
    /// 4. Returns a guard that restores the previous context when dropped
    ///
    /// OTel SDK components would check `is_current_telemetry_suppressed()` before
    /// generating new telemetry, but not end users.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Context;
    ///
    /// // Example: Inside an exporter's implementation
    /// fn example_export_function() {
    ///     // Prevent telemetry-generating operations from creating more telemetry
    ///     let _guard = Context::enter_telemetry_suppressed_scope();
    ///     
    ///     // Verify suppression is active
    ///     assert_eq!(Context::is_current_telemetry_suppressed(), true);
    ///     
    ///     // Here you would normally perform operations that might generate telemetry
    ///     // but now they won't because the context has suppression enabled
    /// }
    ///
    /// // Demonstrate the function
    /// example_export_function();
    /// ```
    pub fn enter_telemetry_suppressed_scope() -> ContextGuard {
        Self::map_current(|cx| cx.with_telemetry_suppressed()).attach()
    }

    /// Returns whether telemetry is suppressed in the current context.
    ///
    /// This method is used by OpenTelemetry components to determine whether they should
    /// generate new telemetry in the current execution context. It provides a performant
    /// way to check the suppression state.
    ///
    /// End-users generally should not use this method directly, as it is primarily intended for
    /// OpenTelemetry SDK components.
    ///
    ///
    #[inline]
    pub fn is_current_telemetry_suppressed() -> bool {
        Self::map_current(|cx| cx.is_telemetry_suppressed())
    }

    #[cfg(feature = "trace")]
    pub(crate) fn current_with_synchronized_span(value: SynchronizedSpan) -> Self {
        Self::map_current(|cx| Context {
            span: Some(Arc::new(value)),
            entries: cx.entries.clone(),
            suppress_telemetry: cx.suppress_telemetry,
        })
    }

    #[cfg(feature = "trace")]
    pub(crate) fn with_synchronized_span(&self, value: SynchronizedSpan) -> Self {
        Context {
            span: Some(Arc::new(value)),
            entries: self.entries.clone(),
            suppress_telemetry: self.suppress_telemetry,
        }
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("Context");

        #[cfg(feature = "trace")]
        let mut entries = self.entries.as_ref().map_or(0, |e| e.len());
        #[cfg(feature = "trace")]
        {
            if let Some(span) = &self.span {
                dbg.field("span", &span.span_context());
                entries += 1;
            } else {
                dbg.field("span", &"None");
            }
        }
        #[cfg(not(feature = "trace"))]
        let entries = self.entries.as_ref().map_or(0, |e| e.len());

        dbg.field("entries count", &entries)
            .field("suppress_telemetry", &self.suppress_telemetry)
            .finish()
    }
}

/// A guard that resets the current context to the prior context when dropped.
#[derive(Debug)]
pub struct ContextGuard {
    // The position of the context in the stack. This is used to pop the context.
    cx_pos: u16,
    // Ensure this type is !Send as it relies on thread locals
    _marker: PhantomData<*const ()>,
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        let id = self.cx_pos;
        if id > ContextStack::BASE_POS && id < ContextStack::MAX_POS {
            CURRENT_CONTEXT.with(|context_stack| context_stack.borrow_mut().pop_id(id));
        }
    }
}

/// With TypeIds as keys, there's no need to hash them. They are already hashes
/// themselves, coming from the compiler. The IdHasher holds the u64 of
/// the TypeId, and then returns it, instead of doing any bit fiddling.
#[derive(Clone, Default, Debug)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn write(&mut self, _: &[u8]) {
        unreachable!("TypeId calls write_u64");
    }

    #[inline]
    fn write_u64(&mut self, id: u64) {
        self.0 = id;
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// A stack for keeping track of the [`Context`] instances that have been attached
/// to a thread.
///
/// The stack allows for popping of contexts by position, which is used to do out
/// of order dropping of [`ContextGuard`] instances. Only when the top of the
/// stack is popped, the topmost [`Context`] is actually restored.
///
/// The stack relies on the fact that it is thread local and that the
/// [`ContextGuard`] instances that are constructed using ids from it can't be
/// moved to other threads. That means that the ids are always valid and that
/// they are always within the bounds of the stack.
struct ContextStack {
    /// This is the current [`Context`] that is active on this thread, and the top
    /// of the [`ContextStack`]. It is always present, and if the `stack` is empty
    /// it's an empty [`Context`].
    ///
    /// Having this here allows for fast access to the current [`Context`].
    current_cx: Context,
    /// A `stack` of the other contexts that have been attached to the thread.
    stack: Vec<Option<Context>>,
    /// Ensure this type is !Send as it relies on thread locals
    _marker: PhantomData<*const ()>,
}

impl ContextStack {
    const BASE_POS: u16 = 0;
    const MAX_POS: u16 = u16::MAX;
    const INITIAL_CAPACITY: usize = 8;

    #[inline(always)]
    fn push(&mut self, cx: Context) -> u16 {
        // The next id is the length of the `stack`, plus one since we have the
        // top of the [`ContextStack`] as the `current_cx`.
        let next_id = self.stack.len() + 1;
        if next_id < ContextStack::MAX_POS.into() {
            let current_cx = std::mem::replace(&mut self.current_cx, cx);
            self.stack.push(Some(current_cx));
            next_id as u16
        } else {
            // This is an overflow, log it and ignore it.
            otel_warn!(
                name: "Context.AttachFailed",
                message = format!("Too many contexts. Max limit is {}. \
                  Context::current() remains unchanged as this attach failed. \
                  Dropping the returned ContextGuard will have no impact on Context::current().",
                  ContextStack::MAX_POS)
            );
            ContextStack::MAX_POS
        }
    }

    #[inline(always)]
    fn pop_id(&mut self, pos: u16) {
        if pos == ContextStack::BASE_POS || pos == ContextStack::MAX_POS {
            // The empty context is always at the bottom of the [`ContextStack`]
            // and cannot be popped, and the overflow position is invalid, so do
            // nothing.
            otel_warn!(
                name: "Context.OutOfOrderDrop",
                position = pos,
                message = if pos == ContextStack::BASE_POS {
                    "Attempted to pop the base context which is not allowed"
                } else {
                    "Attempted to pop the overflow position which is not allowed"
                }
            );
            return;
        }
        let len: u16 = self.stack.len() as u16;
        // Are we at the top of the [`ContextStack`]?
        if pos == len {
            // Shrink the stack if possible to clear out any out of order pops.
            while let Some(None) = self.stack.last() {
                _ = self.stack.pop();
            }
            // Restore the previous context. This will always happen since the
            // empty context is always at the bottom of the stack if the
            // [`ContextStack`] is not empty.
            if let Some(Some(next_cx)) = self.stack.pop() {
                self.current_cx = next_cx;
            }
        } else {
            // This is an out of order pop.
            if pos >= len {
                // This is an invalid id, ignore it.
                otel_warn!(
                    name: "Context.PopOutOfBounds",
                    position = pos,
                    stack_length = len,
                    message = "Attempted to pop beyond the end of the context stack"
                );
                return;
            }
            // Clear out the entry at the given id.
            _ = self.stack[pos as usize].take();
        }
    }

    #[inline(always)]
    fn map_current_cx<T>(&self, f: impl FnOnce(&Context) -> T) -> T {
        f(&self.current_cx)
    }
}

impl Default for ContextStack {
    fn default() -> Self {
        ContextStack {
            current_cx: Context::default(),
            stack: Vec::with_capacity(ContextStack::INITIAL_CAPACITY),
            _marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[derive(Debug, PartialEq)]
    struct ValueA(u64);
    #[derive(Debug, PartialEq)]
    struct ValueB(u64);

    #[test]
    fn context_immutable() {
        // start with Current, which should be an empty context
        let cx = Context::current();
        assert_eq!(cx.get::<ValueA>(), None);
        assert_eq!(cx.get::<ValueB>(), None);

        // with_value should return a new context,
        // leaving the original context unchanged
        let cx_new = cx.with_value(ValueA(1));

        // cx should be unchanged
        assert_eq!(cx.get::<ValueA>(), None);
        assert_eq!(cx.get::<ValueB>(), None);

        // cx_new should contain the new value
        assert_eq!(cx_new.get::<ValueA>(), Some(&ValueA(1)));

        // cx_new should be unchanged
        let cx_newer = cx_new.with_value(ValueB(1));

        // Cx and cx_new are unchanged
        assert_eq!(cx.get::<ValueA>(), None);
        assert_eq!(cx.get::<ValueB>(), None);
        assert_eq!(cx_new.get::<ValueA>(), Some(&ValueA(1)));
        assert_eq!(cx_new.get::<ValueB>(), None);

        // cx_newer should contain both values
        assert_eq!(cx_newer.get::<ValueA>(), Some(&ValueA(1)));
        assert_eq!(cx_newer.get::<ValueB>(), Some(&ValueB(1)));
    }

    #[test]
    fn nested_contexts() {
        let _outer_guard = Context::new().with_value(ValueA(1)).attach();

        // Only value `a` is set
        let current = Context::current();
        assert_eq!(current.get(), Some(&ValueA(1)));
        assert_eq!(current.get::<ValueB>(), None);

        {
            let _inner_guard = Context::current_with_value(ValueB(42)).attach();
            // Both values are set in inner context
            let current = Context::current();
            assert_eq!(current.get(), Some(&ValueA(1)));
            assert_eq!(current.get(), Some(&ValueB(42)));

            assert!(Context::map_current(|cx| {
                assert_eq!(cx.get(), Some(&ValueA(1)));
                assert_eq!(cx.get(), Some(&ValueB(42)));
                true
            }));
        }

        // Resets to only value `a` when inner guard is dropped
        let current = Context::current();
        assert_eq!(current.get(), Some(&ValueA(1)));
        assert_eq!(current.get::<ValueB>(), None);

        assert!(Context::map_current(|cx| {
            assert_eq!(cx.get(), Some(&ValueA(1)));
            assert_eq!(cx.get::<ValueB>(), None);
            true
        }));
    }

    #[test]
    fn overlapping_contexts() {
        let outer_guard = Context::new().with_value(ValueA(1)).attach();

        // Only value `a` is set
        let current = Context::current();
        assert_eq!(current.get(), Some(&ValueA(1)));
        assert_eq!(current.get::<ValueB>(), None);

        let inner_guard = Context::current_with_value(ValueB(42)).attach();
        // Both values are set in inner context
        let current = Context::current();
        assert_eq!(current.get(), Some(&ValueA(1)));
        assert_eq!(current.get(), Some(&ValueB(42)));

        assert!(Context::map_current(|cx| {
            assert_eq!(cx.get(), Some(&ValueA(1)));
            assert_eq!(cx.get(), Some(&ValueB(42)));
            true
        }));

        drop(outer_guard);

        // `inner_guard` is still alive so both `ValueA` and `ValueB` should still be accessible
        let current = Context::current();
        assert_eq!(current.get(), Some(&ValueA(1)));
        assert_eq!(current.get(), Some(&ValueB(42)));

        drop(inner_guard);

        // Both guards are dropped and neither value should be accessible.
        let current = Context::current();
        assert_eq!(current.get::<ValueA>(), None);
        assert_eq!(current.get::<ValueB>(), None);
    }

    #[test]
    fn too_many_contexts() {
        let mut guards: Vec<ContextGuard> = Vec::with_capacity(ContextStack::MAX_POS as usize);
        let stack_max_pos = ContextStack::MAX_POS as u64;
        // Fill the stack up until the last position
        for i in 1..stack_max_pos {
            let cx_guard = Context::current().with_value(ValueB(i)).attach();
            assert_eq!(Context::current().get(), Some(&ValueB(i)));
            assert_eq!(cx_guard.cx_pos, i as u16);
            guards.push(cx_guard);
        }
        // Let's overflow the stack a couple of times
        for _ in 0..16 {
            let cx_guard = Context::current().with_value(ValueA(1)).attach();
            assert_eq!(cx_guard.cx_pos, ContextStack::MAX_POS);
            assert_eq!(Context::current().get::<ValueA>(), None);
            assert_eq!(Context::current().get(), Some(&ValueB(stack_max_pos - 1)));
            guards.push(cx_guard);
        }
        // Drop the overflow contexts
        for _ in 0..16 {
            guards.pop();
            assert_eq!(Context::current().get::<ValueA>(), None);
            assert_eq!(Context::current().get(), Some(&ValueB(stack_max_pos - 1)));
        }
        // Drop one more so we can add a new one
        guards.pop();
        assert_eq!(Context::current().get::<ValueA>(), None);
        assert_eq!(Context::current().get(), Some(&ValueB(stack_max_pos - 2)));
        // Push a new context and see that it works
        let cx_guard = Context::current().with_value(ValueA(2)).attach();
        assert_eq!(cx_guard.cx_pos, ContextStack::MAX_POS - 1);
        assert_eq!(Context::current().get(), Some(&ValueA(2)));
        assert_eq!(Context::current().get(), Some(&ValueB(stack_max_pos - 2)));
        guards.push(cx_guard);
        // Let's overflow the stack a couple of times again
        for _ in 0..16 {
            let cx_guard = Context::current().with_value(ValueA(1)).attach();
            assert_eq!(cx_guard.cx_pos, ContextStack::MAX_POS);
            assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA(2)));
            assert_eq!(Context::current().get(), Some(&ValueB(stack_max_pos - 2)));
            guards.push(cx_guard);
        }
    }

    /// Tests that a new ContextStack is created with the correct initial capacity.
    #[test]
    fn test_initial_capacity() {
        let stack = ContextStack::default();
        assert_eq!(stack.stack.capacity(), ContextStack::INITIAL_CAPACITY);
    }

    /// Tests that map_current_cx correctly accesses the current context.
    #[test]
    fn test_map_current_cx() {
        let mut stack = ContextStack::default();
        let test_value = ValueA(42);
        stack.current_cx = Context::new().with_value(test_value);

        let result = stack.map_current_cx(|cx| {
            assert_eq!(cx.get::<ValueA>(), Some(&ValueA(42)));
            true
        });
        assert!(result);
    }

    /// Tests popping contexts in non-sequential order.
    #[test]
    fn test_pop_id_out_of_order() {
        let mut stack = ContextStack::default();

        // Push three contexts
        let cx1 = Context::new().with_value(ValueA(1));
        let cx2 = Context::new().with_value(ValueA(2));
        let cx3 = Context::new().with_value(ValueA(3));

        let id1 = stack.push(cx1);
        let id2 = stack.push(cx2);
        let id3 = stack.push(cx3);

        // Pop middle context first - should not affect current context
        stack.pop_id(id2);
        assert_eq!(stack.current_cx.get::<ValueA>(), Some(&ValueA(3)));
        assert_eq!(stack.stack.len(), 3); // Length unchanged for middle pops

        // Pop last context - should restore previous valid context
        stack.pop_id(id3);
        assert_eq!(stack.current_cx.get::<ValueA>(), Some(&ValueA(1)));
        assert_eq!(stack.stack.len(), 1);

        // Pop first context - should restore to empty state
        stack.pop_id(id1);
        assert_eq!(stack.current_cx.get::<ValueA>(), None);
        assert_eq!(stack.stack.len(), 0);
    }

    /// Tests edge cases in context stack operations. IRL these should log
    /// warnings, and definitely not panic.
    #[test]
    fn test_pop_id_edge_cases() {
        let mut stack = ContextStack::default();

        // Test popping BASE_POS - should be no-op
        stack.pop_id(ContextStack::BASE_POS);
        assert_eq!(stack.stack.len(), 0);

        // Test popping MAX_POS - should be no-op
        stack.pop_id(ContextStack::MAX_POS);
        assert_eq!(stack.stack.len(), 0);

        // Test popping invalid position - should be no-op
        stack.pop_id(1000);
        assert_eq!(stack.stack.len(), 0);

        // Test popping from empty stack - should be safe
        stack.pop_id(1);
        assert_eq!(stack.stack.len(), 0);
    }

    /// Tests stack behavior when reaching maximum capacity.
    /// Once we push beyond this point, we should end up with a context
    /// that points _somewhere_, but mutating it should not affect the current
    /// active context.
    #[test]
    fn test_push_overflow() {
        let mut stack = ContextStack::default();
        let max_pos = ContextStack::MAX_POS as usize;

        // Fill stack up to max position
        for i in 0..max_pos {
            let cx = Context::new().with_value(ValueA(i as u64));
            let id = stack.push(cx);
            assert_eq!(id, (i + 1) as u16);
        }

        // Try to push beyond capacity
        let cx = Context::new().with_value(ValueA(max_pos as u64));
        let id = stack.push(cx);
        assert_eq!(id, ContextStack::MAX_POS);

        // Verify current context remains unchanged after overflow
        assert_eq!(
            stack.current_cx.get::<ValueA>(),
            Some(&ValueA((max_pos - 2) as u64))
        );
    }

    /// Tests that:
    /// 1. Parent context values are properly propagated to async operations
    /// 2. Values added during async operations do not affect parent context
    #[tokio::test]
    async fn test_async_context_propagation() {
        // A nested async operation we'll use to test propagation
        async fn nested_operation() {
            // Verify we can see the parent context's value
            assert_eq!(
                Context::current().get::<ValueA>(),
                Some(&ValueA(42)),
                "Parent context value should be available in async operation"
            );

            // Create new context
            let cx_with_both = Context::current()
                .with_value(ValueA(43)) // override ValueA
                .with_value(ValueB(24)); // Add new ValueB

            // Run nested async operation with both values
            async {
                // Verify both values are available
                assert_eq!(
                    Context::current().get::<ValueA>(),
                    Some(&ValueA(43)),
                    "Parent value should still be available after adding new value"
                );
                assert_eq!(
                    Context::current().get::<ValueB>(),
                    Some(&ValueB(24)),
                    "New value should be available in async operation"
                );

                // Do some async work to simulate real-world scenario
                sleep(Duration::from_millis(10)).await;

                // Values should still be available after async work
                assert_eq!(
                    Context::current().get::<ValueA>(),
                    Some(&ValueA(43)),
                    "Parent value should persist across await points"
                );
                assert_eq!(
                    Context::current().get::<ValueB>(),
                    Some(&ValueB(24)),
                    "New value should persist across await points"
                );
            }
            .with_context(cx_with_both)
            .await;
        }

        // Set up initial context with ValueA
        let parent_cx = Context::new().with_value(ValueA(42));

        // Create and run async operation with the parent context explicitly propagated
        nested_operation().with_context(parent_cx.clone()).await;

        // After async operation completes:
        // 1. Parent context should be unchanged
        assert_eq!(
            parent_cx.get::<ValueA>(),
            Some(&ValueA(42)),
            "Parent context should be unchanged"
        );
        assert_eq!(
            parent_cx.get::<ValueB>(),
            None,
            "Parent context should not see values added in async operation"
        );

        // 2. Current context should be back to default
        assert_eq!(
            Context::current().get::<ValueA>(),
            None,
            "Current context should be back to default"
        );
        assert_eq!(
            Context::current().get::<ValueB>(),
            None,
            "Current context should not have async operation's values"
        );
    }

    ///
    /// Tests that unnatural parent->child relationships in nested async
    /// operations behave properly.
    ///
    #[tokio::test]
    async fn test_out_of_order_context_detachment_futures() {
        // This function returns a future, but doesn't await it
        // It will complete before the future that it creates.
        async fn create_a_future() -> impl std::future::Future<Output = ()> {
            // Create a future that will do some work, referencing our current
            // context, but don't await it.
            async {
                assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA(42)));

                // Longer work
                sleep(Duration::from_millis(50)).await;
            }
            .with_context(Context::current())
        }

        // Create our base context
        let parent_cx = Context::new().with_value(ValueA(42));

        // await our nested function, which will create and detach a context
        let future = create_a_future().with_context(parent_cx).await;

        // Execute the future. The future that created it is long gone, but this shouldn't
        // cause issues.
        let _a = future.await;

        // Nothing terrible (e.g., panics!) should happen, and we should definitely not have any
        // values attached to our current context that were set in the nested operations.
        assert_eq!(Context::current().get::<ValueA>(), None);
        assert_eq!(Context::current().get::<ValueB>(), None);
    }

    #[test]
    fn test_is_telemetry_suppressed() {
        // Default context has suppression disabled
        let cx = Context::new();
        assert!(!cx.is_telemetry_suppressed());

        // With suppression enabled
        let suppressed = cx.with_telemetry_suppressed();
        assert!(suppressed.is_telemetry_suppressed());
    }

    #[test]
    fn test_with_telemetry_suppressed() {
        // Start with a normal context
        let cx = Context::new();
        assert!(!cx.is_telemetry_suppressed());

        // Create a suppressed context
        let suppressed = cx.with_telemetry_suppressed();

        // Original should remain unchanged
        assert!(!cx.is_telemetry_suppressed());

        // New context should be suppressed
        assert!(suppressed.is_telemetry_suppressed());

        // Test with values to ensure they're preserved
        let cx_with_value = cx.with_value(ValueA(42));
        let suppressed_with_value = cx_with_value.with_telemetry_suppressed();

        assert!(!cx_with_value.is_telemetry_suppressed());
        assert!(suppressed_with_value.is_telemetry_suppressed());
        assert_eq!(suppressed_with_value.get::<ValueA>(), Some(&ValueA(42)));
    }

    #[test]
    fn test_enter_telemetry_suppressed_scope() {
        // Ensure we start with a clean context
        let _reset_guard = Context::new().attach();

        // Default context should not be suppressed
        assert!(!Context::is_current_telemetry_suppressed());

        // Add an entry to the current context
        let cx_with_value = Context::current().with_value(ValueA(42));
        let _guard_with_value = cx_with_value.attach();

        // Verify the entry is present and context is not suppressed
        assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA(42)));
        assert!(!Context::is_current_telemetry_suppressed());

        // Enter a suppressed scope
        {
            let _guard = Context::enter_telemetry_suppressed_scope();

            // Verify suppression is active and the entry is still present
            assert!(Context::is_current_telemetry_suppressed());
            assert!(Context::current().is_telemetry_suppressed());
            assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA(42)));
        }

        // After guard is dropped, should be back to unsuppressed and entry should still be present
        assert!(!Context::is_current_telemetry_suppressed());
        assert!(!Context::current().is_telemetry_suppressed());
        assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA(42)));
    }

    #[test]
    fn test_nested_suppression_scopes() {
        // Ensure we start with a clean context
        let _reset_guard = Context::new().attach();

        // Default context should not be suppressed
        assert!(!Context::is_current_telemetry_suppressed());

        // First level suppression
        {
            let _outer = Context::enter_telemetry_suppressed_scope();
            assert!(Context::is_current_telemetry_suppressed());

            // Second level. This component is unaware of Suppression,
            // and just attaches a new context. Since it is from current,
            // it'll already have suppression enabled.
            {
                let _inner = Context::current().with_value(ValueA(1)).attach();
                assert!(Context::is_current_telemetry_suppressed());
                assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA(1)));
            }

            // Another scenario. This component is unaware of Suppression,
            // and just attaches a new context, not from Current. Since it is
            // not from current it will not have suppression enabled.
            {
                let _inner = Context::new().with_value(ValueA(1)).attach();
                assert!(!Context::is_current_telemetry_suppressed());
                assert_eq!(Context::current().get::<ValueA>(), Some(&ValueA(1)));
            }

            // Still suppressed after inner scope
            assert!(Context::is_current_telemetry_suppressed());
        }

        // Back to unsuppressed
        assert!(!Context::is_current_telemetry_suppressed());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_async_suppression() {
        async fn nested_operation() {
            assert!(Context::is_current_telemetry_suppressed());

            let cx_with_additional_value = Context::current().with_value(ValueB(24));

            async {
                assert_eq!(
                    Context::current().get::<ValueB>(),
                    Some(&ValueB(24)),
                    "Parent value should still be available after adding new value"
                );
                assert!(Context::is_current_telemetry_suppressed());

                // Do some async work to simulate real-world scenario
                sleep(Duration::from_millis(10)).await;

                // Values should still be available after async work
                assert_eq!(
                    Context::current().get::<ValueB>(),
                    Some(&ValueB(24)),
                    "Parent value should still be available after adding new value"
                );
                assert!(Context::is_current_telemetry_suppressed());
            }
            .with_context(cx_with_additional_value)
            .await;
        }

        // Set up suppressed context, but don't attach it to current
        let suppressed_parent = Context::new().with_telemetry_suppressed();
        // Current should not be suppressed as we haven't attached it
        assert!(!Context::is_current_telemetry_suppressed());

        // Create and run async operation with the suppressed context explicitly propagated
        nested_operation()
            .with_context(suppressed_parent.clone())
            .await;

        // After async operation completes:
        // Suppression should be active
        assert!(suppressed_parent.is_telemetry_suppressed());

        // Current should still be not suppressed
        assert!(!Context::is_current_telemetry_suppressed());
    }
}
