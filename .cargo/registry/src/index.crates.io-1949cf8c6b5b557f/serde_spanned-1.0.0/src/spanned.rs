use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

// Currently serde itself doesn't have a spanned type, so we map our `Spanned`
// to a special value in the serde data model. Namely one with these special
// fields/struct names.
//
// In general, supported deserializers should catch this and not literally emit
// these strings but rather emit `Spanned` as they're intended.
#[cfg(feature = "serde")]
pub(crate) const NAME: &str = "$__serde_spanned_private_Spanned";
#[cfg(feature = "serde")]
pub(crate) const START_FIELD: &str = "$__serde_spanned_private_start";
#[cfg(feature = "serde")]
pub(crate) const END_FIELD: &str = "$__serde_spanned_private_end";
#[cfg(feature = "serde")]
pub(crate) const VALUE_FIELD: &str = "$__serde_spanned_private_value";
#[cfg(feature = "serde")]
pub(crate) fn is_spanned(name: &'static str) -> bool {
    name == NAME
}

/// A spanned value, indicating the range at which it is defined in the source.
#[derive(Clone, Debug)]
pub struct Spanned<T> {
    /// Byte range
    span: core::ops::Range<usize>,
    /// The spanned value.
    value: T,
}

impl<T> Spanned<T> {
    /// Create a spanned value encompassing the given byte range.
    ///
    /// # Example
    ///
    /// Transposing a `Spanned<Enum<T>>` into `Enum<Spanned<T>>`:
    ///
    /// ```
    /// use serde::de::{Deserialize, Deserializer};
    /// use serde_untagged::UntaggedEnumVisitor;
    /// use toml::Spanned;
    ///
    /// pub enum Dependency {
    ///     Simple(Spanned<String>),
    ///     Detailed(Spanned<DetailedDependency>),
    /// }
    ///
    /// impl<'de> Deserialize<'de> for Dependency {
    ///     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    ///     where
    ///         D: Deserializer<'de>,
    ///     {
    ///         enum DependencyKind {
    ///             Simple(String),
    ///             Detailed(DetailedDependency),
    ///         }
    ///
    ///         impl<'de> Deserialize<'de> for DependencyKind {
    ///             fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    ///             where
    ///                 D: Deserializer<'de>,
    ///             {
    ///                 UntaggedEnumVisitor::new()
    ///                     .expecting(
    ///                         "a version string like \"0.9.8\" or a \
    ///                             detailed dependency like { version = \"0.9.8\" }",
    ///                     )
    ///                     .string(|value| Ok(DependencyKind::Simple(value.to_owned())))
    ///                     .map(|value| value.deserialize().map(DependencyKind::Detailed))
    ///                     .deserialize(deserializer)
    ///             }
    ///         }
    ///
    ///         let spanned: Spanned<DependencyKind> = Deserialize::deserialize(deserializer)?;
    ///         let range = spanned.span();
    ///         Ok(match spanned.into_inner() {
    ///             DependencyKind::Simple(simple) => Dependency::Simple(Spanned::new(range, simple)),
    ///             DependencyKind::Detailed(detailed) => Dependency::Detailed(Spanned::new(range, detailed)),
    ///         })
    ///     }
    /// }
    /// #
    /// # type DetailedDependency = std::collections::BTreeMap<String, String>;
    /// ```
    pub fn new(range: core::ops::Range<usize>, value: T) -> Self {
        Self { span: range, value }
    }

    /// Byte range
    pub fn span(&self) -> core::ops::Range<usize> {
        self.span.clone()
    }

    /// Consumes the spanned value and returns the contained value.
    pub fn into_inner(self) -> T {
        self.value
    }

    /// Returns a reference to the contained value.
    pub fn get_ref(&self) -> &T {
        &self.value
    }

    /// Returns a mutable reference to the contained value.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

#[cfg(feature = "serde")]
impl<T> Spanned<T> {
    pub(crate) const START_FIELD: &str = START_FIELD;
    pub(crate) const END_FIELD: &str = END_FIELD;
    pub(crate) const VALUE_FIELD: &str = VALUE_FIELD;
}

impl<T: core::fmt::Display> core::fmt::Display for Spanned<T> {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.get_ref().fmt(fmt)
    }
}

#[cfg(feature = "alloc")]
#[allow(unused_qualifications)]
impl core::borrow::Borrow<str> for Spanned<alloc::string::String> {
    fn borrow(&self) -> &str {
        self.get_ref()
    }
}

#[cfg(feature = "alloc")]
impl core::borrow::Borrow<str> for Spanned<alloc::borrow::Cow<'_, str>> {
    fn borrow(&self) -> &str {
        self.get_ref()
    }
}

impl<T> AsRef<T> for Spanned<T> {
    fn as_ref(&self) -> &T {
        self.get_ref()
    }
}

impl<T> AsMut<T> for Spanned<T> {
    fn as_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.eq(&other.value)
    }
}

impl<T: Eq> Eq for Spanned<T> {}

impl<T: Hash> Hash for Spanned<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T: PartialOrd> PartialOrd for Spanned<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl<T: Ord> Ord for Spanned<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::de::Deserialize<'de> for Spanned<T>
where
    T: serde::de::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct SpannedVisitor<T>(::core::marker::PhantomData<T>);

        impl<'de, T> serde::de::Visitor<'de> for SpannedVisitor<T>
        where
            T: serde::de::Deserialize<'de>,
        {
            type Value = Spanned<T>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a spanned value")
            }

            fn visit_map<V>(self, mut visitor: V) -> Result<Spanned<T>, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut start: Option<usize> = None;
                let mut end: Option<usize> = None;
                let mut value: Option<T> = None;
                while let Some(key) = visitor.next_key()? {
                    match key {
                        START_FIELD => {
                            if start.is_some() {
                                return Err(serde::de::Error::duplicate_field(START_FIELD));
                            }
                            start = Some(visitor.next_value()?);
                        }
                        END_FIELD => {
                            if end.is_some() {
                                return Err(serde::de::Error::duplicate_field(END_FIELD));
                            }
                            end = Some(visitor.next_value()?);
                        }
                        VALUE_FIELD => {
                            if value.is_some() {
                                return Err(serde::de::Error::duplicate_field(VALUE_FIELD));
                            }
                            value = Some(visitor.next_value()?);
                        }
                        field => {
                            return Err(serde::de::Error::unknown_field(
                                field,
                                &[START_FIELD, END_FIELD, VALUE_FIELD],
                            ));
                        }
                    }
                }
                match (start, end, value) {
                    (Some(start), Some(end), Some(value)) => Ok(Spanned {
                        span: start..end,
                        value,
                    }),
                    (None, _, _) => Err(serde::de::Error::missing_field(START_FIELD)),
                    (_, None, _) => Err(serde::de::Error::missing_field(END_FIELD)),
                    (_, _, None) => Err(serde::de::Error::missing_field(VALUE_FIELD)),
                }
            }
        }

        static FIELDS: [&str; 3] = [START_FIELD, END_FIELD, VALUE_FIELD];

        let visitor = SpannedVisitor(::core::marker::PhantomData);

        deserializer.deserialize_struct(NAME, &FIELDS, visitor)
    }
}

#[cfg(feature = "serde")]
impl<T: serde::ser::Serialize> serde::ser::Serialize for Spanned<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        self.value.serialize(serializer)
    }
}
