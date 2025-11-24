//! Storage for span data shared by multiple [`Layer`]s.
//!
//! ## Using the Span Registry
//!
//! This module provides the [`Registry`] type, a [`Subscriber`] implementation
//! which tracks per-span data and exposes it to [`Layer`]s. When a `Registry`
//! is used as the base `Subscriber` of a `Layer` stack, the
//! [`layer::Context`][ctx] type will provide methods allowing `Layer`s to
//! [look up span data][lookup] stored in the registry. While [`Registry`] is a
//! reasonable default for storing spans and events, other stores that implement
//! [`LookupSpan`] and [`Subscriber`] themselves (with [`SpanData`] implemented
//! by the per-span data they store) can be used as a drop-in replacement.
//!
//! For example, we might create a `Registry` and add multiple `Layer`s like so:
//! ```rust
//! use tracing_subscriber::{registry::Registry, Layer, prelude::*};
//! # use tracing_core::Subscriber;
//! # pub struct FooLayer {}
//! # pub struct BarLayer {}
//! # impl<S: Subscriber> Layer<S> for FooLayer {}
//! # impl<S: Subscriber> Layer<S> for BarLayer {}
//! # impl FooLayer {
//! # fn new() -> Self { Self {} }
//! # }
//! # impl BarLayer {
//! # fn new() -> Self { Self {} }
//! # }
//!
//! let subscriber = Registry::default()
//!     .with(FooLayer::new())
//!     .with(BarLayer::new());
//! ```
//!
//! If a type implementing `Layer` depends on the functionality of a `Registry`
//! implementation, it should bound its `Subscriber` type parameter with the
//! [`LookupSpan`] trait, like so:
//!
//! ```rust
//! use tracing_subscriber::{registry, Layer};
//! use tracing_core::Subscriber;
//!
//! pub struct MyLayer {
//!     // ...
//! }
//!
//! impl<S> Layer<S> for MyLayer
//! where
//!     S: Subscriber + for<'a> registry::LookupSpan<'a>,
//! {
//!     // ...
//! }
//! ```
//! When this bound is added, the `Layer` implementation will be guaranteed
//! access to the [`Context`][ctx] methods, such as [`Context::span`][lookup], that
//! require the root subscriber to be a registry.
//!
//! [`Layer`]: crate::layer::Layer
//! [`Subscriber`]: tracing_core::Subscriber
//! [ctx]: crate::layer::Context
//! [lookup]: crate::layer::Context::span()
use tracing_core::{field::FieldSet, span::Id, Metadata};

feature! {
    #![feature = "std"]
    /// A module containing a type map of span extensions.
    mod extensions;
    pub use extensions::{Extensions, ExtensionsMut};

}

feature! {
    #![all(feature = "registry", feature = "std")]

    mod sharded;
    mod stack;

    pub use sharded::Data;
    pub use sharded::Registry;

    use crate::filter::FilterId;
}

