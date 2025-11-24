use std::borrow::{Borrow, Cow};
use std::sync::Arc;
use std::{fmt, hash};

use std::hash::{Hash, Hasher};

/// The key part of attribute [KeyValue] pairs.
///
/// See the [attribute naming] spec for guidelines.
///
/// [attribute naming]: https://github.com/open-telemetry/semantic-conventions/blob/main/docs/general/attribute-naming.md
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(OtelString);

impl Key {
    /// Create a new `Key`.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::Key;
    /// use std::sync::Arc;
    ///
    /// let key1 = Key::new("my_static_str");
    /// let key2 = Key::new(String::from("my_owned_string"));
    /// let key3 = Key::new(Arc::from("my_ref_counted_str"));
    /// ```
    pub fn new(value: impl Into<Key>) -> Self {
        value.into()
    }

    /// Create a new const `Key`.
    pub const fn from_static_str(value: &'static str) -> Self {
        Key(OtelString::Static(value))
    }

    /// Returns a reference to the underlying key name
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&'static str> for Key {
    /// Convert a `&str` to a `Key`.
    fn from(key_str: &'static str) -> Self {
        Key(OtelString::Static(key_str))
    }
}

impl From<String> for Key {
    /// Convert a `String` to a `Key`.
    fn from(string: String) -> Self {
        Key(OtelString::Owned(string.into_boxed_str()))
    }
}

impl From<Arc<str>> for Key {
    /// Convert a `String` to a `Key`.
    fn from(string: Arc<str>) -> Self {
        Key(OtelString::RefCounted(string))
    }
}

impl From<Cow<'static, str>> for Key {
    /// Convert a `Cow<'static, str>` to a `Key`
    fn from(string: Cow<'static, str>) -> Self {
        match string {
            Cow::Borrowed(s) => Key(OtelString::Static(s)),
            Cow::Owned(s) => Key(OtelString::Owned(s.into_boxed_str())),
        }
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

impl From<Key> for String {
    fn from(key: Key) -> Self {
        match key.0 {
            OtelString::Owned(s) => s.to_string(),
            OtelString::Static(s) => s.to_string(),
            OtelString::RefCounted(s) => s.to_string(),
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            OtelString::Owned(s) => s.fmt(fmt),
            OtelString::Static(s) => s.fmt(fmt),
            OtelString::RefCounted(s) => s.fmt(fmt),
        }
    }
}

impl Borrow<str> for Key {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for Key {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Eq)]
enum OtelString {
    Owned(Box<str>),
    Static(&'static str),
    RefCounted(Arc<str>),
}

impl OtelString {
    fn as_str(&self) -> &str {
        match self {
            OtelString::Owned(s) => s.as_ref(),
            OtelString::Static(s) => s,
            OtelString::RefCounted(s) => s.as_ref(),
        }
    }
}

impl PartialOrd for OtelString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OtelString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialEq for OtelString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl hash::Hash for OtelString {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

/// A [Value::Array] containing homogeneous values.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum Array {
    /// Array of bools
    Bool(Vec<bool>),
    /// Array of integers
    I64(Vec<i64>),
    /// Array of floats
    F64(Vec<f64>),
    /// Array of strings
    String(Vec<StringValue>),
}

impl fmt::Display for Array {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Array::Bool(values) => display_array_str(values, fmt),
            Array::I64(values) => display_array_str(values, fmt),
            Array::F64(values) => display_array_str(values, fmt),
            Array::String(values) => {
                write!(fmt, "[")?;
                for (i, t) in values.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ",")?;
                    }
                    write!(fmt, "\"{}\"", t)?;
                }
                write!(fmt, "]")
            }
        }
    }
}

fn display_array_str<T: fmt::Display>(slice: &[T], fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(fmt, "[")?;
    for (i, t) in slice.iter().enumerate() {
        if i > 0 {
            write!(fmt, ",")?;
        }
        write!(fmt, "{}", t)?;
    }
    write!(fmt, "]")
}

macro_rules! into_array {
    ($(($t:ty, $val:expr),)+) => {
        $(
            impl From<$t> for Array {
                fn from(t: $t) -> Self {
                    $val(t)
                }
            }
        )+
    }
}

into_array!(
    (Vec<bool>, Array::Bool),
    (Vec<i64>, Array::I64),
    (Vec<f64>, Array::F64),
    (Vec<StringValue>, Array::String),
);

