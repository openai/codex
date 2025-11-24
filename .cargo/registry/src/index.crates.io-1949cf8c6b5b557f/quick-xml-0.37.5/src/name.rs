//! Module for handling names according to the W3C [Namespaces in XML 1.1 (Second Edition)][spec]
//! specification
//!
//! [spec]: https://www.w3.org/TR/xml-names11

use crate::events::attributes::Attribute;
use crate::events::BytesStart;
use crate::utils::write_byte_string;
use memchr::memchr;
use std::fmt::{self, Debug, Formatter};

/// Some namespace was invalid
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceError {
    /// Specified namespace prefix is unknown, cannot resolve namespace for it
    UnknownPrefix(Vec<u8>),
    /// Attempts to bind the `xml` prefix to something other than `http://www.w3.org/XML/1998/namespace`.
    ///
    /// `xml` prefix can be bound only to `http://www.w3.org/XML/1998/namespace`.
    ///
    /// Contains the namespace to which `xml` tried to be bound.
    InvalidXmlPrefixBind(Vec<u8>),
    /// Attempts to bind the `xmlns` prefix.
    ///
    /// `xmlns` prefix is always bound to `http://www.w3.org/2000/xmlns/` and cannot be bound
    /// to any other namespace or even to `http://www.w3.org/2000/xmlns/`.
    ///
    /// Contains the namespace to which `xmlns` tried to be bound.
    InvalidXmlnsPrefixBind(Vec<u8>),
    /// Attempts to bind some prefix (except `xml`) to `http://www.w3.org/XML/1998/namespace`.
    ///
    /// Only `xml` prefix can be bound to `http://www.w3.org/XML/1998/namespace`.
    ///
    /// Contains the prefix that is tried to be bound.
    InvalidPrefixForXml(Vec<u8>),
    /// Attempts to bind some prefix to `http://www.w3.org/2000/xmlns/`.
    ///
    /// `http://www.w3.org/2000/xmlns/` cannot be bound to any prefix, even to `xmlns`.
    ///
    /// Contains the prefix that is tried to be bound.
    InvalidPrefixForXmlns(Vec<u8>),
}

impl fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UnknownPrefix(prefix) => {
                f.write_str("unknown namespace prefix '")?;
                write_byte_string(f, prefix)?;
                f.write_str("'")
            }
            Self::InvalidXmlPrefixBind(namespace) => {
                f.write_str("the namespace prefix 'xml' cannot be bound to '")?;
                write_byte_string(f, namespace)?;
                f.write_str("'")
            }
            Self::InvalidXmlnsPrefixBind(namespace) => {
                f.write_str("the namespace prefix 'xmlns' cannot be bound to '")?;
                write_byte_string(f, namespace)?;
                f.write_str("'")
            }
            Self::InvalidPrefixForXml(prefix) => {
                f.write_str("the namespace prefix '")?;
                write_byte_string(f, prefix)?;
                f.write_str("' cannot be bound to 'http://www.w3.org/XML/1998/namespace'")
            }
            Self::InvalidPrefixForXmlns(prefix) => {
                f.write_str("the namespace prefix '")?;
                write_byte_string(f, prefix)?;
                f.write_str("' cannot be bound to 'http://www.w3.org/2000/xmlns/'")
            }
        }
    }
}