/// Provides access to stored span data.
///
/// Subscribers which store span data and associate it with span IDs should
/// implement this trait; if they do, any [`Layer`]s wrapping them can look up
/// metadata via the [`Context`] type's [`span()`] method.
///
/// [`Layer`]: super::layer::Layer
/// [`Context`]: super::layer::Context
/// [`span()`]: super::layer::Context::span
pub trait LookupSpan<'a> {
    /// The type of span data stored in this registry.
    type Data: SpanData<'a>;

    /// Returns the [`SpanData`] for a given `Id`, if it exists.
    ///
    /// <pre class="ignore" style="white-space:normal;font:inherit;">
    /// <strong>Note</strong>: users of the <code>LookupSpan</code> trait should
    /// typically call the <a href="#method.span"><code>span</code></a> method rather
    /// than this method. The <code>span</code> method is implemented by
    /// <em>calling</em> <code>span_data</code>, but returns a reference which is
    /// capable of performing more sophisiticated queries.
    /// </pre>
    ///
    fn span_data(&'a self, id: &Id) -> Option<Self::Data>;

    /// Returns a [`SpanRef`] for the span with the given `Id`, if it exists.
    ///
    /// A `SpanRef` is similar to [`SpanData`], but it allows performing
    /// additional lookups against the registryr that stores the wrapped data.
    ///
    /// In general, _users_ of the `LookupSpan` trait should use this method
    /// rather than the [`span_data`] method; while _implementors_ of this trait
    /// should only implement `span_data`.
    ///
    /// [`span_data`]: LookupSpan::span_data()
    fn span(&'a self, id: &Id) -> Option<SpanRef<'a, Self>>
    where
        Self: Sized,
    {
        let data = self.span_data(id)?;
        Some(SpanRef {
            registry: self,
            data,
            #[cfg(feature = "registry")]
            filter: FilterId::none(),
        })
    }

    /// Registers a [`Filter`] for [per-layer filtering] with this
    /// [`Subscriber`].
    ///
    /// The [`Filter`] can then use the returned [`FilterId`] to
    /// [check if it previously enabled a span][check].
    ///
    /// # Panics
    ///
    /// If this `Subscriber` does not support [per-layer filtering].
    ///
    /// [`Filter`]: crate::layer::Filter
    /// [per-layer filtering]: crate::layer::Layer#per-layer-filtering
    /// [`Subscriber`]: tracing_core::Subscriber
    /// [`FilterId`]: crate::filter::FilterId
    /// [check]: SpanData::is_enabled_for
    #[cfg(feature = "registry")]
    #[cfg_attr(docsrs, doc(cfg(feature = "registry")))]
    fn register_filter(&mut self) -> FilterId {
        panic!(
            "{} does not currently support filters",
            std::any::type_name::<Self>()
        )
    }
}

/// A stored representation of data associated with a span.
pub trait SpanData<'a> {
    /// Returns this span's ID.
    fn id(&self) -> Id;

    /// Returns a reference to the span's `Metadata`.
    fn metadata(&self) -> &'static Metadata<'static>;

    /// Returns a reference to the ID
    fn parent(&self) -> Option<&Id>;

    /// Returns a reference to this span's `Extensions`.
    ///
    /// The extensions may be used by `Layer`s to store additional data
    /// describing the span.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn extensions(&self) -> Extensions<'_>;

    /// Returns a mutable reference to this span's `Extensions`.
    ///
    /// The extensions may be used by `Layer`s to store additional data
    /// describing the span.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn extensions_mut(&self) -> ExtensionsMut<'_>;

    /// Returns `true` if this span is enabled for the [per-layer filter][plf]
    /// corresponding to the provided [`FilterId`].
    ///
    /// ## Default Implementation
    ///
    /// By default, this method assumes that the [`LookupSpan`] implementation
    /// does not support [per-layer filtering][plf], and always returns `true`.
    ///
    /// [plf]: crate::layer::Layer#per-layer-filtering
    /// [`FilterId`]: crate::filter::FilterId
    #[cfg(feature = "registry")]
    #[cfg_attr(docsrs, doc(cfg(feature = "registry")))]
    fn is_enabled_for(&self, filter: FilterId) -> bool {
        let _ = filter;
        true
    }
}

/// A reference to [span data] and the associated [registry].
///
/// This type implements all the same methods as [`SpanData`], and provides
/// additional methods for querying the registry based on values from the span.
///
/// [registry]: LookupSpan
#[derive(Debug)]
pub struct SpanRef<'a, R: LookupSpan<'a>> {
    registry: &'a R,
    data: R::Data,

    #[cfg(feature = "registry")]
    filter: FilterId,
}

/// An iterator over the parents of a span, ordered from leaf to root.
///
/// This is returned by the [`SpanRef::scope`] method.
#[derive(Debug)]
pub struct Scope<'a, R> {
    registry: &'a R,
    next: Option<Id>,

    #[cfg(all(feature = "registry", feature = "std"))]
    filter: FilterId,
}

