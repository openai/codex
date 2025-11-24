//! `MakeVisitor` wrappers for working with `fmt::Debug` fields.
use super::{MakeVisitor, VisitFmt, VisitOutput};
use tracing_core::field::{Field, Visit};

use core::fmt;

/// A visitor wrapper that ensures any `fmt::Debug` fields are formatted using
/// the alternate (`:#`) formatter.
#[derive(Debug, Clone)]
pub struct Alt<V>(V);

// TODO(eliza): When `error` as a primitive type is stable, add a
// `DisplayErrors` wrapper...

// === impl Alt ===
//
impl<V> Alt<V> {
    /// Wraps the provided visitor so that any `fmt::Debug` fields are formatted
    /// using the alternative (`:#`) formatter.
    pub fn new(inner: V) -> Self {
        Alt(inner)
    }
}

impl<T, V> MakeVisitor<T> for Alt<V>
where
    V: MakeVisitor<T>,
{
    type Visitor = Alt<V::Visitor>;

    #[inline]
    fn make_visitor(&self, target: T) -> Self::Visitor {
        Alt(self.0.make_visitor(target))
    }
}

impl<V> Visit for Alt<V>
where
    V: Visit,
{
    #[inline]
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.record_f64(field, value)
    }

    #[inline]
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.record_i64(field, value)
    }

    #[inline]
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.record_u64(field, value)
    }

    #[inline]
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.record_bool(field, value)
    }

    /// Visit a string value.
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.record_str(field, value)
    }

    // TODO(eliza): add RecordError when stable
    // fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
    //     self.record_debug(field, &format_args!("{}", value))
    // }

    #[inline]
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.record_debug(field, &format_args!("{:#?}", value))
    }
}

impl<V, O> VisitOutput<O> for Alt<V>
where
    V: VisitOutput<O>,
{
    #[inline]
    fn finish(self) -> O {
        self.0.finish()
    }
}

feature! {
    #![feature = "std"]
    use super::VisitWrite;
    use std::io;

    impl<V> VisitWrite for Alt<V>
    where
        V: VisitWrite,
    {
        #[inline]
        fn writer(&mut self) -> &mut dyn io::Write {
            self.0.writer()
        }
    }
}

impl<V> VisitFmt for Alt<V>
where
    V: VisitFmt,
{
    #[inline]
    fn writer(&mut self) -> &mut dyn fmt::Write {
        self.0.writer()
    }
}