impl std::error::Error for NamespaceError {}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A [qualified name] of an element or an attribute, including an optional
/// namespace [prefix](Prefix) and a [local name](LocalName).
///
/// [qualified name]: https://www.w3.org/TR/xml-names11/#dt-qualname
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde-types", derive(serde::Deserialize, serde::Serialize))]
pub struct QName<'a>(pub &'a [u8]);
impl<'a> QName<'a> {
    /// Converts this name to an internal slice representation.
    #[inline(always)]
    pub const fn into_inner(self) -> &'a [u8] {
        self.0
    }

    /// Returns local part of this qualified name.
    ///
    /// All content up to and including the first `:` character is removed from
    /// the tag name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quick_xml::name::QName;
    /// let simple = QName(b"simple-name");
    /// assert_eq!(simple.local_name().as_ref(), b"simple-name");
    ///
    /// let qname = QName(b"namespace:simple-name");
    /// assert_eq!(qname.local_name().as_ref(), b"simple-name");
    /// ```
    pub fn local_name(&self) -> LocalName<'a> {
        LocalName(self.index().map_or(self.0, |i| &self.0[i + 1..]))
    }

    /// Returns namespace part of this qualified name or `None` if namespace part
    /// is not defined (symbol `':'` not found).
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::convert::AsRef;
    /// # use quick_xml::name::QName;
    /// let simple = QName(b"simple-name");
    /// assert_eq!(simple.prefix(), None);
    ///
    /// let qname = QName(b"prefix:simple-name");
    /// assert_eq!(qname.prefix().as_ref().map(|n| n.as_ref()), Some(b"prefix".as_ref()));
    /// ```
    pub fn prefix(&self) -> Option<Prefix<'a>> {
        self.index().map(|i| Prefix(&self.0[..i]))
    }

    /// The same as `(qname.local_name(), qname.prefix())`, but does only one
    /// lookup for a `':'` symbol.
    pub fn decompose(&self) -> (LocalName<'a>, Option<Prefix<'a>>) {
        match self.index() {
            None => (LocalName(self.0), None),
            Some(i) => (LocalName(&self.0[i + 1..]), Some(Prefix(&self.0[..i]))),
        }
    }

    /// If that `QName` represents `"xmlns"` series of names, returns `Some`,
    /// otherwise `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quick_xml::name::{QName, PrefixDeclaration};
    /// let qname = QName(b"xmlns");
    /// assert_eq!(qname.as_namespace_binding(), Some(PrefixDeclaration::Default));
    ///
    /// let qname = QName(b"xmlns:prefix");
    /// assert_eq!(qname.as_namespace_binding(), Some(PrefixDeclaration::Named(b"prefix")));
    ///
    /// // Be aware that this method does not check the validity of the prefix - it can be empty!
    /// let qname = QName(b"xmlns:");
    /// assert_eq!(qname.as_namespace_binding(), Some(PrefixDeclaration::Named(b"")));
    ///
    /// let qname = QName(b"other-name");
    /// assert_eq!(qname.as_namespace_binding(), None);
    ///
    /// // https://www.w3.org/TR/xml-names11/#xmlReserved
    /// let qname = QName(b"xmlns-reserved-name");
    /// assert_eq!(qname.as_namespace_binding(), None);
    /// ```
    pub fn as_namespace_binding(&self) -> Option<PrefixDeclaration<'a>> {
        if self.0.starts_with(b"xmlns") {
            return match self.0.get(5) {
                None => Some(PrefixDeclaration::Default),
                Some(&b':') => Some(PrefixDeclaration::Named(&self.0[6..])),
                _ => None,
            };
        }
        None
    }

    /// Returns the index in the name where prefix ended
    #[inline(always)]
    fn index(&self) -> Option<usize> {
        memchr(b':', self.0)
    }
}
impl<'a> Debug for QName<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "QName(")?;
        write_byte_string(f, self.0)?;
        write!(f, ")")
    }
}
impl<'a> AsRef<[u8]> for QName<'a> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A [local (unqualified) name] of an element or an attribute, i.e. a name
/// without [prefix](Prefix).
///
/// [local (unqualified) name]: https://www.w3.org/TR/xml-names11/#dt-localname
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde-types", derive(serde::Deserialize, serde::Serialize))]
pub struct LocalName<'a>(pub(crate) &'a [u8]);
impl<'a> LocalName<'a> {
    /// Converts this name to an internal slice representation.
    #[inline(always)]
    pub const fn into_inner(self) -> &'a [u8] {
        self.0
    }
}
impl<'a> Debug for LocalName<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "LocalName(")?;
        write_byte_string(f, self.0)?;
        write!(f, ")")
    }
}
impl<'a> AsRef<[u8]> for LocalName<'a> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}
impl<'a> From<QName<'a>> for LocalName<'a> {
    /// Creates `LocalName` from a [`QName`]
    ///
    /// # Examples
    ///
    /// ```
    /// # use quick_xml::name::{LocalName, QName};
    ///
    /// let local: LocalName = QName(b"unprefixed").into();
    /// assert_eq!(local.as_ref(), b"unprefixed");
    ///
    /// let local: LocalName = QName(b"some:prefix").into();
    /// assert_eq!(local.as_ref(), b"prefix");
    /// ```
    #[inline]
    fn from(name: QName<'a>) -> Self {
        Self(name.index().map_or(name.0, |i| &name.0[i + 1..]))
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A [namespace prefix] part of the [qualified name](QName) of an element tag
/// or an attribute: a `prefix` in `<prefix:local-element-name>` or
/// `prefix:local-attribute-name="attribute value"`.
///
/// [namespace prefix]: https://www.w3.org/TR/xml-names11/#dt-prefix
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde-types", derive(serde::Deserialize, serde::Serialize))]
pub struct Prefix<'a>(&'a [u8]);
impl<'a> Prefix<'a> {
    /// Extracts internal slice
    #[inline(always)]
    pub const fn into_inner(self) -> &'a [u8] {
        self.0
    }
}
impl<'a> Debug for Prefix<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Prefix(")?;
        write_byte_string(f, self.0)?;
        write!(f, ")")
    }
}
impl<'a> AsRef<[u8]> for Prefix<'a> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A namespace prefix declaration, `xmlns` or `xmlns:<name>`, as defined in
/// [XML Schema specification](https://www.w3.org/TR/xml-names11/#ns-decl)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrefixDeclaration<'a> {
    /// XML attribute binds a default namespace. Corresponds to `xmlns` in `xmlns="..."`
    Default,
    /// XML attribute binds a specified prefix to a namespace. Corresponds to a
    /// `prefix` in `xmlns:prefix="..."`, which is stored as payload of this variant.
    Named(&'a [u8]),
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A [namespace name] that is declared in a `xmlns[:prefix]="namespace name"`.
///
/// [namespace name]: https://www.w3.org/TR/xml-names11/#dt-NSName
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde-types", derive(serde::Deserialize, serde::Serialize))]
pub struct Namespace<'a>(pub &'a [u8]);
impl<'a> Namespace<'a> {
    /// Converts this namespace to an internal slice representation.
    ///
    /// This is [non-normalized] attribute value, i.e. any entity references is
    /// not expanded and space characters are not removed. This means, that
    /// different byte slices, returned from this method, can represent the same
    /// namespace and would be treated by parser as identical.
    ///
    /// For example, if the entity **eacute** has been defined to be **é**,
    /// the empty tags below all contain namespace declarations binding the
    /// prefix `p` to the same [IRI reference], `http://example.org/rosé`.
    ///
    /// ```xml
    /// <p:foo xmlns:p="http://example.org/rosé" />
    /// <p:foo xmlns:p="http://example.org/ros&#xe9;" />
    /// <p:foo xmlns:p="http://example.org/ros&#xE9;" />
    /// <p:foo xmlns:p="http://example.org/ros&#233;" />
    /// <p:foo xmlns:p="http://example.org/ros&eacute;" />
    /// ```
    ///
    /// This is because XML entity references are expanded during attribute value
    /// normalization.
    ///
    /// [non-normalized]: https://www.w3.org/TR/xml11/#AVNormalize
    /// [IRI reference]: https://datatracker.ietf.org/doc/html/rfc3987
    #[inline(always)]
    pub const fn into_inner(self) -> &'a [u8] {
        self.0
    }
    //TODO: implement value normalization and use it when comparing namespaces
}
impl<'a> Debug for Namespace<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Namespace(")?;
        write_byte_string(f, self.0)?;
        write!(f, ")")
    }
}
impl<'a> AsRef<[u8]> for Namespace<'a> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Result of [prefix] resolution which creates by [`NsReader::resolve_attribute`],
/// [`NsReader::resolve_element`], [`NsReader::read_resolved_event`] and
/// [`NsReader::read_resolved_event_into`] methods.
///
/// [prefix]: Prefix
/// [`NsReader::resolve_attribute`]: crate::reader::NsReader::resolve_attribute
/// [`NsReader::resolve_element`]: crate::reader::NsReader::resolve_element
/// [`NsReader::read_resolved_event`]: crate::reader::NsReader::read_resolved_event
/// [`NsReader::read_resolved_event_into`]: crate::reader::NsReader::read_resolved_event_into
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ResolveResult<'ns> {
    /// Qualified name does not contain prefix, and resolver does not define
    /// default namespace, so name is not bound to any namespace
    Unbound,
    /// [`Prefix`] resolved to the specified namespace
    Bound(Namespace<'ns>),
    /// Specified prefix was not found in scope
    Unknown(Vec<u8>),
}
impl<'ns> Debug for ResolveResult<'ns> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Unbound => write!(f, "Unbound"),
            Self::Bound(ns) => write!(f, "Bound({:?})", ns),
            Self::Unknown(p) => {
                write!(f, "Unknown(")?;
                write_byte_string(f, p)?;
                write!(f, ")")
            }
        }
    }
}