feature! {
    #![any(feature = "alloc", feature = "std")]

    #[cfg(not(feature = "smallvec"))]
    use alloc::vec::{self, Vec};

    use core::{fmt,iter};

    /// An iterator over the parents of a span, ordered from root to leaf.
    ///
    /// This is returned by the [`Scope::from_root`] method.
    pub struct ScopeFromRoot<'a, R>
    where
        R: LookupSpan<'a>,
    {
        #[cfg(feature = "smallvec")]
        spans: iter::Rev<smallvec::IntoIter<SpanRefVecArray<'a, R>>>,
        #[cfg(not(feature = "smallvec"))]
        spans: iter::Rev<vec::IntoIter<SpanRef<'a, R>>>,
    }

    #[cfg(feature = "smallvec")]
    type SpanRefVecArray<'span, L> = [SpanRef<'span, L>; 16];

    impl<'a, R> Scope<'a, R>
    where
        R: LookupSpan<'a>,
    {
        /// Flips the order of the iterator, so that it is ordered from root to leaf.
        ///
        /// The iterator will first return the root span, then that span's immediate child,
        /// and so on until it finally returns the span that [`SpanRef::scope`] was called on.
        ///
        /// If any items were consumed from the [`Scope`] before calling this method then they
        /// will *not* be returned from the [`ScopeFromRoot`].
        ///
        /// **Note**: this will allocate if there are many spans remaining, or if the
        /// "smallvec" feature flag is not enabled.
        #[allow(clippy::wrong_self_convention)]
        pub fn from_root(self) -> ScopeFromRoot<'a, R> {
            #[cfg(feature = "smallvec")]
            type Buf<T> = smallvec::SmallVec<T>;
            #[cfg(not(feature = "smallvec"))]
            type Buf<T> = Vec<T>;
            ScopeFromRoot {
                spans: self.collect::<Buf<_>>().into_iter().rev(),
            }
        }
    }

    impl<'a, R> Iterator for ScopeFromRoot<'a, R>
    where
        R: LookupSpan<'a>,
    {
        type Item = SpanRef<'a, R>;

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            self.spans.next()
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.spans.size_hint()
        }
    }

    impl<'a, R> fmt::Debug for ScopeFromRoot<'a, R>
    where
        R: LookupSpan<'a>,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.pad("ScopeFromRoot { .. }")
        }
    }
}

impl<'a, R> Iterator for Scope<'a, R>
where
    R: LookupSpan<'a>,
{
    type Item = SpanRef<'a, R>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let curr = self.registry.span(self.next.as_ref()?)?;

            #[cfg(all(feature = "registry", feature = "std"))]
            let curr = curr.with_filter(self.filter);
            self.next = curr.data.parent().cloned();

            // If the `Scope` is filtered, check if the current span is enabled
            // by the selected filter ID.

            #[cfg(all(feature = "registry", feature = "std"))]
            {
                if !curr.is_enabled_for(self.filter) {
                    // The current span in the chain is disabled for this
                    // filter. Try its parent.
                    continue;
                }
            }

            return Some(curr);
        }
    }
}

