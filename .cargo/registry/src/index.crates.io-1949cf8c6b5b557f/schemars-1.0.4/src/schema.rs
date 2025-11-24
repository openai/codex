/*!
JSON Schema types.
*/

use crate::_alloc_prelude::*;
use ref_cast::{ref_cast_custom, RefCastCustom};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A JSON Schema.
///
/// This wraps a JSON [`Value`] that must be either an [object](Value::Object) or a
/// [bool](Value::Bool).
///
/// A custom JSON schema can be created using the [`json_schema!`](crate::json_schema) macro:
/// ```
/// use schemars::{Schema, json_schema};
///
/// let my_schema: Schema = json_schema!({
///     "type": ["object", "null"]
/// });
/// ```
///
/// Because a `Schema` is a thin wrapper around a `Value`, you can also use
/// [`TryFrom::try_from`]/[`TryInto::try_into`] to create a `Schema` from an existing `Value`.
/// This operation is fallible, because only [objects](Value::Object) and [bools](Value::Bool) can
/// be converted in this way.
///
/// ```
/// use schemars::{Schema, json_schema};
/// use serde_json::json;
///
/// let json_object = json!({"type": ["object", "null"]});
/// let object_schema: Schema = json_object.try_into().unwrap();
///
/// let json_bool = json!(true);
/// let bool_schema: Schema = json_bool.try_into().unwrap();
///
/// let json_string = json!("This is neither an object nor a bool!");
/// assert!(Schema::try_from(json_string).is_err());
///
/// // You can also convert a `&Value`/`&mut Value` to a `&Schema`/`&mut Schema` the same way:
///
/// let json_object = json!({"type": ["object", "null"]});
/// let object_schema_ref: &Schema = (&json_object).try_into().unwrap();
///
/// let mut json_object = json!({"type": ["object", "null"]});
/// let object_schema_mut: &mut Schema = (&mut json_object).try_into().unwrap();
/// ```
///
/// Similarly, you can use [`From`]/[`Into`] to (infallibly) create a `Schema` from an existing
/// [`Map<String, Value>`] or [`bool`].
///
/// ```
/// use schemars::{Schema, json_schema};
/// use serde_json::{Map, json};
///
/// let mut map = Map::new();
/// map.insert("type".to_owned(), json!(["object", "null"]));
/// let object_schema: Schema = map.into();
///
/// let bool_schema: Schema = true.into();
/// ```
#[derive(Debug, Clone, PartialEq, RefCastCustom)]
#[repr(transparent)]
pub struct Schema(Value);

impl<'de> Deserialize<'de> for Schema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Schema::validate(&value)?;
        Ok(Schema(value))
    }
}

impl Serialize for Schema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ser::OrderedKeywordWrapper::from(&self.0).serialize(serializer)
    }
}

impl PartialEq<bool> for Schema {
    fn eq(&self, other: &bool) -> bool {
        self.as_bool() == Some(*other)
    }
}

impl PartialEq<Map<String, Value>> for Schema {
    fn eq(&self, other: &Map<String, Value>) -> bool {
        self.as_object() == Some(other)
    }
}

impl PartialEq<Value> for Schema {
    fn eq(&self, other: &Value) -> bool {
        self.as_value() == other
    }
}

impl PartialEq<Schema> for bool {
    fn eq(&self, other: &Schema) -> bool {
        other == self
    }
}

impl PartialEq<Schema> for Map<String, Value> {
    fn eq(&self, other: &Schema) -> bool {
        other == self
    }
}

impl PartialEq<Schema> for Value {
    fn eq(&self, other: &Schema) -> bool {
        other == self
    }
}

impl Schema {
    /// Creates a new schema object with a single string property `"$ref"`.
    ///
    /// The given reference string should be a URI reference. This will usually be a JSON Pointer
    /// in [URI Fragment representation](https://tools.ietf.org/html/rfc6901#section-6).
    pub fn new_ref(reference: String) -> Self {
        let mut map = Map::new();
        map.insert("$ref".to_owned(), Value::String(reference));
        Self(Value::Object(map))
    }

    /// Borrows the `Schema`'s underlying JSON value.
    pub fn as_value(&self) -> &Value {
        &self.0
    }

    /// If the `Schema`'s underlying JSON value is a bool, returns the bool value.
    pub fn as_bool(&self) -> Option<bool> {
        self.0.as_bool()
    }

    /// If the `Schema`'s underlying JSON value is an object, borrows the object as a `Map` of
    /// properties.
    pub fn as_object(&self) -> Option<&Map<String, Value>> {
        self.0.as_object()
    }

