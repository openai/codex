//! A `MakeVisitor` wrapper that separates formatted fields with a delimiter.
use super::{MakeVisitor, VisitFmt, VisitOutput};

use core::fmt;
use tracing_core::field::{Field, Visit};

/// A `MakeVisitor` wrapper that wraps a visitor that writes formatted output so
/// that a delimiter is inserted between writing formatted field values.
#[derive(Debug, Clone)]
pub struct Delimited<D, V> {
    delimiter: D,
    inner: V,
}

/// A visitor wrapper that inserts a delimiter after the wrapped visitor formats
/// a field value.
#[derive(Debug)]
pub struct VisitDelimited<D, V> {
    delimiter: D,
    seen: bool,
    inner: V,
    err: fmt::Result,
}

// === impl Delimited ===

impl<D, V, T> MakeVisitor<T> for Delimited<D, V>
where
    D: AsRef<str> + Clone,
    V: MakeVisitor<T>,
    V::Visitor: VisitFmt,
{
    type Visitor = VisitDelimited<D, V::Visitor>;
    fn make_visitor(&self, target: T) -> Self::Visitor {
        let inner = self.inner.make_visitor(target);
        VisitDelimited::new(self.delimiter.clone(), inner)
    }
}

impl<D, V> Delimited<D, V> {
    /// Returns a new [`MakeVisitor`] implementation that wraps `inner` so that
    /// it will format each visited field separated by the provided `delimiter`.
    ///
    /// [`MakeVisitor`]: super::MakeVisitor
    pub fn new(delimiter: D, inner: V) -> Self {
        Self { delimiter, inner }
    }
}

// === impl VisitDelimited ===

impl<D, V> VisitDelimited<D, V> {
    /// Returns a new [`Visit`] implementation that wraps `inner` so that
    /// each formatted field is separated by the provided `delimiter`.
    ///
    /// [`Visit`]: tracing_core::field::Visit
    pub fn new(delimiter: D, inner: V) -> Self {
        Self {
            delimiter,
            inner,
            seen: false,
            err: Ok(()),
        }
    }

    fn delimit(&mut self)
    where
        V: VisitFmt,
        D: AsRef<str>,
    {
        if self.err.is_err() {
            return;
        }

        if self.seen {
            self.err = self.inner.writer().write_str(self.delimiter.as_ref());
        }

        self.seen = true;
    }
}

impl<D, V> Visit for VisitDelimited<D, V>
where
    V: VisitFmt,
    D: AsRef<str>,
{
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.delimit();
        self.inner.record_i64(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.delimit();
        self.inner.record_u64(field, value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.delimit();
        self.inner.record_bool(field, value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.delimit();
        self.inner.record_str(field, value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.delimit();
        self.inner.record_debug(field, value);
    }
}

impl<D, V> VisitOutput<fmt::Result> for VisitDelimited<D, V>
where
    V: VisitFmt,
    D: AsRef<str>,
{
    fn finish(self) -> fmt::Result {
        self.err?;
        self.inner.finish()
    }
}

impl<D, V> VisitFmt for VisitDelimited<D, V>
where
    V: VisitFmt,
    D: AsRef<str>,
{
    fn writer(&mut self) -> &mut dyn fmt::Write {
        self.inner.writer()
    }
}

#[cfg(test)]
#[cfg(all(test, feature = "alloc"))]
mod test {
    use super::*;
    use crate::field::test_util::*;

    #[test]
    fn delimited_visitor() {
        let mut s = String::new();
        let visitor = DebugVisitor::new(&mut s);
        let mut visitor = VisitDelimited::new(", ", visitor);

        TestAttrs1::with(|attrs| attrs.record(&mut visitor));
        visitor.finish().unwrap();

        assert_eq!(
            s.as_str(),
            "question=\"life, the universe, and everything\", tricky=true, can_you_do_it=true"
        );
    }

    #[test]
    fn delimited_new_visitor() {
        let make = Delimited::new("; ", MakeDebug);

        TestAttrs1::with(|attrs| {
            let mut s = String::new();
            {
                let mut v = make.make_visitor(&mut s);
                attrs.record(&mut v);
            }
            assert_eq!(
                s.as_str(),
                "question=\"life, the universe, and everything\"; tricky=true; can_you_do_it=true"
            );
        });

        TestAttrs2::with(|attrs| {
            let mut s = String::new();
            {
                let mut v = make.make_visitor(&mut s);
                attrs.record(&mut v);
            }
            assert_eq!(
                s.as_str(),
                "question=None; question.answer=42; tricky=true; can_you_do_it=false"
            );
        });
    }
}
