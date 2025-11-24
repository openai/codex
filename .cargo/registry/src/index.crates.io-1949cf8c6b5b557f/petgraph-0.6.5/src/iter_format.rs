//! Formatting utils

use std::cell::RefCell;
use std::fmt;

/// Format the iterator like a map
pub struct DebugMap<F>(pub F);

impl<F, I, K, V> fmt::Debug for DebugMap<F>
where
    F: Fn() -> I,
    I: IntoIterator<Item = (K, V)>,
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map().entries((self.0)()).finish()
    }
}

/// Avoid "pretty" debug
pub struct NoPretty<T>(pub T);

impl<T> fmt::Debug for NoPretty<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

/// Format all iterator elements lazily, separated by `sep`.
///
/// The format value can only be formatted once, after that the iterator is
/// exhausted.
///
/// See [`.format()`](../trait.Itertools.html#method.format)
/// for more information.
#[derive(Clone)]
pub struct Format<'a, I> {
    sep: &'a str,
    /// Format uses interior mutability because Display::fmt takes &self.
    inner: RefCell<Option<I>>,
}

pub trait IterFormatExt: Iterator {
    fn format(self, separator: &str) -> Format<Self>
    where
        Self: Sized,
    {
        Format {
            sep: separator,
            inner: RefCell::new(Some(self)),
        }
    }
}

impl<I> IterFormatExt for I where I: Iterator {}

impl<'a, I> Format<'a, I>
where
    I: Iterator,
{
    fn format<F>(&self, f: &mut fmt::Formatter, mut cb: F) -> fmt::Result
    where
        F: FnMut(&I::Item, &mut fmt::Formatter) -> fmt::Result,
    {
        let mut iter = match self.inner.borrow_mut().take() {
            Some(t) => t,
            None => panic!("Format: was already formatted once"),
        };

        if let Some(fst) = iter.next() {
            cb(&fst, f)?;
            for elt in iter {
                if !self.sep.is_empty() {
                    f.write_str(self.sep)?;
                }
                cb(&elt, f)?;
            }
        }
        Ok(())
    }
}

macro_rules! impl_format {
    ($($fmt_trait:ident)*) => {
        $(
            impl<'a, I> fmt::$fmt_trait for Format<'a, I>
                where I: Iterator,
                      I::Item: fmt::$fmt_trait,
            {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    self.format(f, fmt::$fmt_trait::fmt)
                }
            }
        )*
    }
}

impl_format!(Debug);
