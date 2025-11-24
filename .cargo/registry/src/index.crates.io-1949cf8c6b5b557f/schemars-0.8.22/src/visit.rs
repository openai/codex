/*!
Contains the [`Visitor`] trait, used to recursively modify a constructed schema and its subschemas.

Sometimes you may want to apply a change to a schema, as well as all schemas contained within it.
The easiest way to achieve this is by defining a type that implements [`Visitor`].
All methods of `Visitor` have a default implementation that makes no change but recursively visits all subschemas.
When overriding one of these methods, you will *usually* want to still call this default implementation.

# Example
To add a custom property to all schemas:
```
use schemars::schema::SchemaObject;
use schemars::visit::{Visitor, visit_schema_object};

pub struct MyVisitor;

impl Visitor for MyVisitor {
    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        // First, make our change to this schema
        schema.extensions.insert("my_property".to_string(), serde_json::json!("hello world"));

        // Then delegate to default implementation to visit any subschemas
        visit_schema_object(self, schema);
    }
}
```
*/
use crate::schema::{RootSchema, Schema, SchemaObject, SingleOrVec};

/// Trait used to recursively modify a constructed schema and its subschemas.
pub trait Visitor {
    /// Override this method to modify a [`RootSchema`] and (optionally) its subschemas.
    ///
    /// When overriding this method, you will usually want to call the [`visit_root_schema`] function to visit subschemas.
    fn visit_root_schema(&mut self, root: &mut RootSchema) {
        visit_root_schema(self, root)
    }

    /// Override this method to modify a [`Schema`] and (optionally) its subschemas.
    ///
    /// When overriding this method, you will usually want to call the [`visit_schema`] function to visit subschemas.
    fn visit_schema(&mut self, schema: &mut Schema) {
        visit_schema(self, schema)
    }

    /// Override this method to modify a [`SchemaObject`] and (optionally) its subschemas.
    ///
    /// When overriding this method, you will usually want to call the [`visit_schema_object`] function to visit subschemas.
    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        visit_schema_object(self, schema)
    }
}

/// Visits all subschemas of the [`RootSchema`].
pub fn visit_root_schema<V: Visitor + ?Sized>(v: &mut V, root: &mut RootSchema) {
    v.visit_schema_object(&mut root.schema);
    visit_map_values(v, &mut root.definitions);
}

/// Visits all subschemas of the [`Schema`].
pub fn visit_schema<V: Visitor + ?Sized>(v: &mut V, schema: &mut Schema) {
    if let Schema::Object(schema) = schema {
        v.visit_schema_object(schema)
    }
}

/// Visits all subschemas of the [`SchemaObject`].
pub fn visit_schema_object<V: Visitor + ?Sized>(v: &mut V, schema: &mut SchemaObject) {
    if let Some(sub) = &mut schema.subschemas {
        visit_vec(v, &mut sub.all_of);
        visit_vec(v, &mut sub.any_of);
        visit_vec(v, &mut sub.one_of);
        visit_box(v, &mut sub.not);
        visit_box(v, &mut sub.if_schema);
        visit_box(v, &mut sub.then_schema);
        visit_box(v, &mut sub.else_schema);
    }

    if let Some(arr) = &mut schema.array {
        visit_single_or_vec(v, &mut arr.items);
        visit_box(v, &mut arr.additional_items);
        visit_box(v, &mut arr.contains);
    }

    if let Some(obj) = &mut schema.object {
        visit_map_values(v, &mut obj.properties);
        visit_map_values(v, &mut obj.pattern_properties);
        visit_box(v, &mut obj.additional_properties);
        visit_box(v, &mut obj.property_names);
    }
}

fn visit_box<V: Visitor + ?Sized>(v: &mut V, target: &mut Option<Box<Schema>>) {
    if let Some(s) = target {
        v.visit_schema(s)
    }
}

fn visit_vec<V: Visitor + ?Sized>(v: &mut V, target: &mut Option<Vec<Schema>>) {
    if let Some(vec) = target {
        for s in vec {
            v.visit_schema(s)
        }
    }
}

fn visit_map_values<V: Visitor + ?Sized>(v: &mut V, target: &mut crate::Map<String, Schema>) {
    for s in target.values_mut() {
        v.visit_schema(s)
    }
}

fn visit_single_or_vec<V: Visitor + ?Sized>(v: &mut V, target: &mut Option<SingleOrVec<Schema>>) {
    match target {
        None => {}
        Some(SingleOrVec::Single(s)) => v.visit_schema(s),
        Some(SingleOrVec::Vec(vec)) => {
            for s in vec {
                v.visit_schema(s)
            }
        }
    }
}

/// This visitor will replace all boolean JSON Schemas with equivalent object schemas.
///
/// This is useful for dialects of JSON Schema (e.g. OpenAPI 3.0) that do not support booleans as schemas.
#[derive(Debug, Clone)]
pub struct ReplaceBoolSchemas {
    /// When set to `true`, a schema's `additionalProperties` property will not be changed from a boolean.
    pub skip_additional_properties: bool,
}

impl Visitor for ReplaceBoolSchemas {
    fn visit_schema(&mut self, schema: &mut Schema) {
        visit_schema(self, schema);

        if let Schema::Bool(b) = *schema {
            *schema = Schema::Bool(b).into_object().into()
        }
    }

    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        if self.skip_additional_properties {
            if let Some(obj) = &mut schema.object {
                if let Some(ap) = &obj.additional_properties {
                    if let Schema::Bool(_) = ap.as_ref() {
                        let additional_properties = obj.additional_properties.take();
                        visit_schema_object(self, schema);
                        schema.object().additional_properties = additional_properties;

                        return;
                    }
                }
            }
        }

        visit_schema_object(self, schema);
    }
}

/// This visitor will restructure JSON Schema objects so that the `$ref` property will never appear alongside any other properties.
///
/// This is useful for dialects of JSON Schema (e.g. Draft 7) that do not support other properties alongside `$ref`.
#[derive(Debug, Clone)]
pub struct RemoveRefSiblings;

impl Visitor for RemoveRefSiblings {
    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        visit_schema_object(self, schema);

        if let Some(reference) = schema.reference.take() {
            if schema == &SchemaObject::default() {
                schema.reference = Some(reference);
            } else {
                let ref_schema = Schema::new_ref(reference);
                let all_of = &mut schema.subschemas().all_of;
                match all_of {
                    Some(vec) => vec.push(ref_schema),
                    None => *all_of = Some(vec![ref_schema]),
                }
            }
        }
    }
}

/// This visitor will remove the `examples` schema property and (if present) set its first value as the `example` property.
///
/// This is useful for dialects of JSON Schema (e.g. OpenAPI 3.0) that do not support the `examples` property.
#[derive(Debug, Clone)]
pub struct SetSingleExample {
    /// When set to `true`, the `examples` property will not be removed, but its first value will still be copied to `example`.
    pub retain_examples: bool,
}

impl Visitor for SetSingleExample {
    fn visit_schema_object(&mut self, schema: &mut SchemaObject) {
        visit_schema_object(self, schema);

        let first_example = schema.metadata.as_mut().and_then(|m| {
            if self.retain_examples {
                m.examples.first().cloned()
            } else {
                m.examples.drain(..).next()
            }
        });

        if let Some(example) = first_example {
            schema.extensions.insert("example".to_owned(), example);
        }
    }
}
