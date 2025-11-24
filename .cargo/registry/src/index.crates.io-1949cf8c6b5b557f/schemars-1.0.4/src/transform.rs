/*!
Contains the [`Transform`] trait, used to modify a constructed schema and optionally its subschemas.
This trait is automatically implemented for functions of the form `fn(&mut Schema) -> ()`.

# Recursive Transforms

To make a transform recursive (i.e. apply it to subschemas), you have two options:
1. call the [`transform_subschemas`] function within the transform function
2. wrap the `Transform` in a [`RecursiveTransform`]

# Examples

To add a custom property to all object schemas:

```
# use schemars::{Schema, json_schema};
use schemars::transform::{Transform, transform_subschemas};

pub struct MyTransform;

impl Transform for MyTransform {
    fn transform(&mut self, schema: &mut Schema) {
        // First, make our change to this schema
        schema.insert("my_property".to_string(), "hello world".into());

        // Then apply the transform to any subschemas
        transform_subschemas(self, schema);
    }
}

let mut schema = json_schema!({
    "type": "array",
    "items": {}
});

MyTransform.transform(&mut schema);

assert_eq!(
    schema,
    json_schema!({
        "type": "array",
        "items": {
            "my_property": "hello world"
        },
        "my_property": "hello world"
    })
);
```

The same example with a `fn` transform:
```
# use schemars::{Schema, json_schema};
use schemars::transform::transform_subschemas;

fn add_property(schema: &mut Schema) {
    schema.insert("my_property".to_string(), "hello world".into());

    transform_subschemas(&mut add_property, schema)
}

let mut schema = json_schema!({
    "type": "array",
    "items": {}
});

add_property(&mut schema);

assert_eq!(
    schema,
    json_schema!({
        "type": "array",
        "items": {
            "my_property": "hello world"
        },
        "my_property": "hello world"
    })
);
```

And the same example using a closure wrapped in a `RecursiveTransform`:
```
# use schemars::{Schema, json_schema};
use schemars::transform::{Transform, RecursiveTransform};

let mut transform = RecursiveTransform(|schema: &mut Schema| {
    schema.insert("my_property".to_string(), "hello world".into());
});

let mut schema = json_schema!({
    "type": "array",
    "items": {}
});

transform.transform(&mut schema);

assert_eq!(
    schema,
    json_schema!({
        "type": "array",
        "items": {
            "my_property": "hello world"
        },
        "my_property": "hello world"
    })
);
```

*/
use crate::_alloc_prelude::*;
use crate::{consts::meta_schemas, Schema};
use alloc::borrow::Cow;
use alloc::collections::BTreeSet;
use serde_json::{json, Map, Value};

/// Trait used to modify a constructed schema and optionally its subschemas.
///
/// See the [module documentation](self) for more details on implementing this trait.
pub trait Transform {
    /// Applies the transform to the given [`Schema`].
    ///
    /// When overriding this method, you may want to call the [`transform_subschemas`] function to
    /// also transform any subschemas.
    fn transform(&mut self, schema: &mut Schema);

    // Not public API
    // Hack to enable implementing Debug on Box<dyn GenTransform> even though closures don't
    // implement Debug
    #[doc(hidden)]
    fn _debug_type_name(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(core::any::type_name::<Self>())
    }
}

impl<F> Transform for F
where
    F: FnMut(&mut Schema),
{
    fn transform(&mut self, schema: &mut Schema) {
        self(schema);
    }
}