impl<'a, R> SpanRef<'a, R>
where
    R: LookupSpan<'a>,
{
    /// Returns this span's ID.
    pub fn id(&self) -> Id {
        self.data.id()
    }

    /// Returns a static reference to the span's metadata.
    pub fn metadata(&self) -> &'static Metadata<'static> {
        self.data.metadata()
    }

    /// Returns the span's name,
    pub fn name(&self) -> &'static str {
        self.data.metadata().name()
    }

    /// Returns a list of [fields] defined by the span.
    ///
    /// [fields]: tracing_core::field
    pub fn fields(&self) -> &FieldSet {
        self.data.metadata().fields()
    }

    /// Returns a `SpanRef` describing this span's parent, or `None` if this
    /// span is the root of its trace tree.
    pub fn parent(&self) -> Option<Self> {
        let id = self.data.parent()?;
        let data = self.registry.span_data(id)?;

        #[cfg(all(feature = "registry", feature = "std"))]
        {
            // move these into mut bindings if the registry feature is enabled,
            // since they may be mutated in the loop.
            let mut data = data;
            loop {
                // Is this parent enabled by our filter?
                if data.is_enabled_for(self.filter) {
                    return Some(Self {
                        registry: self.registry,
                        filter: self.filter,
                        data,
                    });
                }

                // It's not enabled. If the disabled span has a parent, try that!
                let id = data.parent()?;
                data = self.registry.span_data(id)?;
            }
        }

        #[cfg(not(all(feature = "registry", feature = "std")))]
        Some(Self {
            registry: self.registry,
            data,
        })
    }

    /// Returns an iterator over all parents of this span, starting with this span,
    /// ordered from leaf to root.
    ///
    /// The iterator will first return the span, then the span's immediate parent,
    /// followed by that span's parent, and so on, until it reaches a root span.
    ///
    /// ```rust
    /// use tracing::{span, Subscriber};
    /// use tracing_subscriber::{
    ///     layer::{Context, Layer},
    ///     prelude::*,
    ///     registry::LookupSpan,
    /// };
    ///
    /// struct PrintingLayer;
    /// impl<S> Layer<S> for PrintingLayer
    /// where
    ///     S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    /// {
    ///     fn on_enter(&self, id: &span::Id, ctx: Context<S>) {
    ///         let span = ctx.span(id).unwrap();
    ///         let scope = span.scope().map(|span| span.name()).collect::<Vec<_>>();
    ///         println!("Entering span: {:?}", scope);
    ///     }
    /// }
    ///
    /// tracing::subscriber::with_default(tracing_subscriber::registry().with(PrintingLayer), || {
    ///     let _root = tracing::info_span!("root").entered();
    ///     // Prints: Entering span: ["root"]
    ///     let _child = tracing::info_span!("child").entered();
    ///     // Prints: Entering span: ["child", "root"]
    ///     let _leaf = tracing::info_span!("leaf").entered();
    ///     // Prints: Entering span: ["leaf", "child", "root"]
    /// });
    /// ```
    ///
    /// If the opposite order (from the root to this span) is desired, calling [`Scope::from_root`] on
    /// the returned iterator reverses the order.
    ///
    /// ```rust
    /// # use tracing::{span, Subscriber};
    /// # use tracing_subscriber::{
    /// #     layer::{Context, Layer},
    /// #     prelude::*,
    /// #     registry::LookupSpan,
    /// # };
    /// # struct PrintingLayer;
    /// impl<S> Layer<S> for PrintingLayer
    /// where
    ///     S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    /// {
    ///     fn on_enter(&self, id: &span::Id, ctx: Context<S>) {
    ///         let span = ctx.span(id).unwrap();
    ///         let scope = span.scope().from_root().map(|span| span.name()).collect::<Vec<_>>();
    ///         println!("Entering span: {:?}", scope);
    ///     }
    /// }
    ///
    /// tracing::subscriber::with_default(tracing_subscriber::registry().with(PrintingLayer), || {
    ///     let _root = tracing::info_span!("root").entered();
    ///     // Prints: Entering span: ["root"]
    ///     let _child = tracing::info_span!("child").entered();
    ///     // Prints: Entering span: ["root", "child"]
    ///     let _leaf = tracing::info_span!("leaf").entered();
    ///     // Prints: Entering span: ["root", "child", "leaf"]
    /// });
    /// ```
    pub fn scope(&self) -> Scope<'a, R> {
        Scope {
            registry: self.registry,
            next: Some(self.id()),

            #[cfg(feature = "registry")]
            filter: self.filter,
        }
    }

    /// Returns a reference to this span's `Extensions`.
    ///
    /// The extensions may be used by `Layer`s to store additional data
    /// describing the span.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn extensions(&self) -> Extensions<'_> {
        self.data.extensions()
    }

    /// Returns a mutable reference to this span's `Extensions`.
    ///
    /// The extensions may be used by `Layer`s to store additional data
    /// describing the span.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn extensions_mut(&self) -> ExtensionsMut<'_> {
        self.data.extensions_mut()
    }

    #[cfg(all(feature = "registry", feature = "std"))]
    pub(crate) fn try_with_filter(self, filter: FilterId) -> Option<Self> {
        if self.is_enabled_for(filter) {
            return Some(self.with_filter(filter));
        }

        None
    }

    #[inline]
    #[cfg(all(feature = "registry", feature = "std"))]
    pub(crate) fn is_enabled_for(&self, filter: FilterId) -> bool {
        self.data.is_enabled_for(filter)
    }

    #[inline]
    #[cfg(all(feature = "registry", feature = "std"))]
    fn with_filter(self, filter: FilterId) -> Self {
        Self { filter, ..self }
    }
}