impl<'ns> TryFrom<ResolveResult<'ns>> for Option<Namespace<'ns>> {
    type Error = NamespaceError;

    /// Try to convert this result to an optional namespace and returns
    /// [`NamespaceError::UnknownPrefix`] if this result represents unknown prefix
    fn try_from(result: ResolveResult<'ns>) -> Result<Self, NamespaceError> {
        use ResolveResult::*;

        match result {
            Unbound => Ok(None),
            Bound(ns) => Ok(Some(ns)),
            Unknown(p) => Err(NamespaceError::UnknownPrefix(p)),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// An entry that contains index into the buffer with namespace bindings.
///
/// Defines a mapping from *[namespace prefix]* to *[namespace name]*.
/// If prefix is empty, defines a *default namespace* binding that applies to
/// unprefixed element names (unprefixed attribute names do not bind to any
/// namespace and they processing is dependent on the element in which their
/// defined).
///
/// [namespace prefix]: https://www.w3.org/TR/xml-names11/#dt-prefix
/// [namespace name]: https://www.w3.org/TR/xml-names11/#dt-NSName
#[derive(Debug, Clone)]
struct NamespaceEntry {
    /// Index of the namespace in the buffer
    start: usize,
    /// Length of the prefix
    /// * if greater than zero, then binds this namespace to the slice
    ///   `[start..start + prefix_len]` in the buffer.
    /// * else defines the current default namespace.
    prefix_len: usize,
    /// The length of a namespace name (the URI) of this namespace declaration.
    /// Name started just after prefix and extend for `value_len` bytes.
    ///
    /// The XML standard [specifies] that an empty namespace value 'removes' a namespace declaration
    /// for the extent of its scope. For prefix declarations that's not very interesting, but it is
    /// vital for default namespace declarations. With `xmlns=""` you can revert back to the default
    /// behaviour of leaving unqualified element names unqualified.
    ///
    /// [specifies]: https://www.w3.org/TR/xml-names11/#scoping
    value_len: usize,
    /// Level of nesting at which this namespace was declared. The declaring element is included,
    /// i.e., a declaration on the document root has `level = 1`.
    /// This is used to pop the namespace when the element gets closed.
    level: i32,
}

impl NamespaceEntry {
    /// Get the namespace prefix, bound to this namespace declaration, or `None`,
    /// if this declaration is for default namespace (`xmlns="..."`).
    #[inline]
    fn prefix<'b>(&self, ns_buffer: &'b [u8]) -> Option<Prefix<'b>> {
        if self.prefix_len == 0 {
            None
        } else {
            Some(Prefix(&ns_buffer[self.start..self.start + self.prefix_len]))
        }
    }

    /// Gets the namespace name (the URI) slice out of namespace buffer
    ///
    /// Returns `None` if namespace for this prefix was explicitly removed from
    /// scope, using `xmlns[:prefix]=""`
    #[inline]
    fn namespace<'ns>(&self, buffer: &'ns [u8]) -> ResolveResult<'ns> {
        if self.value_len == 0 {
            ResolveResult::Unbound
        } else {
            let start = self.start + self.prefix_len;
            ResolveResult::Bound(Namespace(&buffer[start..start + self.value_len]))
        }
    }
}

