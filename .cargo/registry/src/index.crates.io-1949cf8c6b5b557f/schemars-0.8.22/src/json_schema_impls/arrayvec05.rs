use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use arrayvec05::{Array, ArrayString, ArrayVec};
use std::convert::TryInto;

// Do not set maxLength on the schema as that describes length in characters, but we only
// know max length in bytes.
forward_impl!((<A> JsonSchema for ArrayString<A> where A: Array<Item = u8> + Copy) => String);

impl<A: Array> JsonSchema for ArrayVec<A>
where
    A::Item: JsonSchema,
{
    no_ref_schema!();

    fn schema_name() -> String {
        format!(
            "Array_up_to_size_{}_of_{}",
            A::CAPACITY,
            A::Item::schema_name()
        )
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(generator.subschema_for::<A::Item>().into()),
                max_items: A::CAPACITY.try_into().ok(),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}
