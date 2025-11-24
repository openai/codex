use alloc::borrow::Cow;
use alloc::string::String;

use crate::Captures;

/// Replacer describes types that can be used to replace matches in a string.
///
/// In general, users of this crate shouldn't need to implement this trait,
/// since implementations are already provided for `&str` along with other
/// variants of string types and `FnMut(&Captures) -> String` (or any
/// `FnMut(&Captures) -> T` where `T: AsRef<str>`), which covers most use cases.
pub trait Replacer {
    /// Appends text to `dst` to replace the current match.
    ///
    /// The current match is represented by `caps`, which is guaranteed to
    /// have a match at capture group `0`.
    ///
    /// For example, a no-op replacement would be
    /// `dst.push_str(caps.get(0).unwrap().as_str())`.
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String);

    /// Return a fixed unchanging replacement string.
    ///
    /// When doing replacements, if access to `Captures` is not needed (e.g.,
    /// the replacement byte string does not need `$` expansion), then it can
    /// be beneficial to avoid finding sub-captures.
    ///
    /// In general, this is called once for every call to `replacen`.
    fn no_expansion(&mut self) -> Option<Cow<str>> {
        None
    }

    /// Return a `Replacer` that borrows and wraps this `Replacer`.
    ///
    /// This is useful when you want to take a generic `Replacer` (which might
    /// not be cloneable) and use it without consuming it, so it can be used
    /// more than once.
    ///
    /// # Example
    ///
    /// ```
    /// use fancy_regex::{Regex, Replacer};
    ///
    /// fn replace_all_twice<R: Replacer>(
    ///     re: Regex,
    ///     src: &str,
    ///     mut rep: R,
    /// ) -> String {
    ///     let dst = re.replace_all(src, rep.by_ref());
    ///     let dst = re.replace_all(&dst, rep.by_ref());
    ///     dst.into_owned()
    /// }
    /// ```
    fn by_ref(&mut self) -> ReplacerRef<Self> {
        ReplacerRef(self)
    }
}

/// By-reference adaptor for a `Replacer`
///
/// Returned by [`Replacer::by_ref`](trait.Replacer.html#method.by_ref).
#[derive(Debug)]
pub struct ReplacerRef<'a, R: ?Sized>(&'a mut R);

impl<'a, R: Replacer + ?Sized + 'a> Replacer for ReplacerRef<'a, R> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.0.replace_append(caps, dst)
    }
    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        self.0.no_expansion()
    }
}

impl<'a> Replacer for &'a str {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        caps.expand(*self, dst);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl<'a> Replacer for &'a String {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.as_str().replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl Replacer for String {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.as_str().replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl<'a> Replacer for Cow<'a, str> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.as_ref().replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

impl<'a> Replacer for &'a Cow<'a, str> {
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        self.as_ref().replace_append(caps, dst)
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        no_expansion(self)
    }
}

fn no_expansion<T: AsRef<str>>(t: &T) -> Option<Cow<'_, str>> {
    let s = t.as_ref();
    if s.contains('$') {
        None
    } else {
        Some(Cow::Borrowed(s))
    }
}

impl<F, T> Replacer for F
where
    F: FnMut(&Captures<'_>) -> T,
    T: AsRef<str>,
{
    fn replace_append(&mut self, caps: &Captures<'_>, dst: &mut String) {
        dst.push_str((*self)(caps).as_ref());
    }
}

/// `NoExpand` indicates literal string replacement.
///
/// It can be used with `replace` and `replace_all` to do a literal string
/// replacement without expanding `$name` to their corresponding capture
/// groups. This can be both convenient (to avoid escaping `$`, for example)
/// and performant (since capture groups don't need to be found).
///
/// `'t` is the lifetime of the literal text.
#[derive(Clone, Debug)]
pub struct NoExpand<'t>(pub &'t str);

impl<'t> Replacer for NoExpand<'t> {
    fn replace_append(&mut self, _: &Captures<'_>, dst: &mut String) {
        dst.push_str(self.0);
    }

    fn no_expansion(&mut self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(self.0))
    }
}
