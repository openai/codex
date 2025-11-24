//! Template expansion context.
//!
//! # Examples
//!
//! 1. Define your context type.
//! 2. Implement [`Context`] trait (and [`Context::visit`] method) for the type.
//!     1. Get variable name by [`Visitor::var_name`] method.
//!     2. Feed the corresponding value(s) by one of `Visitor::visit_*` methods.
//!
//! Note that contexts should return consistent result across multiple visits for
//! the same variable. In other words, `Context::visit` should return the same
//! result for the same `Visitor::var_name()` during the context is borrowed.
//! If this condition is violated, the URI template processor can return
//! invalid result or panic at worst.
//!
//! ```
//! use iri_string::template::context::{Context, Visitor, ListVisitor, AssocVisitor};
//!
//! struct MyContext {
//!     name: &'static str,
//!     id: u64,
//!     tags: &'static [&'static str],
//!     children: &'static [(&'static str, usize)],
//! }
//!
//! impl Context for MyContext {
//!     fn visit<V: Visitor>(&self, visitor: V) -> V::Result {
//!         let name = visitor.var_name().as_str();
//!         match name {
//!             "name" => visitor.visit_string(self.name),
//!             "id" => visitor.visit_string(self.id),
//!             "tags" => visitor.visit_list().visit_items_and_finish(self.tags),
//!             "children" => visitor
//!                 .visit_assoc()
//!                 .visit_entries_and_finish(self.children.iter().copied()),
//!             _ => visitor.visit_undefined(),
//!         }
//!    }
//! }
//! ```
//
// # Developers note
//
// Visitor types **should not** be cloneable in order to enforce just one
// visitor is used to visit a variable. If visitors are cloneable, it can make
// the wrong usage to be available, i.e. storing cloned visitors somewhere and
// using the wrong one.
//
// However, if visitors are made cloneable by any chance, it does not indicate
// the whole implementation will be broken. Users can only use the visitors
// through visitor traits (and their API do not allow cloning), so the logic
// would work as expected if the internal usage of the visitors are correct.
// Making visitors noncloneable is an optional safety guard (with no overhead).

use core::fmt;
use core::ops::ControlFlow;

pub use crate::template::components::VarName;

/// A trait for types that can behave as a static URI template expansion context.
///
/// This type is for use with [`UriTemplateStr::expand`] method.
///
/// See [the module documentation][`crate::template`] for usage.
///
/// [`UriTemplateStr::expand`]: `crate::template::UriTemplateStr::expand`
pub trait Context: Sized {
    /// Visits a variable.
    ///
    /// To get variable name, use [`Visitor::var_name()`].
    #[must_use]
    fn visit<V: Visitor>(&self, visitor: V) -> V::Result;
}