/// The value part of attribute [KeyValue] pairs.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// bool values
    Bool(bool),
    /// i64 values
    I64(i64),
    /// f64 values
    F64(f64),
    /// String values
    String(StringValue),
    /// Array of homogeneous values
    Array(Array),
}

/// Wrapper for string-like values
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct StringValue(OtelString);

impl fmt::Debug for StringValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for StringValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            OtelString::Owned(s) => s.fmt(f),
            OtelString::Static(s) => s.fmt(f),
            OtelString::RefCounted(s) => s.fmt(f),
        }
    }
}

impl AsRef<str> for StringValue {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl StringValue {
    /// Returns a string slice to this value
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<StringValue> for String {
    fn from(s: StringValue) -> Self {
        match s.0 {
            OtelString::Owned(s) => s.to_string(),
            OtelString::Static(s) => s.to_string(),
            OtelString::RefCounted(s) => s.to_string(),
        }
    }
}

impl From<&'static str> for StringValue {
    fn from(s: &'static str) -> Self {
        StringValue(OtelString::Static(s))
    }
}

impl From<String> for StringValue {
    fn from(s: String) -> Self {
        StringValue(OtelString::Owned(s.into_boxed_str()))
    }
}

impl From<Arc<str>> for StringValue {
    fn from(s: Arc<str>) -> Self {
        StringValue(OtelString::RefCounted(s))
    }
}

impl From<Cow<'static, str>> for StringValue {
    fn from(s: Cow<'static, str>) -> Self {
        match s {
            Cow::Owned(s) => StringValue(OtelString::Owned(s.into_boxed_str())),
            Cow::Borrowed(s) => StringValue(OtelString::Static(s)),
        }
    }
}

impl From<Value> for StringValue {
    fn from(s: Value) -> Self {
        match s {
            Value::Bool(v) => format!("{}", v).into(),
            Value::I64(v) => format!("{}", v).into(),
            Value::F64(v) => format!("{}", v).into(),
            Value::String(v) => v,
            Value::Array(v) => format!("{}", v).into(),
        }
    }
}

impl Value {
    /// String representation of the `Value`
    ///
    /// This will allocate if the underlying value is not a `String`.
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            Value::Bool(v) => format!("{}", v).into(),
            Value::I64(v) => format!("{}", v).into(),
            Value::F64(v) => format!("{}", v).into(),
            Value::String(v) => Cow::Borrowed(v.as_str()),
            Value::Array(v) => format!("{}", v).into(),
        }
    }
}

macro_rules! from_values {
   (
        $(
            ($t:ty, $val:expr);
        )+
    ) => {
        $(
            impl From<$t> for Value {
                fn from(t: $t) -> Self {
                    $val(t)
                }
            }
        )+
    }
}

from_values!(
    (bool, Value::Bool);
    (i64, Value::I64);
    (f64, Value::F64);
    (StringValue, Value::String);
);

impl From<&'static str> for Value {
    fn from(s: &'static str) -> Self {
        Value::String(s.into())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s.into())
    }
}

impl From<Arc<str>> for Value {
    fn from(s: Arc<str>) -> Self {
        Value::String(s.into())
    }
}

impl From<Cow<'static, str>> for Value {
    fn from(s: Cow<'static, str>) -> Self {
        Value::String(s.into())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(v) => v.fmt(fmt),
            Value::I64(v) => v.fmt(fmt),
            Value::F64(v) => v.fmt(fmt),
            Value::String(v) => fmt.write_str(v.as_str()),
            Value::Array(v) => v.fmt(fmt),
        }
    }
}

/// A key-value pair describing an attribute.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct KeyValue {
    /// The attribute name
    pub key: Key,

    /// The attribute value
    pub value: Value,
}

impl KeyValue {
    /// Create a new `KeyValue` pair.
    pub fn new<K, V>(key: K, value: V) -> Self
    where
        K: Into<Key>,
        V: Into<Value>,
    {
        KeyValue {
            key: key.into(),
            value: value.into(),
        }
    }
}

struct F64Hashable(f64);

impl PartialEq for F64Hashable {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for F64Hashable {}

impl Hash for F64Hashable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl Hash for KeyValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        match &self.value {
            Value::F64(f) => F64Hashable(*f).hash(state),
            Value::Array(a) => match a {
                Array::Bool(b) => b.hash(state),
                Array::I64(i) => i.hash(state),
                Array::F64(f) => f.iter().for_each(|f| F64Hashable(*f).hash(state)),
                Array::String(s) => s.hash(state),
            },
            Value::Bool(b) => b.hash(state),
            Value::I64(i) => i.hash(state),
            Value::String(s) => s.hash(state),
        };
    }
}

