use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use serde_json::json;
use std::borrow::Cow;
use std::ops::{Bound, Range, RangeInclusive};

impl<T: JsonSchema> JsonSchema for Option<T> {
    no_ref_schema!();

    fn schema_name() -> String {
        format!("Nullable_{}", T::schema_name())
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(format!("Option<{}>", T::schema_id()))
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = generator.subschema_for::<T>();
        if generator.settings().option_add_null_type {
            schema = match schema {
                Schema::Bool(true) => Schema::Bool(true),
                Schema::Bool(false) => <()>::json_schema(generator),
                Schema::Object(SchemaObject {
                    instance_type: Some(ref mut instance_type),
                    ..
                }) => {
                    add_null_type(instance_type);
                    schema
                }
                schema => SchemaObject {
                    // TODO technically the schema already accepts null, so this may be unnecessary
                    subschemas: Some(Box::new(SubschemaValidation {
                        any_of: Some(vec![schema, <()>::json_schema(generator)]),
                        ..Default::default()
                    })),
                    ..Default::default()
                }
                .into(),
            }
        }
        if generator.settings().option_nullable {
            let mut schema_obj = schema.into_object();
            schema_obj
                .extensions
                .insert("nullable".to_owned(), json!(true));
            schema = Schema::Object(schema_obj);
        };
        schema
    }

    fn _schemars_private_non_optional_json_schema(generator: &mut SchemaGenerator) -> Schema {
        T::_schemars_private_non_optional_json_schema(generator)
    }

    fn _schemars_private_is_option() -> bool {
        true
    }
}

fn add_null_type(instance_type: &mut SingleOrVec<InstanceType>) {
    match instance_type {
        SingleOrVec::Single(ty) if **ty != InstanceType::Null => {
            *instance_type = vec![**ty, InstanceType::Null].into()
        }
        SingleOrVec::Vec(ty) if !ty.contains(&InstanceType::Null) => ty.push(InstanceType::Null),
        _ => {}
    };
}

impl<T: JsonSchema, E: JsonSchema> JsonSchema for Result<T, E> {
    fn schema_name() -> String {
        format!("Result_of_{}_or_{}", T::schema_name(), E::schema_name())
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(format!("Result<{}, {}>", T::schema_id(), E::schema_id()))
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut ok_schema = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        };
        let obj = ok_schema.object();
        obj.required.insert("Ok".to_owned());
        obj.properties
            .insert("Ok".to_owned(), generator.subschema_for::<T>());

        let mut err_schema = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        };
        let obj = err_schema.object();
        obj.required.insert("Err".to_owned());
        obj.properties
            .insert("Err".to_owned(), generator.subschema_for::<E>());

        let mut schema = SchemaObject::default();
        schema.subschemas().one_of = Some(vec![ok_schema.into(), err_schema.into()]);
        schema.into()
    }
}

impl<T: JsonSchema> JsonSchema for Bound<T> {
    fn schema_name() -> String {
        format!("Bound_of_{}", T::schema_name())
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(format!("Bound<{}>", T::schema_id()))
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut included_schema = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        };
        let obj = included_schema.object();
        obj.required.insert("Included".to_owned());
        obj.properties
            .insert("Included".to_owned(), generator.subschema_for::<T>());

        let mut excluded_schema = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        };
        let obj = excluded_schema.object();
        obj.required.insert("Excluded".to_owned());
        obj.properties
            .insert("Excluded".to_owned(), generator.subschema_for::<T>());

        let unbounded_schema = SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            const_value: Some(json!("Unbounded")),
            ..Default::default()
        };

        let mut schema = SchemaObject::default();
        schema.subschemas().one_of = Some(vec![
            included_schema.into(),
            excluded_schema.into(),
            unbounded_schema.into(),
        ]);
        schema.into()
    }
}

impl<T: JsonSchema> JsonSchema for Range<T> {
    fn schema_name() -> String {
        format!("Range_of_{}", T::schema_name())
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(format!("Range<{}>", T::schema_id()))
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        };
        let obj = schema.object();
        obj.required.insert("start".to_owned());
        obj.required.insert("end".to_owned());
        obj.properties
            .insert("start".to_owned(), generator.subschema_for::<T>());
        obj.properties
            .insert("end".to_owned(), generator.subschema_for::<T>());
        schema.into()
    }
}

forward_impl!((<T: JsonSchema> JsonSchema for RangeInclusive<T>) => Range<T>);

forward_impl!((<T: ?Sized> JsonSchema for std::marker::PhantomData<T>) => ());

forward_impl!((<'a> JsonSchema for std::fmt::Arguments<'a>) => String);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{schema_for, schema_object_for};
    use pretty_assertions::assert_eq;

    #[test]
    fn schema_for_option() {
        let schema = schema_object_for::<Option<i32>>();
        assert_eq!(
            schema.instance_type,
            Some(vec![InstanceType::Integer, InstanceType::Null].into())
        );
        assert_eq!(schema.extensions.get("nullable"), None);
        assert_eq!(schema.subschemas.is_none(), true);
    }

    #[test]
    fn schema_for_option_with_ref() {
        use crate as schemars;
        #[derive(JsonSchema)]
        struct Foo;

        let schema = schema_object_for::<Option<Foo>>();
        assert_eq!(schema.instance_type, None);
        assert_eq!(schema.extensions.get("nullable"), None);
        assert_eq!(schema.subschemas.is_some(), true);
        let any_of = schema.subschemas.unwrap().any_of.unwrap();
        assert_eq!(any_of.len(), 2);
        assert_eq!(any_of[0], Schema::new_ref("#/definitions/Foo".to_string()));
        assert_eq!(any_of[1], schema_for::<()>());
    }

    #[test]
    fn schema_for_result() {
        let schema = schema_object_for::<Result<bool, String>>();
        let one_of = schema.subschemas.unwrap().one_of.unwrap();
        assert_eq!(one_of.len(), 2);

        let ok_schema: SchemaObject = one_of[0].clone().into();
        let obj = ok_schema.object.unwrap();
        assert!(obj.required.contains("Ok"));
        assert_eq!(obj.properties["Ok"], schema_for::<bool>());

        let err_schema: SchemaObject = one_of[1].clone().into();
        let obj = err_schema.object.unwrap();
        assert!(obj.required.contains("Err"));
        assert_eq!(obj.properties["Err"], schema_for::<String>());
    }
}
