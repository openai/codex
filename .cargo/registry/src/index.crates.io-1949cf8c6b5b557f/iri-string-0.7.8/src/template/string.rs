//! Template string types.

use core::fmt;

#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::rc::Rc;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::sync::Arc;

use crate::spec::Spec;
use crate::template::components::{VarListIter, VarName};
use crate::template::context::{Context, DynamicContext};
use crate::template::error::{Error, ErrorKind};
use crate::template::expand::{expand_whole_dynamic, Chunk, Chunks, Expanded};
use crate::template::parser::validate_template_str;

#[cfg(feature = "alloc")]
pub use self::owned::UriTemplateString;

/// Implements `PartialEq` and `PartialOrd`.
macro_rules! impl_cmp {
    ($ty_common:ty, $ty_lhs:ty, $ty_rhs:ty) => {
        impl PartialEq<$ty_rhs> for $ty_lhs {
            #[inline]
            fn eq(&self, o: &$ty_rhs) -> bool {
                <$ty_common as PartialEq<$ty_common>>::eq(self.as_ref(), o.as_ref())
            }
        }
        impl PartialEq<$ty_lhs> for $ty_rhs {
            #[inline]
            fn eq(&self, o: &$ty_lhs) -> bool {
                <$ty_common as PartialEq<$ty_common>>::eq(self.as_ref(), o.as_ref())
            }
        }
        impl PartialOrd<$ty_rhs> for $ty_lhs {
            #[inline]
            fn partial_cmp(&self, o: &$ty_rhs) -> Option<core::cmp::Ordering> {
                <$ty_common as PartialOrd<$ty_common>>::partial_cmp(self.as_ref(), o.as_ref())
            }
        }
        impl PartialOrd<$ty_lhs> for $ty_rhs {
            #[inline]
            fn partial_cmp(&self, o: &$ty_lhs) -> Option<core::cmp::Ordering> {
                <$ty_common as PartialOrd<$ty_common>>::partial_cmp(self.as_ref(), o.as_ref())
            }
        }
    };
}

#[cfg(feature = "alloc")]
mod owned;

/// A borrowed slice of a URI template.
///
/// URI Template is defined by [RFC 6570].
///
/// Note that "URI Template" can also be used for IRI.
///
/// [RFC 6570]: https://www.rfc-editor.org/rfc/rfc6570.html
///
/// # Valid values
///
/// This type can have a URI template string.
///
/// # Applied errata
///
/// [Errata ID 6937](https://www.rfc-editor.org/errata/eid6937) is applied, so
/// single quotes are allowed to appear in an URI template.
///
/// ```
/// # use iri_string::template::Error;
/// use iri_string::template::UriTemplateStr;
///
/// let template = UriTemplateStr::new("'quoted'")?;
/// # Ok::<_, Error>(())
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UriTemplateStr {
    /// The raw string.
    inner: str,
}

impl UriTemplateStr {
    /// Creates a new string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// use iri_string::template::UriTemplateStr;
    ///
    /// let template = UriTemplateStr::new("/users/{username}")?;
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn new(s: &str) -> Result<&Self, Error> {
        TryFrom::try_from(s)
    }

    /// Creates a new string without validation.
    ///
    /// This does not validate the given string, so it is caller's
    /// responsibility to ensure the given string is valid.
    ///
    /// # Safety
    ///
    /// The given string must be syntactically valid as `Self` type.
    /// If not, any use of the returned value or the call of this
    /// function itself may result in undefined behavior.
    #[inline]
    #[must_use]
    pub unsafe fn new_unchecked(s: &str) -> &Self {
        // SAFETY: `new_always_unchecked` requires the same precondition
        // as `new_always_unchecked`.
        unsafe { Self::new_always_unchecked(s) }
    }

    /// Creates a new string without any validation.
    ///
    /// This does not validate the given string at any time.
    ///
    /// Intended for internal use.
    ///
    /// # Safety
    ///
    /// The given string must be valid.
    #[inline]
    #[must_use]
    unsafe fn new_always_unchecked(s: &str) -> &Self {
        // SAFETY: the cast is safe since `Self` type has `repr(transparent)`
        // attribute and the content is guaranteed as valid by the
        // precondition of the function.
        unsafe { &*(s as *const str as *const Self) }
    }

