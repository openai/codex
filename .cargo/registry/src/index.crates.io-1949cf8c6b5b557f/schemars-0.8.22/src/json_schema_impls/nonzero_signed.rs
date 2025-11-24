use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use std::borrow::Cow;
use std::num::*;

macro_rules! nonzero_unsigned_impl {
    ($type:ty => $primitive:ty) => {
        impl JsonSchema for $type {
            no_ref_schema!();

            fn schema_name() -> String {
                stringify!($type).to_owned()
            }

            fn schema_id() -> Cow<'static, str> {
                Cow::Borrowed(stringify!(std::num::$type))
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                let zero_schema: Schema = SchemaObject {
                    const_value: Some(0.into()),
                    ..Default::default()
                }
                .into();
                let mut schema: SchemaObject = <$primitive>::json_schema(generator).into();
                schema.subschemas().not = Some(Box::from(zero_schema));
                schema.into()
            }
        }
    };
}

nonzero_unsigned_impl!(NonZeroI8 => i8);
nonzero_unsigned_impl!(NonZeroI16 => i16);
nonzero_unsigned_impl!(NonZeroI32 => i32);
nonzero_unsigned_impl!(NonZeroI64 => i64);
nonzero_unsigned_impl!(NonZeroI128 => i128);
nonzero_unsigned_impl!(NonZeroIsize => isize);
