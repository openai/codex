//! Utilities for working with [fields] and [field visitors].
//!
//! [fields]: tracing_core::field
//! [field visitors]: tracing_core::field::Visit
use core::{fmt, marker::PhantomData};
pub use tracing_core::field::Visit;
use tracing_core::{
    span::{Attributes, Record},
    Event,
};
pub mod debug;
pub mod delimited;
pub mod display;

/// Creates new [visitors].
///
/// A type implementing `MakeVisitor` represents a composable factory for types
/// implementing the [`Visit` trait][visitors]. The `MakeVisitor` trait defines
/// a single function, `make_visitor`, which takes in a `T`-typed `target` and
/// returns a type implementing `Visit` configured for that target. A target may
/// be a string, output stream, or data structure that the visitor will record
/// data to, configuration variables that determine the visitor's behavior, or
/// `()` when no input is required to produce a visitor.
///
/// [visitors]: tracing_core::field::Visit
pub trait MakeVisitor<T> {
    /// The visitor type produced by this `MakeVisitor`.
    type Visitor: Visit;

    /// Make a new visitor for the provided `target`.
    fn make_visitor(&self, target: T) -> Self::Visitor;
}

/// A [visitor] that produces output once it has visited a set of fields.
///
/// [visitor]: tracing_core::field::Visit
pub trait VisitOutput<Out>: Visit {
    /// Completes the visitor, returning any output.
    ///
    /// This is called once a full set of fields has been visited.
    fn finish(self) -> Out;

    /// Visit a set of fields, and return the output of finishing the visitor
    /// once the fields have been visited.
    fn visit<R>(mut self, fields: &R) -> Out
    where
        R: RecordFields,
        Self: Sized,
    {
        fields.record(&mut self);
        self.finish()
    }
}

/// Extension trait implemented by types which can be recorded by a [visitor].
///
/// This allows writing code that is generic over `tracing_core`'s
/// [`span::Attributes`][attr], [`span::Record`][rec], and [`Event`]
/// types. These types all provide inherent `record` methods that allow a
/// visitor to record their fields, but there is no common trait representing this.
///
/// With `RecordFields`, we can write code like this:
/// ```
/// use tracing_core::field::Visit;
/// # use tracing_core::field::Field;
/// use tracing_subscriber::field::RecordFields;
///
/// struct MyVisitor {
///     // ...
/// }
/// # impl MyVisitor { fn new() -> Self { Self{} } }
/// impl Visit for MyVisitor {
///     // ...
/// # fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
/// }
///
/// fn record_with_my_visitor<R>(r: R)
/// where
///     R: RecordFields,
/// {
///     let mut visitor = MyVisitor::new();
///     r.record(&mut visitor);
/// }
/// ```
/// [visitor]: tracing_core::field::Visit
/// [attr]: tracing_core::span::Attributes
/// [rec]: tracing_core::span::Record
pub trait RecordFields: crate::sealed::Sealed<RecordFieldsMarker> {
    /// Record all the fields in `self` with the provided `visitor`.
    fn record(&self, visitor: &mut dyn Visit);
}

/// Extension trait implemented for all `MakeVisitor` implementations that
/// produce a visitor implementing `VisitOutput`.
pub trait MakeOutput<T, Out>
where
    Self: MakeVisitor<T> + crate::sealed::Sealed<(T, Out)>,
    Self::Visitor: VisitOutput<Out>,
{
    /// Visits all fields in `fields` with a new visitor constructed from
    /// `target`.
    fn visit_with<F>(&self, target: T, fields: &F) -> Out
    where
        F: RecordFields,
    {
        self.make_visitor(target).visit(fields)
    }
}

feature! {
    #![feature = "std"]
    use std::io;

    /// Extension trait implemented by visitors to indicate that they write to an
    /// `io::Write` instance, and allow access to that writer.
    pub trait VisitWrite: VisitOutput<Result<(), io::Error>> {
        /// Returns the writer that this visitor writes to.
        fn writer(&mut self) -> &mut dyn io::Write;
    }
}

/// Extension trait implemented by visitors to indicate that they write to a
/// `fmt::Write` instance, and allow access to that writer.
pub trait VisitFmt: VisitOutput<fmt::Result> {
    /// Returns the formatter that this visitor writes to.
    fn writer(&mut self) -> &mut dyn fmt::Write;
}

/// Extension trait providing `MakeVisitor` combinators.
pub trait MakeExt<T>
where
    Self: MakeVisitor<T> + Sized,
    Self: crate::sealed::Sealed<MakeExtMarker<T>>,
{
    /// Wraps `self` so that any `fmt::Debug` fields are recorded using the
    /// alternate formatter (`{:#?}`).
    fn debug_alt(self) -> debug::Alt<Self> {
        debug::Alt::new(self)
    }

    /// Wraps `self` so that any string fields named "message" are recorded
    /// using `fmt::Display`.
    fn display_messages(self) -> display::Messages<Self> {
        display::Messages::new(self)
    }

    /// Wraps `self` so that when fields are formatted to a writer, they are
    /// separated by the provided `delimiter`.
    fn delimited<D>(self, delimiter: D) -> delimited::Delimited<D, Self>
    where
        D: AsRef<str> + Clone,
        Self::Visitor: VisitFmt,
    {
        delimited::Delimited::new(delimiter, self)
    }
}

// === impl RecordFields ===

impl crate::sealed::Sealed<RecordFieldsMarker> for Event<'_> {}
impl RecordFields for Event<'_> {
    fn record(&self, visitor: &mut dyn Visit) {
        Event::record(self, visitor)
    }
}

impl crate::sealed::Sealed<RecordFieldsMarker> for Attributes<'_> {}
impl RecordFields for Attributes<'_> {
    fn record(&self, visitor: &mut dyn Visit) {
        Attributes::record(self, visitor)
    }
}