    /// Returns the template as a plain `&str`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// use iri_string::template::UriTemplateStr;
    ///
    /// let template = UriTemplateStr::new("/users/{username}")?;
    /// assert_eq!(template.as_str(), "/users/{username}");
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    /// Returns the template string length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// use iri_string::template::UriTemplateStr;
    ///
    /// let template = UriTemplateStr::new("/users/{username}")?;
    /// assert_eq!(template.len(), "/users/{username}".len());
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_str().len()
    }

    /// Returns whether the string is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// use iri_string::template::UriTemplateStr;
    ///
    /// let template = UriTemplateStr::new("/users/{username}")?;
    /// assert!(!template.is_empty());
    ///
    /// let empty = UriTemplateStr::new("")?;
    /// assert!(empty.is_empty());
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }
}

impl UriTemplateStr {
    /// Expands the template with the given context.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::spec::UriSpec;
    /// use iri_string::template::UriTemplateStr;
    /// use iri_string::template::simple_context::SimpleContext;
    ///
    /// let mut context = SimpleContext::new();
    /// context.insert("username", "foo");
    ///
    /// let template = UriTemplateStr::new("/users/{username}")?;
    /// let expanded = template.expand::<UriSpec, _>(&context)?;
    ///
    /// assert_eq!(
    ///     expanded.to_string(),
    ///     "/users/foo"
    /// );
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// You can control allowed characters in the output by changing spec type.
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::spec::{IriSpec, UriSpec};
    /// use iri_string::template::UriTemplateStr;
    /// use iri_string::template::simple_context::SimpleContext;
    ///
    /// let mut context = SimpleContext::new();
    /// context.insert("alpha", "\u{03B1}");
    ///
    /// let template = UriTemplateStr::new("{?alpha}")?;
    ///
    /// assert_eq!(
    ///     template.expand::<UriSpec, _>(&context)?.to_string(),
    ///     "?alpha=%CE%B1",
    ///     "a URI cannot contain Unicode alpha (U+03B1), so it should be escaped"
    /// );
    /// assert_eq!(
    ///     template.expand::<IriSpec, _>(&context)?.to_string(),
    ///     "?alpha=\u{03B1}",
    ///     "an IRI can contain Unicode alpha (U+03B1), so it written as is"
    /// );
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn expand<'a, S: Spec, C: Context>(
        &'a self,
        context: &'a C,
    ) -> Result<Expanded<'a, S, C>, Error> {
        Expanded::new(self, context)
    }

    /// Expands the template with the given dynamic context.
    ///
    #[cfg_attr(
        feature = "alloc",
        doc = concat!(
            "If you need the allocated [`String`], use",
            "[`expand_dynamic_to_string`][`Self::expand_dynamic_to_string`]."
        )
    )]
    ///
    /// See the documentation for [`DynamicContext`] for usage.
    pub fn expand_dynamic<S: Spec, W: fmt::Write, C: DynamicContext>(
        &self,
        writer: &mut W,
        context: &mut C,
    ) -> Result<(), Error> {
        expand_whole_dynamic::<S, _, _>(self, writer, context)
    }

    /// Expands the template into a string, with the given dynamic context.
    ///
    /// This is basically [`expand_dynamic`][`Self::expand_dynamic`] method
    /// that returns an owned string instead of writing to the given writer.
    ///
    /// See the documentation for [`DynamicContext`] for usage.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")]
    /// # extern crate alloc;
    /// # use iri_string::template::Error;
    /// # #[cfg(feature = "alloc")] {
    /// # use alloc::string::String;
    /// use iri_string::template::UriTemplateStr;
    /// # use iri_string::template::context::{DynamicContext, Visitor, VisitPurpose};
    /// use iri_string::spec::UriSpec;
    ///
    /// struct MyContext<'a> {
    ///     // See the documentation for `DynamicContext`.
    /// #     /// Target path.
    /// #     target: &'a str,
    /// #     /// Username.
    /// #     username: Option<&'a str>,
    /// #     /// A flag to remember whether the URI template
    /// #     /// attempted to use `username` variable.
    /// #     username_visited: bool,
    /// }
    /// #
    /// # impl DynamicContext for MyContext<'_> {
    /// #     fn on_expansion_start(&mut self) {
    /// #         // Reset the state.
    /// #         self.username_visited = false;
    /// #     }
    /// #     fn visit_dynamic<V: Visitor>(&mut self, visitor: V) -> V::Result {
    /// #         match visitor.var_name().as_str() {
    /// #             "target" => visitor.visit_string(self.target),
    /// #             "username" => {
    /// #                 if visitor.purpose() == VisitPurpose::Expand {
    /// #                     // The variable `username` is being used
    /// #                     // on the template expansion.
    /// #                     // Don't care whether `username` is defined or not.
    /// #                     self.username_visited = true;
    /// #                 }
    /// #                 if let Some(username) = &self.username {
    /// #                     visitor.visit_string(username)
    /// #                 } else {
    /// #                     visitor.visit_undefined()
    /// #                 }
    /// #             }
    /// #             _ => visitor.visit_undefined(),
    /// #         }
    /// #     }
    /// # }
    ///
    /// let mut context = MyContext {
    ///     target: "/posts/1",
    ///     username: Some("the_admin"),
    ///     username_visited: false,
    /// };
    ///
    /// // No access to the variable `username`.
    /// let template = UriTemplateStr::new("{+target}{?username}")?;
    /// let s = template.expand_dynamic_to_string::<UriSpec, _>(&mut context)?;
    /// assert_eq!(s, "/posts/1?username=the_admin");
    /// assert!(context.username_visited);
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[cfg(feature = "alloc")]
    pub fn expand_dynamic_to_string<S: Spec, C: DynamicContext>(
        &self,
        context: &mut C,
    ) -> Result<String, Error> {
        let mut buf = String::new();
        expand_whole_dynamic::<S, _, _>(self, &mut buf, context)?;
        Ok(buf)
    }

    /// Returns an iterator of variables in the template.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// use iri_string::template::UriTemplateStr;
    ///
    /// let template = UriTemplateStr::new("foo{/bar*,baz:4}{?qux}{&bar*}")?;
    /// let mut vars = template.variables();
    /// assert_eq!(vars.next().map(|var| var.as_str()), Some("bar"));
    /// assert_eq!(vars.next().map(|var| var.as_str()), Some("baz"));
    /// assert_eq!(vars.next().map(|var| var.as_str()), Some("qux"));
    /// assert_eq!(vars.next().map(|var| var.as_str()), Some("bar"));
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn variables(&self) -> UriTemplateVariables<'_> {
        UriTemplateVariables::new(self)
    }
}

