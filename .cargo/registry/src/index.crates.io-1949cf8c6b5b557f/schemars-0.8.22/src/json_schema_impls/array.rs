use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use std::borrow::Cow;

// Does not require T: JsonSchema.
impl<T> JsonSchema for [T; 0] {
    no_ref_schema!();

    fn schema_name() -> String {
        "EmptyArray".to_owned()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("[]")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                max_items: Some(0),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

macro_rules! array_impls {
    ($($len:tt)+) => {
        $(
            impl<T: JsonSchema> JsonSchema for [T; $len] {
                no_ref_schema!();

                fn schema_name() -> String {
                    format!("Array_size_{}_of_{}", $len, T::schema_name())
                }

                fn schema_id() -> Cow<'static, str> {
                    Cow::Owned(
                        format!("[{}; {}]", $len, T::schema_id()))
                }

                fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                    SchemaObject {
                        instance_type: Some(InstanceType::Array.into()),
                        array: Some(Box::new(ArrayValidation {
                            items: Some(generator.subschema_for::<T>().into()),
                            max_items: Some($len),
                            min_items: Some($len),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }
                    .into()
                }
            }
        )+
    }
}

array_impls! {
     1  2  3  4  5  6  7  8  9 10
    11 12 13 14 15 16 17 18 19 20
    21 22 23 24 25 26 27 28 29 30
    31 32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{schema_for, schema_object_for};
    use pretty_assertions::assert_eq;

    #[test]
    fn schema_for_array() {
        let schema = schema_object_for::<[i32; 8]>();
        assert_eq!(
            schema.instance_type,
            Some(SingleOrVec::from(InstanceType::Array))
        );
        let array_validation = schema.array.unwrap();
        assert_eq!(
            array_validation.items,
            Some(SingleOrVec::from(schema_for::<i32>()))
        );
        assert_eq!(array_validation.max_items, Some(8));
        assert_eq!(array_validation.min_items, Some(8));
    }

    // SomeStruct does not implement JsonSchema
    struct SomeStruct;

    #[test]
    fn schema_for_empty_array() {
        let schema = schema_object_for::<[SomeStruct; 0]>();
        assert_eq!(
            schema.instance_type,
            Some(SingleOrVec::from(InstanceType::Array))
        );
        let array_validation = schema.array.unwrap();
        assert_eq!(array_validation.max_items, Some(0));
    }
}