#[cfg(all(test, feature = "registry", feature = "std"))]
mod tests {
    use crate::{
        layer::{Context, Layer},
        prelude::*,
        registry::LookupSpan,
    };
    use std::sync::{Arc, Mutex};
    use tracing::{span, Subscriber};

    #[test]
    fn spanref_scope_iteration_order() {
        let last_entered_scope = Arc::new(Mutex::new(Vec::new()));

        #[derive(Default)]
        struct PrintingLayer {
            last_entered_scope: Arc<Mutex<Vec<&'static str>>>,
        }

        impl<S> Layer<S> for PrintingLayer
        where
            S: Subscriber + for<'lookup> LookupSpan<'lookup>,
        {
            fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
                let span = ctx.span(id).unwrap();
                let scope = span.scope().map(|span| span.name()).collect::<Vec<_>>();
                *self.last_entered_scope.lock().unwrap() = scope;
            }
        }

        let _guard = tracing::subscriber::set_default(crate::registry().with(PrintingLayer {
            last_entered_scope: last_entered_scope.clone(),
        }));

        let _root = tracing::info_span!("root").entered();
        assert_eq!(&*last_entered_scope.lock().unwrap(), &["root"]);
        let _child = tracing::info_span!("child").entered();
        assert_eq!(&*last_entered_scope.lock().unwrap(), &["child", "root"]);
        let _leaf = tracing::info_span!("leaf").entered();
        assert_eq!(
            &*last_entered_scope.lock().unwrap(),
            &["leaf", "child", "root"]
        );
    }

    #[test]
    fn spanref_scope_fromroot_iteration_order() {
        let last_entered_scope = Arc::new(Mutex::new(Vec::new()));

        #[derive(Default)]
        struct PrintingLayer {
            last_entered_scope: Arc<Mutex<Vec<&'static str>>>,
        }

        impl<S> Layer<S> for PrintingLayer
        where
            S: Subscriber + for<'lookup> LookupSpan<'lookup>,
        {
            fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
                let span = ctx.span(id).unwrap();
                let scope = span
                    .scope()
                    .from_root()
                    .map(|span| span.name())
                    .collect::<Vec<_>>();
                *self.last_entered_scope.lock().unwrap() = scope;
            }
        }

        let _guard = tracing::subscriber::set_default(crate::registry().with(PrintingLayer {
            last_entered_scope: last_entered_scope.clone(),
        }));

        let _root = tracing::info_span!("root").entered();
        assert_eq!(&*last_entered_scope.lock().unwrap(), &["root"]);
        let _child = tracing::info_span!("child").entered();
        assert_eq!(&*last_entered_scope.lock().unwrap(), &["root", "child",]);
        let _leaf = tracing::info_span!("leaf").entered();
        assert_eq!(
            &*last_entered_scope.lock().unwrap(),
            &["root", "child", "leaf"]
        );
    }
}