/// Applies the given [`Transform`] to all direct subschemas of the [`Schema`].
pub fn transform_subschemas<T: Transform + ?Sized>(t: &mut T, schema: &mut Schema) {
    for (key, value) in schema.as_object_mut().into_iter().flatten() {
        // This is intentionally written to work with multiple JSON Schema versions, so that
        // users can add their own transforms on the end of e.g. `SchemaSettings::draft07()` and
        // they will still apply to all subschemas "as expected".
        // This is why this match statement contains both `additionalProperties` (which was
        // dropped in draft 2020-12) and `prefixItems` (which was added in draft 2020-12).
        match key.as_str() {
            "not"
            | "if"
            | "then"
            | "else"
            | "contains"
            | "additionalProperties"
            | "propertyNames"
            | "additionalItems" => {
                if let Ok(subschema) = value.try_into() {
                    t.transform(subschema);
                }
            }
            "allOf" | "anyOf" | "oneOf" | "prefixItems" => {
                if let Some(array) = value.as_array_mut() {
                    for value in array {
                        if let Ok(subschema) = value.try_into() {
                            t.transform(subschema);
                        }
                    }
                }
            }
            // Support `items` array even though this is not allowed in draft 2020-12 (see above
            // comment)
            "items" => {
                if let Some(array) = value.as_array_mut() {
                    for value in array {
                        if let Ok(subschema) = value.try_into() {
                            t.transform(subschema);
                        }
                    }
                } else if let Ok(subschema) = value.try_into() {
                    t.transform(subschema);
                }
            }
            "properties" | "patternProperties" | "$defs" | "definitions" => {
                if let Some(obj) = value.as_object_mut() {
                    for value in obj.values_mut() {
                        if let Ok(subschema) = value.try_into() {
                            t.transform(subschema);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// Similar to `transform_subschemas`, but only transforms subschemas that apply to the top-level
// object, e.g. "oneOf" but not "properties".
pub(crate) fn transform_immediate_subschemas<T: Transform + ?Sized>(
    t: &mut T,
    schema: &mut Schema,
) {
    for (key, value) in schema.as_object_mut().into_iter().flatten() {
        match key.as_str() {
            "if" | "then" | "else" => {
                if let Ok(subschema) = value.try_into() {
                    t.transform(subschema);
                }
            }
            "allOf" | "anyOf" | "oneOf" => {
                if let Some(array) = value.as_array_mut() {
                    for value in array {
                        if let Ok(subschema) = value.try_into() {
                            t.transform(subschema);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// A helper struct that can wrap a non-recursive [`Transform`] (i.e. one that does not apply to
/// subschemas) into a recursive one.
///
/// Its implementation of `Transform` will first apply the inner transform to the "parent" schema,
/// and then its subschemas (and their subschemas, and so on).
///
/// # Example
/// ```
/// # use schemars::{Schema, json_schema};
/// use schemars::transform::{Transform, RecursiveTransform};
///
/// let mut transform = RecursiveTransform(|schema: &mut Schema| {
///     schema.insert("my_property".to_string(), "hello world".into());
/// });
///
/// let mut schema = json_schema!({
///     "type": "array",
///     "items": {}
/// });
///
/// transform.transform(&mut schema);
///
/// assert_eq!(
///     schema,
///     json_schema!({
///         "type": "array",
///         "items": {
///             "my_property": "hello world"
///         },
///         "my_property": "hello world"
///     })
/// );
/// ```
#[derive(Debug, Clone)]
#[allow(clippy::exhaustive_structs)]
pub struct RecursiveTransform<T>(pub T);

impl<T> Transform for RecursiveTransform<T>
where
    T: Transform,
{
    fn transform(&mut self, schema: &mut Schema) {
        self.0.transform(schema);
        transform_subschemas(self, schema);
    }
}

/// Replaces boolean JSON Schemas with equivalent object schemas.
///
/// This also applies to subschemas.
///
/// This is useful for dialects of JSON Schema (e.g. OpenAPI 3.0) that do not support booleans as
/// schemas.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ReplaceBoolSchemas {
    /// When set to `true`, a schema's `additionalProperties` property will not be changed from a
    /// boolean.
    ///
    /// Defaults to `false`.
    pub skip_additional_properties: bool,
}

impl Transform for ReplaceBoolSchemas {
    fn transform(&mut self, schema: &mut Schema) {
        if let Some(obj) = schema.as_object_mut() {
            if self.skip_additional_properties {
                if let Some((ap_key, ap_value)) = obj.remove_entry("additionalProperties") {
                    transform_subschemas(self, schema);

                    schema.insert(ap_key, ap_value);

                    return;
                }
            }

            transform_subschemas(self, schema);
        } else {
            schema.ensure_object();
        }
    }
}

/// Restructures JSON Schema objects so that the `$ref` property will never appear alongside any
/// other properties.
///
/// This also applies to subschemas.
///
/// This is useful for versions of JSON Schema (e.g. Draft 7) that do not support other properties
/// alongside `$ref`.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct RemoveRefSiblings;

impl Transform for RemoveRefSiblings {
    fn transform(&mut self, schema: &mut Schema) {
        transform_subschemas(self, schema);

        if let Some(obj) = schema.as_object_mut().filter(|o| o.len() > 1) {
            if let Some(ref_value) = obj.remove("$ref") {
                if let Value::Array(all_of) = obj.entry("allOf").or_insert(Value::Array(Vec::new()))
                {
                    all_of.push(json!({
                        "$ref": ref_value
                    }));
                }
            }
        }
    }
}

/// Removes the `examples` schema property and (if present) set its first value as the `example`
/// property.
///
/// This also applies to subschemas.
///
/// This is useful for dialects of JSON Schema (e.g. OpenAPI 3.0) that do not support the `examples`
/// property.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SetSingleExample;

impl Transform for SetSingleExample {
    fn transform(&mut self, schema: &mut Schema) {
        transform_subschemas(self, schema);

        if let Some(Value::Array(examples)) = schema.remove("examples") {
            if let Some(first_example) = examples.into_iter().next() {
                schema.insert("example".into(), first_example);
            }
        }
    }
}

/// Replaces the `const` schema property with a single-valued `enum` property.
///
/// This also applies to subschemas.
///
/// This is useful for dialects of JSON Schema (e.g. OpenAPI 3.0) that do not support the `const`
/// property.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ReplaceConstValue;

impl Transform for ReplaceConstValue {
    fn transform(&mut self, schema: &mut Schema) {
        transform_subschemas(self, schema);

        if let Some(value) = schema.remove("const") {
            schema.insert("enum".into(), Value::Array(vec![value]));
        }
    }
}

/// Rename the `prefixItems` schema property to `items`.
///
/// This also applies to subschemas.
///
/// If the schema contains both `prefixItems` and `items`, then this additionally renames `items` to
/// `additionalItems`.
///
/// This is useful for versions of JSON Schema (e.g. Draft 7) that do not support the `prefixItems`
/// property.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ReplacePrefixItems;

impl Transform for ReplacePrefixItems {
    fn transform(&mut self, schema: &mut Schema) {
        transform_subschemas(self, schema);

        if let Some(prefix_items) = schema.remove("prefixItems") {
            let previous_items = schema.insert("items".to_owned(), prefix_items);

            if let Some(previous_items) = previous_items {
                schema.insert("additionalItems".to_owned(), previous_items);
            }
        }
    }
}

/// Adds a `"nullable": true` property to schemas that allow `null` types.
///
/// This also applies to subschemas.
///
/// This is useful for dialects of JSON Schema (e.g. OpenAPI 3.0) that use `nullable` instead of
/// explicit null types.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AddNullable {
    /// When set to `true` (the default), `"null"` will also be removed from the schemas `type`.
    pub remove_null_type: bool,
    /// When set to `true` (the default), a schema that has a type only allowing `null` will also
    /// have the equivalent `"const": null` inserted.
    pub add_const_null: bool,
}

impl Default for AddNullable {
    fn default() -> Self {
        Self {
            remove_null_type: true,
            add_const_null: true,
        }
    }
}

impl Transform for AddNullable {
    fn transform(&mut self, schema: &mut Schema) {
        if schema.has_type("null") {
            schema.insert("nullable".into(), true.into());

            // has_type returned true so we know "type" exists and is a string or array
            let ty = schema.get_mut("type").unwrap();
            let only_allows_null =
                ty.is_string() || ty.as_array().unwrap().iter().all(|v| v == "null");

            if only_allows_null {
                if self.add_const_null {
                    schema.insert("const".to_string(), Value::Null);

                    if self.remove_null_type {
                        schema.remove("type");
                    }
                } else if self.remove_null_type {
                    *ty = Value::Array(Vec::new());
                }
            } else if self.remove_null_type {
                // We know `type` is an array containing at least one non-null type
                let array = ty.as_array_mut().unwrap();
                array.retain(|t| t != "null");

                if array.len() == 1 {
                    *ty = array.remove(0);
                }
            }
        }

        transform_subschemas(self, schema);
    }
}

/// Replaces the `unevaluatedProperties` schema property with the `additionalProperties` property,
/// adding properties from a schema's subschemas to its `properties` where necessary.
///
/// This also applies to subschemas.
///
/// This is useful for versions of JSON Schema (e.g. Draft 7) that do not support the
/// `unevaluatedProperties` property.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ReplaceUnevaluatedProperties;

impl Transform for ReplaceUnevaluatedProperties {
    fn transform(&mut self, schema: &mut Schema) {
        transform_subschemas(self, schema);

        let Some(up) = schema.remove("unevaluatedProperties") else {
            return;
        };

        schema.insert("additionalProperties".to_owned(), up);

        let mut gather_property_names = GatherPropertyNames::default();
        gather_property_names.transform(schema);
        let property_names = gather_property_names.0;

        if property_names.is_empty() {
            return;
        }

        if let Some(properties) = schema
            .ensure_object()
            .entry("properties")
            .or_insert(Map::new().into())
            .as_object_mut()
        {
            for name in property_names {
                properties.entry(name).or_insert(true.into());
            }
        }
    }
}

// Helper for getting property names for all *immediate* subschemas
#[derive(Default)]
struct GatherPropertyNames(BTreeSet<String>);

impl Transform for GatherPropertyNames {
    fn transform(&mut self, schema: &mut Schema) {
        self.0.extend(
            schema
                .as_object()
                .iter()
                .filter_map(|o| o.get("properties"))
                .filter_map(Value::as_object)
                .flat_map(Map::keys)
                .cloned(),
        );

        transform_immediate_subschemas(self, schema);
    }
}

/// Removes any `format` values that are not defined by the JSON Schema standard or explicitly
/// allowed by a custom list.
///
/// This also applies to subschemas.
///
/// By default, this will infer the version of JSON Schema from the schema's `$schema` property,
/// and no additional formats will be allowed (even when the JSON schema allows nonstandard
/// formats).
///
/// # Example
/// ```
/// use schemars::json_schema;
/// use schemars::transform::{RestrictFormats, Transform};
///
/// let mut schema = schemars::json_schema!({
///     "$schema": "https://json-schema.org/draft/2020-12/schema",
///     "anyOf": [
///         {
///             "type": "string",
///             "format": "uuid"
///         },
///         {
///             "$schema": "http://json-schema.org/draft-07/schema#",
///             "type": "string",
///             "format": "uuid"
///         },
///         {
///             "type": "string",
///             "format": "allowed-custom-format"
///         },
///         {
///             "type": "string",
///             "format": "forbidden-custom-format"
///         }
///     ]
/// });
///
/// let mut transform = RestrictFormats::default();
/// transform.allowed_formats.insert("allowed-custom-format".into());
/// transform.transform(&mut schema);
///
/// assert_eq!(
///     schema,
///     json_schema!({
///         "$schema": "https://json-schema.org/draft/2020-12/schema",
///         "anyOf": [
///             {
///                 // "uuid" format is defined in draft 2020-12.
///                 "type": "string",
///                 "format": "uuid"
///             },
///             {
///                 // "uuid" format is not defined in draft-07, so is removed from this subschema.
///                 "$schema": "http://json-schema.org/draft-07/schema#",
///                 "type": "string"
///             },
///             {
///                 // "allowed-custom-format" format was present in `allowed_formats`...
///                 "type": "string",
///                 "format": "allowed-custom-format"
///             },
///             {
///                 // ...but "forbidden-custom-format" format was not, so is also removed.
///                 "type": "string"
///             }
///         ]
///     })
/// );
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RestrictFormats {
    /// Whether to read the schema's `$schema` property to determine which version of JSON Schema
    /// is being used, and allow only formats defined in that standard. If this is `true` but the
    /// JSON Schema version can't be determined because `$schema` is missing or unknown, then no
    /// `format` values will be removed.
    ///
    /// If this is set to `false`, then only the formats explicitly included in
    /// [`allowed_formats`](Self::allowed_formats) will be allowed.
    ///
    /// By default, this is `true`.
    pub infer_from_meta_schema: bool,
    /// Values of the `format` property in schemas that will always be allowed, regardless of the
    /// inferred version of JSON Schema.
    pub allowed_formats: BTreeSet<Cow<'static, str>>,
}

impl Default for RestrictFormats {
    fn default() -> Self {
        Self {
            infer_from_meta_schema: true,
            allowed_formats: BTreeSet::new(),
        }
    }
}

impl Transform for RestrictFormats {
    fn transform(&mut self, schema: &mut Schema) {
        let mut implementation = RestrictFormatsImpl {
            infer_from_meta_schema: self.infer_from_meta_schema,
            inferred_formats: None,
            allowed_formats: &self.allowed_formats,
        };

        implementation.transform(schema);
    }
}

static DEFINED_FORMATS: &[&str] = &[
    // `duration` and `uuid` are defined only in draft 2019-09+
    "duration",
    "uuid",
    // The rest are also defined in draft-07:
    "date-time",
    "date",
    "time",
    "email",
    "idn-email",
    "hostname",
    "idn-hostname",
    "ipv4",
    "ipv6",
    "uri",
    "uri-reference",
    "iri",
    "iri-reference",
    "uri-template",
    "json-pointer",
    "relative-json-pointer",
    "regex",
];

struct RestrictFormatsImpl<'a> {
    infer_from_meta_schema: bool,
    inferred_formats: Option<&'static [&'static str]>,
    allowed_formats: &'a BTreeSet<Cow<'static, str>>,
}

impl Transform for RestrictFormatsImpl<'_> {
    fn transform(&mut self, schema: &mut Schema) {
        let Some(obj) = schema.as_object_mut() else {
            return;
        };

        let previous_inferred_formats = self.inferred_formats;

        if self.infer_from_meta_schema && obj.contains_key("$schema") {
            self.inferred_formats = match obj
                .get("$schema")
                .and_then(Value::as_str)
                .unwrap_or_default()
            {
                meta_schemas::DRAFT07 => Some(&DEFINED_FORMATS[2..]),
                meta_schemas::DRAFT2019_09 | meta_schemas::DRAFT2020_12 => Some(DEFINED_FORMATS),
                _ => {
                    // we can't handle an unrecognised meta-schema
                    return;
                }
            };
        }

        if let Some(format) = obj.get("format").and_then(Value::as_str) {
            if !self.allowed_formats.contains(format)
                && !self
                    .inferred_formats
                    .is_some_and(|formats| formats.contains(&format))
            {
                obj.remove("format");
            }
        }

        transform_subschemas(self, schema);

        self.inferred_formats = previous_inferred_formats;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn restrict_formats() {
        let mut schema = json_schema!({
            "$schema": meta_schemas::DRAFT2020_12,
            "anyOf": [
                { "format": "uuid" },
                { "$schema": meta_schemas::DRAFT07, "format": "uuid" },
                { "$schema": "http://unknown", "format": "uuid" },
                { "format": "date" },
                { "$schema": meta_schemas::DRAFT07, "format": "date" },
                { "$schema": "http://unknown", "format": "date" },
                { "format": "custom1" },
                { "$schema": meta_schemas::DRAFT07, "format": "custom1" },
                { "$schema": "http://unknown", "format": "custom1" },
                { "format": "custom2" },
                { "$schema": meta_schemas::DRAFT07, "format": "custom2" },
                { "$schema": "http://unknown", "format": "custom2" },
            ]
        });

        let mut transform = RestrictFormats::default();
        transform.allowed_formats.insert("custom1".into());
        transform.transform(&mut schema);

        assert_eq!(
            schema,
            json_schema!({
                "$schema": meta_schemas::DRAFT2020_12,
                "anyOf": [
                    { "format": "uuid" },
                    { "$schema": meta_schemas::DRAFT07 },
                    { "$schema": "http://unknown", "format": "uuid" },
                    { "format": "date" },
                    { "$schema": meta_schemas::DRAFT07, "format": "date" },
                    { "$schema": "http://unknown", "format": "date" },
                    { "format": "custom1" },
                    { "$schema": meta_schemas::DRAFT07, "format": "custom1" },
                    { "$schema": "http://unknown", "format": "custom1" },
                    { },
                    { "$schema": meta_schemas::DRAFT07 },
                    { "$schema": "http://unknown", "format": "custom2" },
                ]
            })
        );
    }
}