/// A trait for types that can behave as a dynamic (mutable) URI template expansion context.
///
/// This type is for use with [`UriTemplateStr::expand_dynamic`] method and its
/// family.
///
/// Note that "dynamic" here does not mean that the value of variables can
/// change during a template expansion. The value should be fixed and consistent
/// during each expansion, but the context is allowed to mutate itself if it
/// does not break this rule.
///
/// # Exmaples
///
/// ```
/// # #[cfg(feature = "alloc")]
/// # extern crate alloc;
/// # use iri_string::template::Error;
/// # #[cfg(feature = "alloc")] {
/// # use alloc::string::String;
/// use iri_string::template::UriTemplateStr;
/// use iri_string::template::context::{DynamicContext, Visitor, VisitPurpose};
/// use iri_string::spec::UriSpec;
///
/// struct MyContext<'a> {
///     /// Target path.
///     target: &'a str,
///     /// Username.
///     username: Option<&'a str>,
///     /// A flag to remember whether the URI template
///     /// attempted to use `username` variable.
///     username_visited: bool,
/// }
///
/// impl DynamicContext for MyContext<'_> {
///     fn on_expansion_start(&mut self) {
///         // Reset the state.
///         self.username_visited = false;
///     }
///     fn visit_dynamic<V: Visitor>(&mut self, visitor: V) -> V::Result {
///         match visitor.var_name().as_str() {
///             "target" => visitor.visit_string(self.target),
///             "username" => {
///                 if visitor.purpose() == VisitPurpose::Expand {
///                     // The variable `username` is being used
///                     // on the template expansion.
///                     // Don't care whether `username` is defined or not.
///                     self.username_visited = true;
///                 }
///                 if let Some(username) = &self.username {
///                     visitor.visit_string(username)
///                 } else {
///                     visitor.visit_undefined()
///                 }
///             }
///             _ => visitor.visit_undefined(),
///         }
///     }
/// }
///
/// let mut context = MyContext {
///     target: "/posts/1",
///     username: Some("the_admin"),
///     username_visited: false,
/// };
/// let mut buf = String::new();
///
/// // No access to the variable `username`.
/// let template1 = UriTemplateStr::new("{+target}")?;
/// template1.expand_dynamic::<UriSpec, _, _>(&mut buf, &mut context)?;
/// assert_eq!(buf, "/posts/1");
/// assert!(!context.username_visited);
///
/// buf.clear();
/// // Will access to the variable `username`.
/// let template2 = UriTemplateStr::new("{+target}{?username}")?;
/// template2.expand_dynamic::<UriSpec, _, _>(&mut buf, &mut context)?;
/// assert_eq!(buf, "/posts/1?username=the_admin");
/// assert!(context.username_visited);
///
/// buf.clear();
/// context.username = None;
/// // Will access to the variable `username` but it is undefined.
/// template2.expand_dynamic::<UriSpec, _, _>(&mut buf, &mut context)?;
/// assert_eq!(buf, "/posts/1");
/// assert!(
///     context.username_visited,
///     "`MyContext` can know and remember whether `visit_dynamic()` is called
///      for `username`, even if its value is undefined"
/// );
/// # }
/// # Ok::<_, Error>(())
/// ```
///
/// [`UriTemplateStr::expand_dynamic`]: `crate::template::UriTemplateStr::expand_dynamic`
pub trait DynamicContext: Sized {
    /// Visits a variable.
    ///
    /// To get variable name, use [`Visitor::var_name()`].
    ///
    /// # Restriction
    ///
    /// The visit results should be consistent and unchanged between the last
    /// time [`on_expansion_start`][`Self::on_expansion_start`] was called and
    /// the next time [`on_expansion_end`][`Self::on_expansion_end`] will be
    /// called. If this condition is violated, template expansion will produce
    /// wrong result or may panic at worst.
    #[must_use]
    fn visit_dynamic<V: Visitor>(&mut self, visitor: V) -> V::Result;

    /// A callback that is called before the expansion of a URI template.
    #[inline]
    fn on_expansion_start(&mut self) {}

    /// A callback that is called after the expansion of a URI template.
    #[inline]
    fn on_expansion_end(&mut self) {}
}

impl<C: Context> DynamicContext for C {
    #[inline]
    fn visit_dynamic<V: Visitor>(&mut self, visitor: V) -> V::Result {
        self.visit(visitor)
    }
}

/// A purpose of a visit.
///
/// This enum is nonexhaustive since this partially exposes the internal
/// implementation of the template expansion, and thus this is subject to
/// change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VisitPurpose {
    /// A visit for type checking.
    Typecheck,
    /// A visit for template expansion to retrieve the value.
    Expand,
}

/// Variable visitor.
///
/// See [the module documentation][self] for usage.
// NOTE (internal): Visitor types **should not** be cloneable.
pub trait Visitor: Sized + private::Sealed {
    /// Result of the visit.
    type Result;
    /// List visitor.
    type ListVisitor: ListVisitor<Result = Self::Result>;
    /// Associative array visitor.
    type AssocVisitor: AssocVisitor<Result = Self::Result>;