impl fmt::Debug for UriTemplateStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("UriTemplateStr").field(&&self.inner).finish()
    }
}

impl AsRef<str> for UriTemplateStr {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl AsRef<UriTemplateStr> for UriTemplateStr {
    #[inline]
    fn as_ref(&self) -> &UriTemplateStr {
        self
    }
}

#[cfg(feature = "alloc")]
impl<'a> From<&'a UriTemplateStr> for Cow<'a, UriTemplateStr> {
    #[inline]
    fn from(s: &'a UriTemplateStr) -> Self {
        Cow::Borrowed(s)
    }
}

#[cfg(feature = "alloc")]
impl From<&UriTemplateStr> for Arc<UriTemplateStr> {
    fn from(s: &UriTemplateStr) -> Self {
        let inner: &str = s.as_str();
        let buf = Arc::<str>::from(inner);
        // SAFETY: `UriTemplateStr` has `repr(transparent)` attribute, so
        // the memory layouts of `Arc<str>` and `Arc<UriTemplateStr>` are
        // compatible.
        unsafe {
            let raw: *const str = Arc::into_raw(buf);
            Self::from_raw(raw as *const UriTemplateStr)
        }
    }
}

#[cfg(feature = "alloc")]
impl From<&UriTemplateStr> for Box<UriTemplateStr> {
    fn from(s: &UriTemplateStr) -> Self {
        let inner: &str = s.as_str();
        let buf = Box::<str>::from(inner);
        // SAFETY: `UriTemplateStr` has `repr(transparent)` attribute, so
        // the memory layouts of `Box<str>` and `Box<UriTemplateStr>` are
        // compatible.
        unsafe {
            let raw: *mut str = Box::into_raw(buf);
            Self::from_raw(raw as *mut UriTemplateStr)
        }
    }
}

#[cfg(feature = "alloc")]
impl From<&UriTemplateStr> for Rc<UriTemplateStr> {
    fn from(s: &UriTemplateStr) -> Self {
        let inner: &str = s.as_str();
        let buf = Rc::<str>::from(inner);
        // SAFETY: `UriTemplateStr` has `repr(transparent)` attribute, so
        // the memory layouts of `Rc<str>` and `Rc<UriTemplateStr>` are
        // compatible.
        unsafe {
            let raw: *const str = Rc::into_raw(buf);
            Self::from_raw(raw as *const UriTemplateStr)
        }
    }
}

impl<'a> From<&'a UriTemplateStr> for &'a str {
    #[inline]
    fn from(s: &'a UriTemplateStr) -> &'a str {
        s.as_ref()
    }
}