/// A namespace management buffer.
///
/// Holds all internal logic to push/pop namespaces with their levels.
#[derive(Debug, Clone)]
pub(crate) struct NamespaceResolver {
    /// Buffer that contains names of namespace prefixes (the part between `xmlns:`
    /// and an `=`) and namespace values.
    buffer: Vec<u8>,
    /// A stack of namespace bindings to prefixes that currently in scope
    bindings: Vec<NamespaceEntry>,
    /// The number of open tags at the moment. We need to keep track of this to know which namespace
    /// declarations to remove when we encounter an `End` event.
    nesting_level: i32,
}

/// That constant define the one of [reserved namespaces] for the xml standard.
///
/// The prefix `xml` is by definition bound to the namespace name
/// `http://www.w3.org/XML/1998/namespace`. It may, but need not, be declared, and must not be
/// undeclared or bound to any other namespace name. Other prefixes must not be bound to this
/// namespace name, and it must not be declared as the default namespace.
///
/// [reserved namespaces]: https://www.w3.org/TR/xml-names11/#xmlReserved
const RESERVED_NAMESPACE_XML: (Prefix, Namespace) = (
    Prefix(b"xml"),
    Namespace(b"http://www.w3.org/XML/1998/namespace"),
);
/// That constant define the one of [reserved namespaces] for the xml standard.
///
/// The prefix `xmlns` is used only to declare namespace bindings and is by definition bound
/// to the namespace name `http://www.w3.org/2000/xmlns/`. It must not be declared or
/// undeclared. Other prefixes must not be bound to this namespace name, and it must not be
///  declared as the default namespace. Element names must not have the prefix `xmlns`.
///
/// [reserved namespaces]: https://www.w3.org/TR/xml-names11/#xmlReserved
const RESERVED_NAMESPACE_XMLNS: (Prefix, Namespace) = (
    Prefix(b"xmlns"),
    Namespace(b"http://www.w3.org/2000/xmlns/"),
);

impl Default for NamespaceResolver {
    fn default() -> Self {
        let mut buffer = Vec::new();
        let mut bindings = Vec::new();
        for ent in &[RESERVED_NAMESPACE_XML, RESERVED_NAMESPACE_XMLNS] {
            let prefix = ent.0.into_inner();
            let uri = ent.1.into_inner();
            bindings.push(NamespaceEntry {
                start: buffer.len(),
                prefix_len: prefix.len(),
                value_len: uri.len(),
                level: 0,
            });
            buffer.extend(prefix);
            buffer.extend(uri);
        }

        Self {
            buffer,
            bindings,
            nesting_level: 0,
        }
    }
}

