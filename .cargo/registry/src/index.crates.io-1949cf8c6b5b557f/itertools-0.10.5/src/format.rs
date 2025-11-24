use std::fmt;
use std::cell::RefCell;

/// Format all iterator elements lazily, separated by `sep`.
///
/// The format value can only be formatted once, after that the iterator is
/// exhausted.
///
/// See [`.format_with()`](crate::Itertools::format_with) for more information.
#[derive(Clone)]
pub struct FormatWith<'a, I, F> {
    sep: &'a str,
    /// FormatWith uses interior mutability because Display::fmt takes &self.
    inner: RefCell<Option<(I, F)>>,
}

/// Format all iterator elements lazily, separated by `sep`.
///
/// The format value can only be formatted once, after that the iterator is
/// exhausted.
///
/// See [`.format()`](crate::Itertools::format)
/// for more information.
#[derive(Clone)]
pub struct Format<'a, I> {
    sep: &'a str,
    /// Format uses interior mutability because Display::fmt takes &self.
    inner: RefCell<Option<I>>,
}

pub fn new_format<I, F>(iter: I, separator: &str, f: F) -> FormatWith<'_, I, F>
    where I: Iterator,
          F: FnMut(I::Item, &mut dyn FnMut(&dyn fmt::Display) -> fmt::Result) -> fmt::Result
{
    FormatWith {
        sep: separator,
        inner: RefCell::new(Some((iter, f))),
    }
}

pub fn new_format_default<I>(iter: I, separator: &str) -> Format<'_, I>
    where I: Iterator,
{
    Format {
        sep: separator,
        inner: RefCell::new(Some(iter)),
    }
}

impl<'a, I, F> fmt::Display for FormatWith<'a, I, F>
    where I: Iterator,
          F: FnMut(I::Item, &mut dyn  FnMut(&dyn fmt::Display) -> fmt::Result) -> fmt::Result
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (mut iter, mut format) = match self.inner.borrow_mut().take() {
            Some(t) => t,
            None => panic!("FormatWith: was already formatted once"),
        };

        if let Some(fst) = iter.next() {
            format(fst, &mut |disp: &dyn fmt::Display| disp.fmt(f))?;
            iter.try_for_each(|elt| {
                if !self.sep.is_empty() {
                    f.write_str(self.sep)?;
                }
                format(elt, &mut |disp: &dyn fmt::Display| disp.fmt(f))
            })?;
        }
        Ok(())
    }
}

impl<'a, I> Format<'a, I>
    where I: Iterator,
{
    fn format<F>(&self, f: &mut fmt::Formatter, mut cb: F) -> fmt::Result
        where F: FnMut(&I::Item, &mut fmt::Formatter) -> fmt::Result,
    {
        let mut iter = match self.inner.borrow_mut().take() {
            Some(t) => t,
            None => panic!("Format: was already formatted once"),
        };

        if let Some(fst) = iter.next() {
            cb(&fst, f)?;
            iter.try_for_each(|elt| {
                if !self.sep.is_empty() {
                    f.write_str(self.sep)?;
                }
                cb(&elt, f)
            })?;
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

impl_format!{Display Debug
             UpperExp LowerExp UpperHex LowerHex Octal Binary Pointer}
