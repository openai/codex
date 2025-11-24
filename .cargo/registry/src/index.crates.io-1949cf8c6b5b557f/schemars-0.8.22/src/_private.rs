use crate::r#gen::SchemaGenerator;
use crate::schema::{InstanceType, ObjectValidation, Schema, SchemaObject};
use crate::{JsonSchema, Map, Set};
use serde::Serialize;
use serde_json::Value;

// Helper for generating schemas for flattened `Option` fields.
pub fn json_schema_for_flatten<T: ?Sized + JsonSchema>(
    generator: &mut SchemaGenerator,
    required: bool,
) -> Schema {
    let mut schema = T::_schemars_private_non_optional_json_schema(generator);

    if T::_schemars_private_is_option() && !required {
        if let Schema::Object(SchemaObject {
            object: Some(ref mut object_validation),
            ..
        }) = schema
        {
            object_validation.required.clear();
        }
    }

    schema
}

/// Hack to simulate specialization:
/// `MaybeSerializeWrapper(x).maybe_to_value()` will resolve to either
/// - The inherent method `MaybeSerializeWrapper::maybe_to_value(...)` if x is `Serialize`
/// - The trait method `NoSerialize::maybe_to_value(...)` from the blanket impl otherwise
#[doc(hidden)]
#[macro_export]
macro_rules! _schemars_maybe_to_value {
    ($expression:expr) => {{
        #[allow(unused_imports)]
        use $crate::_private::{MaybeSerializeWrapper, NoSerialize as _};

        MaybeSerializeWrapper($expression).maybe_to_value()
    }};
}

pub struct MaybeSerializeWrapper<T>(pub T);

pub trait NoSerialize: Sized {
    fn maybe_to_value(self) -> Option<Value> {
        None
    }
}

impl<T> NoSerialize for T {}

impl<T: Serialize> MaybeSerializeWrapper<T> {
    pub fn maybe_to_value(self) -> Option<Value> {
        serde_json::value::to_value(self.0).ok()
    }
}

/// Create a schema for a unit enum
pub fn new_unit_enum(variant: &str) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        enum_values: Some(vec![variant.into()]),
        ..SchemaObject::default()
    })
}

/// Create a schema for an externally tagged enum
pub fn new_externally_tagged_enum(variant: &str, sub_schema: Schema) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        object: Some(Box::new(ObjectValidation {
            properties: {
                let mut props = Map::new();
                props.insert(variant.to_owned(), sub_schema);
                props
            },
            required: {
                let mut required = Set::new();
                required.insert(variant.to_owned());
                required
            },
            // Externally tagged variants must prohibit additional
            // properties irrespective of the disposition of
            // `deny_unknown_fields`. If additional properties were allowed
            // one could easily construct an object that validated against
            // multiple variants since here it's the properties rather than
            // the values of a property that distingish between variants.
            additional_properties: Some(Box::new(false.into())),
            ..Default::default()
        })),
        ..SchemaObject::default()
    })
}

/// Create a schema for an internally tagged enum
pub fn new_internally_tagged_enum(
    tag_name: &str,
    variant: &str,
    deny_unknown_fields: bool,
) -> Schema {
    let tag_schema = Schema::Object(SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        enum_values: Some(vec![variant.into()]),
        ..Default::default()
    });
    Schema::Object(SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        object: Some(Box::new(ObjectValidation {
            properties: {
                let mut props = Map::new();
                props.insert(tag_name.to_owned(), tag_schema);
                props
            },
            required: {
                let mut required = Set::new();
                required.insert(tag_name.to_owned());
                required
            },
            additional_properties: deny_unknown_fields.then(|| Box::new(false.into())),
            ..Default::default()
        })),
        ..SchemaObject::default()
    })
}

pub fn insert_object_property<T: ?Sized + JsonSchema>(
    obj: &mut ObjectValidation,
    key: &str,
    has_default: bool,
    required: bool,
    schema: Schema,
) {
    obj.properties.insert(key.to_owned(), schema);
    if !has_default && (required || !T::_schemars_private_is_option()) {
        obj.required.insert(key.to_owned());
    }
}

pub mod metadata {
    use crate::Schema;
    use serde_json::Value;

    macro_rules! add_metadata_fn {
        ($method:ident, $name:ident, $ty:ty) => {
            pub fn $method(schema: Schema, $name: impl Into<$ty>) -> Schema {
                let value = $name.into();
                if value == <$ty>::default() {
                    schema
                } else {
                    let mut schema_obj = schema.into_object();
                    schema_obj.metadata().$name = value.into();
                    Schema::Object(schema_obj)
                }
            }
        };
    }

    add_metadata_fn!(add_description, description, String);
    add_metadata_fn!(add_id, id, String);
    add_metadata_fn!(add_title, title, String);
    add_metadata_fn!(add_deprecated, deprecated, bool);
    add_metadata_fn!(add_read_only, read_only, bool);
    add_metadata_fn!(add_write_only, write_only, bool);
    add_metadata_fn!(add_default, default, Option<Value>);

    pub fn add_examples<I: IntoIterator<Item = Value>>(schema: Schema, examples: I) -> Schema {
        let mut schema_obj = schema.into_object();
        schema_obj.metadata().examples.extend(examples);
        Schema::Object(schema_obj)
    }
}