impl crate::sealed::Sealed<RecordFieldsMarker> for Record<'_> {}
impl RecordFields for Record<'_> {
    fn record(&self, visitor: &mut dyn Visit) {
        Record::record(self, visitor)
    }
}

impl<F> crate::sealed::Sealed<RecordFieldsMarker> for &F where F: RecordFields {}
impl<F> RecordFields for &F
where
    F: RecordFields,
{
    fn record(&self, visitor: &mut dyn Visit) {
        F::record(*self, visitor)
    }
}

// === blanket impls ===

impl<T, V, F> MakeVisitor<T> for F
where
    F: Fn(T) -> V,
    V: Visit,
{
    type Visitor = V;
    fn make_visitor(&self, target: T) -> Self::Visitor {
        (self)(target)
    }
}

impl<T, Out, M> crate::sealed::Sealed<(T, Out)> for M
where
    M: MakeVisitor<T>,
    M::Visitor: VisitOutput<Out>,
{
}

impl<T, Out, M> MakeOutput<T, Out> for M
where
    M: MakeVisitor<T>,
    M::Visitor: VisitOutput<Out>,
{
}

impl<T, M> crate::sealed::Sealed<MakeExtMarker<T>> for M where M: MakeVisitor<T> + Sized {}

impl<T, M> MakeExt<T> for M
where
    M: MakeVisitor<T> + Sized,
    M: crate::sealed::Sealed<MakeExtMarker<T>>,
{
}

#[derive(Debug)]
#[doc(hidden)]
pub struct MakeExtMarker<T> {
    _p: PhantomData<T>,
}

#[derive(Debug)]
#[doc(hidden)]
pub struct RecordFieldsMarker {
    _p: (),
}

#[cfg(all(test, feature = "alloc"))]
#[macro_use]
pub(in crate::field) mod test_util {
    use super::*;
    pub(in crate::field) use alloc::string::String;
    use tracing_core::{
        callsite::Callsite,
        field::{Field, Value},
        metadata::{Kind, Level, Metadata},
    };

    pub(crate) struct TestAttrs1;
    pub(crate) struct TestAttrs2;

    impl TestAttrs1 {
        pub(crate) fn with<T>(f: impl FnOnce(Attributes<'_>) -> T) -> T {
            let fieldset = TEST_META_1.fields();
            let values = &[
                (
                    &fieldset.field("question").unwrap(),
                    Some(&"life, the universe, and everything" as &dyn Value),
                ),
                (&fieldset.field("question.answer").unwrap(), None),
                (
                    &fieldset.field("tricky").unwrap(),
                    Some(&true as &dyn Value),
                ),
                (
                    &fieldset.field("can_you_do_it").unwrap(),
                    Some(&true as &dyn Value),
                ),
            ];
            let valueset = fieldset.value_set(values);
            let attrs = tracing_core::span::Attributes::new(&TEST_META_1, &valueset);
            f(attrs)
        }
    }

    impl TestAttrs2 {
        pub(crate) fn with<T>(f: impl FnOnce(Attributes<'_>) -> T) -> T {
            let fieldset = TEST_META_1.fields();
            let none = tracing_core::field::debug(&Option::<&str>::None);
            let values = &[
                (
                    &fieldset.field("question").unwrap(),
                    Some(&none as &dyn Value),
                ),
                (
                    &fieldset.field("question.answer").unwrap(),
                    Some(&42 as &dyn Value),
                ),
                (
                    &fieldset.field("tricky").unwrap(),
                    Some(&true as &dyn Value),
                ),
                (
                    &fieldset.field("can_you_do_it").unwrap(),
                    Some(&false as &dyn Value),
                ),
            ];
            let valueset = fieldset.value_set(values);
            let attrs = tracing_core::span::Attributes::new(&TEST_META_1, &valueset);
            f(attrs)
        }
    }

    struct TestCallsite1;
    static TEST_CALLSITE_1: &'static dyn Callsite = &TestCallsite1;
    static TEST_META_1: Metadata<'static> = tracing_core::metadata! {
        name: "field_test1",
        target: module_path!(),
        level: Level::INFO,
        fields: &["question", "question.answer", "tricky", "can_you_do_it"],
        callsite: TEST_CALLSITE_1,
        kind: Kind::SPAN,
    };

    impl Callsite for TestCallsite1 {
        fn set_interest(&self, _: tracing_core::subscriber::Interest) {
            unimplemented!()
        }

        fn metadata(&self) -> &Metadata<'_> {
            &TEST_META_1
        }
    }

    pub(crate) struct MakeDebug;
    pub(crate) struct DebugVisitor<'a> {
        writer: &'a mut dyn fmt::Write,
        err: fmt::Result,
    }

    impl<'a> DebugVisitor<'a> {
        pub(crate) fn new(writer: &'a mut dyn fmt::Write) -> Self {
            Self {
                writer,
                err: Ok(()),
            }
        }
    }

    impl Visit for DebugVisitor<'_> {
        fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
            write!(self.writer, "{}={:?}", field, value).unwrap();
        }
    }

    impl VisitOutput<fmt::Result> for DebugVisitor<'_> {
        fn finish(self) -> fmt::Result {
            self.err
        }
    }

    impl VisitFmt for DebugVisitor<'_> {
        fn writer(&mut self) -> &mut dyn fmt::Write {
            self.writer
        }
    }

    impl<'a> MakeVisitor<&'a mut dyn fmt::Write> for MakeDebug {
        type Visitor = DebugVisitor<'a>;
        fn make_visitor(&self, w: &'a mut dyn fmt::Write) -> DebugVisitor<'a> {
            DebugVisitor::new(w)
        }
    }
}