    /// If the `Schema`'s underlying JSON value is an object, mutably borrows the object as a `Map`
    /// of properties.
    pub fn as_object_mut(&mut self) -> Option<&mut Map<String, Value>> {
        self.0.as_object_mut()
    }

    pub(crate) fn try_to_object(self) -> Result<Map<String, Value>, bool> {
        match self.0 {
            Value::Object(m) => Ok(m),
            Value::Bool(b) => Err(b),
            _ => unreachable!(),
        }
    }

    pub(crate) fn try_as_object_mut(&mut self) -> Result<&mut Map<String, Value>, bool> {
        match &mut self.0 {
            Value::Object(m) => Ok(m),
            Value::Bool(b) => Err(*b),
            _ => unreachable!(),
        }
    }

    /// Returns the `Schema`'s underlying JSON value.
    pub fn to_value(self) -> Value {
        self.0
    }

    /// Converts the `Schema` (if it wraps a bool value) into an equivalent object schema. Then
    /// mutably borrows the object as a `Map` of properties.
    ///
    /// `true` is transformed into an empty schema `{}`, which successfully validates against all
    /// possible values. `false` is transformed into the schema `{"not": {}}`, which does not
    /// successfully validate against any value.
    #[allow(clippy::missing_panics_doc)]
    pub fn ensure_object(&mut self) -> &mut Map<String, Value> {
        if let Some(b) = self.as_bool() {
            let mut map = Map::new();
            if !b {
                map.insert("not".into(), Value::Object(Map::new()));
            }
            self.0 = Value::Object(map);
        }

        self.0
            .as_object_mut()
            .expect("Schema value should be of type Object.")
    }

    /// Inserts a property into the schema, replacing any previous value.
    ///
    /// If the schema wraps a bool value, it will first be converted into an equivalent object
    /// schema.
    ///
    /// If the schema did not have this key present, `None` is returned.
    ///
    /// If the schema did have this key present, the value is updated, and the old value is
    /// returned.
    ///
    /// # Example
    /// ```
    /// use schemars::json_schema;
    /// use serde_json::json;
    ///
    /// let mut schema = json_schema!(true);
    /// assert_eq!(schema.insert("type".to_owned(), "array".into()), None);
    /// assert_eq!(schema.insert("type".to_owned(), "object".into()), Some(json!("array")));
    ///
    /// assert_eq!(schema, json_schema!({"type": "object"}));
    /// ```
    pub fn insert(&mut self, k: String, v: Value) -> Option<Value> {
        self.ensure_object().insert(k, v)
    }

    /// If the `Schema`'s underlying JSON value is an object, gets a reference to that object's
    /// value for the given key if it exists.
    ///
    /// This always returns `None` for bool schemas.
    ///
    /// # Example
    /// ```
    /// use schemars::json_schema;
    /// use serde_json::json;
    ///
    /// let obj_schema = json_schema!({"type": "array"});
    /// assert_eq!(obj_schema.get("type"), Some(&json!("array")));
    /// assert_eq!(obj_schema.get("format"), None);
    ///
    /// let bool_schema = json_schema!(true);
    /// assert_eq!(bool_schema.get("type"), None);
    /// ```
    pub fn get<Q>(&self, key: &Q) -> Option<&Value>
    where
        String: core::borrow::Borrow<Q>,
        Q: ?Sized + Ord + Eq + core::hash::Hash,
    {
        self.0.as_object().and_then(|o| o.get(key))
    }

    /// If the `Schema`'s underlying JSON value is an object, gets a mutable reference to that
    /// object's value for the given key if it exists.
    ///
    /// This always returns `None` for bool schemas.
    ///
    /// # Example
    /// ```
    /// use schemars::json_schema;
    /// use serde_json::{json, Value};
    ///
    /// let mut obj_schema = json_schema!({ "properties": {} });
    /// if let Some(Value::Object(properties)) = obj_schema.get_mut("properties") {
    ///     properties.insert("anything".to_owned(), true.into());
    /// }
    /// assert_eq!(obj_schema, json_schema!({ "properties": { "anything": true } }));
    /// ```
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Value>
    where
        String: core::borrow::Borrow<Q>,
        Q: ?Sized + Ord + Eq + core::hash::Hash,
    {
        self.0.as_object_mut().and_then(|o| o.get_mut(key))
    }