impl Eq for KeyValue {}

/// Information about a library or crate providing instrumentation.
///
/// An instrumentation scope should be named to follow any naming conventions
/// of the instrumented library (e.g. 'middleware' for a web framework).
///
/// See the [instrumentation libraries] spec for more information.
///
/// [instrumentation libraries]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/overview.md#instrumentation-libraries
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct InstrumentationScope {
    /// The library name.
    ///
    /// This should be the name of the crate providing the instrumentation.
    name: Cow<'static, str>,

    /// The library version.
    version: Option<Cow<'static, str>>,

    /// [Schema URL] used by this library.
    ///
    /// [Schema URL]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/schemas/overview.md#schema-url
    schema_url: Option<Cow<'static, str>>,

    /// Specifies the instrumentation scope attributes to associate with emitted telemetry.
    attributes: Vec<KeyValue>,
}

impl PartialEq for InstrumentationScope {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.version == other.version
            && self.schema_url == other.schema_url
            && self.attributes.len() == other.attributes.len()
            && {
                let mut self_attrs = Vec::from_iter(&self.attributes);
                let mut other_attrs = Vec::from_iter(&other.attributes);
                self_attrs.sort_unstable_by(|a, b| a.key.cmp(&b.key));
                other_attrs.sort_unstable_by(|a, b| a.key.cmp(&b.key));
                self_attrs == other_attrs
            }
    }
}

impl Eq for InstrumentationScope {}

impl hash::Hash for InstrumentationScope {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.version.hash(state);
        self.schema_url.hash(state);
        let mut sorted_attrs = Vec::from_iter(&self.attributes);
        sorted_attrs.sort_unstable_by(|a, b| a.key.cmp(&b.key));
        for attribute in sorted_attrs {
            attribute.hash(state);
        }
    }
}

impl InstrumentationScope {
    /// Create a new builder to create an [InstrumentationScope]
    pub fn builder<T: Into<Cow<'static, str>>>(name: T) -> InstrumentationScopeBuilder {
        InstrumentationScopeBuilder {
            name: name.into(),
            version: None,
            schema_url: None,
            attributes: None,
        }
    }

    /// Returns the instrumentation library name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the instrumentation library version.
    #[inline]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Returns the [Schema URL] used by this library.
    ///
    /// [Schema URL]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/schemas/overview.md#schema-url
    #[inline]
    pub fn schema_url(&self) -> Option<&str> {
        self.schema_url.as_deref()
    }

    /// Returns the instrumentation scope attributes to associate with emitted telemetry.
    #[inline]
    pub fn attributes(&self) -> impl Iterator<Item = &KeyValue> {
        self.attributes.iter()
    }
}

/// Configuration options for [InstrumentationScope].
///
/// An instrumentation scope is a library or crate providing instrumentation.
/// It should be named to follow any naming conventions of the instrumented
/// library (e.g. 'middleware' for a web framework).
///
/// Apart from the name, all other fields are optional.
///
/// See the [instrumentation libraries] spec for more information.
///
/// [instrumentation libraries]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/overview.md#instrumentation-libraries
#[derive(Debug)]
pub struct InstrumentationScopeBuilder {
    name: Cow<'static, str>,
    version: Option<Cow<'static, str>>,
    schema_url: Option<Cow<'static, str>>,
    attributes: Option<Vec<KeyValue>>,
}

