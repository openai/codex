use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use arrayvec07::{ArrayString, ArrayVec};
use std::convert::TryInto;

// Do not set maxLength on the schema as that describes length in characters, but we only
// know max length in bytes.
forward_impl!((<const CAP: usize> JsonSchema for ArrayString<CAP>) => String);

impl<T, const CAP: usize> JsonSchema for ArrayVec<T, CAP>
where
    T: JsonSchema,
{
    no_ref_schema!();

    fn schema_name() -> String {
        format!("Array_up_to_size_{}_of_{}", CAP, T::schema_name())
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(generator.subschema_for::<T>().into()),
                max_items: CAP.try_into().ok(),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}