    /// If the `Schema`'s underlying JSON value is an object, looks up a value within the schema
    /// by a JSON Pointer.
    ///
    /// If the given pointer begins with a `#`, then the rest of the value is assumed to be in
    /// "URI Fragment Identifier Representation", and will be percent-decoded accordingly.
    ///
    /// For more information on JSON Pointer, read [RFC6901](https://tools.ietf.org/html/rfc6901).
    ///
    /// This always returns `None` for bool schemas.
    ///
    /// # Example
    /// ```
    /// use schemars::json_schema;
    /// use serde_json::json;
    ///
    /// let schema = json_schema!({
    ///     "properties": {
    ///         "anything": true
    ///     },
    ///     "$defs": {
    ///         "🚀": true
    ///     }
    /// });
    ///
    /// assert_eq!(schema.pointer("/properties/anything").unwrap(), &json!(true));
    /// assert_eq!(schema.pointer("#/$defs/%F0%9F%9A%80").unwrap(), &json!(true));
    /// assert_eq!(schema.pointer("/does/not/exist"), None);
    /// ```
    pub fn pointer(&self, pointer: &str) -> Option<&Value> {
        if let Some(percent_encoded) = pointer.strip_prefix('#') {
            let decoded = crate::encoding::percent_decode(percent_encoded)?;
            self.0.pointer(&decoded)
        } else {
            self.0.pointer(pointer)
        }
    }

    /// If the `Schema`'s underlying JSON value is an object, looks up a value by a JSON Pointer
    /// and returns a mutable reference to that value.
    ///
    /// For more information on JSON Pointer, read [RFC6901](https://tools.ietf.org/html/rfc6901).
    ///
    /// This always returns `None` for bool schemas.
    ///
    /// # Example
    /// ```
    /// use schemars::{json_schema, Schema};
    /// use serde_json::json;
    ///
    /// let mut schema = json_schema!({
    ///     "properties": {
    ///         "anything": true
    ///     }
    /// });
    ///
    /// let subschema_value = schema.pointer_mut("/properties/anything").unwrap();
    /// let subschema: &mut Schema = subschema_value.try_into().unwrap();
    /// subschema.ensure_object();
    ///
    /// assert_eq!(schema.pointer_mut("/properties/anything").unwrap(), &json!({}));
    /// ```
    pub fn pointer_mut(&mut self, pointer: &str) -> Option<&mut Value> {
        self.0.pointer_mut(pointer)
    }

    /// If the `Schema`'s underlying JSON value is an object, removes and returns its value for the
    /// given key.
    ///
    /// This always returns `None` for bool schemas, without modifying them.
    ///
    /// # Example
    /// ```
    /// use schemars::json_schema;
    /// use serde_json::json;
    ///
    /// let mut schema = json_schema!({"type": "array"});
    /// assert_eq!(schema.remove("type"), Some(json!("array")));
    /// assert_eq!(schema, json_schema!({}));
    /// ```
    pub fn remove<Q>(&mut self, key: &Q) -> Option<Value>
    where
        String: core::borrow::Borrow<Q>,
        Q: ?Sized + Ord + Eq + core::hash::Hash,
    {
        self.0.as_object_mut().and_then(|o| o.remove(key))
    }

    pub(crate) fn has_type(&self, ty: &str) -> bool {
        match self.0.get("type") {
            Some(Value::Array(values)) => values.iter().any(|v| v.as_str() == Some(ty)),
            Some(Value::String(s)) => s == ty,
            _ => false,
        }
    }

    fn validate<E: serde::de::Error>(value: &Value) -> Result<(), E> {
        use serde::de::Unexpected;
        let unexpected = match value {
            Value::Bool(_) | Value::Object(_) => return Ok(()),
            Value::Null => Unexpected::Unit,
            Value::Number(n) => {
                if let Some(u) = n.as_u64() {
                    Unexpected::Unsigned(u)
                } else if let Some(i) = n.as_i64() {
                    Unexpected::Signed(i)
                } else if let Some(f) = n.as_f64() {
                    Unexpected::Float(f)
                } else {
                    unreachable!()
                }
            }
            Value::String(s) => Unexpected::Str(s),
            Value::Array(_) => Unexpected::Seq,
        };

        Err(E::invalid_type(unexpected, &"object or boolean"))
    }

    #[ref_cast_custom]
    fn ref_cast(value: &Value) -> &Self;

    #[ref_cast_custom]
    fn ref_cast_mut(value: &mut Value) -> &mut Self;
}

impl From<Schema> for Value {
    fn from(v: Schema) -> Value {
        v.0
    }
}

impl core::convert::TryFrom<Value> for Schema {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> serde_json::Result<Schema> {
        Schema::validate(&value)?;
        Ok(Schema(value))
    }
}

impl<'a> core::convert::TryFrom<&'a Value> for &'a Schema {
    type Error = serde_json::Error;

    fn try_from(value: &Value) -> serde_json::Result<&Schema> {
        Schema::validate(value)?;
        Ok(Schema::ref_cast(value))
    }
}