impl<'a> TryFrom<&'a str> for &'a UriTemplateStr {
    type Error = Error;

    #[inline]
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        match validate_template_str(s) {
            // SAFETY: just checked the string is valid.
            Ok(()) => Ok(unsafe { UriTemplateStr::new_always_unchecked(s) }),
            Err(e) => Err(e),
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for &'a UriTemplateStr {
    type Error = Error;

    #[inline]
    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        let s = core::str::from_utf8(bytes)
            .map_err(|e| Error::new(ErrorKind::InvalidUtf8, e.valid_up_to()))?;
        match validate_template_str(s) {
            // SAFETY: just checked the string is valid.
            Ok(()) => Ok(unsafe { UriTemplateStr::new_always_unchecked(s) }),
            Err(e) => Err(e),
        }
    }
}

impl_cmp!(str, str, UriTemplateStr);
impl_cmp!(str, &str, UriTemplateStr);
impl_cmp!(str, str, &UriTemplateStr);

impl fmt::Display for UriTemplateStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Serde deserializer implementation.
#[cfg(feature = "serde")]
mod __serde_slice {
    use super::UriTemplateStr;

    use core::fmt;

    use serde::{
        de::{self, Visitor},
        Deserialize, Deserializer,
    };

    /// Custom borrowed string visitor.
    #[derive(Debug, Clone, Copy)]
    struct CustomStrVisitor;

    impl<'de> Visitor<'de> for CustomStrVisitor {
        type Value = &'de UriTemplateStr;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("URI template string")
        }

        #[inline]
        fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            <&'de UriTemplateStr as TryFrom<&'de str>>::try_from(v).map_err(E::custom)
        }
    }

    // About `'de` and `'a`, see
    // <https://serde.rs/lifetimes.html#the-deserializede-lifetime>.
    impl<'a, 'de: 'a> Deserialize<'de> for &'a UriTemplateStr {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_string(CustomStrVisitor)
        }
    }
}

/// An iterator of variables in a URI template.
#[derive(Debug, Clone)]
pub struct UriTemplateVariables<'a> {
    /// Chunks iterator.
    chunks: Chunks<'a>,
    /// Variables in the last chunk.
    vars_in_chunk: Option<VarListIter<'a>>,
}

impl<'a> UriTemplateVariables<'a> {
    /// Creates a variables iterator from the URI template.
    #[inline]
    #[must_use]
    fn new(template: &'a UriTemplateStr) -> Self {
        Self {
            chunks: Chunks::new(template),
            vars_in_chunk: None,
        }
    }
}

impl<'a> Iterator for UriTemplateVariables<'a> {
    type Item = VarName<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(vars) = &mut self.vars_in_chunk {
                match vars.next() {
                    Some((_len, spec)) => return Some(spec.name()),
                    None => self.vars_in_chunk = None,
                }
            }
            let expr = self.chunks.find_map(|chunk| match chunk {
                Chunk::Literal(_) => None,
                Chunk::Expr(v) => Some(v),
            });
            self.vars_in_chunk = match expr {
                Some(expr) => Some(expr.decompose().1.into_iter()),
                None => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::spec::IriSpec;
    use crate::template::context::{AssocVisitor, ListVisitor, Visitor};

    struct TestContext;
    impl Context for TestContext {
        fn visit<V: Visitor>(&self, visitor: V) -> V::Result {
            match visitor.var_name().as_str() {
                "str" => visitor.visit_string("string"),
                "list" => visitor
                    .visit_list()
                    .visit_items_and_finish(["item0", "item1", "item2"]),
                "assoc" => visitor
                    .visit_assoc()
                    .visit_entries_and_finish([("key0", "value0"), ("key1", "value1")]),
                _ => visitor.visit_undefined(),
            }
        }
    }

    #[test]
    fn expand_error_pos() {
        {
            let e = UriTemplateStr::new("foo{list:4}")
                .unwrap()
                .expand::<IriSpec, _>(&TestContext)
                .err()
                .map(|e| e.location());
            assert_eq!(e, Some("foo{".len()));
        }

        {
            let e = UriTemplateStr::new("foo{/list*,list:4}")
                .unwrap()
                .expand::<IriSpec, _>(&TestContext)
                .err()
                .map(|e| e.location());
            assert_eq!(e, Some("foo{/list*,".len()));
        }

        {
            let e = UriTemplateStr::new("foo{/str:3,list*,assoc:4}")
                .unwrap()
                .expand::<IriSpec, _>(&TestContext)
                .err()
                .map(|e| e.location());
            assert_eq!(e, Some("foo{/str:3,list*,".len()));
        }
    }
}