    /// Returns the name of the variable to visit.
    #[must_use]
    fn var_name(&self) -> VarName<'_>;
    /// Returns the purpose of the visit.
    ///
    /// The template expansion algorithm checks the types for some variables
    /// depending on its usage. To get the usage count correctly, you should
    /// only count visits with [`VisitPurpose::Expand`].
    ///
    /// If you need to know whether the variable is accessed and does not
    /// need dynamic context generation or access counts, consider using
    /// [`UriTemplateStr::variables`] method to iterate the variables in the
    /// URI template.
    ///
    /// [`UriTemplateStr::variables`]: `crate::template::UriTemplateStr::variables`
    #[must_use]
    fn purpose(&self) -> VisitPurpose;
    /// Visits an undefined variable, i.e. indicates that the requested variable is unavailable.
    #[must_use]
    fn visit_undefined(self) -> Self::Result;
    /// Visits a string variable.
    #[must_use]
    fn visit_string<T: fmt::Display>(self, v: T) -> Self::Result;
    /// Visits a list variable.
    #[must_use]
    fn visit_list(self) -> Self::ListVisitor;
    /// Visits an associative array variable.
    #[must_use]
    fn visit_assoc(self) -> Self::AssocVisitor;
}

/// List visitor.
///
/// See [the module documentation][self] for usage.
// NOTE (internal): Visitor types **should not** be cloneable.
pub trait ListVisitor: Sized + private::Sealed {
    /// Result of the visit.
    type Result;

    /// Visits an item.
    ///
    /// If this returned `ControlFlow::Break(v)`, [`Context::visit`] should also
    /// return this `v`.
    ///
    /// To feed multiple items at once, do
    /// `items.into_iter().try_for_each(|item| self.visit_item(item))` for example.
    #[must_use]
    fn visit_item<T: fmt::Display>(&mut self, item: T) -> ControlFlow<Self::Result>;
    /// Finishes visiting the list.
    #[must_use]
    fn finish(self) -> Self::Result;

    /// Visits items and finish.
    #[must_use]
    fn visit_items_and_finish<T, I>(mut self, items: I) -> Self::Result
    where
        T: fmt::Display,
        I: IntoIterator<Item = T>,
    {
        match items.into_iter().try_for_each(|item| self.visit_item(item)) {
            ControlFlow::Break(v) => v,
            ControlFlow::Continue(()) => self.finish(),
        }
    }
}

/// Associative array visitor.
///
/// See [the module documentation][self] for usage.
// NOTE (internal): Visitor types **should not** be cloneable.
pub trait AssocVisitor: Sized + private::Sealed {
    /// Result of the visit.
    type Result;

    /// Visits an entry.
    ///
    /// If this returned `ControlFlow::Break(v)`, [`Context::visit`] should also
    /// return this `v`.
    ///
    /// To feed multiple items at once, do
    /// `entries.into_iter().try_for_each(|(key, value)| self.visit_entry(key, value))`
    /// for example.
    #[must_use]
    fn visit_entry<K: fmt::Display, V: fmt::Display>(
        &mut self,
        key: K,
        value: V,
    ) -> ControlFlow<Self::Result>;
    /// Finishes visiting the associative array.
    #[must_use]
    fn finish(self) -> Self::Result;

    /// Visits entries and finish.
    #[must_use]
    fn visit_entries_and_finish<K, V, I>(mut self, entries: I) -> Self::Result
    where
        K: fmt::Display,
        V: fmt::Display,
        I: IntoIterator<Item = (K, V)>,
    {
        match entries
            .into_iter()
            .try_for_each(|(key, value)| self.visit_entry(key, value))
        {
            ControlFlow::Break(v) => v,
            ControlFlow::Continue(()) => self.finish(),
        }
    }
}

/// Private module to put the trait to seal.
pub(super) mod private {
    /// A trait for visitor types of variables in a context.
    pub trait Sealed {}
}