impl InstrumentationScopeBuilder {
    /// Configure the version for the instrumentation scope
    ///
    /// # Examples
    ///
    /// ```
    /// let scope = opentelemetry::InstrumentationScope::builder("my-crate")
    ///     .with_version("v0.1.0")
    ///     .build();
    /// ```
    pub fn with_version(mut self, version: impl Into<Cow<'static, str>>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Configure the Schema URL for the instrumentation scope
    ///
    /// # Examples
    ///
    /// ```
    /// let scope = opentelemetry::InstrumentationScope::builder("my-crate")
    ///     .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
    ///     .build();
    /// ```
    pub fn with_schema_url(mut self, schema_url: impl Into<Cow<'static, str>>) -> Self {
        self.schema_url = Some(schema_url.into());
        self
    }

    /// Configure the attributes for the instrumentation scope
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::KeyValue;
    ///
    /// let scope = opentelemetry::InstrumentationScope::builder("my-crate")
    ///     .with_attributes([KeyValue::new("k", "v")])
    ///     .build();
    /// ```
    pub fn with_attributes<I>(mut self, attributes: I) -> Self
    where
        I: IntoIterator<Item = KeyValue>,
    {
        self.attributes = Some(attributes.into_iter().collect());
        self
    }

    /// Create a new [InstrumentationScope] from this configuration
    pub fn build(self) -> InstrumentationScope {
        InstrumentationScope {
            name: self.name,
            version: self.version,
            schema_url: self.schema_url,
            attributes: self.attributes.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::hash::{Hash, Hasher};

    use crate::{InstrumentationScope, KeyValue};

    use rand::random;
    use std::collections::hash_map::DefaultHasher;
    use std::f64;

    #[test]
    fn kv_float_equality() {
        let kv1 = KeyValue::new("key", 1.0);
        let kv2 = KeyValue::new("key", 1.0);
        assert_eq!(kv1, kv2);

        let kv1 = KeyValue::new("key", 1.0);
        let kv2 = KeyValue::new("key", 1.01);
        assert_ne!(kv1, kv2);

        let kv1 = KeyValue::new("key", f64::NAN);
        let kv2 = KeyValue::new("key", f64::NAN);
        assert_ne!(kv1, kv2, "NAN is not equal to itself");

        for float_val in [
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
        ]
        .iter()
        {
            let kv1 = KeyValue::new("key", *float_val);
            let kv2 = KeyValue::new("key", *float_val);
            assert_eq!(kv1, kv2);
        }

        for _ in 0..100 {
            let random_value = random::<f64>();
            let kv1 = KeyValue::new("key", random_value);
            let kv2 = KeyValue::new("key", random_value);
            assert_eq!(kv1, kv2);
        }
    }

    #[test]
    fn kv_float_hash() {
        for float_val in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
        ]
        .iter()
        {
            let kv1 = KeyValue::new("key", *float_val);
            let kv2 = KeyValue::new("key", *float_val);
            assert_eq!(hash_helper(&kv1), hash_helper(&kv2));
        }

        for _ in 0..100 {
            let random_value = random::<f64>();
            let kv1 = KeyValue::new("key", random_value);
            let kv2 = KeyValue::new("key", random_value);
            assert_eq!(hash_helper(&kv1), hash_helper(&kv2));
        }
    }

    fn hash_helper<T: Hash>(item: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn instrumentation_scope_equality() {
        let scope1 = InstrumentationScope::builder("my-crate")
            .with_version("v0.1.0")
            .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
            .with_attributes([KeyValue::new("k", "v")])
            .build();
        let scope2 = InstrumentationScope::builder("my-crate")
            .with_version("v0.1.0")
            .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
            .with_attributes([KeyValue::new("k", "v")])
            .build();
        assert_eq!(scope1, scope2);
    }

    #[test]
    fn instrumentation_scope_equality_attributes_diff_order() {
        let scope1 = InstrumentationScope::builder("my-crate")
            .with_version("v0.1.0")
            .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
            .with_attributes([KeyValue::new("k1", "v1"), KeyValue::new("k2", "v2")])
            .build();
        let scope2 = InstrumentationScope::builder("my-crate")
            .with_version("v0.1.0")
            .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
            .with_attributes([KeyValue::new("k2", "v2"), KeyValue::new("k1", "v1")])
            .build();
        assert_eq!(scope1, scope2);

        // assert hash are same for both scopes
        let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
        scope1.hash(&mut hasher1);
        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
        scope2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn instrumentation_scope_equality_different_attributes() {
        let scope1 = InstrumentationScope::builder("my-crate")
            .with_version("v0.1.0")
            .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
            .with_attributes([KeyValue::new("k1", "v1"), KeyValue::new("k2", "v2")])
            .build();
        let scope2 = InstrumentationScope::builder("my-crate")
            .with_version("v0.1.0")
            .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
            .with_attributes([KeyValue::new("k2", "v3"), KeyValue::new("k4", "v5")])
            .build();
        assert_ne!(scope1, scope2);

        // assert hash are same for both scopes
        let mut hasher1 = std::collections::hash_map::DefaultHasher::new();
        scope1.hash(&mut hasher1);
        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
        scope2.hash(&mut hasher2);
        assert_ne!(hasher1.finish(), hasher2.finish());
    }
}