impl<'a> core::convert::TryFrom<&'a mut Value> for &'a mut Schema {
    type Error = serde_json::Error;

    fn try_from(value: &mut Value) -> serde_json::Result<&mut Schema> {
        Schema::validate(value)?;
        Ok(Schema::ref_cast_mut(value))
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self(Value::Object(Map::new()))
    }
}

impl From<Map<String, Value>> for Schema {
    fn from(o: Map<String, Value>) -> Self {
        Schema(Value::Object(o))
    }
}

impl From<bool> for Schema {
    fn from(b: bool) -> Self {
        Schema(Value::Bool(b))
    }
}

impl crate::JsonSchema for Schema {
    fn schema_name() -> alloc::borrow::Cow<'static, str> {
        "Schema".into()
    }

    fn schema_id() -> alloc::borrow::Cow<'static, str> {
        "schemars::Schema".into()
    }

    fn json_schema(_: &mut crate::SchemaGenerator) -> Schema {
        crate::json_schema!({
            "type": ["object", "boolean"]
        })
    }
}

mod ser {
    use serde::ser::{Serialize, SerializeMap, SerializeSeq};
    use serde_json::Value;

    // The order of properties in a JSON Schema object is insignificant, but we explicitly order
    // some of them here to make them easier for a human to read. All other properties are ordered
    // either lexicographically (by default) or by insertion order (if `preserve_order` is enabled)
    const ORDERED_KEYWORDS_START: [&str; 7] = [
        "$id",
        "$schema",
        "title",
        "description",
        "type",
        "format",
        "properties",
    ];
    const ORDERED_KEYWORDS_END: [&str; 2] = ["$defs", "definitions"];

    // `no_reorder` is true when the value is expected to be an object that is NOT a schema,
    // but the object's property values are expected to be schemas. In this case, we do not
    // reorder the object's direct properties, but we do reorder nested (subschema) properties.
    //
    // When `no_reorder` is false, then the value is expected to be one of:
    // - a JSON schema object
    // - an array of JSON schemas
    // - a JSON primitive value (null/string/number/bool)
    //
    // If any of these expectations are not met, then the value should still be serialized in a
    // valid way, but the property ordering may be unclear.
    pub(super) struct OrderedKeywordWrapper<'a> {
        value: &'a Value,
        no_reorder: bool,
    }

    impl<'a> From<&'a Value> for OrderedKeywordWrapper<'a> {
        fn from(value: &'a Value) -> Self {
            Self {
                value,
                no_reorder: false,
            }
        }
    }

    impl Serialize for OrderedKeywordWrapper<'_> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            fn serialize_schema_property<S>(
                map: &mut S::SerializeMap,
                key: &str,
                value: &Value,
            ) -> Result<(), S::Error>
            where
                S: serde::Serializer,
            {
                if matches!(key, "examples" | "default") || key.starts_with("x-") {
                    // Value(s) of `examples`/`default` are plain values, not schemas.
                    // Also don't reorder values of custom properties.
                    map.serialize_entry(key, value)
                } else {
                    let no_reorder = matches!(
                        key,
                        "properties"
                            | "patternProperties"
                            | "dependentSchemas"
                            | "$defs"
                            | "definitions"
                    );
                    map.serialize_entry(key, &OrderedKeywordWrapper { value, no_reorder })
                }
            }

            match self.value {
                Value::Array(array) => {
                    let mut seq = serializer.serialize_seq(Some(array.len()))?;
                    for value in array {
                        seq.serialize_element(&OrderedKeywordWrapper::from(value))?;
                    }
                    seq.end()
                }
                Value::Object(object) if self.no_reorder => {
                    let mut map = serializer.serialize_map(Some(object.len()))?;

                    for (key, value) in object {
                        // Don't use `serialize_schema_property` because `object` is NOT expected
                        // to be a schema (but `value` is expected to be a schema)
                        map.serialize_entry(key, &OrderedKeywordWrapper::from(value))?;
                    }

                    map.end()
                }
                Value::Object(object) => {
                    let mut map = serializer.serialize_map(Some(object.len()))?;

                    for key in ORDERED_KEYWORDS_START {
                        if let Some(value) = object.get(key) {
                            serialize_schema_property::<S>(&mut map, key, value)?;
                        }
                    }

                    for (key, value) in object {
                        if !ORDERED_KEYWORDS_START.contains(&key.as_str())
                            && !ORDERED_KEYWORDS_END.contains(&key.as_str())
                        {
                            serialize_schema_property::<S>(&mut map, key, value)?;
                        }
                    }

                    for key in ORDERED_KEYWORDS_END {
                        if let Some(value) = object.get(key) {
                            serialize_schema_property::<S>(&mut map, key, value)?;
                        }
                    }

                    map.end()
                }
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                    self.value.serialize(serializer)
                }
            }
        }
    }
}