impl NamespaceResolver {
    /// Begins a new scope and add to it all [namespace bindings] that found in
    /// the specified start element.
    ///
    /// [namespace binding]: https://www.w3.org/TR/xml-names11/#dt-NSDecl
    pub fn push(&mut self, start: &BytesStart) -> Result<(), NamespaceError> {
        self.nesting_level += 1;
        let level = self.nesting_level;
        // adds new namespaces for attributes starting with 'xmlns:' and for the 'xmlns'
        // (default namespace) attribute.
        for a in start.attributes().with_checks(false) {
            if let Ok(Attribute { key: k, value: v }) = a {
                match k.as_namespace_binding() {
                    Some(PrefixDeclaration::Default) => {
                        let start = self.buffer.len();
                        self.buffer.extend_from_slice(&v);
                        self.bindings.push(NamespaceEntry {
                            start,
                            prefix_len: 0,
                            value_len: v.len(),
                            level,
                        });
                    }
                    Some(PrefixDeclaration::Named(b"xml")) => {
                        if Namespace(&v) != RESERVED_NAMESPACE_XML.1 {
                            // error, `xml` prefix explicitly set to different value
                            return Err(NamespaceError::InvalidXmlPrefixBind(v.to_vec()));
                        }
                        // don't add another NamespaceEntry for the `xml` namespace prefix
                    }
                    Some(PrefixDeclaration::Named(b"xmlns")) => {
                        // error, `xmlns` prefix explicitly set
                        return Err(NamespaceError::InvalidXmlnsPrefixBind(v.to_vec()));
                    }
                    Some(PrefixDeclaration::Named(prefix)) => {
                        let ns = Namespace(&v);

                        if ns == RESERVED_NAMESPACE_XML.1 {
                            // error, non-`xml` prefix set to xml uri
                            return Err(NamespaceError::InvalidPrefixForXml(prefix.to_vec()));
                        } else if ns == RESERVED_NAMESPACE_XMLNS.1 {
                            // error, non-`xmlns` prefix set to xmlns uri
                            return Err(NamespaceError::InvalidPrefixForXmlns(prefix.to_vec()));
                        }

                        let start = self.buffer.len();
                        self.buffer.extend_from_slice(prefix);
                        self.buffer.extend_from_slice(&v);
                        self.bindings.push(NamespaceEntry {
                            start,
                            prefix_len: prefix.len(),
                            value_len: v.len(),
                            level,
                        });
                    }
                    None => {}
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Ends a top-most scope by popping all [namespace binding], that was added by
    /// last call to [`Self::push()`].
    ///
    /// [namespace binding]: https://www.w3.org/TR/xml-names11/#dt-NSDecl
    pub fn pop(&mut self) {
        self.nesting_level -= 1;
        let current_level = self.nesting_level;
        // from the back (most deeply nested scope), look for the first scope that is still valid
        match self.bindings.iter().rposition(|n| n.level <= current_level) {
            // none of the namespaces are valid, remove all of them
            None => {
                self.buffer.clear();
                self.bindings.clear();
            }
            // drop all namespaces past the last valid namespace
            Some(last_valid_pos) => {
                if let Some(len) = self.bindings.get(last_valid_pos + 1).map(|n| n.start) {
                    self.buffer.truncate(len);
                    self.bindings.truncate(last_valid_pos + 1);
                }
            }
        }
    }

    /// Resolves a potentially qualified **element name** or **attribute name**
    /// into (namespace name, local name).
    ///
    /// *Qualified* names have the form `prefix:local-name` where the `prefix` is
    /// defined on any containing XML element via `xmlns:prefix="the:namespace:uri"`.
    /// The namespace prefix can be defined on the same element as the element or
    /// attribute in question.
    ///
    /// *Unqualified* attribute names do *not* inherit the current *default namespace*.
    ///
    /// # Lifetimes
    ///
    /// - `'n`: lifetime of an attribute or an element name
    #[inline]
    pub fn resolve<'n>(
        &self,
        name: QName<'n>,
        use_default: bool,
    ) -> (ResolveResult, LocalName<'n>) {
        let (local_name, prefix) = name.decompose();
        (self.resolve_prefix(prefix, use_default), local_name)
    }

    /// Finds a [namespace name] for a given qualified **element name**, borrow
    /// it from the internal buffer.
    ///
    /// Returns `None`, if:
    /// - name is unqualified
    /// - prefix not found in the current scope
    /// - prefix was [unbound] using `xmlns:prefix=""`
    ///
    /// [namespace name]: https://www.w3.org/TR/xml-names11/#dt-NSName
    /// [unbound]: https://www.w3.org/TR/xml-names11/#scoping
    #[inline]
    pub fn find(&self, element_name: QName) -> ResolveResult {
        self.resolve_prefix(element_name.prefix(), true)
    }

    fn resolve_prefix(&self, prefix: Option<Prefix>, use_default: bool) -> ResolveResult {
        self.bindings
            .iter()
            // Find the last defined binding that corresponds to the given prefix
            .rev()
            .find_map(|n| match (n.prefix(&self.buffer), prefix) {
                // This is default namespace definition and name has no explicit prefix
                (None, None) if use_default => Some(n.namespace(&self.buffer)),
                (None, None) => Some(ResolveResult::Unbound),

                // One part has prefix but other is not -> skip
                (None, Some(_)) => None,
                (Some(_), None) => None,

                // Prefixes does not match -> skip
                (Some(definition), Some(usage)) if definition != usage => None,

                // Prefixes the same, entry defines binding reset (corresponds to `xmlns:p=""`)
                _ if n.value_len == 0 => Some(Self::maybe_unknown(prefix)),
                // Prefixes the same, returns corresponding namespace
                _ => Some(n.namespace(&self.buffer)),
            })
            .unwrap_or_else(|| Self::maybe_unknown(prefix))
    }

    #[inline]
    fn maybe_unknown(prefix: Option<Prefix>) -> ResolveResult<'static> {
        match prefix {
            Some(p) => ResolveResult::Unknown(p.into_inner().to_vec()),
            None => ResolveResult::Unbound,
        }
    }

    #[inline]
    pub const fn iter(&self) -> PrefixIter {
        PrefixIter {
            resolver: self,
            // We initialize the cursor to 2 to skip the two default namespaces xml: and xmlns:
            bindings_cursor: 2,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Iterator on the current declared prefixes.
///
/// See [`NsReader::prefixes`](crate::NsReader::prefixes) for documentation.
#[derive(Debug, Clone)]
pub struct PrefixIter<'a> {
    resolver: &'a NamespaceResolver,
    bindings_cursor: usize,
}

impl<'a> Iterator for PrefixIter<'a> {
    type Item = (PrefixDeclaration<'a>, Namespace<'a>);

    fn next(&mut self) -> Option<(PrefixDeclaration<'a>, Namespace<'a>)> {
        while let Some(namespace_entry) = self.resolver.bindings.get(self.bindings_cursor) {
            self.bindings_cursor += 1; // We increment for next read

            // We check if the key has not been overridden by having a look
            // at the namespaces declared after in the array
            let prefix = namespace_entry.prefix(&self.resolver.buffer);
            if self.resolver.bindings[self.bindings_cursor..]
                .iter()
                .any(|ne| prefix == ne.prefix(&self.resolver.buffer))
            {
                continue; // Overridden
            }
            let namespace = if let ResolveResult::Bound(namespace) =
                namespace_entry.namespace(&self.resolver.buffer)
            {
                namespace
            } else {
                continue; // We don't return unbound namespaces
            };
            let prefix = if let Some(Prefix(prefix)) = prefix {
                PrefixDeclaration::Named(prefix)
            } else {
                PrefixDeclaration::Default
            };
            return Some((prefix, namespace));
        }
        None // We have exhausted the array
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Real count could be less if some namespaces was overridden
        (0, Some(self.resolver.bindings.len() - self.bindings_cursor))
    }
}

#[cfg(test)]
mod namespaces {
    use super::*;
    use pretty_assertions::assert_eq;
    use ResolveResult::*;

    /// Unprefixed attribute names (resolved with `false` flag) never have a namespace
    /// according to <https://www.w3.org/TR/xml-names11/#defaulting>:
    ///
    /// > A default namespace declaration applies to all unprefixed element names
    /// > within its scope. Default namespace declarations do not apply directly
    /// > to attribute names; the interpretation of unprefixed attributes is
    /// > determined by the element on which they appear.
    mod unprefixed {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Basic tests that checks that basic resolver functionality is working
        #[test]
        fn basic() {
            let name = QName(b"simple");
            let ns = Namespace(b"default");

            let mut resolver = NamespaceResolver::default();
            let s = resolver.buffer.len();

            resolver
                .push(&BytesStart::from_content(" xmlns='default'", 0))
                .unwrap();
            assert_eq!(&resolver.buffer[s..], b"default");

            // Check that tags without namespaces does not change result
            resolver.push(&BytesStart::from_content("", 0)).unwrap();
            assert_eq!(&resolver.buffer[s..], b"default");
            resolver.pop();

            assert_eq!(&resolver.buffer[s..], b"default");
            assert_eq!(
                resolver.resolve(name, true),
                (Bound(ns), LocalName(b"simple"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Unbound, LocalName(b"simple"))
            );
            assert_eq!(resolver.find(name), Bound(ns));
        }

        /// Test adding a second level of namespaces, which replaces the previous binding
        #[test]
        fn override_namespace() {
            let name = QName(b"simple");
            let old_ns = Namespace(b"old");
            let new_ns = Namespace(b"new");

            let mut resolver = NamespaceResolver::default();
            let s = resolver.buffer.len();

            resolver
                .push(&BytesStart::from_content(" xmlns='old'", 0))
                .unwrap();
            resolver
                .push(&BytesStart::from_content(" xmlns='new'", 0))
                .unwrap();

            assert_eq!(&resolver.buffer[s..], b"oldnew");
            assert_eq!(
                resolver.resolve(name, true),
                (Bound(new_ns), LocalName(b"simple"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Unbound, LocalName(b"simple"))
            );
            assert_eq!(resolver.find(name), Bound(new_ns));

            resolver.pop();
            assert_eq!(&resolver.buffer[s..], b"old");
            assert_eq!(
                resolver.resolve(name, true),
                (Bound(old_ns), LocalName(b"simple"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Unbound, LocalName(b"simple"))
            );
            assert_eq!(resolver.find(name), Bound(old_ns));
        }

        /// Test adding a second level of namespaces, which reset the previous binding
        /// to not bound state by specifying an empty namespace name.
        ///
        /// See <https://www.w3.org/TR/xml-names11/#scoping>
        #[test]
        fn reset() {
            let name = QName(b"simple");
            let old_ns = Namespace(b"old");

            let mut resolver = NamespaceResolver::default();
            let s = resolver.buffer.len();

            resolver
                .push(&BytesStart::from_content(" xmlns='old'", 0))
                .unwrap();
            resolver
                .push(&BytesStart::from_content(" xmlns=''", 0))
                .unwrap();

            assert_eq!(&resolver.buffer[s..], b"old");
            assert_eq!(
                resolver.resolve(name, true),
                (Unbound, LocalName(b"simple"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Unbound, LocalName(b"simple"))
            );
            assert_eq!(resolver.find(name), Unbound);

            resolver.pop();
            assert_eq!(&resolver.buffer[s..], b"old");
            assert_eq!(
                resolver.resolve(name, true),
                (Bound(old_ns), LocalName(b"simple"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Unbound, LocalName(b"simple"))
            );
            assert_eq!(resolver.find(name), Bound(old_ns));
        }
    }

    mod declared_prefix {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Basic tests that checks that basic resolver functionality is working
        #[test]
        fn basic() {
            let name = QName(b"p:with-declared-prefix");
            let ns = Namespace(b"default");

            let mut resolver = NamespaceResolver::default();
            let s = resolver.buffer.len();

            resolver
                .push(&BytesStart::from_content(" xmlns:p='default'", 0))
                .unwrap();
            assert_eq!(&resolver.buffer[s..], b"pdefault");

            // Check that tags without namespaces does not change result
            resolver.push(&BytesStart::from_content("", 0)).unwrap();
            assert_eq!(&resolver.buffer[s..], b"pdefault");
            resolver.pop();

            assert_eq!(&resolver.buffer[s..], b"pdefault");
            assert_eq!(
                resolver.resolve(name, true),
                (Bound(ns), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Bound(ns), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(resolver.find(name), Bound(ns));
        }

        /// Test adding a second level of namespaces, which replaces the previous binding
        #[test]
        fn override_namespace() {
            let name = QName(b"p:with-declared-prefix");
            let old_ns = Namespace(b"old");
            let new_ns = Namespace(b"new");

            let mut resolver = NamespaceResolver::default();
            let s = resolver.buffer.len();

            resolver
                .push(&BytesStart::from_content(" xmlns:p='old'", 0))
                .unwrap();
            resolver
                .push(&BytesStart::from_content(" xmlns:p='new'", 0))
                .unwrap();

            assert_eq!(&resolver.buffer[s..], b"poldpnew");
            assert_eq!(
                resolver.resolve(name, true),
                (Bound(new_ns), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Bound(new_ns), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(resolver.find(name), Bound(new_ns));

            resolver.pop();
            assert_eq!(&resolver.buffer[s..], b"pold");
            assert_eq!(
                resolver.resolve(name, true),
                (Bound(old_ns), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Bound(old_ns), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(resolver.find(name), Bound(old_ns));
        }

        /// Test adding a second level of namespaces, which reset the previous binding
        /// to not bound state by specifying an empty namespace name.
        ///
        /// See <https://www.w3.org/TR/xml-names11/#scoping>
        #[test]
        fn reset() {
            let name = QName(b"p:with-declared-prefix");
            let old_ns = Namespace(b"old");

            let mut resolver = NamespaceResolver::default();
            let s = resolver.buffer.len();

            resolver
                .push(&BytesStart::from_content(" xmlns:p='old'", 0))
                .unwrap();
            resolver
                .push(&BytesStart::from_content(" xmlns:p=''", 0))
                .unwrap();

            assert_eq!(&resolver.buffer[s..], b"poldp");
            assert_eq!(
                resolver.resolve(name, true),
                (Unknown(b"p".to_vec()), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Unknown(b"p".to_vec()), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(resolver.find(name), Unknown(b"p".to_vec()));

            resolver.pop();
            assert_eq!(&resolver.buffer[s..], b"pold");
            assert_eq!(
                resolver.resolve(name, true),
                (Bound(old_ns), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(
                resolver.resolve(name, false),
                (Bound(old_ns), LocalName(b"with-declared-prefix"))
            );
            assert_eq!(resolver.find(name), Bound(old_ns));
        }
    }

    /// Tests for `xml` and `xmlns` built-in prefixes.
    ///
    /// See <https://www.w3.org/TR/xml-names11/#xmlReserved>
    mod builtin_prefixes {
        use super::*;

        mod xml {
            use super::*;
            use pretty_assertions::assert_eq;

            /// `xml` prefix are always defined, it is not required to define it explicitly.
            #[test]
            fn undeclared() {
                let name = QName(b"xml:random");
                let namespace = RESERVED_NAMESPACE_XML.1;

                let resolver = NamespaceResolver::default();

                assert_eq!(
                    resolver.resolve(name, true),
                    (Bound(namespace), LocalName(b"random"))
                );

                assert_eq!(
                    resolver.resolve(name, false),
                    (Bound(namespace), LocalName(b"random"))
                );
                assert_eq!(resolver.find(name), Bound(namespace));
            }

            /// `xml` prefix can be declared but it must be bound to the value
            /// `http://www.w3.org/XML/1998/namespace`
            #[test]
            fn rebound_to_correct_ns() {
                let mut resolver = NamespaceResolver::default();
                let s = resolver.buffer.len();
                resolver.push(
                    &BytesStart::from_content(
                        " xmlns:xml='http://www.w3.org/XML/1998/namespace'",
                        0,
                    ),
                ).expect("`xml` prefix should be possible to bound to `http://www.w3.org/XML/1998/namespace`");
                assert_eq!(&resolver.buffer[s..], b"");
            }

            /// `xml` prefix cannot be re-declared to another namespace
            #[test]
            fn rebound_to_incorrect_ns() {
                let mut resolver = NamespaceResolver::default();
                let s = resolver.buffer.len();
                assert_eq!(
                    resolver.push(&BytesStart::from_content(
                        " xmlns:xml='not_correct_namespace'",
                        0,
                    )),
                    Err(NamespaceError::InvalidXmlPrefixBind(
                        b"not_correct_namespace".to_vec()
                    )),
                );
                assert_eq!(&resolver.buffer[s..], b"");
            }

            /// `xml` prefix cannot be unbound
            #[test]
            fn unbound() {
                let mut resolver = NamespaceResolver::default();
                let s = resolver.buffer.len();
                assert_eq!(
                    resolver.push(&BytesStart::from_content(" xmlns:xml=''", 0)),
                    Err(NamespaceError::InvalidXmlPrefixBind(b"".to_vec())),
                );
                assert_eq!(&resolver.buffer[s..], b"");
            }

            /// Other prefix cannot be bound to `xml` namespace
            #[test]
            fn other_prefix_bound_to_xml_namespace() {
                let mut resolver = NamespaceResolver::default();
                let s = resolver.buffer.len();
                assert_eq!(
                    resolver.push(&BytesStart::from_content(
                        " xmlns:not_xml='http://www.w3.org/XML/1998/namespace'",
                        0,
                    )),
                    Err(NamespaceError::InvalidPrefixForXml(b"not_xml".to_vec())),
                );
                assert_eq!(&resolver.buffer[s..], b"");
            }
        }

        mod xmlns {
            use super::*;
            use pretty_assertions::assert_eq;

            /// `xmlns` prefix are always defined, it is forbidden to define it explicitly
            #[test]
            fn undeclared() {
                let name = QName(b"xmlns:random");
                let namespace = RESERVED_NAMESPACE_XMLNS.1;

                let resolver = NamespaceResolver::default();

                assert_eq!(
                    resolver.resolve(name, true),
                    (Bound(namespace), LocalName(b"random"))
                );

                assert_eq!(
                    resolver.resolve(name, false),
                    (Bound(namespace), LocalName(b"random"))
                );
                assert_eq!(resolver.find(name), Bound(namespace));
            }

            /// `xmlns` prefix cannot be re-declared event to its own namespace
            #[test]
            fn rebound_to_correct_ns() {
                let mut resolver = NamespaceResolver::default();
                let s = resolver.buffer.len();
                assert_eq!(
                    resolver.push(&BytesStart::from_content(
                        " xmlns:xmlns='http://www.w3.org/2000/xmlns/'",
                        0,
                    )),
                    Err(NamespaceError::InvalidXmlnsPrefixBind(
                        b"http://www.w3.org/2000/xmlns/".to_vec()
                    )),
                );
                assert_eq!(&resolver.buffer[s..], b"");
            }

            /// `xmlns` prefix cannot be re-declared
            #[test]
            fn rebound_to_incorrect_ns() {
                let mut resolver = NamespaceResolver::default();
                let s = resolver.buffer.len();
                assert_eq!(
                    resolver.push(&BytesStart::from_content(
                        " xmlns:xmlns='not_correct_namespace'",
                        0,
                    )),
                    Err(NamespaceError::InvalidXmlnsPrefixBind(
                        b"not_correct_namespace".to_vec()
                    )),
                );
                assert_eq!(&resolver.buffer[s..], b"");
            }

            /// `xmlns` prefix cannot be unbound
            #[test]
            fn unbound() {
                let mut resolver = NamespaceResolver::default();
                let s = resolver.buffer.len();
                assert_eq!(
                    resolver.push(&BytesStart::from_content(" xmlns:xmlns=''", 0)),
                    Err(NamespaceError::InvalidXmlnsPrefixBind(b"".to_vec())),
                );
                assert_eq!(&resolver.buffer[s..], b"");
            }

            /// Other prefix cannot be bound to `xmlns` namespace
            #[test]
            fn other_prefix_bound_to_xmlns_namespace() {
                let mut resolver = NamespaceResolver::default();
                let s = resolver.buffer.len();
                assert_eq!(
                    resolver.push(&BytesStart::from_content(
                        " xmlns:not_xmlns='http://www.w3.org/2000/xmlns/'",
                        0,
                    )),
                    Err(NamespaceError::InvalidPrefixForXmlns(b"not_xmlns".to_vec())),
                );
                assert_eq!(&resolver.buffer[s..], b"");
            }
        }
    }

    #[test]
    fn undeclared_prefix() {
        let name = QName(b"unknown:prefix");

        let resolver = NamespaceResolver::default();

        assert_eq!(
            resolver.buffer,
            b"xmlhttp://www.w3.org/XML/1998/namespacexmlnshttp://www.w3.org/2000/xmlns/"
        );
        assert_eq!(
            resolver.resolve(name, true),
            (Unknown(b"unknown".to_vec()), LocalName(b"prefix"))
        );
        assert_eq!(
            resolver.resolve(name, false),
            (Unknown(b"unknown".to_vec()), LocalName(b"prefix"))
        );
        assert_eq!(resolver.find(name), Unknown(b"unknown".to_vec()));
    }

    /// Checks how the QName is decomposed to a prefix and a local name
    #[test]
    fn prefix_and_local_name() {
        let name = QName(b"foo:bus");
        assert_eq!(name.prefix(), Some(Prefix(b"foo")));
        assert_eq!(name.local_name(), LocalName(b"bus"));
        assert_eq!(name.decompose(), (LocalName(b"bus"), Some(Prefix(b"foo"))));

        let name = QName(b"foo:");
        assert_eq!(name.prefix(), Some(Prefix(b"foo")));
        assert_eq!(name.local_name(), LocalName(b""));
        assert_eq!(name.decompose(), (LocalName(b""), Some(Prefix(b"foo"))));

        let name = QName(b":foo");
        assert_eq!(name.prefix(), Some(Prefix(b"")));
        assert_eq!(name.local_name(), LocalName(b"foo"));
        assert_eq!(name.decompose(), (LocalName(b"foo"), Some(Prefix(b""))));

        let name = QName(b"foo:bus:baz");
        assert_eq!(name.prefix(), Some(Prefix(b"foo")));
        assert_eq!(name.local_name(), LocalName(b"bus:baz"));
        assert_eq!(
            name.decompose(),
            (LocalName(b"bus:baz"), Some(Prefix(b"foo")))
        );
    }
}
