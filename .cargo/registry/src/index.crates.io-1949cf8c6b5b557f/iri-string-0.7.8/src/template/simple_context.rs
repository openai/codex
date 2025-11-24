//! Simple general-purpose context type.

use core::ops::ControlFlow;

use alloc::collections::BTreeMap;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::vec::Vec;

use crate::template::context::{Context, VarName, Visitor};

/// Value.
#[derive(Debug, Clone)]
pub enum Value {
    /// Undefined (i.e. null).
    Undefined,
    /// String value.
    String(String),
    /// List.
    List(Vec<String>),
    /// Associative array.
    Assoc(Vec<(String, String)>),
}

impl From<&str> for Value {
    #[inline]
    fn from(v: &str) -> Self {
        Self::String(v.into())
    }
}

impl From<String> for Value {
    #[inline]
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

/// Simple template expansion context.
#[derive(Default, Debug, Clone)]
pub struct SimpleContext {
    /// Variable values.
    // Any map types (including `HashMap`) is ok, but the hash map is not provided by `alloc`.
    //
    // QUESTION: Should hexdigits in percent-encoded triplets in varnames be
    // compared case sensitively?
    variables: BTreeMap<String, Value>,
}

impl SimpleContext {
    /// Creates a new empty context.
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
    /// let empty_ctx = SimpleContext::new();
    /// let template = UriTemplateStr::new("{no_such_variable}")?;
    /// let expanded = template.expand::<UriSpec, _>(&empty_ctx)?;
    ///
    /// assert_eq!(
    ///     expanded.to_string(),
    ///     ""
    /// );
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a variable.
    ///
    /// Passing [`Value::Undefined`] removes the value from the context.
    ///
    /// The entry will be inserted or removed even if the key is invalid as a
    /// variable name. Such entries will be simply ignored on expansion.
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
    /// Passing [`Value::Undefined`] removes the value from the context.
    ///
    /// ```
    /// # use iri_string::template::Error;
    /// ## [cfg(feature = "alloc")] {
    /// use iri_string::spec::UriSpec;
    /// use iri_string::template::UriTemplateStr;
    /// use iri_string::template::simple_context::{SimpleContext, Value};
    ///
    /// let mut context = SimpleContext::new();
    /// context.insert("username", "foo");
    /// context.insert("username", Value::Undefined);
    ///
    /// let template = UriTemplateStr::new("/users/{username}")?;
    /// let expanded = template.expand::<UriSpec, _>(&context)?;
    ///
    /// assert_eq!(
    ///     expanded.to_string(),
    ///     "/users/"
    /// );
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    pub fn insert<K, V>(&mut self, key: K, value: V) -> Option<Value>
    where
        K: Into<String>,
        V: Into<Value>,
    {
        let key = key.into();
        match value.into() {
            Value::Undefined => self.variables.remove(&key),
            value => self.variables.insert(key, value),
        }
    }

    /// Removes all entries in the context.
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
    /// let template = UriTemplateStr::new("{foo,bar}")?;
    /// let mut context = SimpleContext::new();
    ///
    /// context.insert("foo", "FOO");
    /// context.insert("bar", "BAR");
    /// assert_eq!(
    ///     template.expand::<UriSpec, _>(&context)?.to_string(),
    ///     "FOO,BAR"
    /// );
    ///
    /// context.clear();
    /// assert_eq!(
    ///     template.expand::<UriSpec, _>(&context)?.to_string(),
    ///     ""
    /// );
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.variables.clear();
    }

    /// Returns a reference to the value for the key.
    //
    // QUESTION: Should hexdigits in percent-encoded triplets in varnames be
    // compared case sensitively?
    #[inline]
    #[must_use]
    pub fn get(&self, key: VarName<'_>) -> Option<&Value> {
        self.variables.get(key.as_str())
    }
}

impl Context for SimpleContext {
    fn visit<V: Visitor>(&self, visitor: V) -> V::Result {
        use crate::template::context::{AssocVisitor, ListVisitor};

        let name = visitor.var_name().as_str();
        match self.variables.get(name) {
            None | Some(Value::Undefined) => visitor.visit_undefined(),
            Some(Value::String(s)) => visitor.visit_string(s),
            Some(Value::List(list)) => {
                let mut visitor = visitor.visit_list();
                if let ControlFlow::Break(res) =
                    list.iter().try_for_each(|item| visitor.visit_item(item))
                {
                    return res;
                }
                visitor.finish()
            }
            Some(Value::Assoc(list)) => {
                let mut visitor = visitor.visit_assoc();
                if let ControlFlow::Break(res) =
                    list.iter().try_for_each(|(k, v)| visitor.visit_entry(k, v))
                {
                    return res;
                }
                visitor.finish()
            }
        }
    }
}
